import { h, type ComputedRef, type Ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  TodoAssignee,
  TodoItem,
  TodoItemUpsertPayload,
  TodoKind,
  TodoRule,
  TodoStatus,
  TodoType,
} from "../types";
import { isFiveMinuteDateTime, isFiveMinuteTime } from "../utils/todoSchedule";
import {
  asRecord,
  effectiveReminderPresets,
  getResponseItems,
  normalizeTodoItem,
  readNullableNumber,
} from "./useTodoItem";
import type { SelectAssigneeValue, SelectTypeValue, TodoItemDraft } from "./useTodoScheduleFields";

export interface TodoCrudActionsDeps {
  items: Ref<TodoItem[]>;
  types: Ref<TodoType[]>;
  assignees: Ref<TodoAssignee[]>;
  projectOptions: Ref<{ id: number; name: string; color: string }[]>;
  filterProjectId: Ref<number | string | null>;
  itemDraft: TodoItemDraft;
  itemDialogMode: Ref<"create" | "edit_item">;
  todoDetailEditRef: Ref<{
    runAfterSubmit?: (realId: number) => Promise<void>;
    runBeforeSubmit?: () => Promise<void>;
  } | null>;
  closeTodoContextMenu: () => void;
  isRepeating: ComputedRef<boolean>;
  showRecurrenceFields: ComputedRef<boolean>;
  showCustomRepeatFields: ComputedRef<boolean>;
  showCronRepeatFields: ComputedRef<boolean>;
  buildEventAt: () => string | null;
  buildRulePayload: () => TodoRule;
  buildEndValue: () => string | number | null;
  isAvailableActionTarget: (actionType: string | null, targetId: string | null) => boolean;
}

export function useTodoCrudActions(deps: TodoCrudActionsDeps) {
  const {
    items,
    types,
    assignees,
    projectOptions,
    filterProjectId,
    itemDraft,
    itemDialogMode,
    todoDetailEditRef,
    closeTodoContextMenu,
    isRepeating,
    showRecurrenceFields,
    showCustomRepeatFields,
    showCronRepeatFields,
    buildEventAt,
    buildRulePayload,
    buildEndValue,
    isAvailableActionTarget,
  } = deps;

  function normalizeName(value: string) {
    return value.trim().toLocaleLowerCase();
  }

  function getNextTypeSortOrder() {
    return types.value.reduce((max, item) => Math.max(max, item.sortOrder), 0) + 10;
  }

  async function loadTypes() {
    types.value =
      ((await invokeToolByChannel("tool:todo:type-list", {})) as { items: TodoType[] }).items || [];
  }
  async function loadAssignees() {
    assignees.value =
      ((await invokeToolByChannel("tool:todo:assignee-list", {})) as { items: TodoAssignee[] })
        .items || [];
  }
  async function loadItems() {
    closeTodoContextMenu();
    const params: Record<string, unknown> = {};
    if (filterProjectId.value === "none") {
      params.projectFilter = "none";
    } else if (typeof filterProjectId.value === "number") {
      params.projectId = filterProjectId.value;
    }
    items.value = getResponseItems(await invokeToolByChannel("tool:todo:item-list", params)).map(
      normalizeTodoItem,
    );
  }

  async function loadProjects() {
    try {
      const list = (await invokeToolByChannel("tool:pm:project-list", {})) as { id: number; name: string; color: string; status: string }[];
      projectOptions.value = (list || []).filter((p) => p.status === "active");
    } catch {
      projectOptions.value = [];
    }
  }

  async function resolveTypeId(value: SelectTypeValue) {
    if (typeof value === "number") return value;
    const name = typeof value === "string" ? value.trim() : "";
    if (!name) return null;
    const existed = types.value.find((item) => normalizeName(item.name) === normalizeName(name));
    if (existed) return existed.id;
    const result = (await invokeToolByChannel("tool:todo:type-upsert", {
      name,
      sortOrder: getNextTypeSortOrder(),
    })) as { id?: number };
    await loadTypes();
    if (typeof result.id !== "number") throw new Error("分类创建失败");
    return result.id;
  }

  async function resolveAssigneeIds(values: SelectAssigneeValue[]) {
    const ids = new Set<number>();
    let created = false;
    for (const value of values) {
      if (typeof value === "number") {
        ids.add(value);
        continue;
      }
      const name = value.trim();
      if (!name) continue;
      const existed = assignees.value.find(
        (item) => normalizeName(item.name) === normalizeName(name),
      );
      if (existed) {
        ids.add(existed.id);
        continue;
      }
      const result = (await invokeToolByChannel("tool:todo:assignee-upsert", { name })) as {
        id?: number;
      };
      if (typeof result.id !== "number") throw new Error("执行人创建失败");
      ids.add(result.id);
      created = true;
    }
    if (created) await loadAssignees();
    return [...ids];
  }

  async function submitItemChanges(showSuccess = true) {
    const title = itemDraft.title.trim();
    const eventAt = buildEventAt();
    const selectedReminderPresets = effectiveReminderPresets(itemDraft.reminderPresets);
    const hasEventDate = !!itemDraft.eventDate.trim();
    const hasEventTime = !!itemDraft.eventTime.trim();
    if (!title) {
      ElMessage.warning("请输入事项标题");
      return { ok: false, id: null as number | null };
    }
    if (hasEventDate !== hasEventTime) {
      ElMessage.warning("日期和时间需要同时填写或同时清空");
      return { ok: false, id: null as number | null };
    }
    if (hasEventTime && !isFiveMinuteTime(itemDraft.eventTime)) {
      ElMessage.warning("事件时间仅支持5分钟刻度");
      return { ok: false, id: null as number | null };
    }
    if (selectedReminderPresets.length > 0 && !eventAt) {
      ElMessage.warning("请先填写日期和时间，再设置提醒方式");
      return { ok: false, id: null as number | null };
    }
    if (
      !isRepeating.value &&
      itemDraft.actionType &&
      !isAvailableActionTarget(itemDraft.actionType, itemDraft.actionTargetId)
    ) {
      ElMessage.warning("请选择打包配置");
      return { ok: false, id: null as number | null };
    }
    if (isRepeating.value && showRecurrenceFields.value) {
      if (!hasEventDate || !hasEventTime) {
        ElMessage.warning("重复事项需要同时填写日期和时间");
        return { ok: false, id: null as number | null };
      }
      if (!isFiveMinuteTime(itemDraft.eventTime)) {
        ElMessage.warning("时间仅支持5分钟刻度");
        return { ok: false, id: null as number | null };
      }
      if (!eventAt) {
        ElMessage.warning("日期或时间格式不正确");
        return { ok: false, id: null as number | null };
      }
      if (showCronRepeatFields.value && !itemDraft.cronExpression.trim()) {
        ElMessage.warning("请输入 Cron 表达式");
        return { ok: false, id: null as number | null };
      }
      if (
        showCustomRepeatFields.value &&
        itemDraft.simple.frequency === "weekly" &&
        Number(itemDraft.simple.interval || 1) > 1
      ) {
        ElMessage.warning("按周自定义暂不支持大于 1 的间隔，请改用高级 Cron");
        return { ok: false, id: null as number | null };
      }
      if (
        itemDraft.endMode === "until_date" &&
        itemDraft.endValueDate &&
        !isFiveMinuteDateTime(itemDraft.endValueDate)
      ) {
        ElMessage.warning("结束时间仅支持5分钟刻度");
        return { ok: false, id: null as number | null };
      }
    }
    try {
      const typeId = await resolveTypeId(itemDraft.typeId);
      const assigneeIds = await resolveAssigneeIds(itemDraft.assigneeIds);
      const commonPayload = {
        title,
        typeId,
        priority: itemDraft.priority,
        description: itemDraft.description,
        assigneeIds,
        links: itemDraft.links.filter((l) => l.url.trim()),
        reminderPresets: selectedReminderPresets,
      };

      const kind: TodoKind = isRepeating.value ? "recurring" : "one_off";
      const payload: TodoItemUpsertPayload & Record<string, unknown> = {
        ...commonPayload,
        kind,
        projectId: itemDraft.projectId,
        actionBinding:
          !isRepeating.value && itemDraft.actionType && itemDraft.actionTargetId
            ? {
                actionType: itemDraft.actionType,
                targetId: itemDraft.actionTargetId,
              }
            : null,
      };

      if (!isRepeating.value) {
        payload.eventAt = eventAt;
      }

      if (isRepeating.value) {
        payload.recurrence = {
          startAt: eventAt,
          ruleMode: itemDraft.ruleMode,
          rule: buildRulePayload(),
          timezone: itemDraft.timezone || "local",
          endMode: itemDraft.endMode,
          endValue: buildEndValue(),
        };
      }

      let response: unknown;
      if (itemDialogMode.value === "create") {
        response = await invokeToolByChannel("tool:todo:item-create", payload);
      } else {
        payload.id = itemDraft.id;
        if (itemDraft.rootId) payload.rootId = itemDraft.rootId;
        // 编辑：update 前按当前 doc attIds 清理被删图的附件
        try {
          await todoDetailEditRef.value?.runBeforeSubmit?.();
        } catch (error) {
          console.warn("清理已移除的待办附件失败", error);
        }
        response = await invokeToolByChannel("tool:todo:item-update", payload);
      }

      const savedId =
        readNullableNumber(asRecord(response), ["id"]) ??
        (itemDialogMode.value === "edit_item" ? itemDraft.id : null);
      // 新建：把 tmp owner 改写为 savedId
      if (itemDialogMode.value === "create" && typeof savedId === "number") {
        try {
          await todoDetailEditRef.value?.runAfterSubmit?.(savedId);
        } catch (error) {
          console.warn("迁移新建待办的临时附件失败", error);
        }
      }
      await loadItems();
      if (showSuccess) ElMessage.success("保存成功");
      return { ok: true, id: savedId };
    } catch (error) {
      ElMessage.error((error as Error).message);
      return { ok: false, id: null as number | null };
    }
  }

  async function changeItemStatus(id: number, status: TodoStatus) {
    try {
      await invokeToolByChannel("tool:todo:item-change-status", { id, status });
      await loadItems();
    } catch (error) {
      ElMessage.error((error as Error).message);
    }
  }

  async function toggleItemPin(id: number) {
    try {
      await invokeToolByChannel("tool:todo:item-toggle-pin", { id });
      await loadItems();
    } catch (error) {
      ElMessage.error((error as Error).message);
    }
  }

  async function openLink(url: string) {
    try {
      await invokeToolByChannel("tool:todo:open-link", { url });
    } catch (error) {
      ElMessage.error((error as Error).message);
    }
  }

  async function snoozeItem(id: number, taskReminderId?: number | null) {
    try {
      await invokeToolByChannel("tool:todo:item-snooze", { id, taskReminderId, minutes: 10 });
      await loadItems();
    } catch (error) {
      ElMessage.error((error as Error).message);
    }
  }

  async function deleteItem(item: TodoItem) {
    try {
      // 普通事项：直接删除
      if (item.kind !== "recurring") {
        await ElMessageBox.confirm("确认删除该事项吗？", "删除确认", {
          type: "warning",
        });
        await invokeToolByChannel("tool:todo:item-delete", {
          id: item.id,
          scope: "this_instance",
        });
        await loadItems();
        return;
      }

      // 重复事项：显示选择对话框
      const scope = await showDeleteScopeDialog(item.title);
      if (scope === null) return; // 用户取消

      await invokeToolByChannel("tool:todo:item-delete", {
        id: item.id,
        scope: scope, // "this_instance" | "future_instances"
      });
      await loadItems();
    } catch (error) {
      if ((error as Error).message) ElMessage.error((error as Error).message);
    }
  }

  async function showDeleteScopeDialog(itemTitle: string): Promise<string | null> {
    const baseStyle =
      "display: flex; align-items: flex-start; gap: 12px; padding: 14px 16px; border: 1.5px solid var(--lc-border); border-radius: 10px; background: var(--lc-surface-1); cursor: pointer; text-align: left; transition: border-color 0.2s, background 0.2s, box-shadow 0.2s, transform 0.15s; width: 100%; outline: none;";
    const iconBoxBase =
      "flex-shrink: 0; width: 34px; height: 34px; border-radius: 8px; display: flex; align-items: center; justify-content: center; font-size: 16px; transition: background 0.2s;";
    const labelStyle =
      "font-size: 14px; font-weight: 600; line-height: 1.4; transition: color 0.2s;";
    const descStyle = "font-size: 12px; color: var(--lc-text-muted); line-height: 1.4; margin-top: 2px;";

    // SVG trash icon (single instance / mild)
    const svgTrashOne =
      '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>';
    // SVG trash-x icon (all instances / destructive)
    const svgTrashAll =
      '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/><line x1="10" y1="11" x2="14" y2="16"/><line x1="14" y1="11" x2="10" y2="16"/></svg>';

    interface OptionCfg {
      label: string;
      desc: string;
      scope: string;
      iconSvg: string;
      accentColor: string;
      accentBg: string;
    }

    const makeOption = (cfg: OptionCfg, resolveFn: (v: string) => void) =>
      h(
        "button",
        {
          style: baseStyle,
          onMouseenter: (e: MouseEvent) => {
            const el = e.currentTarget as HTMLElement;
            el.style.borderColor = cfg.accentColor;
            el.style.background = "var(--lc-surface-2)";
            el.style.boxShadow = `0 2px 8px ${cfg.accentColor}18`;
            el.style.transform = "translateY(-1px)";
          },
          onMouseleave: (e: MouseEvent) => {
            const el = e.currentTarget as HTMLElement;
            el.style.borderColor = "var(--lc-border)";
            el.style.background = "var(--lc-surface-1)";
            el.style.boxShadow = "none";
            el.style.transform = "none";
          },
          onFocus: (e: FocusEvent) => {
            const el = e.currentTarget as HTMLElement;
            el.style.borderColor = cfg.accentColor;
            el.style.boxShadow = `0 0 0 2px ${cfg.accentColor}30`;
          },
          onBlur: (e: FocusEvent) => {
            const el = e.currentTarget as HTMLElement;
            el.style.borderColor = "var(--lc-border)";
            el.style.boxShadow = "none";
          },
          onClick: () => {
            ElMessageBox.close();
            resolveFn(cfg.scope);
          },
        },
        [
          h("span", {
            style: `${iconBoxBase} background: ${cfg.accentBg}; color: ${cfg.accentColor};`,
            innerHTML: cfg.iconSvg,
          }),
          h("div", { style: "flex: 1; min-width: 0;" }, [
            h("div", { style: `${labelStyle} color: var(--lc-text);` }, cfg.label),
            h("div", { style: descStyle }, cfg.desc),
          ]),
        ],
      );

    return new Promise((resolve) => {
      ElMessageBox({
        title: "删除重复事项",
        message: h("div", { style: "padding: 8px 0 4px;" }, [
          h(
            "p",
            {
              style:
                "margin-bottom: 16px; font-size: 13px; color: var(--lc-text-muted); line-height: 1.5;",
            },
            [
              h("span", null, "「"),
              h(
                "span",
                { style: "font-weight: 600; color: var(--lc-text);" },
                itemTitle,
              ),
              h("span", null, "」是重复事项，请选择删除范围："),
            ],
          ),
          h(
            "div",
            { style: "display: flex; flex-direction: row; gap: 10px;" },
            [
              makeOption(
                {
                  label: "仅删除本次",
                  desc: "后续重复事项将继续按规则生成",
                  scope: "this_instance",
                  iconSvg: svgTrashOne,
                  accentColor: "var(--lc-accent)",
                  accentBg: "var(--lc-accent-bg, rgba(64,150,255,0.08))",
                },
                resolve,
              ),
              makeOption(
                {
                  label: "删除本次及后续所有",
                  desc: "停止后续自动生成，已完成的实例不受影响",
                  scope: "future_instances",
                  iconSvg: svgTrashAll,
                  accentColor: "var(--lc-danger, #e25050)",
                  accentBg: "rgba(226,80,80,0.08)",
                },
                resolve,
              ),
            ],
          ),
        ]),
        showCancelButton: true,
        showConfirmButton: false,
        cancelButtonText: "取消",
        customClass: "todo-delete-scope-dialog",
        closeOnClickModal: true,
        closeOnPressEscape: true,
        beforeClose: (_action: string, _instance: unknown, done: () => void) => {
          resolve(null);
          done();
        },
      });
    });
  }

  async function onBasicsChanged() {
    await Promise.all([loadTypes(), loadAssignees(), loadItems()]);
  }

  return {
    loadTypes,
    loadAssignees,
    loadItems,
    loadProjects,
    submitItemChanges,
    changeItemStatus,
    toggleItemPin,
    openLink,
    snoozeItem,
    deleteItem,
    onBasicsChanged,
  };
}
