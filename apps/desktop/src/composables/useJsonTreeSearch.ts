import { computed, getCurrentScope, onScopeDispose, ref, shallowRef, watch } from "vue";
import type { Ref } from "vue";
import type { JsonTreeNode } from "../utils/jsonTreeView";
import {
  collectJsonTreeAncestorKeys,
  collectJsonTreeSearchMatches,
  jsonTreeSearchMatchId,
} from "../utils/jsonTreeSearch";
import type { JsonTreeSearchMatch } from "../utils/jsonTreeSearch";

export const JSON_TREE_SEARCH_DEBOUNCE_MS = 200;

/**
 * JsonTreeViewer 树内搜索状态机:query 防抖重算、命中导航、
 * 树变化时按 key 尽力保持当前命中。
 */
export function useJsonTreeSearch(tree: Ref<JsonTreeNode>) {
  const query = ref("");
  const matches = shallowRef<JsonTreeSearchMatch[]>([]);
  const activeIndex = ref(-1);

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  function clearPending() {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
  }

  function resetToFirst() {
    const next = collectJsonTreeSearchMatches(tree.value, query.value);
    matches.value = next;
    activeIndex.value = next.length ? 0 : -1;
  }

  function recomputeKeepingActive() {
    const current = matches.value[activeIndex.value] ?? null;
    const next = collectJsonTreeSearchMatches(tree.value, query.value);
    let nextIndex = next.length ? 0 : -1;
    if (current) {
      const sameKeyAndField = next.findIndex(
        (match) => match.key === current.key && match.field === current.field,
      );
      const retained =
        sameKeyAndField >= 0
          ? sameKeyAndField
          : next.findIndex((match) => match.key === current.key);
      if (retained >= 0) nextIndex = retained;
    }
    matches.value = next;
    activeIndex.value = nextIndex;
  }

  /** 有待防抖的重算时立即提交;返回是否发生了提交(提交后已落在第 1 处)。 */
  function flushPending(): boolean {
    if (debounceTimer === null) return false;
    clearPending();
    resetToFirst();
    return true;
  }

  watch(
    query,
    (value) => {
      clearPending();
      if (!value) {
        resetToFirst();
        return;
      }
      debounceTimer = setTimeout(() => {
        debounceTimer = null;
        resetToFirst();
      }, JSON_TREE_SEARCH_DEBOUNCE_MS);
    },
    { flush: "sync" },
  );

  watch(tree, recomputeKeepingActive, { flush: "sync" });

  function goNext() {
    if (flushPending()) return;
    const total = matches.value.length;
    if (!total) return;
    activeIndex.value = (activeIndex.value + 1) % total;
  }

  function goPrev() {
    if (flushPending()) return;
    const total = matches.value.length;
    if (!total) return;
    activeIndex.value = (activeIndex.value - 1 + total) % total;
  }

  const activeMatch = computed<JsonTreeSearchMatch | null>(
    () => matches.value[activeIndex.value] ?? null,
  );
  const activeKey = computed(() => activeMatch.value?.key ?? null);
  const activeMatchId = computed(() =>
    activeMatch.value ? jsonTreeSearchMatchId(activeMatch.value) : null,
  );
  const matchedIds = computed(() => new Set(matches.value.map(jsonTreeSearchMatchId)));
  const revealKeys = computed(() => {
    const match = activeMatch.value;
    return match ? new Set(collectJsonTreeAncestorKeys(match.path)) : new Set<string>();
  });

  if (getCurrentScope()) onScopeDispose(clearPending);

  return {
    query,
    matches,
    activeIndex,
    activeKey,
    activeMatchId,
    matchedIds,
    revealKeys,
    goNext,
    goPrev,
  };
}
