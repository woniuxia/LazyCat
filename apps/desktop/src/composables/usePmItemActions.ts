import { ElMessage, ElMessageBox } from "element-plus";
import type { ComputedRef, Ref } from "vue";
import type {
  PmItem,
  PmItemType,
  PmPriority,
  PmItemStatus,
  PmSiyuanLocation,
  PmSiyuanPageRef,
} from "../types/pm";
import { normalizePmDateRangeForDraft } from "../utils/pmDate";
import { usePmTodoLinking } from "./usePmTodoLinking";

const PM_ITEM_STATUS_ORDER: PmItemStatus[] = ["todo", "in_progress", "testing", "done"];

export interface PmItemActionsDeps {
  invoke: <T = unknown>(channel: string, payload: Record<string, unknown>) => Promise<T>;
  items: Ref<PmItem[]>;
  selectedItem: ComputedRef<PmItem | null>;
  selectedItemId: Ref<number | null>;
  selectedProjectId: Ref<number | "overview" | null>;
  isOverview: ComputedRef<boolean>;
  editingItem: Ref<PmItem | null>;
  itemFormProjectId: Ref<number | null>;
  itemPrimaryPage: Ref<PmSiyuanPageRef | null>;
  itemExtraPages: Ref<PmSiyuanPageRef[]>;
  itemSubmitting: Ref<boolean>;
  itemDialogVisible: Ref<boolean>;
  pendingTodoCreates: Ref<Array<{ title: string; priority: string; description: string }>>;
  pendingTodoLinkIds: Ref<number[]>;
  resetPendingTodos: () => void;
  dialogPmTodo: { loadItems: (pmItemId?: number) => Promise<void> };
  siyuanConfigReady: Ref<boolean>;
  itemEffectiveLocation: Ref<PmSiyuanLocation | null>;
  ensureSiyuanDirectoryLoaded: () => Promise<unknown>;
  cloneSiyuanPage: (page: PmSiyuanPageRef | null | undefined) => PmSiyuanPageRef | null;
  cloneSiyuanPages: (pages: PmSiyuanPageRef[] | null | undefined) => PmSiyuanPageRef[];
  normalizeItemLinkUrl: (value: string | null | undefined) => string;
  runDialogBeforeSubmit: () => Promise<void> | undefined;
  runDialogAfterSubmit: (id: number) => Promise<void> | undefined;
  loadItems: () => Promise<void>;
  loadItemCounts: () => Promise<void>;
  loadTodayCounts: () => Promise<void>;
  todayRefreshSignal: Ref<number>;
}

export function usePmItemActions(deps: PmItemActionsDeps) {
  const {
    invoke,
    items,
    selectedItem,
    selectedItemId,
    selectedProjectId,
    isOverview,
    editingItem,
    itemFormProjectId,
    itemPrimaryPage,
    itemExtraPages,
    itemSubmitting,
    itemDialogVisible,
    pendingTodoCreates,
    pendingTodoLinkIds,
    resetPendingTodos,
    dialogPmTodo,
    siyuanConfigReady,
    itemEffectiveLocation,
    ensureSiyuanDirectoryLoaded,
    cloneSiyuanPage,
    cloneSiyuanPages,
    normalizeItemLinkUrl,
    runDialogBeforeSubmit,
    runDialogAfterSubmit,
    loadItems,
    loadItemCounts,
    loadTodayCounts,
    todayRefreshSignal,
  } = deps;

  function editItem(item: PmItem) {
    editingItem.value = item;
    itemFormProjectId.value = item.projectId;
    itemPrimaryPage.value = cloneSiyuanPage(item.siyuanPrimaryPage);
    itemExtraPages.value = cloneSiyuanPages(item.siyuanExtraPages);
    itemDialogVisible.value = true;
    dialogPmTodo.loadItems(item.id);
    if (siyuanConfigReady.value && itemEffectiveLocation.value) {
      void ensureSiyuanDirectoryLoaded();
    }
  }

  function formatItemTimestampValue(value: string | Date | null | undefined): string | null {
    if (!value) return null;
    return value instanceof Date ? value.toISOString() : value;
  }

  async function submitItem(form: {
    title: string;
    refCode?: string;
    itemType: PmItemType;
    priority: PmPriority;
    status: PmItemStatus;
    startAt: string | null;
    endAt: string | null;
    linkUrl: string;
    description: string;
    startedAt?: string | null;
    testingAt?: string | null;
    completedAt?: string | null;
  }) {
    if (itemSubmitting.value) return;
    if (!form.title.trim()) {
      ElMessage.warning("请输入标题");
      return;
    }
    itemSubmitting.value = true;
    try {
      const normalizedDateRange = normalizePmDateRangeForDraft(form.startAt, form.endAt);
      const payload: Record<string, unknown> = {
        title: form.title,
        refCode: form.refCode?.trim() || null,
        itemType: form.itemType,
        priority: form.priority,
        status: form.status,
        startAt: normalizedDateRange.startAt,
        endAt: normalizedDateRange.endAt,
        linkUrl: normalizeItemLinkUrl(form.linkUrl) || null,
        description: form.description,
        siyuanPrimaryPage: itemPrimaryPage.value,
        siyuanExtraPages: itemExtraPages.value,
      };
      if (editingItem.value) {
        const targetProjectId = itemFormProjectId.value;
        if (!targetProjectId) {
          ElMessage.warning("请选择所属项目");
          return;
        }
        if (targetProjectId !== editingItem.value.projectId) {
          await invoke("tool:pm:item-move-project", {
            id: editingItem.value.id,
            projectId: targetProjectId,
          });
        }

        const timestampFields = ["startedAt", "testingAt", "completedAt"] as const;
        for (const field of timestampFields) {
          const draftValue = formatItemTimestampValue(form[field]);
          const originalValue = formatItemTimestampValue(editingItem.value[field]);
          if (draftValue !== originalValue) {
            payload[field] = draftValue;
          }
        }

        await invoke("tool:pm:item-update", {
          id: editingItem.value.id,
          ...payload,
        });
        // 编辑：保存完成后按当前 doc 的 attIds 清理被删除的附件
        try {
          await runDialogBeforeSubmit();
        } catch (error) {
          console.warn("清理已移除的项目附件失败", error);
        }
      } else {
        const projectId = isOverview.value ? itemFormProjectId.value : selectedProjectId.value;
        if (!projectId || projectId === "overview") {
          ElMessage.warning("请选择所属项目");
          return;
        }
        const result = await invoke<{ id: number }>("tool:pm:item-create", {
          projectId,
          ...payload,
        });
        try {
          // 新建：把 tmp-<uuid> 下的附件 rebind 到 realId
          try {
            await runDialogAfterSubmit(result.id);
          } catch (error) {
            console.warn("迁移新建项目的临时附件失败", error);
          }
          // Process pending todo data for newly created item
          if (
            result.id &&
            (pendingTodoCreates.value.length > 0 || pendingTodoLinkIds.value.length > 0)
          ) {
            const tempTodo = usePmTodoLinking(() => result.id);
            for (const c of pendingTodoCreates.value) {
              await tempTodo.quickCreate(c.title, c.priority, c.description);
            }
            if (pendingTodoLinkIds.value.length > 0) {
              await tempTodo.linkBatch(pendingTodoLinkIds.value);
            }
          }
        } finally {
          // 无论 todo 子流程成败，都清空暂存，避免下次新建重复创建
          resetPendingTodos();
        }
      }
      itemDialogVisible.value = false;
      await loadItems();
    } catch (e) {
      ElMessage.error((e as Error).message);
      if (editingItem.value) {
        await loadItems();
      }
    } finally {
      itemSubmitting.value = false;
    }
  }

  async function togglePin() {
    if (!selectedItem.value) return;
    await toggleItemPinFor(selectedItem.value);
  }

  async function advanceStatus() {
    if (!selectedItem.value) return;
    await advanceItemStatusFor(selectedItem.value);
  }

  async function quickAdvance(item: PmItem) {
    await advanceItemStatusFor(item);
  }

  async function deleteItem() {
    if (!selectedItem.value) return;
    await deleteItemRecord(selectedItem.value);
  }

  function findNextStatus(item: PmItem): PmItemStatus | null {
    const index = PM_ITEM_STATUS_ORDER.indexOf(item.status);
    if (index < 0 || index >= PM_ITEM_STATUS_ORDER.length - 1) return null;
    return PM_ITEM_STATUS_ORDER[index + 1];
  }

  async function toggleItemPinFor(item: PmItem) {
    // 乐观更新本地数据，避免全量 loadItems 导致的闪烁与多次 IPC
    const target = items.value.find((i) => i.id === item.id);
    const previousPinned = target?.pinned ?? false;
    if (target) target.pinned = !previousPinned;
    try {
      await invoke("tool:pm:item-toggle-pin", { id: item.id });
      // 计数受 pinned 影响较小,只刷一次 itemCounts(异步,不阻塞)
      loadItemCounts();
    } catch (e) {
      if (target) target.pinned = previousPinned;
      ElMessage.error((e as Error).message);
    }
  }

  async function advanceItemStatusFor(item: PmItem) {
    const nextStatus = findNextStatus(item);
    if (!nextStatus) return;
    // 乐观更新：直接修改本地数据并同步 updatedAt
    const target = items.value.find((i) => i.id === item.id);
    const previousStatus = target?.status;
    const previousCompletedAt = target?.completedAt ?? null;
    const previousUpdatedAt = target?.updatedAt;
    if (target) {
      target.status = nextStatus;
      target.updatedAt = new Date().toISOString();
      if (nextStatus === "done" && !target.completedAt) {
        target.completedAt = target.updatedAt;
      }
    }
    try {
      await invoke("tool:pm:item-change-status", { id: item.id, status: nextStatus });
      // 状态变化会影响项目计数和今日 badge,异步刷新但不阻塞
      loadItemCounts();
      void loadTodayCounts();
      todayRefreshSignal.value++;
    } catch (e) {
      // 失败回滚
      if (target && previousStatus) {
        target.status = previousStatus;
        target.completedAt = previousCompletedAt;
        if (previousUpdatedAt) target.updatedAt = previousUpdatedAt;
      }
      ElMessage.error((e as Error).message);
    }
  }

  async function deleteItemRecord(item: PmItem) {
    try {
      await ElMessageBox.confirm("确定删除该工作项？", "删除确认", { type: "warning" });
      await invoke("tool:pm:item-delete", { id: item.id });
      if (selectedItemId.value === item.id) {
        selectedItemId.value = null;
      }
      await loadItems();
    } catch (e) {
      if ((e as string) !== "cancel") {
        ElMessage.error((e as Error).message);
      }
    }
  }

  return {
    editItem,
    submitItem,
    togglePin,
    advanceStatus,
    quickAdvance,
    deleteItem,
    findNextStatus,
    toggleItemPinFor,
    advanceItemStatusFor,
    deleteItemRecord,
  };
}
