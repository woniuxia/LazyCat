<template>
  <div class="pm-list-view">
    <!-- Toolbar: tag / date / groupBy / cols (search/type/priority/status come from PmPanel toolbar) -->
    <div class="pm-list-toolbar">
      <el-select
        v-if="availableTags.length > 0"
        v-model="filters.tags"
        multiple
        collapse-tags
        collapse-tags-tooltip
        filterable
        placeholder="标签"
        size="small"
        class="toolbar-select"
      >
        <el-option v-for="tag in availableTags" :key="tag" :label="tag" :value="tag" />
      </el-select>

      <el-date-picker
        v-model="dateRangeModel"
        type="daterange"
        size="small"
        value-format="YYYY-MM-DD"
        range-separator="~"
        start-placeholder="起始时间"
        end-placeholder="截止时间"
        class="toolbar-date"
        :clearable="true"
      />

      <el-button
        size="small"
        :disabled="!hasActiveFilters"
        @click="onClearFilters"
      >
        重置
      </el-button>

      <div class="toolbar-spacer" />

      <el-select
        :model-value="groupBy"
        size="small"
        class="toolbar-group"
        placeholder="分组"
        @update:model-value="(v) => setGroupBy(v as PmListGroupBy)"
      >
        <el-option label="不分组" value="none" />
        <el-option label="按项目" value="project" :disabled="!isOverview" />
        <el-option label="按状态" value="status" />
        <el-option label="按优先级" value="priority" />
        <el-option label="按标签" value="tag" />
      </el-select>

      <el-popover placement="bottom-end" trigger="click" :width="180">
        <template #reference>
          <el-button size="small">
            <el-icon><Grid /></el-icon>
            <span class="btn-label">列</span>
          </el-button>
        </template>
        <div class="cols-popover">
          <el-checkbox-group :model-value="visibleCols" @change="onToggleCols">
            <el-checkbox
              v-for="col in ALL_LIST_COLS"
              :key="col"
              :value="col"
              :disabled="col === 'title' || (col === 'project' && !isOverview)"
            >
              {{ COL_LABELS[col] }}
            </el-checkbox>
          </el-checkbox-group>
        </div>
      </el-popover>
    </div>

    <!-- Data area -->
    <div
      ref="scrollEl"
      class="pm-list-scroll"
      :class="{ 'has-batch': selectedIds.size > 0 }"
      @scroll="onScroll"
    >
      <div
        v-if="filteredItems.length === 0"
        class="pm-list-empty"
      >
        <el-empty :description="hasActiveFilters ? '无匹配工作项，试试清空筛选' : '暂无工作项'" />
      </div>
      <template v-else>
        <div v-for="group in groups" :key="group.key" class="pm-list-group">
          <div
            v-if="group.key !== 'all'"
            class="pm-list-group-header"
            @click="toggleGroup(group.key)"
          >
            <el-icon class="group-caret" :class="{ 'is-open': isGroupOpen(group.key) }">
              <CaretRight />
            </el-icon>
            <span v-if="group.color" class="group-color-dot" :style="{ backgroundColor: group.color }" />
            <span class="group-label">{{ group.label }}</span>
            <span class="group-count">{{ group.items.length }}</span>
          </div>
          <el-table
            v-show="group.key === 'all' || isGroupOpen(group.key)"
            :ref="(el) => setTableRef(group.key, el)"
            :data="windowedItemsOf(group)"
            size="small"
            stripe
            row-key="id"
            empty-text="该组无数据"
            class="pm-list-table"
            :row-class-name="rowClassName"
            @selection-change="(rows) => onSelectionChange(group.key, rows)"
            @row-click="onRowClick"
            @row-dblclick="onRowDblclick"
            @row-contextmenu="onRowContextmenu"
            @sort-change="onSortChange"
          >
            <el-table-column type="selection" width="42" :selectable="rowSelectable" />

            <el-table-column
              prop="title"
              label="标题"
              min-width="220"
              sortable="custom"
              show-overflow-tooltip
            >
              <template #default="{ row }">
                <div class="cell-title">
                  <span v-if="row.pinned" class="title-pin" title="已置顶">📌</span>
                  <span class="title-text">{{ row.title }}</span>
                </div>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('project') && isOverview"
              label="项目"
              min-width="140"
              prop="projectName"
              sortable="custom"
            >
              <template #default="{ row }">
                <span
                  v-if="row.projectName"
                  class="cell-project"
                  :style="{
                    backgroundColor: (row.projectColor || '#4d7df2') + '18',
                    color: row.projectColor || '#4d7df2',
                  }"
                >
                  <span
                    class="cell-project-dot"
                    :style="{ backgroundColor: row.projectColor || '#4d7df2' }"
                  />
                  {{ row.projectName }}
                </span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('itemType')"
              label="类型"
              width="92"
              prop="itemType"
              sortable="custom"
            >
              <template #default="{ row }">
                <span
                  class="cell-pill"
                  :style="{
                    color: PM_ITEM_TYPE_MAP[row.itemType]?.color,
                    borderColor: PM_ITEM_TYPE_MAP[row.itemType]?.color + '40',
                  }"
                >
                  {{ PM_ITEM_TYPE_MAP[row.itemType]?.label }}
                </span>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('priority')"
              label="优先级"
              width="100"
              prop="priority"
              sortable="custom"
            >
              <template #default="{ row }">
                <el-dropdown
                  trigger="click"
                  @command="(cmd) => onInlinePriority(row, cmd)"
                >
                  <span
                    class="cell-pill cell-editable"
                    :style="{
                      color: PM_PRIORITY_MAP[row.priority]?.color,
                      borderColor: PM_PRIORITY_MAP[row.priority]?.color + '40',
                    }"
                    @click.stop
                  >
                    {{ PM_PRIORITY_MAP[row.priority]?.label }}
                  </span>
                  <template #dropdown>
                    <el-dropdown-menu>
                      <el-dropdown-item
                        v-for="(meta, key) in PM_PRIORITY_MAP"
                        :key="key"
                        :command="key"
                        :disabled="row.priority === key"
                      >
                        <span
                          class="cell-pill"
                          :style="{
                            color: meta.color,
                            borderColor: meta.color + '40',
                          }"
                        >
                          {{ meta.label }}
                        </span>
                      </el-dropdown-item>
                    </el-dropdown-menu>
                  </template>
                </el-dropdown>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('status')"
              label="状态"
              width="100"
              prop="status"
              sortable="custom"
            >
              <template #default="{ row }">
                <el-dropdown
                  trigger="click"
                  @command="(cmd) => onInlineStatus(row, cmd)"
                >
                  <span
                    class="cell-pill cell-editable"
                    :style="{
                      color: statusMeta(row.status).color,
                      borderColor: statusMeta(row.status).color + '40',
                    }"
                    @click.stop
                  >
                    {{ statusMeta(row.status).label }}
                  </span>
                  <template #dropdown>
                    <el-dropdown-menu>
                      <el-dropdown-item
                        v-for="col in PM_STATUS_COLUMNS"
                        :key="col.key"
                        :command="col.key"
                        :disabled="row.status === col.key"
                      >
                        <span
                          class="cell-pill"
                          :style="{
                            color: col.color,
                            borderColor: col.color + '40',
                          }"
                        >
                          {{ col.label }}
                        </span>
                      </el-dropdown-item>
                    </el-dropdown-menu>
                  </template>
                </el-dropdown>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('endAt')"
              label="截止"
              width="130"
              prop="endAt"
              sortable="custom"
            >
              <template #default="{ row }">
                <el-popover
                  trigger="click"
                  placement="bottom-start"
                  :width="260"
                  :popper-options="{ modifiers: [{ name: 'preventOverflow', enabled: true }] }"
                >
                  <template #reference>
                    <span class="cell-date-trigger" @click.stop>
                      <span
                        v-if="row.endAt"
                        class="cell-date"
                        :class="{ 'is-overdue': isPmItemOverdue(row) }"
                      >
                        {{ formatPmDateForDisplay(row.endAt, 'short') }}
                      </span>
                      <span v-else class="cell-empty">设置日期</span>
                    </span>
                  </template>
                  <div class="inline-date-editor">
                    <el-date-picker
                      :model-value="row.endAt"
                      type="date"
                      value-format="YYYY-MM-DD"
                      placeholder="选择截止日期"
                      size="small"
                      style="width: 100%;"
                      @update:model-value="(val) => onInlineEndAt(row, val as string | null)"
                    />
                    <el-button
                      v-if="row.endAt"
                      size="small"
                      text
                      class="inline-date-clear"
                      @click="onInlineEndAt(row, null)"
                    >
                      清除
                    </el-button>
                  </div>
                </el-popover>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('tags')"
              label="标签"
              min-width="160"
            >
              <template #default="{ row }">
                <el-popover
                  trigger="click"
                  placement="bottom-start"
                  :width="260"
                >
                  <template #reference>
                    <span class="cell-tags" @click.stop>
                      <el-tag
                        v-for="tag in (row.tags || []).slice(0, 3)"
                        :key="tag"
                        size="small"
                        class="cell-tag"
                      >
                        {{ tag }}
                      </el-tag>
                      <span v-if="(row.tags || []).length > 3" class="tag-more">
                        +{{ row.tags.length - 3 }}
                      </span>
                      <span v-if="(row.tags || []).length === 0" class="cell-empty">
                        添加标签
                      </span>
                    </span>
                  </template>
                  <el-select
                    :model-value="row.tags"
                    multiple
                    filterable
                    allow-create
                    default-first-option
                    placeholder="输入标签后回车"
                    size="small"
                    style="width: 100%;"
                    @update:model-value="(val) => onInlineTags(row, val as string[])"
                  >
                    <el-option v-for="tag in availableTags" :key="tag" :label="tag" :value="tag" />
                  </el-select>
                </el-popover>
              </template>
            </el-table-column>

            <el-table-column
              v-if="colVisible('updatedAt')"
              label="更新"
              width="110"
              prop="updatedAt"
              sortable="custom"
            >
              <template #default="{ row }">
                <span class="cell-date">{{ formatUpdatedAt(row.updatedAt) }}</span>
              </template>
            </el-table-column>
          </el-table>
        </div>
        <div v-if="virtualActive && renderedTotal < filteredItems.length" class="pm-list-more-hint">
          已加载 {{ renderedTotal }} / {{ filteredItems.length }} 项，继续滚动加载更多
        </div>
      </template>
    </div>

    <!-- Batch bar -->
    <Transition name="pm-list-batch-slide">
      <div v-if="selectedIds.size > 0" class="pm-list-batch-bar">
        <div class="batch-info">
          <span class="batch-count">已选 {{ selectedIds.size }} 项</span>
          <el-button size="small" text @click="clearSelection">清除</el-button>
        </div>
        <div class="batch-actions">
          <el-dropdown trigger="click" @command="onBatchStatus">
            <el-button size="small">
              改状态
              <el-icon class="dropdown-caret"><CaretBottom /></el-icon>
            </el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item
                  v-for="col in PM_STATUS_COLUMNS"
                  :key="col.key"
                  :command="col.key"
                >
                  {{ col.label }}
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>

          <el-dropdown trigger="click" @command="onBatchPriority">
            <el-button size="small">
              改优先级
              <el-icon class="dropdown-caret"><CaretBottom /></el-icon>
            </el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item
                  v-for="(meta, key) in PM_PRIORITY_MAP"
                  :key="key"
                  :command="key"
                >
                  {{ meta.label }}
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>

          <el-dropdown
            v-if="movableProjects.length > 0"
            trigger="click"
            @command="onBatchProject"
          >
            <el-button size="small">
              改项目
              <el-icon class="dropdown-caret"><CaretBottom /></el-icon>
            </el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item
                  v-for="project in movableProjects"
                  :key="project.id"
                  :command="project.id"
                >
                  {{ project.name }}
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>

          <el-button size="small" @click="onBatchPin(true)">置顶</el-button>
          <el-button size="small" @click="onBatchPin(false)">取消置顶</el-button>
          <el-button size="small" type="danger" @click="onBatchDelete">删除</el-button>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch, nextTick } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { CaretBottom, CaretRight, Grid } from "@element-plus/icons-vue";
import type { PmItem, PmItemStatus, PmPriority, PmProject } from "../types/pm";
import {
  PM_ITEM_TYPE_MAP,
  PM_PRIORITY_MAP,
  PM_STATUS_COLUMNS,
} from "../types/pm";
import { useToolInvoke } from "../composables/useToolInvoke";
import { formatPmDateForDisplay } from "../utils/pmDate";
import { isPmItemOverdue } from "../utils/pmDate";
import {
  usePmListPrefs,
  ALL_LIST_COLS,
  COL_LABELS,
  type PmListColId,
  type PmListGroupBy,
} from "../composables/usePmListPrefs";
import type { PmContextId } from "../composables/usePmViewMemory";

interface SortState {
  prop: string | null;
  order: "asc" | "desc" | null;
}

interface GroupItem {
  key: string;
  label: string;
  color?: string | null;
  items: PmItem[];
}

const props = defineProps<{
  items: PmItem[];
  projects: PmProject[];
  selectedItemId: number | null;
  isOverview: boolean;
  selectedProjectId: number | "overview" | null;
}>();

const emit = defineEmits<{
  (e: "select", item: PmItem): void;
  (e: "edit", item: PmItem): void;
  (e: "item-context", event: MouseEvent, item: PmItem): void;
  (e: "items-changed"): void;
}>();

const { invoke } = useToolInvoke();

// Preferences
const contextRef = computed<PmContextId | null>(() => {
  if (props.selectedProjectId === null) return null;
  return props.selectedProjectId;
});
const { visibleCols, filters, groupBy, setVisibleCols, setFilters, setGroupBy, resetFilters } =
  usePmListPrefs(contextRef);

// Overview 切换时若分组为 project 且当前不是 overview，降级为 none
watch(
  () => props.isOverview,
  (ov) => {
    if (!ov && groupBy.value === "project") {
      setGroupBy("none");
    }
  },
  { immediate: true },
);

// Selection
const tableRefs = ref<Map<string, any>>(new Map());
function setTableRef(key: string, el: unknown) {
  if (el) tableRefs.value.set(key, el);
  else tableRefs.value.delete(key);
}
const selectionMap = ref<Map<string, Set<number>>>(new Map());
const selectedIds = computed<Set<number>>(() => {
  const all = new Set<number>();
  for (const set of selectionMap.value.values()) {
    for (const id of set) all.add(id);
  }
  return all;
});

// Sort
const sortState = ref<SortState>({ prop: null, order: null });

// Progressive rendering (Phase 4.2) — declared after `filteredItems` below
const VIRTUAL_THRESHOLD = 500;
const VIRTUAL_CHUNK = 200;
const SCROLL_TRIGGER_PX = 240;
const scrollEl = ref<HTMLElement | null>(null);
const renderLimit = ref(VIRTUAL_CHUNK);

function statusMeta(status: PmItemStatus) {
  return PM_STATUS_COLUMNS.find((c) => c.key === status) ?? { label: status, color: "#909399" };
}

function formatUpdatedAt(value: string): string {
  if (!value) return "-";
  return value.slice(0, 10);
}

function rowClassName({ row }: { row: PmItem }) {
  return row.id === props.selectedItemId ? "is-selected-row" : "";
}

function rowSelectable(row: PmItem): boolean {
  return row.status !== undefined;
}

const priorityRank: Record<PmPriority, number> = { P0: 0, P1: 1, P2: 2, P3: 3 };
const statusRank: Record<PmItemStatus, number> = {
  todo: 0,
  in_progress: 1,
  testing: 2,
  done: 3,
};

function sortValue(item: PmItem, prop: string): string | number | null {
  switch (prop) {
    case "title":
      return item.title.toLowerCase();
    case "projectName":
      return (item.projectName ?? "").toLowerCase();
    case "itemType":
      return item.itemType;
    case "priority":
      return priorityRank[item.priority] ?? 99;
    case "status":
      return statusRank[item.status] ?? 99;
    case "endAt":
      return item.endAt ?? null;
    case "updatedAt":
      return item.updatedAt ?? null;
    default:
      return null;
  }
}

function sortedItemsOf(list: PmItem[]): PmItem[] {
  const { prop, order } = sortState.value;
  if (!prop || !order) return list;
  const dir = order === "asc" ? 1 : -1;
  return [...list].sort((a, b) => {
    const va = sortValue(a, prop);
    const vb = sortValue(b, prop);
    if (va === vb) return 0;
    if (va === null || va === undefined) return 1;
    if (vb === null || vb === undefined) return -1;
    if (typeof va === "number" && typeof vb === "number") return (va - vb) * dir;
    return String(va).localeCompare(String(vb)) * dir;
  });
}

function onSortChange(payload: { prop: string; order: "ascending" | "descending" | null }) {
  if (!payload.order) {
    sortState.value = { prop: null, order: null };
  } else {
    sortState.value = {
      prop: payload.prop,
      order: payload.order === "ascending" ? "asc" : "desc",
    };
  }
}

function onSelectionChange(groupKey: string, rows: PmItem[]) {
  selectionMap.value.set(groupKey, new Set(rows.map((r) => r.id)));
  // trigger reactivity
  selectionMap.value = new Map(selectionMap.value);
}

function onRowClick(row: PmItem) {
  emit("select", row);
}

function onRowDblclick(row: PmItem) {
  emit("edit", row);
}

function onRowContextmenu(row: PmItem, _column: unknown, event: MouseEvent) {
  emit("item-context", event, row);
}

function clearSelection() {
  for (const [, table] of tableRefs.value) {
    table?.clearSelection?.();
  }
  selectionMap.value = new Map();
}

// Column visibility
function colVisible(id: PmListColId): boolean {
  return visibleCols.value.includes(id);
}
function onToggleCols(next: PmListColId[] | (string | number | boolean)[]) {
  const cleaned = (next as unknown as string[]).filter((v) =>
    (ALL_LIST_COLS as string[]).includes(v),
  ) as PmListColId[];
  if (!cleaned.includes("title")) cleaned.unshift("title");
  setVisibleCols(cleaned);
}

// Filters
const dateRangeModel = computed<[string, string] | null>({
  get: () => filters.value.dateRange,
  set: (val) => {
    setFilters({ ...filters.value, dateRange: val ?? null });
  },
});

watch(
  () => ({ ...filters.value }),
  (next, prev) => {
    if (!prev) return;
    if (JSON.stringify(next) !== JSON.stringify(prev)) {
      setFilters(next);
    }
  },
  { deep: true },
);

const hasActiveFilters = computed(() => {
  const f = filters.value;
  return !!(f.tags.length || f.dateRange);
});

function onClearFilters() {
  resetFilters();
}

const availableTags = computed<string[]>(() => {
  const set = new Set<string>();
  for (const item of props.items) {
    for (const tag of item.tags || []) {
      set.add(tag);
    }
  }
  return Array.from(set).sort((a, b) => a.localeCompare(b));
});

const filteredItems = computed<PmItem[]>(() => {
  const f = filters.value;
  return props.items.filter((item) => {
    if (f.tags.length > 0) {
      const itemTags = new Set(item.tags || []);
      const allHit = f.tags.every((t) => itemTags.has(t));
      if (!allHit) return false;
    }
    if (f.dateRange) {
      const [start, end] = f.dateRange;
      const inRange = (d: string | null): boolean =>
        d !== null && d >= start && d <= end;
      if (!inRange(item.startAt) && !inRange(item.endAt)) return false;
    }
    return true;
  });
});

const virtualActive = computed(
  () => groupBy.value === "none" && filteredItems.value.length > VIRTUAL_THRESHOLD,
);

function windowedItemsOf(group: GroupItem): PmItem[] {
  const sorted = sortedItemsOf(group.items);
  if (!virtualActive.value) return sorted;
  return sorted.slice(0, renderLimit.value);
}

const renderedTotal = computed<number>(() => {
  if (!virtualActive.value) return filteredItems.value.length;
  return Math.min(renderLimit.value, filteredItems.value.length);
});

function onScroll() {
  if (!virtualActive.value) return;
  const el = scrollEl.value;
  if (!el) return;
  const distanceToBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
  if (distanceToBottom > SCROLL_TRIGGER_PX) return;
  if (renderLimit.value >= filteredItems.value.length) return;
  renderLimit.value = Math.min(
    renderLimit.value + VIRTUAL_CHUNK,
    filteredItems.value.length,
  );
}

watch(
  [
    () => filteredItems.value.length,
    () => sortState.value.prop,
    () => sortState.value.order,
    () => groupBy.value,
  ],
  () => {
    renderLimit.value = VIRTUAL_CHUNK;
    nextTick(() => {
      scrollEl.value?.scrollTo({ top: 0 });
    });
  },
);

// Groups
const groupExpanded = ref<Record<string, boolean>>({});
function isGroupOpen(key: string): boolean {
  return groupExpanded.value[key] !== false;
}
function toggleGroup(key: string) {
  groupExpanded.value[key] = !isGroupOpen(key);
}

const groups = computed<GroupItem[]>(() => {
  if (groupBy.value === "none") {
    return [{ key: "all", label: "", items: filteredItems.value }];
  }
  const buckets = new Map<string, GroupItem>();
  for (const item of filteredItems.value) {
    let gkey: string;
    let label: string;
    let color: string | null = null;
    switch (groupBy.value) {
      case "project": {
        const pid = item.projectId ?? 0;
        gkey = `project-${pid}`;
        label = item.projectName ?? `项目 #${pid}`;
        color = item.projectColor ?? null;
        break;
      }
      case "status": {
        gkey = `status-${item.status}`;
        label = statusMeta(item.status).label;
        color = statusMeta(item.status).color;
        break;
      }
      case "priority": {
        gkey = `priority-${item.priority}`;
        label = PM_PRIORITY_MAP[item.priority]?.label ?? item.priority;
        color = PM_PRIORITY_MAP[item.priority]?.color ?? null;
        break;
      }
      case "tag": {
        const tags = item.tags && item.tags.length > 0 ? item.tags : ["(无标签)"];
        for (const tag of tags) {
          const key = `tag-${tag}`;
          if (!buckets.has(key)) {
            buckets.set(key, { key, label: tag, items: [] });
          }
          buckets.get(key)!.items.push(item);
        }
        continue;
      }
      default:
        gkey = "all";
        label = "";
    }
    if (!buckets.has(gkey)) {
      buckets.set(gkey, { key: gkey, label, color, items: [] });
    }
    buckets.get(gkey)!.items.push(item);
  }
  const list = Array.from(buckets.values());
  list.sort((a, b) => {
    if (groupBy.value === "priority") {
      const rank = (k: string): number => {
        const p = k.replace("priority-", "") as PmPriority;
        return priorityRank[p] ?? 99;
      };
      return rank(a.key) - rank(b.key);
    }
    if (groupBy.value === "status") {
      const rank = (k: string): number => {
        const s = k.replace("status-", "") as PmItemStatus;
        return statusRank[s] ?? 99;
      };
      return rank(a.key) - rank(b.key);
    }
    return a.label.localeCompare(b.label);
  });
  return list;
});

// Keep selection stable across item refresh
watch(
  () => props.items.map((i) => i.id).join(","),
  () => {
    nextTick(() => {
      const ids = new Set(props.items.map((i) => i.id));
      const next = new Map<string, Set<number>>();
      for (const [gkey, set] of selectionMap.value) {
        const retained = new Set<number>();
        for (const id of set) {
          if (ids.has(id)) retained.add(id);
        }
        if (retained.size > 0) next.set(gkey, retained);
      }
      selectionMap.value = next;
      for (const [gkey, table] of tableRefs.value) {
        table?.clearSelection?.();
        const retained = next.get(gkey);
        if (!retained) continue;
        const group = groups.value.find((g) => g.key === gkey);
        const rows = (group?.items ?? []).filter((i) => retained.has(i.id));
        rows.forEach((row) => table.toggleRowSelection?.(row, true));
      }
    });
  },
);

// Movable projects
const movableProjects = computed(() => {
  return props.projects.filter((p) => p.status === "active");
});

// Inline edit
async function onInlineStatus(row: PmItem, command: unknown) {
  const status = command as PmItemStatus;
  if (row.status === status) return;
  try {
    await invoke("tool:pm:item-change-status", { id: row.id, status });
    ElMessage.success({ message: `已改为「${statusMeta(status).label}」`, duration: 1200 });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function onInlinePriority(row: PmItem, command: unknown) {
  const priority = command as PmPriority;
  if (row.priority === priority) return;
  try {
    await invoke("tool:pm:item-update", { id: row.id, priority });
    ElMessage.success({ message: `已改为 ${PM_PRIORITY_MAP[priority].label}`, duration: 1200 });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function onInlineEndAt(row: PmItem, value: string | null) {
  if ((row.endAt ?? null) === (value ?? null)) return;
  try {
    await invoke("tool:pm:item-update", { id: row.id, endAt: value });
    ElMessage.success({ message: value ? "已更新截止日期" : "已清除截止日期", duration: 1200 });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function onInlineTags(row: PmItem, tags: string[]) {
  const current = (row.tags ?? []).slice().sort();
  const next = tags.slice().sort();
  if (current.join("|") === next.join("|")) return;
  try {
    await invoke("tool:pm:item-update", { id: row.id, tags });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

// Batch ops
async function runBatch(fields: Record<string, unknown>, successMsg: string) {
  if (selectedIds.value.size === 0) return;
  const ids = Array.from(selectedIds.value);
  try {
    const result = (await invoke<{ updated: number }>("tool:pm:item-batch-update", {
      ids,
      fields,
    })) ?? { updated: 0 };
    ElMessage.success({ message: `${successMsg}（${result.updated} 项）`, duration: 1500 });
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function onBatchStatus(command: unknown) {
  const status = command as PmItemStatus;
  await runBatch({ status }, `已改为「${statusMeta(status).label}」`);
}

async function onBatchPriority(command: unknown) {
  const priority = command as PmPriority;
  await runBatch({ priority }, `已改为 ${PM_PRIORITY_MAP[priority].label}`);
}

async function onBatchProject(command: unknown) {
  const projectId = command as number;
  const target = props.projects.find((p) => p.id === projectId);
  await runBatch({ projectId }, `已移至「${target?.name ?? projectId}」`);
}

async function onBatchPin(pinned: boolean) {
  await runBatch({ pinned }, pinned ? "已置顶" : "已取消置顶");
}

async function onBatchDelete() {
  if (selectedIds.value.size === 0) return;
  try {
    await ElMessageBox.confirm(
      `确定删除选中的 ${selectedIds.value.size} 项工作项？`,
      "批量删除确认",
      { type: "warning" },
    );
  } catch {
    return;
  }
  const ids = Array.from(selectedIds.value);
  let success = 0;
  for (const id of ids) {
    try {
      await invoke("tool:pm:item-delete", { id });
      success += 1;
    } catch (e) {
      ElMessage.error((e as Error).message);
    }
  }
  if (success > 0) {
    ElMessage.success({ message: `已删除 ${success} 项`, duration: 1500 });
    emit("items-changed");
  }
}
</script>

<style scoped>
.pm-list-view {
  position: relative;
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.pm-list-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 10px 20px;
  align-items: center;
  border-bottom: 1px solid var(--el-border-color-lighter, #ebeef5);
  background: var(--el-bg-color, #fff);
}
.toolbar-search {
  width: 200px;
  min-width: 140px;
}
.toolbar-select {
  width: 140px;
  min-width: 110px;
}
.toolbar-date {
  width: 240px;
}
.toolbar-group {
  width: 120px;
}
.toolbar-spacer {
  flex: 1 1 auto;
}
.btn-label {
  margin-left: 4px;
}

.cols-popover {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.cols-popover :deep(.el-checkbox) {
  margin-right: 0;
  display: flex;
}

.pm-list-scroll {
  flex: 1;
  overflow: auto;
  padding: 12px 20px 24px;
  transition: padding-bottom 0.18s;
}
.pm-list-scroll.has-batch {
  padding-bottom: 72px;
}

.pm-list-empty {
  padding: 40px 0;
  display: flex;
  justify-content: center;
}

.pm-list-more-hint {
  padding: 12px 0 4px;
  text-align: center;
  font-size: 12px;
  color: var(--el-text-color-secondary, #909399);
  letter-spacing: 0.02em;
}

.pm-list-group {
  margin-bottom: 12px;
}

.pm-list-group-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  margin-bottom: 4px;
  border-radius: 4px;
  cursor: pointer;
  user-select: none;
  background: var(--el-fill-color-light, #f5f7fa);
  transition: background 0.15s;
}
.pm-list-group-header:hover {
  background: var(--el-fill-color, #f0f2f5);
}
.group-caret {
  font-size: 12px;
  transition: transform 0.15s;
}
.group-caret.is-open {
  transform: rotate(90deg);
}
.group-color-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}
.group-label {
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-primary, #303133);
}
.group-count {
  font-size: 12px;
  color: var(--el-text-color-secondary, #606266);
  background: var(--el-bg-color, #fff);
  padding: 0 6px;
  border-radius: 8px;
  border: 1px solid var(--el-border-color-lighter, #ebeef5);
}

.pm-list-table :deep(.el-table__row) {
  cursor: pointer;
}
.pm-list-table :deep(.el-table__row.is-selected-row) {
  background-color: var(--el-color-primary-light-9, #ecf5ff) !important;
}
.pm-list-table :deep(.el-table__row.is-selected-row td) {
  background-color: var(--el-color-primary-light-9, #ecf5ff) !important;
}

.cell-title {
  display: flex;
  align-items: center;
  gap: 6px;
}
.title-pin {
  font-size: 12px;
  line-height: 1;
}
.title-text {
  font-weight: 500;
  color: var(--el-text-color-primary, #303133);
}

.cell-project {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 1px 8px;
  border-radius: 10px;
  font-size: 12px;
  line-height: 1.6;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cell-project-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.cell-pill {
  display: inline-block;
  font-size: 11px;
  line-height: 1.6;
  padding: 0 6px;
  border: 1px solid;
  border-radius: 4px;
}
.cell-editable {
  cursor: pointer;
  transition: background 0.15s, filter 0.15s;
}
.cell-editable:hover {
  filter: brightness(1.05);
  background: var(--el-fill-color-lighter, #fafafa);
}

.cell-date {
  font-size: 12px;
  color: var(--el-text-color-secondary, #606266);
}
.cell-date.is-overdue {
  color: #f56c6c;
}
.cell-empty {
  color: var(--el-text-color-placeholder, #a8abb2);
  font-size: 12px;
}
.cell-date-trigger {
  display: inline-block;
  padding: 1px 4px;
  border-radius: 3px;
  cursor: pointer;
  transition: background 0.15s;
}
.cell-date-trigger:hover {
  background: var(--el-fill-color-lighter, #fafafa);
}

.inline-date-editor {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.inline-date-clear {
  align-self: flex-end;
}

.cell-tags {
  display: inline-flex;
  flex-wrap: wrap;
  gap: 4px;
  align-items: center;
  max-width: 100%;
  cursor: pointer;
  padding: 1px 2px;
  border-radius: 3px;
  transition: background 0.15s;
}
.cell-tags:hover {
  background: var(--el-fill-color-lighter, #fafafa);
}
.cell-tag {
  max-width: 120px;
}
.tag-more {
  font-size: 11px;
  color: var(--el-text-color-secondary, #606266);
  padding: 0 4px;
}

.pm-list-batch-bar {
  position: absolute;
  left: 20px;
  right: 20px;
  bottom: 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 10px 16px;
  background: var(--el-bg-color, #fff);
  border: 1px solid var(--el-border-color, #dcdfe6);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.08);
}

.batch-info {
  display: flex;
  align-items: center;
  gap: 8px;
}
.batch-count {
  font-size: 13px;
  color: var(--el-text-color-primary, #303133);
  font-weight: 500;
}

.batch-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.dropdown-caret {
  margin-left: 4px;
  font-size: 11px;
}

.pm-list-batch-slide-enter-active,
.pm-list-batch-slide-leave-active {
  transition: transform 0.22s, opacity 0.22s;
}
.pm-list-batch-slide-enter-from,
.pm-list-batch-slide-leave-to {
  transform: translateY(20px);
  opacity: 0;
}
</style>
