<template>
  <div class="status-table-scroll">
    <el-table
      :data="data"
      stripe
      size="small"
      :show-header="showHeader"
      row-key="code"
      :expand-row-keys="expandedCodes"
      class="status-table"
      style="width: 100%; min-width: 902px"
      @expand-change="handleExpandChange"
      @row-click="handleRowClick"
    >
      <el-table-column type="expand" width="42">
        <template #default="{ row }">
          <HttpStatusDetail :status="row" />
        </template>
      </el-table-column>
      <el-table-column label="状态码" width="80" align="center">
        <template #default="{ row }">
          <span class="code-cell" :class="codeClass(row.code)">{{ row.code }}</span>
        </template>
      </el-table-column>
      <el-table-column prop="name" label="名称" width="220" />
      <el-table-column prop="desc" label="说明" width="120" />
      <el-table-column prop="usage" label="用途" min-width="180" />
      <el-table-column label="常见原因" min-width="260">
        <template #default="{ row }">
          <div v-if="row.causes" class="causes-cell">
            <span v-for="cause in splitItems(row.causes)" :key="cause" class="cause-tag">
              {{ cause }}
            </span>
          </div>
          <span v-else class="no-cause">-</span>
        </template>
      </el-table-column>
    </el-table>
  </div>
</template>

<script setup lang="ts">
import HttpStatusDetail from "./HttpStatusDetail.vue";
import type { HttpStatusCode } from "../types/httpStatus";

const props = defineProps<{
  data: HttpStatusCode[];
  expandedCodes: number[];
  showHeader?: boolean;
}>();

const emit = defineEmits<{
  "expand-change": [row: HttpStatusCode, expandedRows: HttpStatusCode[]];
}>();

function codeClass(code: number): string {
  return `code-${Math.floor(code / 100)}xx`;
}

function splitItems(value: string): string[] {
  return value
    .split("; ")
    .map((item) => item.trim())
    .filter(Boolean);
}

function handleExpandChange(row: HttpStatusCode, expandedRows: HttpStatusCode[]): void {
  emit("expand-change", row, expandedRows);
}

function handleRowClick(row: HttpStatusCode, _column: unknown, event: MouseEvent): void {
  if ((event.target as HTMLElement).closest(".el-table__expand-icon")) return;

  const isExpanded = props.expandedCodes.includes(row.code);
  const expandedRows = isExpanded
    ? props.data.filter((item) => item.code !== row.code)
    : [...props.data.filter((item) => item.code !== row.code), row];
  emit("expand-change", row, expandedRows);
}
</script>

<style scoped>
.code-cell {
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 13px;
  font-weight: 700;
}

.status-table-scroll {
  min-width: 0;
  width: 100%;
  overflow-x: auto;
}

.code-1xx {
  color: #909399;
}

.code-2xx {
  color: #67c23a;
}

.code-3xx {
  color: #e6a23c;
}

.code-4xx,
.code-5xx {
  color: #f56c6c;
}

.causes-cell {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.cause-tag {
  display: inline-block;
  padding: 1px 6px;
  border-radius: 3px;
  background: var(--el-fill-color);
  color: var(--el-text-color-regular);
  font-size: 11px;
  line-height: 1.6;
}

.no-cause {
  color: var(--el-text-color-placeholder);
}

.status-table :deep(.el-table__row) {
  cursor: pointer;
}
</style>
