<script setup lang="ts">
import { computed, nextTick } from "vue";
import type { ComponentPublicInstance } from "vue";
import type { RequestForwardLogRow } from "../../types/request-forward";
import { parseRequestForwardLogTimestamp } from "../../utils/requestForward";

const props = defineProps<{
  items: RequestForwardLogRow[];
  selectedId: number | null;
  loading: boolean;
  loadingMore: boolean;
  error: string;
  hasMore: boolean;
}>();

const emit = defineEmits<{
  select: [id: number];
  retry: [];
  "load-more": [];
}>();

const rowElements = new Map<number, HTMLButtonElement>();
const selectedIndex = computed(() =>
  props.items.findIndex((item) => item.id === props.selectedId),
);

function setLogRowRef(
  id: number,
  element: Element | ComponentPublicInstance | null,
) {
  const candidate = element instanceof HTMLElement ? element : element?.$el;
  if (candidate instanceof HTMLButtonElement) rowElements.set(id, candidate);
  else rowElements.delete(id);
}

function rowTabIndex(index: number): 0 | -1 {
  return index === selectedIndex.value || (selectedIndex.value < 0 && index === 0)
    ? 0
    : -1;
}

async function moveSelectionTo(index: number) {
  const target = props.items[index];
  if (!target) return;
  emit("select", target.id);
  await nextTick();
  rowElements.get(target.id)?.focus();
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function formatTime(value: string): string {
  const date = parseRequestForwardLogTimestamp(value);
  if (!date) return value;
  return date.toLocaleTimeString([], { hour12: false });
}

function requestTitle(log: RequestForwardLogRow): string {
  if (log.protocol !== "http") return `${log.protocol.toUpperCase()} 转发`;
  return [log.method, log.path].filter(Boolean).join(" ") || "HTTP 请求";
}

function outcomeLabel(log: RequestForwardLogRow): string {
  if (log.error) return "失败";
  return log.statusCode == null ? "成功" : String(log.statusCode);
}
</script>

<template>
  <div class="log-list" aria-live="polite">
    <div v-if="loading" class="log-state" role="status">正在加载转发日志…</div>
    <div v-else-if="error" class="log-state is-error" role="alert">
      <span>{{ error }}</span>
      <el-button size="small" @click="$emit('retry')">重新加载</el-button>
    </div>
    <div v-else-if="!items.length" class="log-state">暂无转发日志</div>

    <div
      v-else
      class="log-table"
      role="grid"
      aria-label="转发日志"
      :aria-colcount="8"
      :aria-rowcount="items.length + 1"
    >
      <div class="log-table__header" role="row" aria-rowindex="1">
        <span role="columnheader">结果</span>
        <span role="columnheader">请求 / 协议</span>
        <span role="columnheader">客户端</span>
        <span role="columnheader">目标</span>
        <span role="columnheader">上传</span>
        <span role="columnheader">下载</span>
        <span role="columnheader">耗时</span>
        <span role="columnheader">时间</span>
      </div>
      <button
        v-for="(log, index) in items"
        :key="log.id"
        :ref="(element) => setLogRowRef(log.id, element)"
        type="button"
        class="log-table__row"
        :class="{ 'is-selected': log.id === selectedId, 'is-error': Boolean(log.error) }"
        :aria-selected="log.id === selectedId"
        :aria-rowindex="index + 2"
        :tabindex="rowTabIndex(index)"
        :title="log.error || requestTitle(log)"
        role="row"
        @click="$emit('select', log.id)"
        @keydown.down.prevent="moveSelectionTo(Math.min(index + 1, items.length - 1))"
        @keydown.up.prevent="moveSelectionTo(Math.max(index - 1, 0))"
        @keydown.home.prevent="moveSelectionTo(0)"
        @keydown.end.prevent="moveSelectionTo(items.length - 1)"
      >
        <span role="gridcell"><b class="outcome" :class="log.error ? 'is-error' : 'is-success'">{{ outcomeLabel(log) }}</b></span>
        <span class="request-cell" role="gridcell"><i>{{ log.protocol.toUpperCase() }}</i>{{ requestTitle(log) }}</span>
        <span role="gridcell">{{ log.clientAddr ?? "未知" }}</span>
        <span role="gridcell">{{ log.targetAddr }}</span>
        <span role="gridcell">{{ formatBytes(log.uploadBytes) }}</span>
        <span role="gridcell">{{ formatBytes(log.downloadBytes) }}</span>
        <span role="gridcell">{{ log.durationMs == null ? "—" : `${log.durationMs} ms` }}</span>
        <time role="gridcell" :datetime="log.createdAt">{{ formatTime(log.createdAt) }}</time>
      </button>
    </div>

    <div v-if="items.length && hasMore" class="load-more">
      <el-button size="small" :loading="loadingMore" @click="$emit('load-more')">加载更多</el-button>
    </div>
  </div>
</template>

<style scoped>
.log-list {
  min-width: 0;
  container-name: forward-log-list;
  container-type: inline-size;
}
.log-state { display: flex; min-height: 104px; align-items: center; justify-content: center; gap: 10px; border: 1px dashed #d8dde5; color: #56667a; font-size: 16px; }
.log-state.is-error { border-color: #efc8c5; background: #fff8f7; color: #a9332d; }
.log-table { width: 100%; min-width: 0; border: 1px solid #dfe4e9; border-radius: 5px; overflow: hidden; }
.log-table__header,
.log-table__row {
  display: grid;
  grid-template-columns: 58px minmax(180px, 1.5fr) minmax(118px, 1fr) minmax(118px, 1fr) 68px 68px 68px 84px;
  align-items: center;
}
.log-table__header { min-height: 36px; border-bottom: 1px solid #dfe4e9; background: #f5f7f9; color: #5f6e81; font-size: 14px; font-weight: 700; }
.log-table__row { width: 100%; min-height: 42px; border: 0; border-bottom: 1px solid #edf0f3; padding: 0; background: #fff; color: #3f4e62; cursor: pointer; font: inherit; font-size: 14px; text-align: left; transition: background-color 150ms ease, box-shadow 150ms ease; }
.log-table__row:last-child { border-bottom: 0; }
.log-table__row:hover { background: #f7fafc; }
.log-table__row.is-selected { background: #eaf3f8; box-shadow: inset 3px 0 0 var(--el-color-primary, #409eff); }
.log-table__row:focus-visible { position: relative; z-index: 1; outline: 2px solid var(--el-color-primary, #409eff); outline-offset: -2px; }
.log-table__header > span,
.log-table__row > span,
.log-table__row > time { min-width: 0; overflow: hidden; padding: 0 6px; text-overflow: ellipsis; white-space: nowrap; }
.request-cell { display: flex; align-items: center; gap: 6px; color: #273548; font-size: 16px; }
.request-cell i { flex: none; color: #45627b; font-size: 12px; font-style: normal; font-weight: 800; }
.outcome { display: inline-block; min-width: 36px; border-radius: 3px; padding: 3px 5px; font-size: 12px; text-align: center; }
.outcome.is-success { background: #e8f6ef; color: #16724e; }
.outcome.is-error { background: #fdeceb; color: #ad3731; }
.load-more { display: flex; justify-content: center; padding: 8px 0 2px; }

@container forward-log-list (max-width: 820px) {
  .log-table__header,
  .log-table__row { grid-template-columns: 58px minmax(150px, 1.5fr) minmax(96px, 1fr) 64px 68px 80px; }
  .log-table__header > :nth-child(4),
  .log-table__row > :nth-child(4),
  .log-table__header > :nth-child(6),
  .log-table__row > :nth-child(6) { display: none; }
}

@container forward-log-list (max-width: 620px) {
  .log-table__header,
  .log-table__row { grid-template-columns: 54px minmax(0, 1fr) 66px 80px; }
  .log-table__header > :nth-child(3),
  .log-table__row > :nth-child(3),
  .log-table__header > :nth-child(5),
  .log-table__row > :nth-child(5) { display: none; }
}
</style>
