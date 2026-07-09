<template>
  <el-form label-position="top" class="api-mock-form">
    <div v-if="fullUrl" class="route-url-line">
      <span class="route-url">{{ fullUrl }}</span>
      <el-button :icon="CopyDocument" size="small" text title="复制完整 URL" @click="copyFullUrl" />
    </div>

    <section class="form-section">
      <div class="route-grid">
        <el-form-item label="名称" class="fi-name">
          <el-input v-model="form.name" />
        </el-form-item>
        <el-form-item label="方法" class="fi-method">
          <el-select v-model="form.method">
            <el-option v-for="method in API_MOCK_METHODS" :key="method" :label="method" :value="method" />
          </el-select>
        </el-form-item>
        <el-form-item label="路径匹配" class="fi-path">
          <el-input v-model="form.pathPattern" placeholder="/api/users/:id 或 /files/*" />
        </el-form-item>
        <el-form-item label="状态码" class="fi-status">
          <el-input-number v-model="form.statusCode" :min="100" :max="599" controls-position="right" />
        </el-form-item>
        <el-form-item label="延迟 (ms)" class="fi-delay">
          <el-input-number
            v-model="form.delayMs"
            :min="0"
            :max="MAX_API_MOCK_DELAY_MS"
            :step="100"
            controls-position="right"
          />
        </el-form-item>
        <el-form-item label="启用" class="fi-enabled">
          <el-switch v-model="form.enabled" />
        </el-form-item>
      </div>
      <div v-if="pathError" class="api-mock-error">{{ pathError }}</div>
    </section>

    <section class="form-section">
      <div class="subsection-head">
        <h3>响应内容</h3>
        <el-button
          v-if="form.responseKind === 'static_body' && canFormatJson"
          size="small"
          text
          type="primary"
          @click="formatBody"
        >
          格式化 JSON
        </el-button>
      </div>
      <div class="response-grid">
        <el-form-item label="响应类型">
          <el-radio-group v-model="form.responseKind">
            <el-radio-button label="static_body">文本</el-radio-button>
            <el-radio-button label="file">文件</el-radio-button>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="Content-Type">
          <el-select
            v-model="form.contentType"
            filterable
            allow-create
            clearable
            default-first-option
            :value-on-clear="''"
            placeholder="application/json; charset=utf-8"
          >
            <el-option
              v-for="preset in API_MOCK_CONTENT_TYPE_PRESETS"
              :key="preset.value"
              :label="preset.value"
              :value="preset.value"
            >
              <div class="content-type-option">
                <span>{{ preset.label }}</span>
                <small>{{ preset.value }}</small>
              </div>
            </el-option>
          </el-select>
        </el-form-item>
      </div>

      <div v-if="form.responseKind === 'static_body'" class="body-editor">
        <MonacoPane v-model="form.bodyText" :language="bodyLanguage" />
      </div>

      <div v-else class="file-box">
        <div>
          <strong>{{ file?.originalName || "未选择文件" }}</strong>
          <span v-if="file">
            {{ file.contentType || "application/octet-stream" }} · {{ formatMockFileSize(file.size) }}
          </span>
        </div>
        <el-button :icon="Upload" @click="emit('pick-file')">导入文件</el-button>
      </div>
    </section>

    <section class="form-section">
      <div class="subsection-head">
        <h3>响应头</h3>
        <el-button :icon="Plus" size="small" text @click="addHeader">添加</el-button>
      </div>
      <div v-if="form.headers.length" class="header-rows">
        <div v-for="(row, index) in form.headers" :key="index" class="header-row">
          <el-checkbox v-model="row.enabled" />
          <el-input v-model="row.key" placeholder="Header" />
          <el-input v-model="row.value" placeholder="Value" />
          <el-button :icon="Delete" text @click="form.headers.splice(index, 1)" />
        </div>
      </div>
      <div v-else class="section-hint">未配置自定义响应头；Content-Type 与 CORS 头会按配置自动附加。</div>
    </section>

    <el-collapse class="cors-collapse">
      <el-collapse-item :title="corsTitle" name="cors">
        <div class="cors-grid">
          <el-form-item label="启用 CORS">
            <el-switch v-model="form.cors.enabled" />
          </el-form-item>
          <el-form-item label="Allow-Origin">
            <el-input
              v-model="form.cors.allowOrigin"
              placeholder="* 或多个来源，英文逗号分隔"
              :disabled="!form.cors.enabled"
            />
          </el-form-item>
          <el-form-item label="Allow-Methods">
            <el-select
              v-model="form.cors.allowMethods"
              multiple
              clearable
              placeholder="留空自动使用路由方法"
              :disabled="!form.cors.enabled"
            >
              <el-option v-for="method in API_MOCK_METHODS" :key="method" :label="method" :value="method" />
            </el-select>
          </el-form-item>
          <el-form-item label="Allow-Headers">
            <el-input v-model="form.cors.allowHeaders" :disabled="!form.cors.enabled" />
          </el-form-item>
          <el-form-item label="Expose-Headers">
            <el-input v-model="form.cors.exposeHeaders" :disabled="!form.cors.enabled" />
          </el-form-item>
          <el-form-item label="Allow-Credentials">
            <el-switch v-model="form.cors.allowCredentials" :disabled="!form.cors.enabled" />
          </el-form-item>
          <el-form-item label="Max-Age">
            <el-input-number
              v-model="form.cors.maxAgeSeconds"
              :min="0"
              controls-position="right"
              :disabled="!form.cors.enabled"
            />
          </el-form-item>
        </div>
        <div v-if="form.cors.enabled" class="cors-hint">
          启用后自动响应 OPTIONS 预检（204）；配置多个来源时按请求 Origin 回显，并自动附带 Vary: Origin。
        </div>
        <div v-if="corsError" class="api-mock-error">{{ corsError }}</div>
      </el-collapse-item>
    </el-collapse>

    <div class="form-footer">
      <span>{{ getMockRouteSpecificityLabel(form.pathPattern) }}匹配</span>
      <div class="footer-actions">
        <el-button v-if="form.id !== null" :icon="Delete" type="danger" text @click="emit('delete')">删除</el-button>
        <el-button type="primary" :disabled="!canSave" @click="emit('save')">保存路由</el-button>
      </div>
    </div>
  </el-form>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { CopyDocument, Delete, Plus, Upload } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import MonacoPane from "../MonacoPane.vue";
import type { ApiMockFileInfo, ApiMockProjectSummary, ApiMockRouteFormModel } from "../../types/api-mock";
import {
  API_MOCK_CONTENT_TYPE_PRESETS,
  API_MOCK_METHODS,
  MAX_API_MOCK_DELAY_MS,
  buildMockRouteUrl,
  formatMockFileSize,
  getMockBodyEditorLanguage,
  getMockRouteSpecificityLabel,
} from "../../utils/apiMock";

const props = defineProps<{
  form: ApiMockRouteFormModel;
  file: ApiMockFileInfo | null;
  project: Pick<ApiMockProjectSummary, "host" | "port"> | null;
  pathError: string;
  corsError: string;
  canSave: boolean;
}>();

const emit = defineEmits<{
  (event: "save"): void;
  (event: "delete"): void;
  (event: "pick-file"): void;
}>();

const fullUrl = computed(() => {
  if (!props.project) return "";
  return buildMockRouteUrl(props.project, props.form.pathPattern);
});

const bodyLanguage = computed(() => getMockBodyEditorLanguage(props.form.contentType));

const canFormatJson = computed(() => bodyLanguage.value === "json");

const corsTitle = computed(() => `CORS（${props.form.cors.enabled ? "已启用" : "已关闭"}）`);

async function copyFullUrl() {
  try {
    await navigator.clipboard.writeText(fullUrl.value);
    ElMessage.success("已复制完整 URL");
  } catch {
    ElMessage.error("复制失败");
  }
}

function formatBody() {
  try {
    const parsed = JSON.parse(props.form.bodyText);
    props.form.bodyText = JSON.stringify(parsed, null, 2);
  } catch {
    ElMessage.error("响应 Body 不是合法 JSON，无法格式化");
  }
}

function addHeader() {
  props.form.headers.push({ enabled: true, key: "", value: "" });
}
</script>

<style scoped>
/* 间距节奏统一由表单布局层（flex gap + grid gap）控制 */
.api-mock-form {
  container-type: inline-size;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.api-mock-form :deep(.el-form-item) {
  margin-bottom: 0;
}

.form-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.route-url-line {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 10px;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #f8fafc;
}

.route-url {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  color: #334155;
}

/* 基础信息：命名区域网格，随容器宽度重排 */
.route-grid {
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(140px, 1.1fr) 100px minmax(200px, 1.5fr) 104px 116px 72px;
  grid-template-areas: "name method path status delay enabled";
}

.fi-name {
  grid-area: name;
}

.fi-method {
  grid-area: method;
}

.fi-path {
  grid-area: path;
}

.fi-status {
  grid-area: status;
}

.fi-delay {
  grid-area: delay;
}

.fi-enabled {
  grid-area: enabled;
}

.route-grid :deep(.el-select),
.route-grid :deep(.el-input-number),
.cors-grid :deep(.el-select),
.cors-grid :deep(.el-input-number) {
  width: 100%;
}

/* 响应内容 */
.response-grid {
  display: grid;
  gap: 12px;
  grid-template-columns: max-content minmax(220px, 400px);
}

.content-type-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  min-width: 0;
}

.content-type-option span,
.content-type-option small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.content-type-option span {
  font-weight: 600;
  color: #172033;
}

.content-type-option small {
  color: #64748b;
}

.subsection-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.subsection-head h3 {
  margin: 0;
  font-size: 14px;
  line-height: 1.4;
}

.body-editor {
  /* 高度跟随窗口：全屏时利用富余空间，仍可拖拽微调（边框由 MonacoPane 自带） */
  height: clamp(240px, 40vh, 640px);
  min-height: 180px;
  max-height: 75vh;
  overflow: hidden;
  resize: vertical;
}

.header-rows {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.header-row {
  display: grid;
  grid-template-columns: 28px minmax(0, 1fr) minmax(0, 1.4fr) 36px;
  gap: 8px;
  align-items: center;
}

.section-hint {
  padding: 10px 12px;
  border: 1px dashed #dbe4ef;
  border-radius: 6px;
  font-size: 12px;
  color: #94a3b8;
}

.file-box {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px;
  border: 1px solid #dbe4ef;
  border-radius: 8px;
  background: #f8fafc;
}

.file-box > div {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 4px;
}

.file-box span {
  font-size: 12px;
  color: #64748b;
}

/* CORS：折叠区标题与分组标题同级，内容间距与 section 一致 */
.cors-collapse :deep(.el-collapse-item__header) {
  font-size: 14px;
  font-weight: 600;
}

.cors-collapse :deep(.el-collapse-item__content) {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-bottom: 14px;
}

.cors-grid {
  display: grid;
  gap: 12px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.api-mock-error {
  margin: 0;
  font-size: 13px;
  color: #b91c1c;
}

.cors-hint {
  margin: 0;
  font-size: 12px;
  color: #64748b;
}

/* 底部操作栏吸底：长表单滚动时保存入口不丢失 */
.form-footer {
  position: sticky;
  bottom: 0;
  z-index: 10;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 0;
  background: #ffffff;
  border-top: 1px solid #e2e8f0;
}

.form-footer > span {
  font-size: 12px;
  color: #64748b;
}

.footer-actions {
  display: flex;
  align-items: center;
}

/* —— 容器断点：按表单实际宽度而非视口折行 —— */
@container (max-width: 880px) {
  .route-grid {
    grid-template-columns: repeat(6, minmax(0, 1fr));
    grid-template-areas:
      "name name name name method method"
      "path path path path path path"
      "status status delay delay enabled enabled";
  }
}

@container (max-width: 720px) {
  .cors-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@container (max-width: 560px) {
  .route-grid {
    grid-template-columns: 1fr;
    grid-template-areas:
      "name"
      "method"
      "path"
      "status"
      "delay"
      "enabled";
  }

  .response-grid,
  .cors-grid {
    grid-template-columns: 1fr;
  }
}
</style>
