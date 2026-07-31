<template>
  <el-dialog
    v-model="visible"
    title="导入工作项"
    width="660px"
    :close-on-click-modal="false"
    destroy-on-close
    @closed="reset"
  >
    <!-- File -->
    <div class="import-section">
      <div class="import-section-title">Excel 文件</div>
      <div class="import-file-row">
        <el-input :model-value="fileName" readonly placeholder="选择 .xlsx / .xls 文件" />
        <el-button :disabled="importing" @click="pickFile">选择文件</el-button>
      </div>
    </div>

    <!-- Template & Mapping -->
    <div v-if="headers.length > 0" class="import-section">
      <div class="import-section-title">
        列映射
        <span class="import-section-hint">（* 标题为必填，其余可选）</span>
      </div>

      <div class="import-mapping-row">
        <el-select
          v-model="templateId"
          placeholder="使用模板"
          clearable
          style="flex: 1"
          @change="applyTemplate"
        >
          <el-option v-for="t in templates" :key="t.id" :label="t.name" :value="t.id" />
        </el-select>
        <el-button size="small" :disabled="!canSaveTemplate" @click="saveTemplate"
          >保存模板</el-button
        >
        <el-button size="small" :disabled="!templateId" @click="deleteTemplate">删除</el-button>
      </div>

      <div class="import-mapping-grid">
        <div class="import-mapping-field">
          <span class="import-mapping-label">标题 *</span>
          <el-select v-model="mapping.title" placeholder="选择列">
            <el-option v-for="h in headers" :key="h" :label="h" :value="h" />
          </el-select>
        </div>
        <div class="import-mapping-field">
          <span class="import-mapping-label">系统名称</span>
          <el-select v-model="mapping.projectName" placeholder="不映射" clearable>
            <el-option v-for="h in headers" :key="h" :label="h" :value="h" />
          </el-select>
        </div>
        <div class="import-mapping-field">
          <span class="import-mapping-label">编号</span>
          <el-select v-model="mapping.refCode" placeholder="不映射" clearable>
            <el-option v-for="h in headers" :key="h" :label="h" :value="h" />
          </el-select>
        </div>
        <div class="import-mapping-field">
          <span class="import-mapping-label">开始时间</span>
          <el-select v-model="mapping.startAt" placeholder="不映射" clearable>
            <el-option v-for="h in headers" :key="h" :label="h" :value="h" />
          </el-select>
        </div>
        <div class="import-mapping-field">
          <span class="import-mapping-label">结束时间</span>
          <el-select v-model="mapping.endAt" placeholder="不映射" clearable>
            <el-option v-for="h in headers" :key="h" :label="h" :value="h" />
          </el-select>
        </div>
        <div class="import-mapping-field">
          <span class="import-mapping-label">描述 A</span>
          <el-select v-model="mapping.descriptionA" placeholder="不映射" clearable>
            <el-option v-for="h in headers" :key="h" :label="h" :value="h" />
          </el-select>
        </div>
        <div class="import-mapping-field">
          <span class="import-mapping-label">描述 B</span>
          <el-select v-model="mapping.descriptionB" placeholder="不映射" clearable>
            <el-option v-for="h in headers" :key="h" :label="h" :value="h" />
          </el-select>
        </div>
      </div>
    </div>

    <!-- Filter rules -->
    <div v-if="headers.length > 0" class="import-section">
      <div class="import-section-title">
        过滤规则
        <span class="import-section-hint">（仅导入匹配的行）</span>
      </div>
      <div v-for="(rule, idx) in filterRules" :key="idx" class="import-filter-row">
        <el-select v-model="rule.column" placeholder="列" style="width: 140px">
          <el-option v-for="h in headers" :key="h" :label="h" :value="h" />
        </el-select>
        <el-select v-model="rule.operator" placeholder="条件" style="width: 120px">
          <el-option label="包含" value="contains" />
          <el-option label="不包含" value="not_contains" />
          <el-option label="等于" value="equals" />
          <el-option label="不等于" value="not_equals" />
          <el-option label="为空" value="empty" />
          <el-option label="不为空" value="not_empty" />
        </el-select>
        <el-input
          v-model="rule.value"
          placeholder="值"
          style="flex: 1"
          :disabled="rule.operator === 'empty' || rule.operator === 'not_empty'"
        />
        <el-button text type="danger" @click="filterRules.splice(idx, 1)">
          <el-icon><Delete /></el-icon>
        </el-button>
      </div>
      <el-button text type="primary" @click="addFilter">+ 添加过滤规则</el-button>
    </div>

    <!-- Preview -->
    <div v-if="sampleRows.length > 0" class="import-section">
      <div class="import-section-title">数据预览（前 5 行）</div>
      <div class="import-preview-table-wrap">
        <table class="import-preview-table">
          <thead>
            <tr>
              <th v-for="h in headers" :key="h">{{ h }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, ri) in sampleRows" :key="ri">
              <td v-for="(cell, ci) in row" :key="ci">{{ cell }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Result -->
    <div v-if="result" class="import-section import-result">
      <el-alert type="success" :closable="false">
        <template #title>
          导入完成：成功 {{ result.imported }} 条
          <span v-if="result.projectsCreated">，新建项目 {{ result.projectsCreated }} 个</span>
          <span v-if="result.skippedDuplicate"> | 编号重复跳过 {{ result.skippedDuplicate }}</span>
          <span v-if="result.skippedFilter"> | 未匹配跳过 {{ result.skippedFilter }}</span>
          <span v-if="result.skippedEmptyTitle">
            | 标题为空跳过 {{ result.skippedEmptyTitle }}</span
          >
          <span v-if="result.skippedNoProject"> | 无项目跳过 {{ result.skippedNoProject }}</span>
        </template>
      </el-alert>
    </div>

    <template #footer>
      <el-button @click="visible = false">{{ result ? "关闭" : "取消" }}</el-button>
      <el-button type="primary" :loading="importing" :disabled="!canImport" @click="doImport">
        {{ importing ? "导入中..." : "开始导入" }}
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { Delete } from "@element-plus/icons-vue";
import { invokeToolByChannel } from "../../bridge/tauri";
import { getSettingJson, setSettingJson } from "../../composables/useSettings";
import type {
  PmImportMapping,
  PmImportFilterRule,
  PmImportTemplate,
  PmImportPreview,
  PmImportResult,
} from "../../types/pm";

defineOptions({ name: "PmImportDialog" });

const TEMPLATES_KEY = "pm:import-templates";

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", val: boolean): void;
  (e: "imported"): void;
}>();

const visible = computed({
  get: () => props.modelValue,
  set: (val) => emit("update:modelValue", val),
});
const importing = ref(false);
const filePath = ref("");
const fileName = ref("");
const headers = ref<string[]>([]);
const sampleRows = ref<string[][]>([]);
const result = ref<PmImportResult | null>(null);

const mapping = ref<PmImportMapping>({ title: "" });
const filterRules = ref<PmImportFilterRule[]>([]);
const templates = ref<PmImportTemplate[]>(loadTemplates());
const templateId = ref<string | null>(null);

function loadTemplates(): PmImportTemplate[] {
  return getSettingJson<PmImportTemplate[]>(TEMPLATES_KEY, []);
}
function persistTemplates() {
  setSettingJson(TEMPLATES_KEY, templates.value);
}

const canImport = computed(
  () => filePath.value !== "" && mapping.value.title !== "" && !result.value,
);

const canSaveTemplate = computed(() => mapping.value.title !== "" && headers.value.length > 0);

function reset() {
  filePath.value = "";
  fileName.value = "";
  headers.value = [];
  sampleRows.value = [];
  result.value = null;
  mapping.value = { title: "" };
  filterRules.value = [];
  templateId.value = null;
}

async function pickFile() {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Excel", extensions: ["xlsx", "xls", "ods"] }],
    });
    if (!selected) return;
    const path = typeof selected === "string" ? selected : selected.path;
    if (!path) return;
    filePath.value = path;
    fileName.value = path.split(/[\\/]/).pop() || path;
    result.value = null;

    const preview = await invokeToolByChannel<PmImportPreview>("tool:pm:item-import-preview", {
      filePath: path,
    });
    headers.value = preview.headers;
    sampleRows.value = preview.sampleRows;
    mapping.value = { title: "" };
  } catch (e) {
    ElMessage.error(String(e));
  }
}

function addFilter() {
  filterRules.value.push({
    column: headers.value[0] || "",
    operator: "contains",
    value: "",
  });
}

function applyTemplate(id: string | null) {
  if (!id) return;
  const tpl = templates.value.find((t) => t.id === id);
  if (!tpl) return;
  mapping.value = { ...tpl.mapping };
  filterRules.value = tpl.filters.map((f) => ({ ...f }));
}

function saveTemplate() {
  const name = prompt("输入模板名称");
  if (!name) return;
  const id = Date.now().toString(36);
  const tpl: PmImportTemplate = {
    id,
    name,
    mapping: { ...mapping.value },
    filters: filterRules.value.map((f) => ({ ...f })),
  };
  templates.value.push(tpl);
  templateId.value = id;
  persistTemplates();
  ElMessage.success("模板已保存");
}

function deleteTemplate() {
  if (!templateId.value) return;
  templates.value = templates.value.filter((t) => t.id !== templateId.value);
  templateId.value = null;
  persistTemplates();
  ElMessage.success("模板已删除");
}

async function doImport() {
  if (!filePath.value || !mapping.value.title) return;
  importing.value = true;
  try {
    const res = await invokeToolByChannel<PmImportResult>("tool:pm:item-import", {
      filePath: filePath.value,
      mapping: mapping.value,
      filters: filterRules.value,
    });
    result.value = res;
    emit("imported");
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    importing.value = false;
  }
}
</script>

<style scoped>
.import-section {
  margin-bottom: 16px;
}
.import-section-title {
  font-weight: 600;
  margin-bottom: 8px;
  font-size: 14px;
}
.import-section-hint {
  font-weight: 400;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.import-file-row {
  display: flex;
  gap: 8px;
}
.import-mapping-row {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-bottom: 12px;
}
.import-mapping-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px 16px;
}
.import-mapping-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.import-mapping-label {
  font-size: 12px;
  color: var(--el-text-color-regular);
}
.import-filter-row {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-bottom: 8px;
}
.import-preview-table-wrap {
  overflow-x: auto;
  border: 1px solid var(--el-border-color-light);
  border-radius: 4px;
}
.import-preview-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}
.import-preview-table th,
.import-preview-table td {
  padding: 4px 8px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  text-align: left;
  white-space: nowrap;
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.import-preview-table th {
  background: var(--el-fill-color-light);
  font-weight: 600;
}
.import-result {
  margin-top: 12px;
}
</style>
