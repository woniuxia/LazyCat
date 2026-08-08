<template>
  <main class="exception-stack-panel" aria-labelledby="exception-stack-title">
    <header class="exception-stack-header">
      <div class="exception-stack-heading">
        <span class="exception-stack-eyebrow">离线诊断</span>
        <h1 id="exception-stack-title">异常堆栈整理器</h1>
        <span class="exception-stack-status" role="status" aria-live="polite">
          {{ parseStatusLabel }}
        </span>
      </div>

      <div class="exception-stack-actions">
        <el-select
          v-model="formatOverride"
          class="exception-stack-format-select"
          size="small"
          aria-label="堆栈格式"
        >
          <el-option label="自动识别" value="auto" />
          <el-option label="JavaScript / TypeScript" value="javascript" />
          <el-option label="Java" value="java" />
        </el-select>
        <el-button
          size="small"
          :icon="FolderOpened"
          :loading="busyAction === 'open'"
          :disabled="busyAction !== ''"
          @click="openFile"
        >
          打开文件
        </el-button>
        <el-button
          size="small"
          type="primary"
          :icon="Promotion"
          :loading="busyAction === 'parse'"
          :disabled="busyAction !== ''"
          @click="parseStack"
        >
          解析
          <kbd>Ctrl+Enter</kbd>
        </el-button>
        <el-button
          size="small"
          text
          :icon="Delete"
          :disabled="busyAction !== '' || (!rawInput && !result)"
          @click="clearAll"
        >
          清空
        </el-button>
      </div>
    </header>

    <p v-if="errorMessage" class="exception-stack-error" role="alert">{{ errorMessage }}</p>

    <div class="exception-stack-workspace">
      <section
        class="exception-stack-section exception-stack-source"
        aria-labelledby="source-title"
      >
        <div class="exception-stack-section-head">
          <div>
            <span class="exception-stack-section-kicker">Evidence</span>
            <h2 id="source-title">原始输入</h2>
          </div>
          <span class="exception-stack-file" :title="sourcePath || '未关联文件'">
            {{ sourcePath ? sourceFileName : "未关联文件" }}
          </span>
        </div>

        <el-input
          v-model="rawInput"
          class="exception-stack-raw-input"
          type="textarea"
          resize="none"
          :autosize="{ minRows: 18, maxRows: 30 }"
          aria-label="原始异常堆栈"
          placeholder="粘贴一条 JavaScript / TypeScript 或 Java 异常堆栈"
          @keydown.ctrl.enter.prevent="parseStack"
        />

        <div class="exception-stack-source-footer">
          <span>{{ rawInput.length }} 字符</span>
          <span v-if="hasStaleResult" class="exception-stack-stale">结果对应上次解析</span>
          <span v-else-if="lastParseAttempted">{{ result?.ok ? "解析完成" : "解析失败" }}</span>
          <span v-else>等待一次明确解析</span>
        </div>
      </section>

      <section
        class="exception-stack-section exception-stack-result"
        aria-labelledby="result-title"
      >
        <div class="exception-stack-section-head">
          <div>
            <span class="exception-stack-section-kicker">Structured output</span>
            <h2 id="result-title">解析结果</h2>
          </div>
          <span class="exception-stack-detection">{{ detectionLabel }}</span>
        </div>

        <div v-if="result?.ok" class="exception-stack-result-body">
          <dl class="exception-stack-overview">
            <div>
              <dt>异常类型</dt>
              <dd>{{ result.rootException?.type || "未识别" }}</dd>
            </div>
            <div>
              <dt>消息</dt>
              <dd>{{ result.rootException?.message || "无消息" }}</dd>
            </div>
            <div>
              <dt>调用帧</dt>
              <dd>
                {{ result.frames.length }} 个<span v-if="result.omittedFrameCount"
                  >，省略 {{ result.omittedFrameCount }} 个</span
                >
                <span v-if="result.abbreviatedFrameCount"
                  >，公共帧标记省略 {{ result.abbreviatedFrameCount }} 个</span
                >
              </dd>
            </div>
            <div>
              <dt>原因链</dt>
              <dd>{{ result.causes.length ? `${result.causes.length} 层` : "无" }}</dd>
            </div>
          </dl>

          <section v-if="result.causes.length" class="exception-stack-subsection">
            <div class="exception-stack-subsection-head">
              <h3>原因链</h3>
              <span>{{ result.causes.length }} 层</span>
            </div>
            <ol class="exception-stack-cause-list">
              <li v-for="cause in result.causes" :key="`${cause.lineNumber}-${cause.type}`">
                <strong>{{ cause.type }}</strong>
                <span v-if="cause.message">{{ cause.message }}</span>
              </li>
            </ol>
          </section>

          <section class="exception-stack-subsection">
            <div class="exception-stack-subsection-head">
              <h3>调用帧</h3>
              <span>按原文顺序</span>
            </div>
            <div v-if="result.frames.length" class="exception-stack-frame-list">
              <div
                v-for="frame in result.frames"
                :key="`${frame.lineNumber}-${frame.raw}`"
                class="exception-stack-frame-row"
              >
                <span class="exception-stack-frame-number">{{ frame.lineNumber }}</span>
                <div class="exception-stack-frame-main">
                  <strong>{{ frame.functionName || "匿名调用" }}</strong>
                  <code :title="frame.filePath">{{ frame.filePath }}</code>
                </div>
                <span class="exception-stack-frame-location">{{ frame.line ?? "-" }}</span>
                <span class="exception-stack-frame-location">{{ frame.column ?? "-" }}</span>
              </div>
            </div>
            <p v-else class="exception-stack-muted">没有识别到调用帧。</p>
          </section>

          <section class="exception-stack-summary">
            <div class="exception-stack-subsection-head">
              <h3>规范化摘要</h3>
              <div class="exception-stack-summary-actions">
                <el-button
                  size="small"
                  :icon="CopyDocument"
                  :loading="busyAction === 'copy'"
                  :disabled="busyAction !== '' || hasStaleResult || !result.summary"
                  @click="copySummary"
                >
                  复制
                </el-button>
                <el-button
                  size="small"
                  :icon="Download"
                  :loading="busyAction === 'save'"
                  :disabled="busyAction !== '' || hasStaleResult || !result.summary"
                  @click="saveSummary"
                >
                  另存为
                </el-button>
              </div>
            </div>
            <pre class="exception-stack-summary-text">{{ result.summary }}</pre>
          </section>
        </div>

        <div v-else class="exception-stack-empty" role="status">
          <el-icon class="exception-stack-empty-icon"><Document /></el-icon>
          <strong>{{ result ? "这次解析没有生成摘要" : "尚未生成解析结果" }}</strong>
          <span>{{
            result ? "请查看下方诊断并调整格式或输入。" : "原文会保持不变，结果只在明确解析后更新。"
          }}</span>
        </div>
      </section>
    </div>

    <section
      v-if="result && result.diagnostics.length"
      class="exception-stack-diagnostics"
      aria-labelledby="diagnostics-title"
    >
      <div class="exception-stack-subsection-head">
        <h2 id="diagnostics-title">解析诊断</h2>
        <span>{{ result.diagnostics.length }} 项</span>
      </div>
      <ul class="exception-stack-diagnostic-list">
        <li v-for="diagnostic in result.diagnostics" :key="diagnostic">{{ diagnostic }}</li>
      </ul>
      <div v-if="result.unrecognizedLines.length" class="exception-stack-unrecognized">
        <span class="exception-stack-unrecognized-label">未识别原文</span>
        <ul>
          <li v-for="line in result.unrecognizedLines" :key="line.lineNumber">
            <span>第 {{ line.lineNumber }} 行</span>
            <code>{{ line.text }}</code>
          </li>
        </ul>
      </div>
    </section>
  </main>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  CopyDocument,
  Delete,
  Document,
  Download,
  FolderOpened,
  Promotion,
} from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type { FileReadTextResponse } from "../types";
import { fileNameFromPath, filePathsMatch } from "../utils/textWorkbench";
import {
  parseExceptionStack,
  type ExceptionStackFormat,
  type ExceptionStackFormatOverride,
  type ExceptionStackResult,
} from "../utils/exceptionStack";

type BusyAction = "" | "open" | "parse" | "copy" | "save";

const rawInput = ref("");
const sourcePath = ref("");
const formatOverride = ref<ExceptionStackFormatOverride>("auto");
const result = ref<ExceptionStackResult | null>(null);
const lastParsedInput = ref("");
const lastParsedFormat = ref<ExceptionStackFormatOverride>("auto");
const lastParseAttempted = ref(false);
const errorMessage = ref("");
const busyAction = ref<BusyAction>("");

const sourceFileName = computed(() => fileNameFromPath(sourcePath.value));
const hasStaleResult = computed(
  () =>
    lastParseAttempted.value &&
    (rawInput.value !== lastParsedInput.value || formatOverride.value !== lastParsedFormat.value),
);
const parseStatusLabel = computed(() => {
  if (hasStaleResult.value) return "原文或格式已修改，等待重新解析";
  if (!lastParseAttempted.value) return "等待解析";
  return result.value?.ok ? "已生成摘要" : "解析未完成";
});
const detectionLabel = computed(() => {
  if (!result.value) {
    return formatOverride.value === "auto"
      ? "自动识别"
      : `${formatLabel(formatOverride.value)}（手动）`;
  }
  if (result.value.formatSource === "manual") {
    return `${formatLabel(result.value.format)}（手动）`;
  }
  if (result.value.detection === "ambiguous") return "格式不明确";
  if (result.value.detection === "unsupported") return "格式不支持";
  return `已识别：${formatLabel(result.value.format)}`;
});

function formatLabel(format: ExceptionStackFormat | null): string {
  if (format === "java") return "Java";
  if (format === "javascript") return "JavaScript / TypeScript";
  return "未识别格式";
}

function errorText(error: unknown): string {
  if (error instanceof Error) return error.message;
  return typeof error === "string" ? error : String(error);
}

function isDialogCancellation(error: unknown): boolean {
  const message = errorText(error).toLowerCase();
  return (
    message === "cancel" || message === "canceled" || message === "cancelled" || message === "close"
  );
}

function showActionError(action: string, error: unknown): void {
  const detail = errorText(error) || "未知错误";
  errorMessage.value = `${action}失败：${detail}`;
  ElMessage.error(errorMessage.value);
}

function parseStack(): void {
  if (busyAction.value !== "") return;
  errorMessage.value = "";
  busyAction.value = "parse";
  try {
    result.value = parseExceptionStack(rawInput.value, formatOverride.value);
    lastParsedInput.value = rawInput.value;
    lastParsedFormat.value = formatOverride.value;
    lastParseAttempted.value = true;
  } finally {
    busyAction.value = "";
  }
}

async function openFile(): Promise<void> {
  if (busyAction.value !== "") return;
  errorMessage.value = "";
  busyAction.value = "open";
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: "文本堆栈", extensions: ["txt", "log", "stack", "trace"] }],
    });
    if (typeof selected !== "string") return;
    const response = await invokeToolByChannel<FileReadTextResponse>("tool:file:read-text", {
      path: selected,
    });
    rawInput.value = response.content;
    sourcePath.value = response.path;
    result.value = null;
    lastParsedInput.value = "";
    lastParsedFormat.value = "auto";
    lastParseAttempted.value = false;
    ElMessage.success(`已载入 ${fileNameFromPath(response.path)}`);
  } catch (error) {
    if (!isDialogCancellation(error)) showActionError("打开文件", error);
  } finally {
    busyAction.value = "";
  }
}

async function saveSummary(): Promise<void> {
  if (busyAction.value !== "" || !result.value?.summary) return;
  errorMessage.value = "";
  busyAction.value = "save";
  try {
    const sourceName = sourcePath.value ? fileNameFromPath(sourcePath.value) : "stack.txt";
    const selected = await save({
      defaultPath: `exception-summary-${sourceName}`,
      filters: [{ name: "文本摘要", extensions: ["txt"] }],
    });
    if (!selected) return;
    if (sourcePath.value && filePathsMatch(selected, sourcePath.value)) {
      showActionError("另存为", new Error("目标路径不能覆盖原始堆栈文件"));
      return;
    }
    await invokeToolByChannel("tool:file:write-text", {
      path: selected,
      content: result.value.summary,
    });
    ElMessage.success(`已保存 ${fileNameFromPath(selected)}`);
  } catch (error) {
    if (!isDialogCancellation(error)) showActionError("另存为", error);
  } finally {
    busyAction.value = "";
  }
}

async function copySummary(): Promise<void> {
  if (busyAction.value !== "" || !result.value?.summary) return;
  errorMessage.value = "";
  busyAction.value = "copy";
  try {
    await navigator.clipboard.writeText(result.value.summary);
    ElMessage.success("摘要已复制");
  } catch (error) {
    showActionError("复制摘要", error);
  } finally {
    busyAction.value = "";
  }
}

function clearAll(): void {
  if (busyAction.value !== "") return;
  rawInput.value = "";
  sourcePath.value = "";
  result.value = null;
  lastParsedInput.value = "";
  lastParsedFormat.value = "auto";
  lastParseAttempted.value = false;
  errorMessage.value = "";
}
</script>

<style scoped>
.exception-stack-panel {
  display: flex;
  flex: 1;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  gap: 14px;
  container: exception-stack / inline-size;
}

.exception-stack-header,
.exception-stack-heading,
.exception-stack-actions,
.exception-stack-section-head,
.exception-stack-subsection-head,
.exception-stack-source-footer,
.exception-stack-summary-actions {
  display: flex;
  align-items: center;
}

.exception-stack-header {
  justify-content: space-between;
  gap: 18px;
  min-width: 0;
}

.exception-stack-heading {
  min-width: 0;
  flex-wrap: wrap;
  gap: 10px 14px;
}

.exception-stack-eyebrow,
.exception-stack-section-kicker,
.exception-stack-unrecognized-label {
  color: var(--lc-accent);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.exception-stack-heading h1,
.exception-stack-section-head h2,
.exception-stack-subsection-head h3 {
  margin: 0;
  color: var(--lc-text);
}

.exception-stack-heading h1 {
  font-size: 20px;
  font-weight: 700;
}

.exception-stack-status,
.exception-stack-detection,
.exception-stack-file,
.exception-stack-source-footer,
.exception-stack-subsection-head > span {
  color: var(--lc-text-muted);
  font-size: 12px;
}

.exception-stack-actions {
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
}

.exception-stack-actions :deep(.el-button + .el-button) {
  margin-left: 0;
}

.exception-stack-format-select {
  width: 174px;
}

kbd {
  margin-left: 6px;
  padding: 1px 4px;
  border: 1px solid color-mix(in srgb, var(--el-button-text-color) 25%, transparent);
  border-radius: 4px;
  font-family: inherit;
  font-size: 10px;
  opacity: 0.75;
}

.exception-stack-error {
  margin: 0;
  padding: 9px 12px;
  border: 1px solid color-mix(in srgb, var(--el-color-danger) 30%, transparent);
  border-left: 3px solid var(--el-color-danger);
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger-dark-2);
  font-size: 13px;
  overflow-wrap: anywhere;
}

.exception-stack-workspace {
  display: grid;
  grid-template-columns: minmax(280px, 0.95fr) minmax(360px, 1.05fr);
  align-items: stretch;
  gap: 14px;
  min-width: 0;
}

.exception-stack-section,
.exception-stack-diagnostics {
  min-width: 0;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md, 10px);
  background: var(--el-bg-color);
}

.exception-stack-section {
  display: flex;
  min-height: 520px;
  flex-direction: column;
  overflow: hidden;
}

.exception-stack-section-head {
  justify-content: space-between;
  gap: 12px;
  min-height: 58px;
  padding: 11px 14px;
  border-bottom: 1px solid var(--lc-border);
}

.exception-stack-section-head > div:first-child {
  min-width: 0;
}

.exception-stack-section-head h2 {
  margin-top: 3px;
  font-size: 15px;
  font-weight: 700;
}

.exception-stack-file,
.exception-stack-detection {
  max-width: 48%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.exception-stack-raw-input {
  flex: 1;
  min-height: 0;
}

.exception-stack-raw-input :deep(.el-textarea__inner) {
  height: 100% !important;
  min-height: 420px !important;
  padding: 14px;
  border: 0;
  border-radius: 0;
  box-shadow: none !important;
  background: transparent;
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 12px;
  line-height: 1.65;
  resize: none;
}

.exception-stack-source-footer {
  justify-content: space-between;
  gap: 12px;
  min-height: 34px;
  padding: 0 14px;
  border-top: 1px solid var(--lc-border);
}

.exception-stack-stale {
  color: var(--el-color-warning-dark-2);
}

.exception-stack-result-body {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 16px;
  padding: 14px;
  overflow: auto;
}

.exception-stack-overview {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px 16px;
  margin: 0;
}

.exception-stack-overview > div {
  min-width: 0;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--lc-border);
}

.exception-stack-overview dt {
  color: var(--lc-text-muted);
  font-size: 11px;
}

.exception-stack-overview dd {
  margin: 4px 0 0;
  color: var(--lc-text);
  font-size: 13px;
  font-weight: 600;
  overflow-wrap: anywhere;
}

.exception-stack-subsection {
  min-width: 0;
}

.exception-stack-subsection-head {
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 8px;
}

.exception-stack-subsection-head h3,
.exception-stack-diagnostics h2 {
  font-size: 12px;
  font-weight: 700;
}

.exception-stack-cause-list,
.exception-stack-diagnostic-list,
.exception-stack-unrecognized ul {
  margin: 0;
  padding-left: 18px;
}

.exception-stack-cause-list li,
.exception-stack-diagnostic-list li {
  padding: 4px 0;
  color: var(--lc-text-secondary);
  font-size: 12px;
  overflow-wrap: anywhere;
}

.exception-stack-cause-list strong {
  color: var(--lc-text);
  margin-right: 6px;
}

.exception-stack-frame-list {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.exception-stack-frame-row {
  display: grid;
  grid-template-columns: 28px minmax(0, 1fr) 42px 42px;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding: 7px 8px;
  border: 1px solid var(--lc-border);
  background: var(--el-fill-color-lighter);
}

.exception-stack-frame-number,
.exception-stack-frame-location {
  color: var(--lc-text-muted);
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 11px;
}

.exception-stack-frame-location {
  text-align: right;
}

.exception-stack-frame-main {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}

.exception-stack-frame-main strong,
.exception-stack-frame-main code {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.exception-stack-frame-main strong {
  color: var(--lc-text);
  font-size: 12px;
}

.exception-stack-frame-main code,
.exception-stack-unrecognized code {
  color: var(--lc-text-secondary);
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 11px;
}

.exception-stack-summary {
  min-width: 0;
  padding-top: 13px;
  border-top: 1px solid var(--lc-border);
}

.exception-stack-summary-actions {
  gap: 6px;
}

.exception-stack-summary-actions :deep(.el-button + .el-button) {
  margin-left: 0;
}

.exception-stack-summary-text {
  max-height: 220px;
  margin: 0;
  padding: 11px 12px;
  border: 1px solid var(--lc-border);
  background: var(--el-fill-color-lighter);
  color: var(--lc-text);
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 11px;
  line-height: 1.6;
  overflow: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.exception-stack-empty {
  display: flex;
  flex: 1;
  min-height: 400px;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 8px;
  padding: 24px;
  color: var(--lc-text-muted);
  text-align: center;
}

.exception-stack-empty strong {
  color: var(--lc-text);
  font-size: 14px;
}

.exception-stack-empty span {
  max-width: 280px;
  font-size: 12px;
  line-height: 1.6;
}

.exception-stack-empty-icon {
  margin-bottom: 4px;
  color: var(--lc-accent);
  font-size: 28px;
}

.exception-stack-muted {
  margin: 0;
  color: var(--lc-text-muted);
  font-size: 12px;
}

.exception-stack-diagnostics {
  padding: 13px 14px;
  border-left: 3px solid var(--el-color-warning);
}

.exception-stack-diagnostics .exception-stack-subsection-head {
  margin-bottom: 5px;
}

.exception-stack-unrecognized {
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid var(--lc-border);
}

.exception-stack-unrecognized-label {
  display: block;
  margin-bottom: 5px;
  color: var(--el-color-warning-dark-2);
}

.exception-stack-unrecognized li {
  display: grid;
  grid-template-columns: 68px minmax(0, 1fr);
  gap: 8px;
  padding: 3px 0;
  color: var(--lc-text-muted);
  font-size: 11px;
}

.exception-stack-unrecognized code {
  min-width: 0;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

@container exception-stack (max-width: 1000px) {
  .exception-stack-header {
    align-items: flex-start;
    flex-direction: column;
  }

  .exception-stack-actions {
    justify-content: flex-start;
  }
}

@container exception-stack (max-width: 760px) {
  .exception-stack-workspace {
    grid-template-columns: minmax(0, 1fr);
  }

  .exception-stack-section {
    min-height: 0;
  }

  .exception-stack-source {
    min-height: 390px;
  }

  .exception-stack-result {
    min-height: 420px;
  }
}

@container exception-stack (max-width: 560px) {
  .exception-stack-actions {
    width: 100%;
  }

  .exception-stack-format-select {
    width: 100%;
  }

  .exception-stack-actions :deep(.el-button) {
    flex: 1;
  }

  .exception-stack-actions :deep(.el-button:last-child) {
    flex: 0 0 auto;
  }

  .exception-stack-overview {
    grid-template-columns: minmax(0, 1fr);
  }

  .exception-stack-frame-row {
    grid-template-columns: 24px minmax(0, 1fr) 32px 32px;
    gap: 5px;
  }

  .exception-stack-summary .exception-stack-subsection-head {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
