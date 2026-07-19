<script setup lang="ts">
import { Close, CopyDocument } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import type { RequestForwardLogRow } from "../../types/request-forward";
import {
  formatRequestForwardLogBody,
  getRequestForwardLogCopyText,
  parseRequestForwardLogTimestamp,
} from "../../utils/requestForward";
import type { RequestForwardLogCopySection } from "../../utils/requestForward";

defineProps<{ log: RequestForwardLogRow | null }>();
defineEmits<{ close: [] }>();

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function formatTime(value: string): string {
  return parseRequestForwardLogTimestamp(value)?.toLocaleString() ?? value;
}

function requestTitle(log: RequestForwardLogRow): string {
  if (log.protocol !== "http") return `${log.protocol.toUpperCase()} 转发`;
  return [log.method, log.path].filter(Boolean).join(" ") || "HTTP 请求";
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function copyLogSection(
  log: RequestForwardLogRow,
  section: RequestForwardLogCopySection,
  label: string,
) {
  try {
    await navigator.clipboard.writeText(getRequestForwardLogCopyText(log, section));
    ElMessage.success(`已复制${label}`);
  } catch (error) {
    ElMessage.error(`复制${label}失败：${errorMessage(error)}`);
  }
}
</script>

<template>
  <div class="log-inspector">
    <div v-if="!log" class="log-inspector__empty">
      <strong>选择一条日志查看详情</strong>
      <span>HTTP 请求头与正文预览会在这里展开。</span>
    </div>

    <template v-else>
      <header class="log-inspector__header">
        <div>
          <p>{{ log.protocol.toUpperCase() }} · LOG #{{ log.id }}</p>
          <h2>{{ requestTitle(log) }}</h2>
        </div>
        <div class="log-inspector__header-actions">
          <el-button
            size="small"
            :icon="CopyDocument"
            @click="copyLogSection(log, 'full', '完整日志')"
          >
            复制完整日志
          </el-button>
          <el-tooltip content="关闭详情" placement="bottom">
            <el-button text circle :icon="Close" aria-label="关闭日志详情" @click="$emit('close')" />
          </el-tooltip>
        </div>
      </header>

      <div class="log-inspector__scroll">
        <section class="summary-grid" aria-label="日志概要">
          <div><span>结果</span><strong :class="{ 'is-error': log.error }">{{ log.error ? "失败" : log.statusCode ?? "成功" }}</strong></div>
          <div><span>耗时</span><strong>{{ log.durationMs == null ? "—" : `${log.durationMs} ms` }}</strong></div>
          <div><span>客户端</span><strong>{{ log.clientAddr ?? "未知" }}</strong></div>
          <div><span>目标</span><strong>{{ log.targetAddr }}</strong></div>
          <div><span>上传</span><strong>{{ formatBytes(log.uploadBytes) }}</strong></div>
          <div><span>下载</span><strong>{{ formatBytes(log.downloadBytes) }}</strong></div>
          <div class="is-wide"><span>时间</span><strong>{{ formatTime(log.createdAt) }}</strong></div>
        </section>

        <section v-if="log.error" class="error-detail">
          <div class="detail-section__heading">
            <h3>错误信息</h3>
            <el-tooltip content="复制错误信息" placement="left">
              <el-button
                text
                circle
                :icon="CopyDocument"
                aria-label="复制错误信息"
                @click="copyLogSection(log, 'error', '错误信息')"
              />
            </el-tooltip>
          </div>
          <p>{{ log.error }}</p>
        </section>

        <template v-if="log.protocol === 'http'">
          <section v-if="log.requestHeaders?.length" class="detail-section">
            <div class="detail-section__heading">
              <h3>请求头（已脱敏）</h3>
              <el-tooltip content="复制请求头" placement="left">
                <el-button
                  text
                  circle
                  :icon="CopyDocument"
                  aria-label="复制请求头"
                  @click="copyLogSection(log, 'requestHeaders', '请求头')"
                />
              </el-tooltip>
            </div>
            <dl class="header-list">
              <div v-for="([name, value], index) in log.requestHeaders" :key="`${name}-${index}`">
                <dt>{{ name }}</dt><dd>{{ value }}</dd>
              </div>
            </dl>
          </section>
          <section v-if="log.responseHeaders?.length" class="detail-section">
            <div class="detail-section__heading">
              <h3>响应头（已脱敏）</h3>
              <el-tooltip content="复制响应头" placement="left">
                <el-button
                  text
                  circle
                  :icon="CopyDocument"
                  aria-label="复制响应头"
                  @click="copyLogSection(log, 'responseHeaders', '响应头')"
                />
              </el-tooltip>
            </div>
            <dl class="header-list">
              <div v-for="([name, value], index) in log.responseHeaders" :key="`${name}-${index}`">
                <dt>{{ name }}</dt><dd>{{ value }}</dd>
              </div>
            </dl>
          </section>
          <section v-if="log.requestBodyPreview != null" class="detail-section">
            <div class="detail-section__heading">
              <h3>请求体预览 <span v-if="log.requestBodyTruncated">· 内容已截断</span></h3>
              <el-tooltip content="复制请求体" placement="left">
                <el-button
                  text
                  circle
                  :icon="CopyDocument"
                  aria-label="复制请求体"
                  @click="copyLogSection(log, 'requestBody', '请求体')"
                />
              </el-tooltip>
            </div>
            <pre>{{ formatRequestForwardLogBody(log.requestBodyPreview, log.requestHeaders) }}</pre>
          </section>
          <section v-if="log.responseBodyPreview != null" class="detail-section">
            <div class="detail-section__heading">
              <h3>响应体预览 <span v-if="log.responseBodyTruncated">· 内容已截断</span></h3>
              <el-tooltip content="复制响应体" placement="left">
                <el-button
                  text
                  circle
                  :icon="CopyDocument"
                  aria-label="复制响应体"
                  @click="copyLogSection(log, 'responseBody', '响应体')"
                />
              </el-tooltip>
            </div>
            <pre>{{ formatRequestForwardLogBody(log.responseBodyPreview, log.responseHeaders) }}</pre>
          </section>
          <p
            v-if="!log.requestHeaders?.length && !log.responseHeaders?.length && log.requestBodyPreview == null && log.responseBodyPreview == null"
            class="detail-empty"
          >
            本条日志未采集 HTTP 头或正文预览。
          </p>
        </template>
      </div>
    </template>
  </div>
</template>

<style scoped>
.log-inspector { display: flex; min-width: 0; min-height: 0; flex: 1; flex-direction: column; background: #fbfcfd; }
.log-inspector__empty { display: grid; margin: auto; justify-items: center; gap: 6px; padding: 24px; color: #7a8797; text-align: center; }
.log-inspector__empty strong { color: #405065; font-size: 16px; }
.log-inspector__empty span { max-width: 280px; font-size: 14px; line-height: 1.5; }
.log-inspector__header { display: flex; min-height: 58px; flex: none; align-items: flex-start; justify-content: space-between; gap: 12px; padding: 11px 12px 9px; border-bottom: 1px solid #dfe4e9; background: #fff; }
.log-inspector__header > div:first-child { min-width: 0; }
.log-inspector__header-actions { display: flex; flex: none; align-items: center; gap: 4px; }
.log-inspector__header p { margin: 0 0 4px; color: #657386; font-size: 12px; font-weight: 800; letter-spacing: .08em; }
.log-inspector__header h2 { overflow-wrap: anywhere; margin: 0; color: #273548; font-size: 18px; line-height: 1.4; }
.log-inspector__scroll { min-height: 0; flex: 1; overflow: auto; padding: 12px; }
.summary-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); border: 1px solid #e0e5ea; border-radius: 5px; background: #fff; overflow: hidden; }
.summary-grid div { min-width: 0; padding: 7px 8px; border-right: 1px solid #e8ecf0; border-bottom: 1px solid #e8ecf0; }
.summary-grid div:nth-child(even) { border-right: 0; }
.summary-grid .is-wide { grid-column: 1 / -1; border-right: 0; border-bottom: 0; }
.summary-grid span { display: block; margin-bottom: 3px; color: #657386; font-size: 12px; }
.summary-grid strong { display: block; overflow: hidden; color: #3d4b5f; font: 14px/1.5 ui-monospace, SFMono-Regular, Consolas, monospace; text-overflow: ellipsis; white-space: nowrap; }
.summary-grid strong.is-error { color: #b23d36; }
.detail-section,
.error-detail { margin-top: 12px; padding-top: 11px; border-top: 1px solid #e2e7ec; }
.detail-section h3,
.error-detail h3 { margin: 0; color: #48576a; font-size: 14px; }
.detail-section__heading { display: flex; min-height: 28px; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 5px; }
.detail-section__heading :deep(.el-button) { flex: none; }
.detail-section h3 span { color: #a86608; font-weight: 500; }
.error-detail p { margin: 0; overflow-wrap: anywhere; color: #ae3b35; font-size: 14px; line-height: 1.55; }
.header-list { display: grid; gap: 4px; margin: 0; }
.header-list div { display: grid; grid-template-columns: minmax(90px, 30%) minmax(0, 1fr); gap: 8px; }
.header-list dt,
.header-list dd { overflow-wrap: anywhere; font: 14px/1.5 ui-monospace, SFMono-Regular, Consolas, monospace; }
.header-list dt { color: #59687a; }
.header-list dd { margin: 0; color: #344256; }
.detail-section pre { max-height: 300px; margin: 0; overflow: auto; padding: 10px; border: 1px solid #e0e5ea; border-radius: 4px; background: #f5f7f9; white-space: pre-wrap; overflow-wrap: anywhere; color: #344256; font: 14px/1.55 ui-monospace, SFMono-Regular, Consolas, monospace; }
.detail-empty { margin: 18px 0 0; color: #657386; font-size: 14px; text-align: center; }
</style>
