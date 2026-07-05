import { computed, ref, watch, onScopeDispose, type Ref } from "vue";
import type { PmItem, PmItemType, PmPriority, PmItemStatus } from "../types/pm";
import { filterPmItemsBySelectedStatuses } from "../utils/pmStatusFilter";

export interface PmItemFiltersOptions {
  items: Ref<PmItem[]>;
  filterType: Ref<PmItemType | "">;
  filterPriority: Ref<PmPriority | "">;
  selectedStatuses: Ref<PmItemStatus[]>;
}

export function usePmItemFilters(options: PmItemFiltersOptions) {
  const { items, filterType, filterPriority, selectedStatuses } = options;

  const searchInput = ref(""); // 用户输入,实时同步到 input
  const searchText = ref(""); // 防抖后的值,用于过滤计算
  let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  watch(searchInput, (v) => {
    if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
    searchDebounceTimer = setTimeout(() => {
      searchText.value = v;
    }, 200);
  });
  onScopeDispose(() => {
    if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
  });

  const baseFilteredItems = computed(() => {
    let result = items.value;
    if (searchText.value) {
      const q = searchText.value.toLowerCase();
      result = result.filter(
        (i) =>
          i.title.toLowerCase().includes(q) ||
          i.description.toLowerCase().includes(q) ||
          i.tags.some((t) => t.toLowerCase().includes(q))
      );
    }
    if (filterType.value) {
      result = result.filter((i) => i.itemType === filterType.value);
    }
    if (filterPriority.value) {
      result = result.filter((i) => i.priority === filterPriority.value);
    }
    return result;
  });

  const statusFilteredItems = computed(() =>
    filterPmItemsBySelectedStatuses(baseFilteredItems.value, selectedStatuses.value),
  );

  return { searchInput, searchText, baseFilteredItems, statusFilteredItems };
}
