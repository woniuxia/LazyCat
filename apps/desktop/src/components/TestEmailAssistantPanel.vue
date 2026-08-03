<template>
  <section class="test-email-assistant-panel" aria-labelledby="test-email-assistant-title">
    <header class="assistant-header">
      <div class="assistant-heading">
        <span class="assistant-kicker">DOCX + EMAIL</span>
        <h2 id="test-email-assistant-title">测试邮件助手</h2>
        <p>填写一次测试信息，复制邮件正文并生成一份新的 Word 测试报告。</p>
      </div>
      <div class="assistant-actions">
        <el-button :icon="FolderOpened" :loading="inspecting" @click="chooseTemplate">
          {{ templatePath ? "重新选择模板" : "选择 Word 模板" }}
        </el-button>
      </div>
    </header>

    <section class="assistant-section template-section" aria-labelledby="template-section-title">
      <div class="section-heading">
        <div>
          <span class="section-index">01</span>
          <h3 id="template-section-title">Word 模板</h3>
        </div>
        <span v-if="templatePath" class="section-status">已检查</span>
      </div>
      <el-input
        :model-value="templatePath"
        readonly
        class="template-path"
        placeholder="选择含有 &#123;&#123;占位符&#125;&#125; 的 .docx 文件"
        :title="templatePath || undefined"
      >
        <template #append>
          <el-button :icon="Refresh" :loading="inspecting" @click="chooseTemplate">
            {{ templatePath ? "更换" : "选择" }}
          </el-button>
        </template>
      </el-input>
      <div v-if="wordPlaceholders.length > 0" class="placeholder-summary">
        <span class="summary-label">已识别字段</span>
        <el-tag v-for="name in wordPlaceholders" :key="name" size="small" effect="plain">
          {{ name }}
        </el-tag>
      </div>
      <p v-else class="field-help">选择模板后会自动读取正文、页眉、页脚等 Word XML 中的字段。</p>
    </section>

    <div class="assistant-workspace">
      <section class="assistant-section email-section" aria-labelledby="email-section-title">
        <div class="section-heading">
          <div>
            <span class="section-index">02</span>
            <h3 id="email-section-title">邮件正文模板</h3>
          </div>
          <el-button text :icon="Delete" :disabled="!hasValues" @click="clearValues">
            清空填写
          </el-button>
        </div>
        <div class="email-template-toolbar" aria-label="邮件正文模板管理">
          <div class="template-library-controls">
            <el-select
              :model-value="activeEmailTemplateId"
              class="email-template-select"
              :disabled="templatePersistencePending"
              aria-label="选择邮件正文模板"
              @change="selectEmailTemplate"
            >
              <el-option :value="BUILTIN_TEST_EMAIL_TEMPLATE_ID" label="默认模板（内置）" />
              <el-option
                v-for="template in customEmailTemplates"
                :key="template.id"
                :value="template.id"
                :label="template.name"
                :title="template.name"
              />
            </el-select>
            <div class="template-library-status" aria-live="polite">
              <span>自定义 {{ customEmailTemplates.length }} 个</span>
              <el-tag
                size="small"
                effect="plain"
                :type="isEmailTemplateDirty ? 'warning' : 'info'"
                class="template-dirty-status"
              >
                {{ isEmailTemplateDirty ? "未保存" : "无修改" }}
              </el-tag>
            </div>
          </div>
          <div class="template-library-actions">
            <el-button
              class="template-primary-action"
              :disabled="templatePersistencePending || !emailTemplate.trim()"
              @click="saveEmailTemplateAs"
            >
              另存为
            </el-button>
            <el-button
              class="template-primary-action"
              :disabled="!canSaveEmailTemplateChanges"
              @click="saveEmailTemplateChanges"
            >
              保存修改
            </el-button>
            <el-tooltip content="重命名当前模板" placement="top">
              <el-button
                :icon="EditPen"
                circle
                aria-label="重命名当前模板"
                :disabled="templatePersistencePending || !activeCustomEmailTemplate"
                @click="renameEmailTemplate"
              />
            </el-tooltip>
            <el-tooltip content="删除当前模板" placement="top">
              <el-button
                :icon="Delete"
                circle
                aria-label="删除当前模板"
                :disabled="templatePersistencePending || !activeCustomEmailTemplate"
                @click="deleteEmailTemplate"
              />
            </el-tooltip>
          </div>
        </div>
        <el-input
          v-model="emailTemplate"
          type="textarea"
          :rows="8"
          :disabled="templatePersistencePending"
          resize="vertical"
          class="email-template-input"
          aria-label="邮件正文模板"
        />
        <div class="template-note">
          使用 &#123;&#123;占位符&#125;&#125; 语法，字段会随模板内容自动更新。
        </div>

        <div class="fields-heading">
          <span>填写字段</span>
          <span class="fields-count">{{ allPlaceholders.length }} 项</span>
        </div>
        <div v-if="allPlaceholders.length > 0" class="fields-list">
          <el-form label-position="top" class="fields-form">
            <el-form-item v-for="name in allPlaceholders" :key="name" :label="name" required>
              <el-input
                v-model="values[name]"
                :type="isMultilineFieldName(name) ? 'textarea' : 'text'"
                :autosize="isMultilineFieldName(name) ? { minRows: 3, maxRows: 10 } : false"
                resize="vertical"
                :placeholder="`请输入${name}`"
                :aria-label="name"
                clearable
              />
            </el-form-item>
          </el-form>
        </div>
        <el-empty v-else :image-size="48" description="邮件模板中暂无有效占位符" />
      </section>

      <section class="assistant-section preview-section" aria-labelledby="preview-section-title">
        <div class="section-heading">
          <div>
            <span class="section-index">03</span>
            <h3 id="preview-section-title">实时预览</h3>
          </div>
          <el-button
            type="primary"
            :icon="CopyDocument"
            :disabled="missingEmailPlaceholders.length > 0 || !emailTemplate"
            @click="copyEmail"
          >
            复制正文
          </el-button>
        </div>
        <pre class="email-preview" aria-label="邮件正文预览">{{ emailPreview }}</pre>
        <div v-if="missingEmailPlaceholders.length > 0" class="validation-message" role="alert">
          复制前请填写：{{ missingEmailPlaceholders.join("、") }}
        </div>
        <div v-else class="preview-ready">
          <el-icon><CircleCheck /></el-icon>
          邮件字段已填写完整，可复制正文
        </div>
      </section>
    </div>

    <section class="assistant-section generate-section" aria-labelledby="generate-section-title">
      <div class="section-heading">
        <div>
          <span class="section-index">04</span>
          <h3 id="generate-section-title">生成测试报告</h3>
        </div>
      </div>
      <div class="generate-row">
        <div class="generate-hint">
          <p>生成文件会放在 Word 模板所在目录，原模板不会被修改。</p>
          <div v-if="missingWordPlaceholders.length > 0" class="validation-message" role="alert">
            生成前请填写：{{ missingWordPlaceholders.join("、") }}
          </div>
        </div>
        <el-button
          type="primary"
          :icon="DocumentAdd"
          :loading="generating"
          :disabled="
            !templatePath || wordPlaceholders.length === 0 || missingWordPlaceholders.length > 0
          "
          @click="generateDocument"
        >
          生成 Word 测试报告
        </el-button>
      </div>
      <div v-if="outputPath" class="output-result" role="status">
        <div class="output-copy">
          <el-icon><CircleCheck /></el-icon>
          <div>
            <strong>已生成测试报告</strong>
            <span :title="outputPath">{{ outputPath }}</span>
          </div>
        </div>
        <el-button :icon="FolderOpened" @click="revealOutput">打开所在位置</el-button>
      </div>
    </section>

    <p v-if="errorMessage" class="assistant-error" role="alert">{{ errorMessage }}</p>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import {
  CircleCheck,
  CopyDocument,
  Delete,
  DocumentAdd,
  EditPen,
  FolderOpened,
  Refresh,
} from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type { TestEmailAssistantGenerateResult, TestEmailAssistantInspectResult } from "../types";
import {
  BUILTIN_TEST_EMAIL_TEMPLATE_ID,
  DEFAULT_TEST_EMAIL_TEMPLATE,
  type TestEmailBodyTemplate,
  getMissingPlaceholders,
  hasTestEmailTemplateNameConflict,
  isMultilineFieldName,
  mergePlaceholders,
  extractPlaceholders,
  normalizeTestEmailBodyTemplates,
  renderEmailTemplate,
} from "../utils/testEmailAssistant";

const templatePath = ref("");
const wordPlaceholders = ref<string[]>([]);
const emailTemplate = ref(DEFAULT_TEST_EMAIL_TEMPLATE);
const customEmailTemplates = ref<TestEmailBodyTemplate[]>([]);
const activeEmailTemplateId = ref(BUILTIN_TEST_EMAIL_TEMPLATE_ID);
const loadedEmailTemplateContent = ref(DEFAULT_TEST_EMAIL_TEMPLATE);
const templatePersistencePending = ref(true);
const values = reactive<Record<string, string>>({});
const inspecting = ref(false);
const generating = ref(false);
const outputPath = ref("");
const errorMessage = ref("");

const emailPlaceholders = computed(() => extractPlaceholders(emailTemplate.value));
const allPlaceholders = computed(() =>
  mergePlaceholders(
    wordPlaceholders.value.map((name) => `{{${name}}}`).join(" "),
    emailTemplate.value,
  ),
);
const missingEmailPlaceholders = computed(() =>
  getMissingPlaceholders(emailPlaceholders.value, values),
);
const missingWordPlaceholders = computed(() =>
  getMissingPlaceholders(wordPlaceholders.value, values),
);
const emailPreview = computed(() => renderEmailTemplate(emailTemplate.value, values));
const hasValues = computed(() => Object.values(values).some((value) => value.length > 0));
const isEmailTemplateDirty = computed(
  () => emailTemplate.value !== loadedEmailTemplateContent.value,
);
const activeCustomEmailTemplate = computed(
  () =>
    customEmailTemplates.value.find((template) => template.id === activeEmailTemplateId.value) ??
    null,
);
const canSaveEmailTemplateChanges = computed(
  () =>
    !templatePersistencePending.value &&
    !!activeCustomEmailTemplate.value &&
    isEmailTemplateDirty.value &&
    !!emailTemplate.value.trim(),
);

watch(
  allPlaceholders,
  (names) => {
    for (const name of names) {
      if (!(name in values)) values[name] = "";
    }
  },
  { immediate: true },
);

onMounted(() => {
  void loadEmailTemplates();
});

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function isMessageBoxCancellation(error: unknown): boolean {
  return error === "cancel" || error === "close";
}

function findEmailTemplateById(id: string): TestEmailBodyTemplate {
  if (id === BUILTIN_TEST_EMAIL_TEMPLATE_ID) {
    return {
      id: BUILTIN_TEST_EMAIL_TEMPLATE_ID,
      name: "默认模板",
      content: DEFAULT_TEST_EMAIL_TEMPLATE,
    };
  }
  const template = customEmailTemplates.value.find((item) => item.id === id);
  if (!template) throw new Error(`找不到邮件正文模板：${id}`);
  return template;
}

function requireActiveCustomEmailTemplate(action: string): TestEmailBodyTemplate | null {
  if (activeEmailTemplateId.value === BUILTIN_TEST_EMAIL_TEMPLATE_ID) {
    errorMessage.value = `${action}失败：内置默认模板不支持此操作`;
    return null;
  }
  const template = activeCustomEmailTemplate.value;
  if (!template) {
    errorMessage.value = `${action}失败：找不到当前自定义模板`;
    return null;
  }
  return template;
}

function applyEmailTemplate(template: TestEmailBodyTemplate) {
  activeEmailTemplateId.value = template.id;
  loadedEmailTemplateContent.value = template.content;
  emailTemplate.value = template.content;
  errorMessage.value = "";
}

async function selectEmailTemplate(id: string) {
  if (templatePersistencePending.value || id === activeEmailTemplateId.value) return;

  let template: TestEmailBodyTemplate;
  try {
    template = findEmailTemplateById(id);
  } catch (error) {
    errorMessage.value = `切换邮件正文模板失败：${formatError(error)}`;
    return;
  }

  if (isEmailTemplateDirty.value) {
    try {
      await ElMessageBox.confirm(
        "当前邮件正文尚未保存，切换模板会丢失这些修改。",
        "切换邮件正文模板",
        {
          type: "warning",
          confirmButtonText: "放弃修改并切换",
          cancelButtonText: "继续编辑",
        },
      );
    } catch (error) {
      if (isMessageBoxCancellation(error)) return;
      errorMessage.value = `切换邮件正文模板失败：${formatError(error)}`;
      return;
    }
  }

  applyEmailTemplate(template);
}

function validateEmailTemplateName(name: string, excludedId?: string): true | string {
  const trimmedName = name.trim();
  if (!trimmedName) return "请输入模板名称";
  if (Array.from(trimmedName).length > 50) return "模板名称不能超过 50 个字符";
  if (hasTestEmailTemplateNameConflict(customEmailTemplates.value, trimmedName, excludedId)) {
    return "模板名称已存在";
  }
  return true;
}

async function loadEmailTemplates() {
  templatePersistencePending.value = true;
  errorMessage.value = "";
  try {
    const templates = await invokeToolByChannel(
      "tool:test-email-assistant:list-email-templates",
      {},
    );
    customEmailTemplates.value = normalizeTestEmailBodyTemplates(templates);
  } catch (error) {
    errorMessage.value = `加载邮件正文模板失败：${formatError(error)}`;
  } finally {
    templatePersistencePending.value = false;
  }
}

async function persistEmailTemplateChange<T>(
  operation: () => Promise<T>,
  failureContext: string,
): Promise<T | null> {
  templatePersistencePending.value = true;
  errorMessage.value = "";
  try {
    return await operation();
  } catch (error) {
    errorMessage.value = `${failureContext}：${formatError(error)}`;
    return null;
  } finally {
    templatePersistencePending.value = false;
  }
}

async function saveEmailTemplateAs() {
  if (templatePersistencePending.value) return;
  if (!emailTemplate.value.trim()) {
    errorMessage.value = "另存邮件正文模板失败：正文不能为空";
    return;
  }

  let name: string;
  try {
    const result = await ElMessageBox.prompt("输入新模板名称", "另存邮件正文模板", {
      confirmButtonText: "保存",
      cancelButtonText: "取消",
      inputPlaceholder: "模板名称",
      inputValidator: (value) => validateEmailTemplateName(value),
    });
    name = result.value.trim();
  } catch (error) {
    if (isMessageBoxCancellation(error)) return;
    errorMessage.value = `另存邮件正文模板失败：${formatError(error)}`;
    return;
  }

  const validationResult = validateEmailTemplateName(name);
  if (validationResult !== true) {
    errorMessage.value = `另存邮件正文模板失败：${validationResult}`;
    return;
  }

  const newTemplate = await persistEmailTemplateChange(
    async () =>
      (await invokeToolByChannel("tool:test-email-assistant:create-email-template", {
        name,
        content: emailTemplate.value,
      })) as TestEmailBodyTemplate,
    "另存邮件正文模板失败",
  );
  if (!newTemplate) return;

  customEmailTemplates.value = [...customEmailTemplates.value, newTemplate];
  applyEmailTemplate(newTemplate);
  ElMessage.success("邮件正文模板已保存");
}

async function saveEmailTemplateChanges() {
  if (templatePersistencePending.value) return;
  const currentTemplate = requireActiveCustomEmailTemplate("保存邮件正文模板");
  if (!currentTemplate) return;
  if (!emailTemplate.value.trim()) {
    errorMessage.value = "保存邮件正文模板失败：正文不能为空";
    return;
  }
  if (!isEmailTemplateDirty.value) return;

  const savedTemplate = await persistEmailTemplateChange(
    async () =>
      (await invokeToolByChannel("tool:test-email-assistant:update-email-template", {
        id: currentTemplate.id,
        name: currentTemplate.name,
        content: emailTemplate.value,
      })) as TestEmailBodyTemplate,
    "保存邮件正文模板失败",
  );
  if (!savedTemplate) return;

  customEmailTemplates.value = customEmailTemplates.value.map((template) =>
    template.id === currentTemplate.id ? savedTemplate : template,
  );
  loadedEmailTemplateContent.value = savedTemplate.content;
  errorMessage.value = "";
  ElMessage.success("邮件正文模板修改已保存");
}

async function renameEmailTemplate() {
  if (templatePersistencePending.value) return;
  const currentTemplate = requireActiveCustomEmailTemplate("重命名邮件正文模板");
  if (!currentTemplate) return;

  let name: string;
  try {
    const result = await ElMessageBox.prompt("输入新的模板名称", "重命名邮件正文模板", {
      inputValue: currentTemplate.name,
      confirmButtonText: "保存",
      cancelButtonText: "取消",
      inputValidator: (value) => validateEmailTemplateName(value, currentTemplate.id),
    });
    name = result.value.trim();
  } catch (error) {
    if (isMessageBoxCancellation(error)) return;
    errorMessage.value = `重命名邮件正文模板失败：${formatError(error)}`;
    return;
  }

  const validationResult = validateEmailTemplateName(name, currentTemplate.id);
  if (validationResult !== true) {
    errorMessage.value = `重命名邮件正文模板失败：${validationResult}`;
    return;
  }
  if (name === currentTemplate.name) return;

  const savedTemplate = await persistEmailTemplateChange(
    async () =>
      (await invokeToolByChannel("tool:test-email-assistant:update-email-template", {
        id: currentTemplate.id,
        name,
        content: currentTemplate.content,
      })) as TestEmailBodyTemplate,
    "重命名邮件正文模板失败",
  );
  if (!savedTemplate) return;

  customEmailTemplates.value = customEmailTemplates.value.map((template) =>
    template.id === currentTemplate.id ? savedTemplate : template,
  );
  errorMessage.value = "";
  ElMessage.success("邮件正文模板已重命名");
}

async function deleteEmailTemplate() {
  if (templatePersistencePending.value) return;
  const currentTemplate = requireActiveCustomEmailTemplate("删除邮件正文模板");
  if (!currentTemplate) return;

  const message = isEmailTemplateDirty.value
    ? `确定删除“${currentTemplate.name}”吗？当前未保存的正文修改也会丢失。`
    : `确定删除“${currentTemplate.name}”吗？删除后无法恢复。`;
  try {
    await ElMessageBox.confirm(message, "删除邮件正文模板", {
      type: "warning",
      confirmButtonText: "删除",
      cancelButtonText: "取消",
    });
  } catch (error) {
    if (isMessageBoxCancellation(error)) return;
    errorMessage.value = `删除邮件正文模板失败：${formatError(error)}`;
    return;
  }

  const deleted = await persistEmailTemplateChange(
    async () =>
      (await invokeToolByChannel("tool:test-email-assistant:delete-email-template", {
        id: currentTemplate.id,
      })) as { ok: boolean },
    "删除邮件正文模板失败",
  );
  if (!deleted) return;

  customEmailTemplates.value = customEmailTemplates.value.filter(
    (template) => template.id !== currentTemplate.id,
  );
  applyEmailTemplate(findEmailTemplateById(BUILTIN_TEST_EMAIL_TEMPLATE_ID));
  ElMessage.success("邮件正文模板已删除");
}

async function chooseTemplate() {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Word 文档", extensions: ["docx"] }],
    });
    if (!selected) return;
    const path = typeof selected === "string" ? selected : selected.path;
    if (!path) return;

    inspecting.value = true;
    errorMessage.value = "";
    outputPath.value = "";
    const result = (await invokeToolByChannel("tool:test-email-assistant:inspect-template", {
      templatePath: path,
    })) as TestEmailAssistantInspectResult;
    templatePath.value = result.templatePath || path;
    wordPlaceholders.value = result.placeholders;
    ElMessage.success(`已识别 ${result.placeholders.length} 个 Word 字段`);
  } catch (error) {
    templatePath.value = "";
    wordPlaceholders.value = [];
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    inspecting.value = false;
  }
}

function clearValues() {
  for (const name of Object.keys(values)) values[name] = "";
  outputPath.value = "";
  errorMessage.value = "";
}

async function copyEmail() {
  if (missingEmailPlaceholders.value.length > 0) {
    errorMessage.value = `复制正文失败：请填写 ${missingEmailPlaceholders.value.join("、")}`;
    return;
  }
  try {
    await navigator.clipboard.writeText(emailPreview.value);
    errorMessage.value = "";
    ElMessage.success("邮件正文已复制");
  } catch (error) {
    errorMessage.value = `复制正文失败：${error instanceof Error ? error.message : String(error)}`;
  }
}

async function generateDocument() {
  if (missingWordPlaceholders.value.length > 0) {
    errorMessage.value = `生成测试报告失败：请填写 ${missingWordPlaceholders.value.join("、")}`;
    return;
  }
  generating.value = true;
  errorMessage.value = "";
  outputPath.value = "";
  try {
    const result = (await invokeToolByChannel("tool:test-email-assistant:generate-document", {
      templatePath: templatePath.value,
      values: { ...values },
    })) as TestEmailAssistantGenerateResult;
    outputPath.value = result.outputPath;
    ElMessage.success("Word 测试报告已生成");
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    generating.value = false;
  }
}

async function revealOutput() {
  if (!outputPath.value) return;
  try {
    await invokeToolByChannel("tool:system:reveal-in-folder", { path: outputPath.value });
  } catch (error) {
    errorMessage.value = `打开所在位置失败：${error instanceof Error ? error.message : String(error)}`;
  }
}
</script>

<style scoped>
.test-email-assistant-panel {
  min-width: 0;
  padding: 2px 0 24px;
  color: var(--lc-text);
}

.assistant-header,
.section-heading,
.generate-row,
.output-result,
.placeholder-summary,
.output-copy {
  display: flex;
  align-items: center;
  min-width: 0;
}

.assistant-header {
  justify-content: space-between;
  gap: 20px;
  padding: 4px 2px 18px;
}

.assistant-heading {
  min-width: 0;
}

.assistant-kicker,
.section-index {
  color: var(--el-color-primary);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.08em;
}

.assistant-heading h2 {
  margin: 4px 0 6px;
  font-size: 22px;
  line-height: 1.25;
}

.assistant-heading p,
.generate-hint p {
  margin: 0;
  color: var(--el-text-color-secondary);
  font-size: 13px;
  line-height: 1.6;
}

.assistant-actions,
.section-heading > :last-child,
.generate-row > .el-button,
.output-result > .el-button {
  flex: 0 0 auto;
}

.assistant-section {
  min-width: 0;
  padding: 18px 2px;
  border-top: 1px solid var(--lc-border);
}

.section-heading {
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 12px;
}

.section-heading > div {
  display: flex;
  align-items: baseline;
  gap: 9px;
  min-width: 0;
}

.section-heading h3 {
  margin: 0;
  font-size: 15px;
  line-height: 1.4;
}

.section-status,
.fields-count,
.suggested-name {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.template-path {
  width: 100%;
}

.placeholder-summary {
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 11px;
}

.summary-label,
.fields-heading {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.summary-label {
  margin-right: 4px;
}

.field-help,
.template-note {
  margin: 10px 0 0;
  color: var(--el-text-color-secondary);
  font-size: 12px;
  line-height: 1.5;
}

.email-template-toolbar,
.template-library-controls,
.template-library-actions,
.template-library-status {
  display: flex;
  align-items: center;
  min-width: 0;
}

.email-template-toolbar {
  flex-wrap: wrap;
  justify-content: space-between;
  gap: 8px 12px;
  margin-bottom: 10px;
}

.template-library-controls {
  flex: 1 1 250px;
  gap: 9px;
}

.email-template-select {
  flex: 1 1 150px;
  min-width: 0;
}

.email-template-select :deep(.el-select__wrapper),
.email-template-select :deep(.el-select__selection),
.email-template-select :deep(.el-select__selected-item) {
  min-width: 0;
}

.email-template-select :deep(.el-select__selected-item) {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.template-library-status {
  flex: 0 0 auto;
  gap: 6px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
  white-space: nowrap;
}

.template-dirty-status {
  box-sizing: border-box;
  width: 58px;
  justify-content: center;
}

.template-library-actions {
  flex: 0 0 auto;
  gap: 6px;
}

.template-library-actions > .el-button + .el-button {
  margin-left: 0;
}

.assistant-workspace {
  display: grid;
  grid-template-columns: minmax(260px, 0.92fr) minmax(0, 1.08fr);
  min-width: 0;
}

.assistant-workspace > .assistant-section {
  min-width: 0;
}

.email-section {
  padding-right: 20px;
  border-right: 1px solid var(--lc-border);
}

.preview-section {
  padding-left: 20px;
}

.email-template-input :deep(textarea) {
  min-height: 150px;
  line-height: 1.6;
}

.fields-heading {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  margin: 18px 0 10px;
  font-weight: 600;
}

.fields-form :deep(.el-form-item) {
  margin-bottom: 12px;
}

.fields-form :deep(.el-form-item__label) {
  margin-bottom: 4px;
  line-height: 1.3;
  white-space: normal;
  overflow-wrap: anywhere;
}

.email-preview {
  box-sizing: border-box;
  min-height: 230px;
  max-height: 430px;
  margin: 0;
  padding: 14px;
  overflow: auto;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-1);
  color: var(--lc-text);
  font-family: inherit;
  font-size: 13px;
  line-height: 1.7;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.validation-message {
  margin-top: 10px;
  color: var(--el-color-danger);
  font-size: 12px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}

.preview-ready {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 10px;
  color: var(--el-color-success);
  font-size: 12px;
}

.generate-section {
  padding-bottom: 6px;
}

.generate-row {
  justify-content: space-between;
  gap: 18px;
}

.generate-hint {
  min-width: 0;
}

.output-result {
  justify-content: space-between;
  gap: 14px;
  margin-top: 16px;
  padding: 11px 12px;
  border: 1px solid var(--el-color-success-light-5);
  border-radius: var(--lc-radius-md);
  background: var(--el-color-success-light-9);
}

.output-copy {
  gap: 9px;
  min-width: 0;
  color: var(--el-color-success);
}

.output-copy > div {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: 3px;
}

.output-copy span {
  color: var(--el-text-color-secondary);
  font-size: 12px;
  overflow-wrap: anywhere;
}

.assistant-error {
  margin: 14px 2px 0;
  padding: 10px 12px;
  border: 1px solid var(--el-color-danger-light-5);
  border-radius: var(--lc-radius-md);
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
  font-size: 12px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}

@media (max-width: 760px) {
  .assistant-header,
  .generate-row,
  .output-result {
    align-items: stretch;
    flex-direction: column;
  }

  .assistant-header {
    gap: 12px;
  }

  .assistant-actions,
  .assistant-actions .el-button,
  .generate-row > .el-button,
  .output-result > .el-button {
    width: 100%;
  }

  .email-template-toolbar {
    align-items: stretch;
    flex-direction: column;
  }

  .template-library-controls {
    flex: 0 1 auto;
    flex-wrap: wrap;
    width: 100%;
  }

  .email-template-select {
    flex-basis: 220px;
  }

  .template-library-actions {
    width: 100%;
  }

  .template-primary-action {
    flex: 1 1 120px;
  }

  .assistant-workspace {
    display: block;
  }

  .email-section {
    padding-right: 2px;
    border-right: 0;
  }

  .preview-section {
    padding-left: 2px;
  }
}
</style>
