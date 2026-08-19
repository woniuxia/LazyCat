import { computed, nextTick, onBeforeUnmount, watch, type Ref } from "vue";
import type {
  TodoAssignee,
  TodoItem,
  TodoKind,
  TodoRecurrence,
  TodoReminderPreset,
  TodoRule,
} from "../types";
import { deriveRepeatPreset, splitDateTime } from "../utils/todoSchedule";
import {
  effectiveReminderPresets,
  getRootItemId,
  normalizeReminderPresets,
  toDraftReminderPresets,
} from "./useTodoItem";
import type { PmCandidateItem } from "../types/pm";
import type { SelectAssigneeValue, SelectTypeValue, TodoItemDraft } from "./useTodoScheduleFields";

export type TodoDetailMode = "empty" | "view" | "edit" | "create";

export interface TodoDetailStateDeps {
  items: Ref<TodoItem[]>;
  itemDraft: TodoItemDraft;
  detailMode: Ref<TodoDetailMode>;
  itemDialogMode: Ref<"create" | "edit_item">;
  selectedItemId: Ref<number | null>;
  draftBaseline: Ref<string>;
  editingItemSnapshot: Ref<TodoItem | null>;
  showMoreFields: Ref<boolean>;
  lastReminderPresetSelection: Ref<TodoReminderPreset[]>;
  defaultReminderPresets: TodoReminderPreset[];
  todoDetailEditRef: Ref<{
    focusTitleInput: () => void;
    runOnCancel?: () => Promise<void>;
  } | null>;
  todoPmLinkItemId: Ref<number | null>;
  todoPmCandidates: Ref<PmCandidateItem[]>;
  todoLinkedPmItem: Ref<{
    id: number;
    title: string;
    status: string;
    projectId: number | null;
  } | null>;
  skipProjectWatch: Ref<boolean>;
  loadTodoPmCandidates: (
    projectId: number,
    linkedPmItemId?: number | null,
    keyword?: string,
  ) => Promise<void>;
  submitItemChanges: (showSuccess?: boolean) => Promise<{ ok: boolean; id: number | null }>;
  syncSimpleDraftFromRule: (rule: TodoRule) => void;
  itemKindOf: (item: TodoItem) => TodoKind;
  hasRepeatRule: (item: TodoItem) => boolean;
  getItemRecurrence: (item: TodoItem) => TodoRecurrence | null;
}

export function useTodoDetailState(deps: TodoDetailStateDeps) {
  const {
    items,
    itemDraft,
    detailMode,
    itemDialogMode,
    selectedItemId,
    draftBaseline,
    editingItemSnapshot,
    showMoreFields,
    lastReminderPresetSelection,
    defaultReminderPresets,
    todoDetailEditRef,
    todoPmLinkItemId,
    todoPmCandidates,
    todoLinkedPmItem,
    skipProjectWatch,
    loadTodoPmCandidates,
    submitItemChanges,
    syncSimpleDraftFromRule,
    itemKindOf,
    hasRepeatRule,
    getItemRecurrence,
  } = deps;

  let titleFocusTimer: ReturnType<typeof setTimeout> | null = null;

  const selectedItem = computed(() =>
    selectedItemId.value == null
      ? null
      : items.value.find((item) => item.id === selectedItemId.value) || null,
  );
  const isDetailEditing = computed(
    () => detailMode.value === "edit" || detailMode.value === "create",
  );
  const isDraftDirty = computed(
    () => isDetailEditing.value && draftBaseline.value !== snapshotItemDraft(),
  );

  function normalizeDraftTypeValue(value: SelectTypeValue) {
    if (typeof value === "number") return value;
    const name = typeof value === "string" ? value.trim() : "";
    return name || null;
  }

  function normalizeDraftAssigneeValues(values: SelectAssigneeValue[]) {
    return values
      .map((value) => (typeof value === "number" ? `id:${value}` : `name:${value.trim()}`))
      .filter((value) => !value.endsWith(":"))
      .sort();
  }

  function snapshotItemDraft() {
    return JSON.stringify({
      mode: itemDialogMode.value,
      title: itemDraft.title.trim(),
      typeId: normalizeDraftTypeValue(itemDraft.typeId),
      priority: itemDraft.priority,
      description: itemDraft.description,
      assigneeIds: normalizeDraftAssigneeValues(itemDraft.assigneeIds),
      eventDate: itemDraft.eventDate,
      eventTime: itemDraft.eventTime,
      reminderPresets: normalizeReminderPresets(itemDraft.reminderPresets),
      repeatPreset: itemDraft.repeatPreset,
      actionType: itemDraft.actionType,
      actionTargetId: itemDraft.actionTargetId,
      ruleMode: itemDraft.ruleMode,
      timezone: itemDraft.timezone,
      cronExpression: itemDraft.cronExpression.trim(),
      endMode: itemDraft.endMode,
      endValueDate: itemDraft.endValueDate,
      endValueCount: Number(itemDraft.endValueCount || 1),
      simple: {
        frequency: itemDraft.simple.frequency,
        interval: Number(itemDraft.simple.interval || 1),
        time: itemDraft.simple.time,
        weekdays: [...itemDraft.simple.weekdays].sort((left, right) => left - right),
        dayOfMonth: Number(itemDraft.simple.dayOfMonth || 1),
      },
    });
  }

  function markDraftBaseline() {
    draftBaseline.value = snapshotItemDraft();
  }

  async function ensureDetailCanLeave() {
    if (!isDetailEditing.value || !isDraftDirty.value) return true;
    const result = await submitItemChanges(false);
    if (!result.ok) return false;
    finalizeDetailAfterSave(result.id);
    return true;
  }

  function finalizeDetailAfterSave(savedId?: number | null) {
    const fallbackId = itemDialogMode.value === "edit_item" ? itemDraft.id : null;
    const nextId = savedId ?? selectedItemId.value ?? fallbackId;
    resetItemDraft();
    draftBaseline.value = "";
    if (typeof nextId === "number" && nextId > 0) {
      selectedItemId.value = nextId;
      detailMode.value = "view";
      return;
    }
    selectedItemId.value = null;
    detailMode.value = "empty";
  }

  function selectItem(item: TodoItem) {
    if (selectedItemId.value === item.id && detailMode.value === "view") return;
    if (isDetailEditing.value && isDraftDirty.value) {
      selectItemAsync(item);
      return;
    }
    selectedItemId.value = item.id;
    detailMode.value = "view";
  }

  async function selectItemAsync(item: TodoItem) {
    if (!(await ensureDetailCanLeave())) return;
    selectedItemId.value = item.id;
    detailMode.value = "view";
  }

  async function prepareItemForInlineAction(item: TodoItem) {
    if (!(await ensureDetailCanLeave())) return false;
    selectedItemId.value = item.id;
    detailMode.value = "view";
    return true;
  }

  async function focusTitleInputWhenActive(
    expectedDetailMode: TodoDetailMode,
    expectedDialogMode: "create" | "edit_item",
  ) {
    if (titleFocusTimer) {
      clearTimeout(titleFocusTimer);
      titleFocusTimer = null;
    }
    await nextTick();
    titleFocusTimer = setTimeout(() => {
      titleFocusTimer = null;
      if (detailMode.value !== expectedDetailMode || itemDialogMode.value !== expectedDialogMode)
        return;
      todoDetailEditRef.value?.focusTitleInput();
    }, 0);
  }

  async function focusCreateTitleInput() {
    await focusTitleInputWhenActive("create", "create");
  }

  async function startCreate() {
    if (!(await ensureDetailCanLeave())) return;
    resetItemDraft();
    itemDialogMode.value = "create";
    detailMode.value = "create";
    selectedItemId.value = null;
    showMoreFields.value = false;
    markDraftBaseline();
    await focusCreateTitleInput();
  }

  async function createOnDate(dateKey: string) {
    if (!(await ensureDetailCanLeave())) return;
    resetItemDraft();
    itemDialogMode.value = "create";
    detailMode.value = "create";
    selectedItemId.value = null;
    itemDraft.eventDate = dateKey;
    itemDraft.eventTime = "09:00";
    showMoreFields.value = true;
    markDraftBaseline();
    await focusCreateTitleInput();
  }

  function cancelDetailEdit() {
    // 先让 Editor 清理 tmp 附件（编辑场景会静默跳过）
    const cleanup = todoDetailEditRef.value?.runOnCancel?.();
    if (cleanup) {
      void cleanup.catch((error) => {
        console.warn("清理取消编辑产生的临时附件失败", error);
      });
    }
    resetItemDraft();
    draftBaseline.value = "";
    if (selectedItemId.value !== null && selectedItem.value) {
      detailMode.value = "view";
      return;
    }
    detailMode.value = "empty";
  }

  async function closeDetail() {
    if (!(await ensureDetailCanLeave())) return;
    resetItemDraft();
    draftBaseline.value = "";
    selectedItemId.value = null;
    detailMode.value = "empty";
  }

  function toDraftAssigneeValues(assigneeList: TodoAssignee[]): SelectAssigneeValue[] {
    return assigneeList
      .map((assignee) =>
        typeof assignee.id === "number" && assignee.id > 0 ? assignee.id : assignee.name,
      )
      .filter(
        (value): value is SelectAssigneeValue =>
          (typeof value === "number" && value > 0) ||
          (typeof value === "string" && value.trim().length > 0),
      );
  }

  function resetItemDraft() {
    itemDraft.id = 0;
    itemDraft.rootId = 0;
    itemDraft.title = "";
    itemDraft.typeId = undefined;
    itemDraft.priority = "P2";
    itemDraft.description = "";
    itemDraft.assigneeIds = [];
    itemDraft.links = [];
    itemDraft.eventDate = "";
    itemDraft.eventTime = "";
    itemDraft.reminderPresets = [...defaultReminderPresets];
    itemDraft.repeatPreset = "none";
    itemDraft.ruleMode = "simple";
    itemDraft.timezone = "local";
    itemDraft.cronExpression = "0 0 9 * * Mon-Fri";
    itemDraft.endMode = "never";
    itemDraft.endValueDate = "";
    itemDraft.endValueCount = 1;
    itemDraft.simple.frequency = "daily";
    itemDraft.simple.interval = 1;
    itemDraft.simple.time = "";
    itemDraft.simple.weekdays = [1, 2, 3, 4, 5];
    itemDraft.simple.dayOfMonth = 1;
    itemDraft.projectId = null;
    itemDraft.pmItemId = null;
    itemDraft.pmItemTitle = null;
    itemDraft.pmItemProjectId = null;
    itemDraft.pmItemStatus = null;
    itemDraft.actionType = null;
    itemDraft.actionTargetId = null;
    todoPmLinkItemId.value = null;
    todoPmCandidates.value = [];
    todoLinkedPmItem.value = null;
    lastReminderPresetSelection.value = [...itemDraft.reminderPresets];
    editingItemSnapshot.value = null;
    itemDialogMode.value = "create";
  }

  function applyItemToDraft(item: TodoItem) {
    const { date, time } = splitDateTime(item.eventAt, "");
    itemDraft.id = item.id;
    itemDraft.rootId = getRootItemId(item);
    itemDraft.title = item.title;
    itemDraft.typeId = item.typeId ?? undefined;
    itemDraft.priority = item.priority;
    itemDraft.description = item.description;
    itemDraft.assigneeIds = toDraftAssigneeValues(item.assignees);
    itemDraft.links = (item.links || []).map((l) => ({ url: l.url, title: l.title }));
    itemDraft.eventDate = date;
    itemDraft.eventTime = time;
    itemDraft.reminderPresets = toDraftReminderPresets(item.reminderPresets);
    lastReminderPresetSelection.value = [...itemDraft.reminderPresets];
    const recurrence = getItemRecurrence(item);
    itemDraft.repeatPreset =
      itemKindOf(item) === "recurring" ? deriveRepeatPreset(recurrence) : "none";

    // 新增：加载详细规则到 simple 对象
    if (itemKindOf(item) === "recurring" && recurrence?.rule) {
      itemDraft.ruleMode = recurrence.ruleMode || "simple";
      itemDraft.timezone = recurrence.timezone || "local";
      if (itemDraft.ruleMode === "simple") {
        syncSimpleDraftFromRule(recurrence.rule as TodoRule);
      } else if (itemDraft.ruleMode === "cron") {
        itemDraft.cronExpression =
          recurrence.cronExpression ||
          (recurrence.rule as { expression?: string }).expression ||
          itemDraft.cronExpression;
      }
    }
    skipProjectWatch.value = true;
    itemDraft.projectId = item.projectId ?? null;
    itemDraft.pmItemId = item.pmItemId ?? null;
    itemDraft.pmItemTitle = item.pmItemTitle ?? null;
    itemDraft.pmItemProjectId = item.pmItemProjectId ?? null;
    itemDraft.pmItemStatus = item.pmItemStatus ?? null;
    itemDraft.actionType = item.actionBinding?.actionType ?? null;
    itemDraft.actionTargetId = item.actionBinding?.targetId ?? null;
    todoPmLinkItemId.value = item.pmItemId ?? null;
    // Populate linked PM item info for display in dropdown
    if (item.pmItemId) {
      todoLinkedPmItem.value = {
        id: item.pmItemId,
        title: item.pmItemTitle ?? "",
        status: item.pmItemStatus ?? "todo",
        projectId: item.pmItemProjectId ?? item.projectId ?? 0,
      };
    } else {
      todoLinkedPmItem.value = null;
    }
    if (item.projectId && item.kind !== "recurring") {
      loadTodoPmCandidates(item.projectId, item.pmItemId);
    } else {
      todoPmCandidates.value = [];
    }
    // Ensure skipProjectWatch is consumed: if projectId didn't change (e.g. both null),
    // the watcher won't fire, so we reset here after the scheduler flushes.
    nextTick(() => {
      skipProjectWatch.value = false;
    });
  }

  async function enterEditMode(item?: TodoItem | null, options: { focusTitle?: boolean } = {}) {
    const target = item || selectedItem.value;
    if (!target) return;
    if (detailMode.value === "edit" && selectedItemId.value === target.id) return;
    if (!(await ensureDetailCanLeave())) return;
    selectedItemId.value = target.id;
    resetItemDraft();
    itemDialogMode.value = "edit_item";
    editingItemSnapshot.value = target;
    applyItemToDraft(target);
    detailMode.value = "edit";
    showMoreFields.value =
      target.assignees.length > 0 ||
      !!target.eventAt ||
      effectiveReminderPresets(target.reminderPresets).length > 0 ||
      hasRepeatRule(target) ||
      !!target.actionBinding;
    markDraftBaseline();
    if (options.focusTitle !== false) {
      await focusTitleInputWhenActive("edit", "edit_item");
    }
  }

  watch(selectedItem, (item) => {
    if (detailMode.value === "create") return;
    if (selectedItemId.value !== null && !item) {
      selectedItemId.value = null;
      draftBaseline.value = "";
      resetItemDraft();
      detailMode.value = "empty";
    }
  });

  onBeforeUnmount(() => {
    if (titleFocusTimer) {
      clearTimeout(titleFocusTimer);
      titleFocusTimer = null;
    }
  });

  return {
    selectedItem,
    isDetailEditing,
    isDraftDirty,
    markDraftBaseline,
    ensureDetailCanLeave,
    finalizeDetailAfterSave,
    selectItem,
    prepareItemForInlineAction,
    focusCreateTitleInput,
    startCreate,
    createOnDate,
    cancelDetailEdit,
    closeDetail,
    resetItemDraft,
    enterEditMode,
  };
}
