<template>
  <el-form label-position="top" class="api-mock-form">
    <div v-if="fullUrl" class="route-url-line">
      <span class="route-url">{{ fullUrl }}</span>
      <el-button :icon="CopyDocument" size="small" text title="复制完整 URL" @click="copyFullUrl" />
    </div>

    <div class="form-grid route-grid">
      <el-form-item label="名称">
        <el-input v-model="form.name" />
      </el-form-item>
      <el-form-item label="方法">
        <el-select v-model="form.method">
          <el-option v-for="method in API_MOCK_METHODS" :key="method" :label="method" :value="method" />
        </el-select>
      </el-form-item>
      <el-form-item label="路径匹配">
        <el-input v-model="form.pathPattern" placeholder="/api/users/:id 或 /files/*" />
      </el-form-item>
      <el-form-item label="状态码">
        <el-input-number v-model="form.statusCode" :min="100" :max="599" controls-position="right" />
      </el-form-item>
      <el-form-item label="延迟 (ms)">
        <el-input-number
          v-model="form.delayMs"
          :min="0"
          :max="MAX_API_MOCK_DELAY_MS"
          :step="100"
          controls-position="right"
        />
      </el-form-item>
    </div>

    <div v-if="pathError" class="api-mock-error">{{ pathError }}</div>

    <div class="form-grid">
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
      <el-form-item label="启用">
        <el-switch v-model="form.enabled" />
      </el-form-item>
    </div>

    <template v-if="form.responseKind === 'static_body'">
      <div class="subsection-head">
        <h3>响应 Body</h3>
        <el-button v-if="canFormatJson" size="small" text type="primary" @click="formatBody">格式化 JSON</el-button>
      </div>
      <div class="body-editor">
        <MonacoPane v-model="form.bodyText" :language="bodyLanguage" />
      </div>
    </template>

    <div v-else class="file-box">
      <div>
        <strong>{{ file?.originalName || "未选择文件" }}</strong>
        <span v-if="file">
          {{ file.contentType || "application/octet-stream" }} · {{ formatMockFileSize(file.size) }}
        </span>
      </div>
      <el-button :icon="Upload" @click="emit('pick-file')">导入文件</el-button>
    </div>

    <div class="subsection-head">
      <h3>响应头</h3>
      <el-button :icon="Plus" size="small" text @click="addHeader">添加</el-button>
    </div>
    <div class="header-rows">
      <div v-for="(row, index) in form.headers" :key="index" class="header-row">
        <el-checkbox v-model="row.enabled" />
        <el-input v-model="row.key" placeholder="Header" />
        <el-input v-model="row.value" placeholder="Value" />
        <el-button :icon="Delete" text @click="form.headers.splice(index, 1)" />
      </div>
    </div>

    <el-collapse>
      <el-collapse-item title="CORS" name="cors">
        <div class="form-grid">
          <el-form-item label="启用 CORS">
            <el-switch v-model="form.cors.enabled" />
          </el-form-item>
          <el-form-item label="Allow-Origin">
            <el-input v-model="form.cors.allowOrigin" />
          </el-form-item>
          <el-form-item label="Allow-Headers">
            <el-input v-model="form.cors.allowHeaders" />
          </el-form-item>
          <el-form-item label="Expose-Headers">
            <el-input v-model="form.cors.exposeHeaders" />
          </el-form-item>
          <el-form-item label="Allow-Credentials">
            <el-switch v-model="form.cors.allowCredentials" />
          </el-form-item>
          <el-form-item label="Max-Age">
            <el-input-number v-model="form.cors.maxAgeSeconds" :min="0" controls-position="right" />
          </el-form-item>
        </div>
        <div v-if="corsError" class="api-mock-error">{{ corsError }}</div>
      </el-collapse-item>
    </el-collapse>

    <div class="form-footer">
      <span>{{ getMockRouteSpecificityLabel(form.pathPattern) }}匹配</span>
      <div>
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
.api-mock-form {
  max-width: 980px;
}

.route-url-line {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-bottom: 12px;
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

.form-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.route-grid {
  grid-template-columns: minmax(120px, 1fr) 104px minmax(200px, 1.3fr) 112px 120px;
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
  margin-bottom: 8px;
}

.subsection-head h3 {
  margin: 0;
  font-size: 14px;
  line-height: 1.4;
}

.body-editor {
  height: 280px;
  margin-bottom: 14px;
}

.header-rows {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 14px;
}

.header-row {
  display: grid;
  grid-template-columns: 28px minmax(0, 1fr) minmax(0, 1.4fr) 36px;
  gap: 8px;
  align-items: center;
}

.file-box {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
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

.api-mock-error {
  margin: -6px 0 12px;
  font-size: 13px;
  color: #b91c1c;
}

.form-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 12px;
}

.form-footer span {
  font-size: 12px;
  color: #64748b;
}

@media (max-width: 860px) {
  .form-grid,
  .route-grid {
    grid-template-columns: 1fr;
  }
}
</style>
