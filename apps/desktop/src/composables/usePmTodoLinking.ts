import { ref, computed } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type { PmTodoLinkItem, PmTodoSummary, PmTodoCandidateItem } from "../types/pm";

type PmItemIdGetter = () => number | null | undefined;

/**
 * Composable for PM-Todo linking logic.
 * Provides data loading, inline creation, candidate search, and batch operations.
 */
export function usePmTodoLinking(getPmItemId: PmItemIdGetter) {
  // ── State ────────────────────────────────────────────────
  const items = ref<PmTodoLinkItem[]>([]);
  const summary = ref<PmTodoSummary | null>(null);
  const loading = ref(false);

  const candidates = ref<PmTodoCandidateItem[]>([]);
  const candidateKeyword = ref("");
  const candidateLoading = ref(false);
  const candidateEligibleCount = ref(0);
  const candidateBlockedCount = ref(0);
  const candidateReason = ref("");

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

  function debouncedSearchCandidates(delay = 300) {
    if (_debounceTimer) clearTimeout(_debounceTimer);
    _debounceTimer = setTimeout(() => searchCandidates(), delay);
  }

  function onCandidateInput() {
    debouncedSearchCandidates();
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

  async function toggleCompleteById(todoItemId: number) {
    const found = items.value.find((t) => t.id === todoItemId);
    if (found) await toggleComplete(found);
  }

  async function quickCreate(title: string, priority: string, description?: string) {
    const id = getPmItemId();
    if (id == null) return;
    if (!title.trim()) return;
    try {
      await invokeToolByChannel("tool:pm:item-todo-create", {
        pmItemId: id,
        title: title.trim(),
        priority,
        description: description ?? "",
        eventAt: null,
      });
      ElMessage.success("执行任务已创建");
      loadItems(id);
    } catch (e) {
      ElMessage.error((e as Error).message);
    }
  }

  async function searchCandidates(keyword?: string) {
    const id = getPmItemId();
    if (id == null) return;
    candidateLoading.value = true;
    try {
      const result = await invokeToolByChannel("tool:pm:item-todo-candidates", {
        pmItemId: id,
        keyword: keyword ?? (candidateKeyword.value || undefined),
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

  async function linkBatch(ids: number[]) {
    const id = getPmItemId();
    if (id == null || ids.length === 0) return;
    try {
      await invokeToolByChannel("tool:pm:item-todo-link", {
        pmItemId: id,
        todoItemIds: ids,
      });
      ElMessage.success(`已关联 ${ids.length} 项任务`);
      candidateKeyword.value = "";
      loadItems(id);
    } catch (e) {
      ElMessage.error((e as Error).message);
    }
  }

  function reset() {
    items.value = [];
    summary.value = null;
    candidates.value = [];
    candidateKeyword.value = "";
    candidateLoading.value = false;
    candidateReason.value = "";
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
    candidates,
    candidateKeyword,
    candidateLoading,
    candidateEligibleCount,
    candidateBlockedCount,
    candidateReason,
    // Computed
    progressPercent,
    allCompleted,
    // Actions
    loadItems,
    unlink,
    toggleComplete,
    toggleCompleteById,
    quickCreate,
    searchCandidates,
    linkBatch,
    onCandidateInput,
    reset,
  };
}
