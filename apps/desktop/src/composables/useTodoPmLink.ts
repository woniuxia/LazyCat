import { ref, watch, type Ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import { PM_STATUS_COLUMNS } from "../types/pm";
import type { PmCandidateItem } from "../types/pm";
import type { TodoItemDraft } from "./useTodoScheduleFields";

export interface TodoPmLinkDeps {
  itemDraft: TodoItemDraft;
  itemDialogMode: Ref<"create" | "edit_item">;
  selectedItemId: Ref<number | null>;
  submitItemChanges: (showSuccess?: boolean) => Promise<{ ok: boolean; id: number | null }>;
  loadItems: () => Promise<void>;
  requestPmFocus: (pmItemId: number, pmProjectId: number | null) => void;
  openTab: (id: string, title: string) => void;
}

export function useTodoPmLink(deps: TodoPmLinkDeps) {
  const {
    itemDraft,
    itemDialogMode,
    selectedItemId,
    submitItemChanges,
    loadItems,
    requestPmFocus,
    openTab,
  } = deps;

  const todoPmLinkItemId = ref<number | null>(null);
  const todoPmCandidates = ref<PmCandidateItem[]>([]);
  const skipProjectWatch = ref(false);
  // projectId 放宽为可空：候选项 projectId 本身可空，原 .vue 内声明未经 tsc 检查
  const todoLinkedPmItem = ref<{ id: number; title: string; status: string; projectId: number | null } | null>(null);
  const pmCreateDialogVisible = ref(false);
  const pmCreateTitle = ref("");
  const pmCreateProjectId = ref<number | null>(null);

  function pmStatusColor(status: string | null | undefined): string {
    return PM_STATUS_COLUMNS.find(c => c.key === (status || "todo"))?.color ?? "#909399";
  }
  function pmStatusLabel(status: string | null | undefined): string {
    return PM_STATUS_COLUMNS.find(c => c.key === (status || "todo"))?.label ?? "待办";
  }

  async function loadTodoPmCandidates(projectId: number, linkedPmItemId?: number | null, keyword?: string) {
    try {
      const result = await invokeToolByChannel("tool:todo:pm-candidates", { projectId, keyword: keyword || undefined }) as { items: PmCandidateItem[] };
      let candidates = result?.items || [];
      // Ensure currently linked PM item is in the list (it may be filtered out by other criteria)
      if (linkedPmItemId && !candidates.some((c) => c.id === linkedPmItemId)) {
        const linked = todoLinkedPmItem.value;
        if (linked && linked.id === linkedPmItemId) {
          candidates = [
            { id: linked.id, title: linked.title, status: linked.status, priority: "P2", projectId: linked.projectId, projectName: null, projectColor: null },
            ...candidates,
          ];
        }
      }
      todoPmCandidates.value = candidates;
    } catch {
      todoPmCandidates.value = [];
    }
  }

  async function onPmCreateConfirm() {
    const title = pmCreateTitle.value.trim();
    if (!title) {
      ElMessage.warning("请输入工作项标题");
      return;
    }
    const projectId = itemDraft.projectId ?? pmCreateProjectId.value;
    if (!projectId) {
      ElMessage.warning("请选择所属项目");
      return;
    }
    try {
      // Set project on draft if not already set
      if (!itemDraft.projectId) {
        skipProjectWatch.value = true;
        itemDraft.projectId = projectId;
      }
      // If the todo item hasn't been saved yet (create mode), save it first
      let todoId = itemDraft.id;
      if (!todoId) {
        const saveResult = await submitItemChanges(false);
        if (!saveResult.ok || !saveResult.id) {
          return;
        }
        todoId = saveResult.id;
        itemDraft.id = todoId;
      } else {
        // Existing todo — persist the project change before linking
        await submitItemChanges(false);
      }
      const result = await invokeToolByChannel("tool:pm:item-create", {
        projectId,
        title,
        itemType: "task",
        priority: "P2",
        status: "todo",
      }) as { id: number };
      await invokeToolByChannel("tool:todo:item-set-pm-link", {
        todoItemId: todoId,
        pmItemId: result.id,
      });
      itemDraft.pmItemId = result.id;
      itemDraft.pmItemTitle = title;
      itemDraft.pmItemProjectId = projectId;
      itemDraft.pmItemStatus = "todo";
      todoPmLinkItemId.value = result.id;
      todoLinkedPmItem.value = { id: result.id, title, status: "todo", projectId };
      pmCreateDialogVisible.value = false;
      pmCreateTitle.value = "";
      pmCreateProjectId.value = null;
      await loadTodoPmCandidates(projectId);
      await loadItems();
      // If the item was just created (was in create mode), switch to edit mode so user can continue editing
      if (itemDialogMode.value === "create") {
        itemDialogMode.value = "edit_item";
        selectedItemId.value = todoId;
      }
      ElMessage.success("工作项已创建并关联");
    } catch (error) {
      ElMessage.error((error as Error).message);
    }
  }

  function onPmCreateClosed() {
    pmCreateTitle.value = "";
    pmCreateProjectId.value = null;
  }

  async function onTodoPmLinkChange(pmItemId: number | null) {
    if (!itemDraft.id) return;
    try {
      // Ensure the project assignment is persisted before linking/unlinking PM item
      await submitItemChanges(false);
      if (pmItemId) {
        await invokeToolByChannel("tool:todo:item-set-pm-link", {
          todoItemId: itemDraft.id,
          pmItemId,
        });
      } else {
        await invokeToolByChannel("tool:todo:item-set-pm-link", {
          todoItemId: itemDraft.id,
          pmItemId: null,
        });
      }
      itemDraft.pmItemId = pmItemId;
      todoPmLinkItemId.value = pmItemId;
      const candidate = pmItemId ? todoPmCandidates.value.find((c) => c.id === pmItemId) : null;
      itemDraft.pmItemTitle = candidate?.title ?? null;
      itemDraft.pmItemProjectId = candidate?.projectId ?? null;
      itemDraft.pmItemStatus = candidate?.status ?? null;
      if (candidate) {
        todoLinkedPmItem.value = { id: candidate.id, title: candidate.title, status: candidate.status, projectId: candidate.projectId };
      } else {
        todoLinkedPmItem.value = null;
      }
      await loadItems();
    } catch (error) {
      ElMessage.error((error as Error).message);
      todoPmLinkItemId.value = itemDraft.pmItemId;
    }
  }

  function handlePmSelectChange(value: number | null) {
    if (value === -1) {
      // Handled by InlinePmSelector now - create mode is inline
      return;
    }
    if (value) {
      const candidate = todoPmCandidates.value.find((c) => c.id === value);
      if (candidate && candidate.projectId && candidate.projectId !== itemDraft.projectId) {
        skipProjectWatch.value = true;
        itemDraft.projectId = candidate.projectId;
      }
    }
    void onTodoPmLinkChange(value);
  }

  async function handlePmProjectChange(projectId: number | null) {
    if (itemDraft.pmItemId) {
      await onTodoPmLinkChange(null);
    }
    itemDraft.projectId = projectId;
  }

  async function handlePmCreate(title: string, projectId: number) {
    if (!title.trim()) return;
    try {
      if (itemDraft.projectId !== projectId) {
        skipProjectWatch.value = true;
        itemDraft.projectId = projectId;
      }
      if (itemDraft.id) {
        await submitItemChanges(false);
      }
      const result = await invokeToolByChannel("tool:pm:item-create", {
        projectId,
        title: title.trim(),
        itemType: "task",
        priority: "P2",
        status: "todo",
      }) as { id: number };
      if (itemDraft.id) {
        await invokeToolByChannel("tool:todo:item-set-pm-link", {
          todoItemId: itemDraft.id,
          pmItemId: result.id,
        });
      }
      itemDraft.pmItemId = result.id;
      itemDraft.pmItemTitle = title.trim();
      itemDraft.pmItemProjectId = projectId;
      itemDraft.pmItemStatus = "todo";
      todoPmLinkItemId.value = result.id;
      todoLinkedPmItem.value = { id: result.id, title: title.trim(), status: "todo", projectId };
      await loadTodoPmCandidates(projectId);
      await loadItems();
    } catch (error) {
      ElMessage.error((error as Error).message);
    }
  }

  function handlePmSearch(keyword: string) {
    if (itemDraft.projectId && itemDraft.kind !== "recurring") {
      loadTodoPmCandidates(itemDraft.projectId, itemDraft.pmItemId, keyword);
    }
  }

  function navigateToPmItem(pmItemId: number, pmProjectId: number | null) {
    requestPmFocus(pmItemId, pmProjectId);
    openTab("pm", "项目管理");
  }

  function pmItemTagStyle(status: string | null | undefined): Record<string, string> {
    const color = pmStatusColor(status);
    return { backgroundColor: color + "14", borderColor: color + "33", color };
  }

  watch(
    () => itemDraft.projectId,
    (newProjectId) => {
      if (skipProjectWatch.value) {
        skipProjectWatch.value = false;
        return;
      }
      todoPmLinkItemId.value = null;
      itemDraft.pmItemId = null;
      itemDraft.pmItemTitle = null;
      itemDraft.pmItemProjectId = null;
      itemDraft.pmItemStatus = null;
      todoLinkedPmItem.value = null;
      if (newProjectId && itemDraft.kind !== "recurring") {
        loadTodoPmCandidates(newProjectId);
      } else {
        todoPmCandidates.value = [];
      }
    },
  );

  return {
    todoPmLinkItemId,
    todoPmCandidates,
    skipProjectWatch,
    todoLinkedPmItem,
    pmCreateDialogVisible,
    pmCreateTitle,
    pmCreateProjectId,
    pmStatusColor,
    pmStatusLabel,
    loadTodoPmCandidates,
    onPmCreateConfirm,
    onPmCreateClosed,
    onTodoPmLinkChange,
    handlePmSelectChange,
    handlePmProjectChange,
    handlePmCreate,
    handlePmSearch,
    navigateToPmItem,
    pmItemTagStyle,
  };
}
