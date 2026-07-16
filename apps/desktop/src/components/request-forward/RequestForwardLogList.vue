<script setup lang="ts">
import { ref } from "vue";
import type { RequestForwardLogRow } from "../../types/request-forward";

defineProps<{
  items: RequestForwardLogRow[];
  loading: boolean;
  loadingMore: boolean;
  error: string;
  hasMore: boolean;
}>();

defineEmits<{
  retry: [];
  "load-more": [];
}>();

const expandedIds = ref(new Set<number>());

function toggleDetails(id: number) {
  const next = new Set(expandedIds.value);
  next.has(id) ? next.delete(id) : next.add(id);
  expandedIds.value = next;
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function formatTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

function requestTitle(log: RequestForwardLogRow): string {
  if (log.protocol !== "http") return log.protocol.toUpperCase();
  return [log.method, log.path].filter(Boolean).join(" ") || "HTTP 请求";
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

    <article v-for="log in items" :key="log.id" class="log-row">
      <div class="log-row__summary">
        <div class="log-row__main">
          <span class="protocol-badge">{{ log.protocol.toUpperCase() }}</span>
          <strong>{{ requestTitle(log) }}</strong>
          <span
            class="outcome-badge"
            :class="log.error ? 'is-error' : 'is-success'"
          >
            {{ log.error ? "失败" : log.statusCode ?? "成功" }}
          </span>
        </div>
        <time :datetime="log.createdAt">{{ formatTime(log.createdAt) }}</time>
      </div>

      <dl class="log-row__facts">
        <div><dt>客户端</dt><dd>{{ log.clientAddr ?? "未知" }}</dd></div>
        <div><dt>目标</dt><dd>{{ log.targetAddr }}</dd></div>
        <div><dt>上传</dt><dd>{{ formatBytes(log.uploadBytes) }}</dd></div>
        <div><dt>下载</dt><dd>{{ formatBytes(log.downloadBytes) }}</dd></div>
        <div><dt>耗时</dt><dd>{{ log.durationMs == null ? "—" : `${log.durationMs} ms` }}</dd></div>
      </dl>

      <p v-if="log.error" class="log-row__error">{{ log.error }}</p>

      <template v-if="log.protocol === 'http'">
        <button
          class="detail-toggle"
          type="button"
          :aria-expanded="expandedIds.has(log.id)"
          @click="toggleDetails(log.id)"
        >
          {{ expandedIds.has(log.id) ? "收起 HTTP 详情" : "展开 HTTP 详情" }}
        </button>

        <div v-if="expandedIds.has(log.id)" class="http-details">
          <section v-if="log.requestHeaders?.length">
            <h4>请求头（已脱敏）</h4>
            <dl class="header-list">
              <div v-for="([name, value], index) in log.requestHeaders" :key="`${name}-${index}`">
                <dt>{{ name }}</dt><dd>{{ value }}</dd>
              </div>
            </dl>
          </section>
          <section v-if="log.responseHeaders?.length">
            <h4>响应头（已脱敏）</h4>
            <dl class="header-list">
              <div v-for="([name, value], index) in log.responseHeaders" :key="`${name}-${index}`">
                <dt>{{ name }}</dt><dd>{{ value }}</dd>
              </div>
            </dl>
          </section>
          <section v-if="log.requestBodyPreview != null">
            <h4>请求体预览 <span v-if="log.requestBodyTruncated">· 内容已截断</span></h4>
            <pre>{{ log.requestBodyPreview }}</pre>
          </section>
          <section v-if="log.responseBodyPreview != null">
            <h4>响应体预览 <span v-if="log.responseBodyTruncated">· 内容已截断</span></h4>
            <pre>{{ log.responseBodyPreview }}</pre>
          </section>
          <p
            v-if="!log.requestHeaders?.length && !log.responseHeaders?.length && log.requestBodyPreview == null && log.responseBodyPreview == null"
            class="http-details__empty"
          >
            本条日志未采集 HTTP 头或正文预览。
          </p>
        </div>
      </template>
    </article>

    <div v-if="items.length && hasMore" class="load-more">
      <el-button :loading="loadingMore" @click="$emit('load-more')">加载更多</el-button>
    </div>
  </div>
</template>

<style scoped>
.log-list { display: grid; gap: 8px; }
.log-state {
  display: flex;
  min-height: 82px;
  align-items: center;
  justify-content: center;
  gap: 12px;
  border: 1px dashed #d8dde5;
  border-radius: 6px;
  color: #64748b;
  font-size: 13px;
}
.log-state.is-error { border-color: #efc8c5; background: #fff8f7; color: #a9332d; }
.log-row { padding: 10px 12px; border: 1px solid #e2e6eb; border-radius: 6px; background: #fff; }
.log-row__summary { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.log-row__summary time { flex: none; color: #7a8695; font-size: 11px; }
.log-row__main { display: flex; min-width: 0; align-items: center; gap: 6px; }
.log-row__main strong { overflow: hidden; color: #273548; font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
.protocol-badge,
.outcome-badge { flex: none; border-radius: 3px; padding: 2px 5px; font-size: 10px; font-weight: 800; letter-spacing: .04em; }
.protocol-badge { background: #edf3f8; color: #45627b; }
.outcome-badge.is-success { background: #e8f6ef; color: #16724e; }
.outcome-badge.is-error { background: #fdeceb; color: #ad3731; }
.log-row__facts { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 8px; margin: 8px 0 0; }
.log-row__facts div { min-width: 0; }
.log-row__facts dt { color: #8792a0; font-size: 10px; }
.log-row__facts dd { overflow: hidden; margin: 2px 0 0; color: #475569; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
.log-row__error { margin: 8px 0 0; color: #ae3b35; font-size: 12px; line-height: 1.4; }
.detail-toggle { margin-top: 8px; border: 0; padding: 0; background: transparent; color: var(--el-color-primary, #409eff); cursor: pointer; font: inherit; font-size: 12px; }
.detail-toggle:focus-visible { outline: 2px solid var(--el-color-primary, #409eff); outline-offset: 3px; border-radius: 2px; }
.http-details { display: grid; gap: 10px; margin-top: 8px; padding: 10px; border-radius: 5px; background: #f7f9fb; }
.http-details h4 { margin: 0 0 6px; color: #48576a; font-size: 11px; }
.http-details h4 span { color: #a86608; font-weight: 500; }
.header-list { display: grid; gap: 4px; margin: 0; }
.header-list div { display: grid; grid-template-columns: minmax(100px, 28%) minmax(0, 1fr); gap: 10px; }
.header-list dt,
.header-list dd { overflow-wrap: anywhere; font: 11px/1.45 ui-monospace, SFMono-Regular, Consolas, monospace; }
.header-list dt { color: #59687a; }
.header-list dd { margin: 0; color: #344256; }
.http-details pre { max-height: 180px; margin: 0; overflow: auto; white-space: pre-wrap; overflow-wrap: anywhere; color: #344256; font: 11px/1.5 ui-monospace, SFMono-Regular, Consolas, monospace; }
.http-details__empty { margin: 0; color: #7a8695; font-size: 12px; }
.load-more { display: flex; justify-content: center; padding: 2px 0; }

@media (max-width: 880px) {
  .log-row__facts { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}

@media (max-width: 560px) {
  .log-row__summary { align-items: flex-start; flex-direction: column; gap: 6px; }
  .log-row__facts { grid-template-columns: minmax(0, 1fr); }
}
</style>
