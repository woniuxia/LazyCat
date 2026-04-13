import { ref, computed, type Ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type { PmTodoLinkItem, PmTodoSummary, PmTodoCandidateItem } from "../types/pm";

type PmItemIdGetter = () => number | null | undefined;

interface PmTodoCreateForm {
  title: string;
  priority: string;
  description: string;
  eventAt: string | null;
}

const EMPTY_CREATE_FORM: PmTodoCreateForm = {
  title: "",
  priority: "P2",
  description: "",
  eventAt: null,
};

/**
 * Reusable composable for PM-Todo linking UI logic.
 * Used in both the detail panel and the edit dialog of PmPanel.vue.
 *
 * @param getPmItemId - Getter returning the current PM item ID
 */
export function usePmTodoLinking(getPmItemId: PmItemIdGetter) {
  // ── State ────────────────────────────────────────────────
  const items = ref<PmTodoLinkItem[]>([]);
  const summary = ref<PmTodoSummary | null>(null);
  const loading = ref(false);

  const createDialogVisible = ref(false);
  const createForm = ref<PmTodoCreateForm>({ ...EMPTY_CREATE_FORM });

  const linkDialogVisible = ref(false);
  const candidates = ref<PmTodoCandidateItem[]>([]);
  const candidateKeyword = ref("");
  const candidateLoading = ref(false);
  const candidateEligibleCount = ref(0);
  const candidateBlockedCount = ref(0);
  const candidateReason = ref("");
  const linkSelectedIds = ref<number[]>([]);

  // ── Computed ─────────────────────────────────────────────
  const progressPercent = computed(() => {
    if (!summary.value || summary.value.totalCount === 0) return 0;
    return Math.round((summary.value.completedCount / summary.value.totalCount) * 100);
  });

  const allCompleted = computed(() => {
    return summary.value != null && summary.value.totalCount > 0 && summary.value.completedCount === summary.value.totalCount;
  });

  // ── Debounce helper ──────────────────────────────────────
  let _debounceTimer: ReturnType<typeof setTimeout> | null = null;

  function debouncedLoadCandidates(delay = 300) {
    if (_debounceTimer) clearTimeout(_debounceTimer);
    _debounceTimer = setTimeout(() => loadCandidates(), delay);
  }

  function onCandidateInput() {
    debouncedLoadCandidates();
  }

  // ── Actions ──────────────────────────────────────────────
  async function loadItems(pmItemId?: number) {
    const id = pmItemId ?? getPmItemId();
    if (id == null) return;
    loading.value = true;
    try {
      const result = await invokeToolByChannel("tool:pm:item-todo-list", { pmItemId: id }) as {
        items: PmTodoLinkItem[];
        totalCount: number;
        completedCount: number;
        pendingCount: number;
        projectId: number | null;
      };
      items.value = result.items ?? [];
      summary.value = {
        totalCount: result.totalCount ?? 0,
        completedCount: result.completedCount ?? 0,
        pendingCount: result.pendingCount ?? 0,
        projectId: result.projectId ?? null,
      };
    } catch {
      items.value = [];
      summary.value = null;
    } finally {
      loading.value = false;
    }
  }

  async function unlink(todoItemId: number) {
    const id = getPmItemId();
    if (id == null) return;
    try {
      await invokeToolByChannel("tool:pm:item-todo-unlink", { pmItemId: id, todoItemId });
      ElMessage.success("已解除关联");
      loadItems(id);
    } catch (e) {
      ElMessage.error((e as Error).message);
    }
  }

  async function toggleComplete(todoItem: PmTodoLinkItem) {
    const newStatus = todoItem.status === "completed" ? "pending" : "completed";
    try {
      await invokeToolByChannel("tool:todo:item-change-status", { id: todoItem.id, status: newStatus });
      loadItems();
    } catch (e) {
      ElMessage.error((e as Error).message);
    }
  }

  async function submitCreate(formData?: PmTodoCreateForm) {
    const id = getPmItemId();
    if (id == null) return;
    const form = formData ?? createForm.value;
    if (!form.title.trim()) {
      ElMessage.warning("请输入任务标题");
      return;
    }
    try {
      await invokeToolByChannel("tool:pm:item-todo-create", {
        pmItemId: id,
        title: form.title.trim(),
        priority: form.priority,
        description: form.description,
        eventAt: form.eventAt || null,
      });
      ElMessage.success("执行任务已创建");
      createDialogVisible.value = false;
      createForm.value = { ...EMPTY_CREATE_FORM };
      loadItems(id);
    } catch (e) {
      ElMessage.error((e as Error).message);
    }
  }

  async function loadCandidates() {
    const id = getPmItemId();
    if (id == null) return;
    candidateLoading.value = true;
    linkSelectedIds.value = [];
    try {
      const result = await invokeToolByChannel("tool:pm:item-todo-candidates", {
        pmItemId: id,
        keyword: candidateKeyword.value || undefined,
        limit: 50,
      }) as {
        items: PmTodoCandidateItem[];
        total: number;
        eligibleCount: number;
        blockedCount: number;
        reason: string;
      };
      candidates.value = result.items ?? [];
      candidateEligibleCount.value = result.eligibleCount ?? 0;
      candidateBlockedCount.value = result.blockedCount ?? 0;
      candidateReason.value = result.reason ?? "";
    } catch (e) {
      ElMessage.error((e as Error).message);
      candidates.value = [];
    } finally {
      candidateLoading.value = false;
    }
  }

  async function submitLink() {
    const id = getPmItemId();
    if (id == null || linkSelectedIds.value.length === 0) return;
    try {
      await invokeToolByChannel("tool:pm:item-todo-link", {
        pmItemId: id,
        todoItemIds: linkSelectedIds.value,
      });
      ElMessage.success(`已关联 ${linkSelectedIds.value.length} 项任务`);
      linkDialogVisible.value = false;
      linkSelectedIds.value = [];
      candidateKeyword.value = "";
      loadItems(id);
    } catch (e) {
      ElMessage.error((e as Error).message);
    }
  }

  function openLinkDialog() {
    linkDialogVisible.value = true;
    candidateKeyword.value = "";
    linkSelectedIds.value = [];
    loadCandidates();
  }

  function reset() {
    items.value = [];
    summary.value = null;
    createDialogVisible.value = false;
    createForm.value = { ...EMPTY_CREATE_FORM };
    linkDialogVisible.value = false;
    candidates.value = [];
    candidateKeyword.value = "";
    candidateLoading.value = false;
    candidateReason.value = "";
    linkSelectedIds.value = [];
    if (_debounceTimer) {
      clearTimeout(_debounceTimer);
      _debounceTimer = null;
    }
  }

  return {
    // State
    items,
    summary,
    loading,
    createDialogVisible,
    createForm,
    linkDialogVisible,
    candidates,
    candidateKeyword,
    candidateLoading,
    candidateEligibleCount,
    candidateBlockedCount,
    candidateReason,
    linkSelectedIds,
    // Computed
    progressPercent,
    allCompleted,
    // Actions
    loadItems,
    unlink,
    toggleComplete,
    submitCreate,
    loadCandidates,
    submitLink,
    openLinkDialog,
    onCandidateInput,
    reset,
  };
}
