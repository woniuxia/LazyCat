<template>
  <div class="api-response-viewer">
    <div class="api-response-viewer__toolbar">
      <el-radio-group v-model="mode" size="small">
        <el-radio-button v-if="hasPreview" label="preview">预览</el-radio-button>
        <el-radio-button label="raw">{{ rawModeLabel }}</el-radio-button>
        <el-radio-button label="meta">元信息</el-radio-button>
      </el-radio-group>
      <div class="api-response-viewer__actions">
        <el-button size="small" :icon="CopyDocument" @click="$emit('copy-body')">
          复制响应体
        </el-button>
        <el-button size="small" :icon="CopyDocument" @click="$emit('copy-url')">
          复制 URL
        </el-button>
        <el-button
          v-if="response.bodyFilePath"
          size="small"
          :icon="CopyDocument"
          @click="copyCachePath"
        >
          复制路径
        </el-button>
        <el-button v-if="response.bodyFilePath" size="small" @click="openCacheFile">
          打开文件
        </el-button>
        <el-button v-if="response.bodyFilePath" size="small" @click="revealCacheFile">
          定位
        </el-button>
        <el-button size="small" type="primary" :icon="DocumentChecked" @click="$emit('save-example')">
          保存为示例
        </el-button>
      </div>
    </div>

    <el-alert
      v-if="response.bodyPreviewError"
      class="api-response-viewer__alert"
      type="warning"
      :title="response.bodyPreviewError"
      show-icon
      :closable="false"
    />

    <div v-if="mode === 'preview'" class="api-response-viewer__content">
      <el-empty v-if="viewerKind === 'empty'" description="无响应体" />

      <pre v-else-if="viewerKind === 'json'" class="api-response-viewer__code">{{ previewText }}</pre>

      <iframe
        v-else-if="viewerKind === 'html'"
        class="api-response-viewer__frame"
        sandbox
        :srcdoc="response.bodyText"
      />

      <div v-else-if="viewerKind === 'image'" class="api-response-viewer__media">
        <img :src="assetUrl" alt="响应图片预览" />
      </div>

      <iframe
        v-else-if="viewerKind === 'pdf'"
        class="api-response-viewer__frame"
        :src="assetUrl"
      />

      <div v-else-if="isOfficeKind" class="api-response-viewer__office">
        <div v-if="officeLoading" class="api-response-viewer__state">正在解析基础预览...</div>
        <el-alert
          v-else-if="officeError"
          type="warning"
          :title="officeError"
          show-icon
          :closable="false"
        />
        <template v-else-if="officePreview">
          <el-alert
            v-if="officePreview.unsupported"
            type="info"
            :title="officePreview.message || '该格式暂不支持基础预览'"
            show-icon
            :closable="false"
          />
          <div v-else-if="viewerKind === 'office-word'" class="api-response-viewer__document">
            <p v-for="(paragraph, index) in officePreview.paragraphs || []" :key="index">
              {{ paragraph }}
            </p>
            <el-empty
              v-if="!(officePreview.paragraphs || []).length"
              description="未提取到可显示文本"
            />
          </div>
          <div v-else-if="viewerKind === 'office-sheet'" class="api-response-viewer__sheet">
            <el-select
              v-if="(officePreview.sheetNames || []).length > 1"
              v-model="activeSheet"
              size="small"
              class="api-response-viewer__sheet-select"
              @change="loadOfficePreview"
            >
              <el-option
                v-for="sheet in officePreview.sheetNames || []"
                :key="sheet"
                :label="sheet"
                :value="sheet"
              />
            </el-select>
            <table>
              <tbody>
                <tr v-for="(row, rowIndex) in officePreview.rows || []" :key="rowIndex">
                  <td v-for="(cell, cellIndex) in row" :key="cellIndex">{{ cell }}</td>
                </tr>
              </tbody>
            </table>
          </div>
          <div v-else-if="viewerKind === 'office-slides'" class="api-response-viewer__slides">
            <section
              v-for="slide in officePreview.slides || []"
              :key="slide.index"
              class="api-response-viewer__slide"
            >
              <strong>{{ slide.index }}. {{ slide.title }}</strong>
              <p v-for="(text, index) in slide.texts || []" :key="index">{{ text }}</p>
            </section>
          </div>
        </template>
      </div>

      <div v-else class="api-response-viewer__binary">
        <strong>{{ response.bodyFileName || "二进制响应" }}</strong>
        <span>{{ response.bodySize }} bytes</span>
        <span v-if="response.contentType">{{ response.contentType }}</span>
      </div>
    </div>

    <pre v-else-if="mode === 'raw'" class="api-response-viewer__code">{{ rawText }}</pre>

    <dl v-else class="api-response-viewer__meta">
      <dt>状态</dt>
      <dd>{{ response.status ?? "ERR" }} {{ response.statusText }}</dd>
      <dt>最终 URL</dt>
      <dd>{{ response.finalUrl }}</dd>
      <dt>Content-Type</dt>
      <dd>{{ response.contentType || "无" }}</dd>
      <dt>响应大小</dt>
      <dd>{{ response.bodySize }} bytes</dd>
      <dt>存储方式</dt>
      <dd>{{ response.bodyStorage }}</dd>
      <dt v-if="response.bodyFilePath">缓存路径</dt>
      <dd v-if="response.bodyFilePath">{{ response.bodyFilePath }}</dd>
      <dt v-if="response.bodyHash">Hash</dt>
      <dd v-if="response.bodyHash">{{ response.bodyHash }}</dd>
    </dl>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { CopyDocument, DocumentChecked } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type { ApiWorkbenchSendResult } from "../types/api-workbench";
import {
  formatApiWorkbenchPreviewBody,
  getApiWorkbenchResponseViewerKind,
  getDefaultApiWorkbenchResponseMode,
  type ApiWorkbenchResponseMode,
} from "../utils/apiWorkbenchResponsePreview";

type OfficePreview = Record<string, any>;

const props = defineProps<{
  response: ApiWorkbenchSendResult;
}>();

defineEmits<{
  (event: "copy-body"): void;
  (event: "copy-url"): void;
  (event: "save-example"): void;
}>();

const mode = ref<ApiWorkbenchResponseMode>(getDefaultApiWorkbenchResponseMode(props.response));
const officePreview = ref<OfficePreview | null>(null);
const officeLoading = ref(false);
const officeError = ref("");
const activeSheet = ref("");
let officeRequestSeq = 0;

const viewerKind = computed(() => getApiWorkbenchResponseViewerKind(props.response));
const previewText = computed(() => formatApiWorkbenchPreviewBody(props.response));
const rawText = computed(() => props.response.bodyText || "该响应没有可显示的文本内容。");
const rawModeLabel = computed(() => (viewerKind.value === "html" ? "源码" : "原文"));
const hasPreview = computed(() => viewerKind.value !== "binary" && viewerKind.value !== "empty");
const isOfficeKind = computed(() => viewerKind.value.startsWith("office-"));
const assetUrl = computed(() =>
  props.response.bodyFilePath ? convertFileSrc(props.response.bodyFilePath) : "",
);

function officeKindParam(): "word" | "sheet" | "slides" {
  if (viewerKind.value === "office-sheet") return "sheet";
  if (viewerKind.value === "office-slides") return "slides";
  return "word";
}

async function copyCachePath() {
  if (!props.response.bodyFilePath) return;
  await navigator.clipboard.writeText(props.response.bodyFilePath);
  ElMessage.success("缓存路径已复制");
}

async function openCacheFile() {
  if (!props.response.bodyFilePath) return;
  await invokeToolByChannel("tool:api-workbench:response-cache-open", {
    filePath: props.response.bodyFilePath,
  });
}

async function revealCacheFile() {
  if (!props.response.bodyFilePath) return;
  await invokeToolByChannel("tool:api-workbench:response-cache-reveal", {
    filePath: props.response.bodyFilePath,
  });
}

async function loadOfficePreview() {
  if (!props.response.bodyFilePath || !isOfficeKind.value) return;
  const seq = ++officeRequestSeq;
  officeLoading.value = true;
  officeError.value = "";
  try {
    const result = (await invokeToolByChannel("tool:api-workbench:response-preview-office", {
      filePath: props.response.bodyFilePath,
      kind: officeKindParam(),
      sheetName: activeSheet.value || undefined,
    })) as OfficePreview;
    if (seq !== officeRequestSeq) return;
    officePreview.value = result;
    if (!activeSheet.value && typeof result.activeSheet === "string") {
      activeSheet.value = result.activeSheet;
    }
  } catch (error) {
    if (seq !== officeRequestSeq) return;
    officeError.value = error instanceof Error ? error.message : "基础预览失败";
    officePreview.value = null;
  } finally {
    if (seq === officeRequestSeq) {
      officeLoading.value = false;
    }
  }
}

watch(
  () => props.response,
  (next) => {
    mode.value = getDefaultApiWorkbenchResponseMode(next);
    officePreview.value = null;
    officeError.value = "";
    activeSheet.value = "";
  },
  { deep: false },
);

watch(
  [mode, viewerKind, () => props.response.bodyFilePath],
  () => {
    if (mode.value === "preview" && isOfficeKind.value) {
      void loadOfficePreview();
    }
  },
  { immediate: true },
);
</script>

<style scoped>
.api-response-viewer {
  display: flex;
  min-height: 0;
  flex-direction: column;
  gap: 8px;
}

.api-response-viewer__toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.api-response-viewer__actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 6px;
}

.api-response-viewer__alert {
  flex: none;
}

.api-response-viewer__content,
.api-response-viewer__office {
  min-height: 0;
}

.api-response-viewer__code {
  min-height: 360px;
  max-height: 58vh;
  margin: 0;
  overflow: auto;
  border: 1px solid var(--el-border-color-extra-light);
  border-radius: 6px;
  background: var(--el-fill-color-blank);
  font-family: var(--lc-font-mono);
  font-size: 12px;
  line-height: 1.55;
  padding: 10px;
  white-space: pre-wrap;
  word-break: break-word;
}

.api-response-viewer__frame {
  width: 100%;
  min-height: 420px;
  border: 1px solid var(--el-border-color-extra-light);
  border-radius: 6px;
  background: var(--el-fill-color-blank);
}

.api-response-viewer__media {
  display: flex;
  min-height: 360px;
  align-items: center;
  justify-content: center;
  overflow: auto;
  border: 1px solid var(--el-border-color-extra-light);
  border-radius: 6px;
  background: var(--el-fill-color-blank);
}

.api-response-viewer__media img {
  max-width: 100%;
  max-height: 58vh;
  object-fit: contain;
}

.api-response-viewer__document,
.api-response-viewer__sheet,
.api-response-viewer__slides,
.api-response-viewer__binary,
.api-response-viewer__meta {
  border: 1px solid var(--el-border-color-extra-light);
  border-radius: 6px;
  background: var(--el-fill-color-blank);
  padding: 10px;
}

.api-response-viewer__document {
  max-height: 58vh;
  overflow: auto;
  line-height: 1.7;
}

.api-response-viewer__sheet {
  max-height: 58vh;
  overflow: auto;
}

.api-response-viewer__sheet-select {
  width: 220px;
  margin-bottom: 8px;
}

.api-response-viewer__sheet table {
  width: max-content;
  min-width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.api-response-viewer__sheet td {
  max-width: 260px;
  border: 1px solid var(--el-border-color-extra-light);
  padding: 5px 7px;
  vertical-align: top;
  word-break: break-word;
}

.api-response-viewer__slides {
  display: grid;
  max-height: 58vh;
  gap: 8px;
  overflow: auto;
}

.api-response-viewer__slide {
  border-bottom: 1px solid var(--el-border-color-extra-light);
  padding-bottom: 8px;
}

.api-response-viewer__slide p {
  margin: 4px 0 0;
}

.api-response-viewer__binary {
  display: grid;
  gap: 6px;
  color: var(--el-text-color-regular);
}

.api-response-viewer__meta {
  display: grid;
  grid-template-columns: 100px minmax(0, 1fr);
  gap: 6px 10px;
  margin: 0;
  font-size: 12px;
}

.api-response-viewer__meta dt {
  color: var(--el-text-color-secondary);
}

.api-response-viewer__meta dd {
  min-width: 0;
  margin: 0;
  overflow-wrap: anywhere;
}

.api-response-viewer__state {
  color: var(--el-text-color-secondary);
  font-size: 13px;
  padding: 12px 0;
}
</style>
