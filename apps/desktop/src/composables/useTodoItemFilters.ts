import { computed, type Ref } from "vue";
import type { TodoItem, TodoPriority, TodoType } from "../types";
import { groupTodoItemsByBucket } from "../utils/todoBuckets";
import { getTodayDateString } from "../utils/todoSchedule";

export interface TodoItemFiltersOptions {
  items: Ref<TodoItem[]>;
  types: Ref<TodoType[]>;
  itemKeyword: Ref<string>;
  filterType: Ref<string | null>;
  filterPriority: Ref<TodoPriority | null>;
  itemScheduleAt: (item: TodoItem) => string | null;
  isItemOverdue: (item: TodoItem) => boolean;
}

export function useTodoItemFilters(options: TodoItemFiltersOptions) {
  const { items, types, itemKeyword, filterType, filterPriority, itemScheduleAt, isItemOverdue } =
    options;

  const filteredItems = computed(() => {
    const keyword = itemKeyword.value.trim().toLowerCase();
    return items.value.filter((item) => {
      if (!keyword) return true;
      return (
        item.title.toLowerCase().includes(keyword) ||
        item.description.toLowerCase().includes(keyword)
      );
    });
  });

  const sortedTypes = computed(() => {
    const typeCounts = new Map<number, number>();
    for (const item of items.value) {
      if (typeof item.typeId !== "number") continue;
      typeCounts.set(item.typeId, (typeCounts.get(item.typeId) || 0) + 1);
    }
    return types.value
      .map((item, index) => ({ item, index, count: typeCounts.get(item.id) || 0 }))
      .sort((left, right) => right.count - left.count || left.index - right.index)
      .map(({ item }) => item);
  });

  const bucketedItems = computed(() => groupTodoItemsByBucket(filteredItems.value));
  const activeItems = computed(() => bucketedItems.value.activeItems);
  const recentWeekItems = computed(() => bucketedItems.value.recentWeekItems);
  const doneItems = computed(() => bucketedItems.value.doneItems);

  const hasActiveFilter = computed(
    () => filterType.value !== null || filterPriority.value !== null,
  );

  function applyDisplayFilter(list: TodoItem[]): TodoItem[] {
    let result = list;
    if (filterType.value !== null) {
      const currentType = filterType.value;
      result = result.filter((item) => (item.typeName || "未分类") === currentType);
    }
    if (filterPriority.value !== null) {
      const currentPriority = filterPriority.value;
      result = result.filter((item) => item.priority === currentPriority);
    }
    return result;
  }

  const displayActiveItems = computed(() => applyDisplayFilter(activeItems.value));
  const displayRecentWeekItems = computed(() => applyDisplayFilter(recentWeekItems.value));
  const displayDoneItems = computed(() => applyDisplayFilter(doneItems.value));

  const todayDueCount = computed(() => {
    const today = getTodayDateString();
    return activeItems.value.filter((item) => {
      const time = itemScheduleAt(item);
      return time && time.startsWith(today);
    }).length;
  });

  const overdueCount = computed(() => {
    return activeItems.value.filter((item) => isItemOverdue(item)).length;
  });

  function clearAllFilters() {
    filterType.value = null;
    filterPriority.value = null;
  }

  return {
    filteredItems,
    sortedTypes,
    bucketedItems,
    activeItems,
    recentWeekItems,
    doneItems,
    hasActiveFilter,
    applyDisplayFilter,
    displayActiveItems,
    displayRecentWeekItems,
    displayDoneItems,
    todayDueCount,
    overdueCount,
    clearAllFilters,
  };
}
