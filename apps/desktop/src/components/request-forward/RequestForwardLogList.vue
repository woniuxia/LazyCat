<script setup lang="ts">
import type { RequestForwardLogRow } from "../../types/request-forward";

defineProps<{
  items: RequestForwardLogRow[];
  selectedId: number | null;
  loading: boolean;
  loadingMore: boolean;
  error: string;
  hasMore: boolean;
}>();

defineEmits<{
  select: [id: number];
  retry: [];
  "load-more": [];
}>();

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
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

    <div v-else class="log-table" role="table" aria-label="转发日志">
      <div class="log-table__header" role="row">
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
        v-for="log in items"
        :key="log.id"
        type="button"
        class="log-table__row"
        :class="{ 'is-selected': log.id === selectedId, 'is-error': Boolean(log.error) }"
        :aria-selected="log.id === selectedId"
        :title="log.error || requestTitle(log)"
        role="row"
        @click="$emit('select', log.id)"
      >
        <span role="cell"><b class="outcome" :class="log.error ? 'is-error' : 'is-success'">{{ outcomeLabel(log) }}</b></span>
        <span class="request-cell" role="cell"><i>{{ log.protocol.toUpperCase() }}</i>{{ requestTitle(log) }}</span>
        <span role="cell">{{ log.clientAddr ?? "未知" }}</span>
        <span role="cell">{{ log.targetAddr }}</span>
        <span role="cell">{{ formatBytes(log.uploadBytes) }}</span>
        <span role="cell">{{ formatBytes(log.downloadBytes) }}</span>
        <span role="cell">{{ log.durationMs == null ? "—" : `${log.durationMs} ms` }}</span>
        <time role="cell" :datetime="log.createdAt">{{ formatTime(log.createdAt) }}</time>
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
.log-state { display: flex; min-height: 96px; align-items: center; justify-content: center; gap: 10px; border: 1px dashed #d8dde5; color: #64748b; font-size: 12px; }
.log-state.is-error { border-color: #efc8c5; background: #fff8f7; color: #a9332d; }
.log-table { width: 100%; min-width: 0; border: 1px solid #dfe4e9; border-radius: 5px; overflow: hidden; }
.log-table__header,
.log-table__row {
  display: grid;
  grid-template-columns: 58px minmax(180px, 1.5fr) minmax(118px, 1fr) minmax(118px, 1fr) 68px 68px 68px 84px;
  align-items: center;
}
.log-table__header { min-height: 28px; border-bottom: 1px solid #dfe4e9; background: #f5f7f9; color: #748194; font-size: 10px; font-weight: 700; }
.log-table__row { width: 100%; min-height: 34px; border: 0; border-bottom: 1px solid #edf0f3; padding: 0; background: #fff; color: #455468; cursor: pointer; font: inherit; font-size: 10px; text-align: left; transition: background-color 150ms ease, box-shadow 150ms ease; }
.log-table__row:last-child { border-bottom: 0; }
.log-table__row:hover { background: #f7fafc; }
.log-table__row.is-selected { background: #eaf3f8; box-shadow: inset 3px 0 0 var(--el-color-primary, #409eff); }
.log-table__row:focus-visible { position: relative; z-index: 1; outline: 2px solid var(--el-color-primary, #409eff); outline-offset: -2px; }
.log-table__header > span,
.log-table__row > span,
.log-table__row > time { min-width: 0; overflow: hidden; padding: 0 6px; text-overflow: ellipsis; white-space: nowrap; }
.request-cell { display: flex; align-items: center; gap: 5px; color: #273548; font-size: 11px; }
.request-cell i { flex: none; color: #527089; font-size: 8px; font-style: normal; font-weight: 800; }
.outcome { display: inline-block; min-width: 30px; border-radius: 3px; padding: 2px 4px; font-size: 9px; text-align: center; }
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
