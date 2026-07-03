<template>
  <div class="db-result-grid">
    <div v-if="editable" class="grid-toolbar">
      <el-button size="small" @click="addInsertRow">新增行</el-button>
      <el-button size="small" :disabled="currentRowKey === null" @click="toggleDeleteCurrent">
        {{ currentRowDeleted ? "取消删除" : "删除行" }}
      </el-button>
      <el-divider direction="vertical" />
      <el-button
        size="small"
        type="primary"
        :disabled="stagedCount === 0"
        :loading="applying"
        @click="emitApply"
      >
        应用更改{{ stagedCount > 0 ? `（${stagedCount}）` : "" }}
      </el-button>
      <el-button size="small" :disabled="stagedCount === 0" @click="clearStaged">放弃更改</el-button>
      <span v-if="!pkColumns.length" class="grid-hint warn">该表没有主键，仅可查看</span>
      <span v-else class="grid-hint">双击单元格编辑；编辑框内可一键设 NULL</span>
    </div>

    <el-alert
      v-if="result.truncated"
      type="warning"
      :closable="false"
      class="truncate-banner"
      :title="`结果已截断至 ${result.rows.length} 行，可提高连接的行数上限或收窄查询范围`"
    />

    <div class="grid-body">
      <el-table
        :data="viewRows"
        size="small"
        border
        height="100%"
        highlight-current-row
        :row-class-name="rowClassName"
        @current-change="onCurrentChange"
        @sort-change="onSortChange"
      >
        <el-table-column type="index" width="48" fixed>
          <template #default="{ row }">
            <span :class="{ 'insert-mark': row.__kind === 'insert' }">
              {{ row.__kind === "insert" ? "+" : row.__idx + 1 + pageOffset }}
            </span>
          </template>
        </el-table-column>
        <el-table-column
          v-for="col in result.columns"
          :key="col.name"
          :label="col.name"
          :min-width="columnWidth(col)"
          :sortable="serverSort ? 'custom' : false"
          :prop="col.name"
          show-overflow-tooltip
        >
          <template #header>
            <span class="col-header">
              {{ col.name }}
              <span class="col-type">{{ col.typeName }}</span>
              <el-tag v-if="pkColumns.includes(col.name)" type="warning" size="small" effect="plain">PK</el-tag>
            </span>
          </template>
          <template #default="{ row }">
            <template v-if="isEditing(row, col.name)">
              <el-input
                :ref="setEditingInput"
                v-model="editingValue"
                size="small"
                @keyup.enter="commitEdit"
                @keyup.esc="cancelEdit"
                @blur="commitEdit"
              >
                <template #append>
                  <el-button size="small" @mousedown.prevent="setEditingNull">NULL</el-button>
                </template>
              </el-input>
            </template>
            <template v-else>
              <div
                :class="cellClass(row, col)"
                @dblclick="startEdit(row, col)"
              >
                <span v-if="cellValue(row, col.name) === null" class="null-text">NULL</span>
                <span v-else>{{ cellValue(row, col.name) }}</span>
              </div>
            </template>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <div class="grid-status">
      <span>{{ statusText }}</span>
      <el-pagination
        v-if="total !== undefined"
        :current-page="page + 1"
        :page-size="pageSize"
        :total="total"
        layout="total, prev, pager, next, jumper"
        small
        @current-change="(p: number) => emit('page-change', p - 1)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { ComponentPublicInstance } from "vue";
import { buildGridChanges } from "../../utils/dbGridChanges";
import type { DbCellValue, DbColumnMeta, DbGridChange, DbStatementResult } from "../../types/db";

/**
 * 结果网格：只读展示任意语句结果；表数据浏览模式（editable）支持
 * 暂存变更集编辑（改动标黄、新增标绿、删除标红），应用动作交给父级。
 */

interface ViewRow {
  __key: string;
  __kind: "row" | "insert";
  __idx: number;
  [column: string]: unknown;
}

const props = withDefaults(
  defineProps<{
    result: DbStatementResult;
    editable?: boolean;
    pkColumns?: string[];
    applying?: boolean;
    /** 提供 total 即渲染分页（服务端分页） */
    total?: number;
    page?: number;
    pageSize?: number;
    /** 列头排序走服务端 */
    serverSort?: boolean;
  }>(),
  {
    editable: false,
    pkColumns: () => [],
    applying: false,
    page: 0,
    pageSize: 200,
    serverSort: false,
  }
);

const emit = defineEmits<{
  (e: "page-change", page: number): void;
  (e: "sort-change", column: string | null, ascending: boolean): void;
  (e: "apply", changes: DbGridChange[]): void;
}>();

const edits = ref<Map<number, Record<string, DbCellValue>>>(new Map());
const inserts = ref<Array<Record<string, DbCellValue>>>([]);
const deletes = ref<Set<number>>(new Set());
const currentRowKey = ref<string | null>(null);

const editing = ref<{ key: string; column: string } | null>(null);
const editingValue = ref("");
const editingIsNull = ref(false);

const pageOffset = computed(() => (props.total !== undefined ? props.page * props.pageSize : 0));

const viewRows = computed<ViewRow[]>(() => {
  const rows: ViewRow[] = props.result.rows.map((r, i) => {
    const row: ViewRow = { __key: `r${i}`, __kind: "row", __idx: i };
    props.result.columns.forEach((c, ci) => {
      const edited = edits.value.get(i);
      row[c.name] = edited && c.name in edited ? edited[c.name] : r[ci];
    });
    return row;
  });
  inserts.value.forEach((ins, i) => {
    const row: ViewRow = { __key: `i${i}`, __kind: "insert", __idx: i };
    props.result.columns.forEach((c) => {
      row[c.name] = c.name in ins ? ins[c.name] : null;
    });
    rows.push(row);
  });
  return rows;
});

const stagedCount = computed(
  () => edits.value.size + inserts.value.length + deletes.value.size
);

const currentRowDeleted = computed(() => {
  if (!currentRowKey.value?.startsWith("r")) return false;
  return deletes.value.has(Number(currentRowKey.value.slice(1)));
});

const statusText = computed(() => {
  const r = props.result;
  const parts: string[] = [];
  if (r.columns.length > 0) {
    parts.push(`${props.total ?? r.rows.length} 行`);
  } else {
    parts.push(`影响 ${r.affected} 行`);
  }
  parts.push(`耗时 ${r.durationMs} ms`);
  if (props.editable && stagedCount.value > 0) {
    parts.push(`待应用变更 ${stagedCount.value} 条`);
  }
  return parts.join(" · ");
});

watch(
  () => props.result,
  () => {
    clearStaged();
    currentRowKey.value = null;
  }
);

function columnWidth(col: DbColumnMeta): number {
  return Math.min(280, Math.max(110, col.name.length * 10 + 60));
}

function cellValue(row: ViewRow, column: string): DbCellValue {
  return row[column] as DbCellValue;
}

function isCellEdited(row: ViewRow, column: string): boolean {
  if (row.__kind === "insert") return false;
  const edited = edits.value.get(row.__idx);
  return !!edited && column in edited;
}

function cellClass(row: ViewRow, col: DbColumnMeta): string[] {
  const classes = ["cell-content"];
  if (props.editable && props.pkColumns.length > 0 && col.kind !== "binary") {
    classes.push("cell-editable");
  }
  if (isCellEdited(row, col.name)) classes.push("cell-edited");
  if (col.kind === "binary") classes.push("cell-binary");
  return classes;
}

function rowClassName({ row }: { row: ViewRow }): string {
  if (row.__kind === "insert") return "row-insert";
  if (deletes.value.has(row.__idx)) return "row-deleted";
  if (edits.value.has(row.__idx)) return "row-edited";
  return "";
}

function onCurrentChange(row: ViewRow | null): void {
  currentRowKey.value = row?.__key ?? null;
}

function onSortChange({ prop, order }: { prop: string; order: string | null }): void {
  if (!order) {
    emit("sort-change", null, true);
  } else {
    emit("sort-change", prop, order === "ascending");
  }
}

// ---------- 编辑 ----------

const editingInput = ref<HTMLInputElement | null>(null);
function setEditingInput(el: Element | ComponentPublicInstance | null): void {
  // el-input 实例：取内部原生 input 聚焦
  const comp = el as { focus?: () => void } | null;
  if (comp?.focus) {
    nextTick(() => comp.focus?.());
  }
  editingInput.value = null;
}

function isEditing(row: ViewRow, column: string): boolean {
  return editing.value?.key === row.__key && editing.value.column === column;
}

function startEdit(row: ViewRow, col: DbColumnMeta): void {
  if (!props.editable || props.pkColumns.length === 0) return;
  if (col.kind === "binary") return;
  if (row.__kind === "row" && deletes.value.has(row.__idx)) return;
  const value = cellValue(row, col.name);
  editing.value = { key: row.__key, column: col.name };
  editingIsNull.value = value === null;
  editingValue.value = value ?? "";
}

function setEditingNull(): void {
  editingIsNull.value = true;
  editingValue.value = "";
  commitEdit();
}

function commitEdit(): void {
  const target = editing.value;
  if (!target) return;
  editing.value = null;
  const newValue: DbCellValue = editingIsNull.value && editingValue.value === "" ? null : editingValue.value;
  applyCellChange(target.key, target.column, newValue);
  editingIsNull.value = false;
}

function cancelEdit(): void {
  editing.value = null;
  editingIsNull.value = false;
}

function applyCellChange(key: string, column: string, value: DbCellValue): void {
  if (key.startsWith("i")) {
    const idx = Number(key.slice(1));
    const ins = inserts.value[idx];
    if (ins) {
      ins[column] = value;
      inserts.value = [...inserts.value];
    }
    return;
  }
  const idx = Number(key.slice(1));
  const colIndex = props.result.columns.findIndex((c) => c.name === column);
  const original = props.result.rows[idx]?.[colIndex] ?? null;
  const rowEdits = edits.value.get(idx) ?? {};
  if (value === original) {
    delete rowEdits[column];
  } else {
    rowEdits[column] = value;
  }
  if (Object.keys(rowEdits).length === 0) {
    edits.value.delete(idx);
  } else {
    edits.value.set(idx, rowEdits);
  }
  edits.value = new Map(edits.value);
}

function addInsertRow(): void {
  if (!props.editable) return;
  inserts.value = [...inserts.value, {}];
}

function toggleDeleteCurrent(): void {
  const key = currentRowKey.value;
  if (!key) return;
  if (key.startsWith("i")) {
    const idx = Number(key.slice(1));
    inserts.value = inserts.value.filter((_, i) => i !== idx);
    currentRowKey.value = null;
    return;
  }
  const idx = Number(key.slice(1));
  if (deletes.value.has(idx)) {
    deletes.value.delete(idx);
  } else {
    deletes.value.add(idx);
    edits.value.delete(idx);
    edits.value = new Map(edits.value);
  }
  deletes.value = new Set(deletes.value);
}

function clearStaged(): void {
  edits.value = new Map();
  inserts.value = [];
  deletes.value = new Set();
  editing.value = null;
}

function emitApply(): void {
  try {
    const changes = buildGridChanges({
      columns: props.result.columns,
      pkColumns: props.pkColumns,
      rows: props.result.rows,
      edits: Array.from(edits.value.entries()).map(([rowIndex, values]) => ({ rowIndex, values })),
      inserts: inserts.value,
      deletes: Array.from(deletes.value),
    });
    if (changes.length === 0) return;
    emit("apply", changes);
  } catch (error) {
    import("element-plus").then(({ ElMessage }) => ElMessage.warning((error as Error).message));
  }
}

defineExpose({ clearStaged });
</script>

<style scoped>
.db-result-grid {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 0;
}
.grid-toolbar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
}
.grid-hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.grid-hint.warn {
  color: var(--el-color-warning);
}
.truncate-banner {
  flex-shrink: 0;
  padding: 4px 12px;
}
.grid-body {
  flex: 1;
  min-height: 0;
}
.col-header {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.col-type {
  font-size: 11px;
  font-weight: normal;
  color: var(--el-text-color-placeholder);
}
.cell-content {
  min-height: 20px;
  line-height: 20px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cell-editable {
  cursor: cell;
}
.cell-edited {
  background: #fdf6ec;
  outline: 1px solid #f3d19e;
  border-radius: 3px;
  padding: 0 4px;
}
.cell-binary {
  color: var(--el-text-color-placeholder);
}
.null-text {
  color: var(--el-text-color-placeholder);
  font-style: italic;
}
.insert-mark {
  color: var(--el-color-success);
  font-weight: 600;
}
.grid-body :deep(.row-deleted) {
  text-decoration: line-through;
  background: #fef0f0 !important;
  color: var(--el-text-color-placeholder);
}
.grid-body :deep(.row-insert) {
  background: #f0f9eb !important;
}
.grid-status {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  padding: 0 2px;
}
</style>
