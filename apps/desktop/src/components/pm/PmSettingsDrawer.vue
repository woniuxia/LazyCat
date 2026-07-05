<template>
  <el-drawer
    :model-value="modelValue"
    direction="rtl"
    size="480px"
    wrapper-class="pm-settings-drawer"
    destroy-on-close
    :with-header="true"
    @update:model-value="$emit('update:modelValue', $event)"
    @close="$emit('update:modelValue', false)"
  >
    <template #header>
      导入与设置
    </template>
    <el-tabs v-model="activeTab" class="pm-settings-tabs">
      <!-- Tab: Import -->
      <el-tab-pane label="导入工作项" name="import">
        <div class="pm-settings-body">
          <div class="import-section">
            <div class="import-section-title">Excel 文件</div>
            <div class="import-file-row">
              <el-input :model-value="fileName" readonly placeholder="选择 .xlsx / .xls 文件" />
              <el-button :disabled="importing" @click="pickFile">选择文件</el-button>
            </div>
          </div>

          <template v-if="headers.length > 0">
            <div class="import-section">
              <div class="import-section-title">
                列映射
                <span class="import-section-hint">（* 标题为必填，其余可选）</span>
              </div>

              <div class="import-mapping-row">
                <el-select v-model="templateId" placeholder="使用模板" clearable style="flex: 1" @change="applyTemplate">
                  <el-option v-for="t in templates" :key="t.id" :label="t.name" :value="t.id" />
                </el-select>
                <el-button size="small" :disabled="!canSaveTemplate" @click="saveTemplate">保存模板</el-button>
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

            <div class="import-section">
              <div class="import-section-title">
                过滤规则
                <span class="import-section-hint">（仅导入匹配的行）</span>
              </div>
              <div v-for="(rule, idx) in filterRules" :key="idx" class="import-filter-row">
                <el-select v-model="rule.column" placeholder="列" style="width: 130px">
                  <el-option v-for="h in headers" :key="h" :label="h" :value="h" />
                </el-select>
                <el-select v-model="rule.operator" placeholder="条件" style="width: 110px">
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

            <div v-if="sampleRows.length > 0" class="import-section">
              <div class="import-section-title">数据预览（前 5 行）</div>
              <div class="import-preview-table-wrap">
                <table class="import-preview-table">
                  <thead>
                    <tr><th v-for="h in headers" :key="h">{{ h }}</th></tr>
                  </thead>
                  <tbody>
                    <tr v-for="(row, ri) in sampleRows" :key="ri">
                      <td v-for="(cell, ci) in row" :key="ci">{{ cell }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
          </template>

          <div v-if="result" class="import-section import-result">
            <el-alert type="success" :closable="false">
              <template #title>
                导入完成：成功 {{ result.imported }} 条
                <span v-if="result.projectsCreated">，新建项目 {{ result.projectsCreated }} 个</span>
                <span v-if="result.skippedDuplicate"> | 编号重复跳过 {{ result.skippedDuplicate }}</span>
                <span v-if="result.skippedFilter"> | 未匹配跳过 {{ result.skippedFilter }}</span>
                <span v-if="result.skippedEmptyTitle"> | 标题为空跳过 {{ result.skippedEmptyTitle }}</span>
                <span v-if="result.skippedNoProject"> | 无项目跳过 {{ result.skippedNoProject }}</span>
              </template>
            </el-alert>
          </div>

          <div class="import-actions">
            <el-button :disabled="!canImport" :loading="importing" type="primary" @click="doImport">
              {{ importing ? '导入中...' : '开始导入' }}
            </el-button>
          </div>
        </div>
      </el-tab-pane>

      <!-- Tab: SiYuan -->
      <el-tab-pane label="思源配置" name="siyuan">
        <div class="pm-settings-body">
          <el-form label-position="top" size="default">
            <el-form-item label="服务地址">
              <el-input v-model="siyuan.form.baseUrl" placeholder="http://127.0.0.1:6806" clearable />
            </el-form-item>
            <el-form-item label="API Token">
              <el-input
                :type="siyuan.showToken.value ? 'text' : 'password'"
                v-model="siyuan.form.token"
                placeholder="填写思源 API Token"
                clearable
              >
                <template #suffix>
                  <el-button type="text" size="small" @click="siyuan.showToken.value = !siyuan.showToken.value">
                    {{ siyuan.showToken.value ? "隐藏" : "显示" }}
                  </el-button>
                </template>
              </el-input>
            </el-form-item>
          </el-form>
          <div class="pm-siyuan-config-card">
            <div class="pm-siyuan-link-title">任务默认存储位置</div>
            <div class="pm-siyuan-config-summary">
              {{ formatPmSiyuanLocationLabel(siyuan.globalSiyuanLocationDraft.value) }}
            </div>
            <div class="pm-siyuan-inline-actions">
              <el-button size="small" @click="siyuan.openLocationPicker('global')">选择位置</el-button>
              <el-button size="small" @click="siyuan.globalSiyuanLocationDraft.value = null">清空</el-button>
            </div>
          </div>
          <div class="siyuan-actions">
            <el-button type="primary" @click="siyuan.saveConfig()">保存配置</el-button>
            <el-button type="info" :loading="siyuan.testing.value" @click="siyuan.testConnection()">测试连接</el-button>
            <el-button type="default" :loading="siyuan.loadingDirectory.value" @click="siyuan.loadDirectory()">加载目录</el-button>
          </div>
          <div class="siyuan-status">
            <el-tag v-if="siyuan.testingVersion.value" type="success" effect="dark">
              已连接 · {{ siyuan.testingVersion.value }}
            </el-tag>
            <el-alert
              v-if="siyuan.error.value"
              :title="siyuan.errorTitle.value"
              :description="siyuan.error.value"
              type="error"
              show-icon
              class="siyuan-error-alert"
            />
            <div v-else-if="siyuan.directoryFetchedAt.value" class="siyuan-fetch-hint">
              最后一次加载：{{ siyuan.directoryFetchedAt.value }}
            </div>
          </div>
          <div class="siyuan-tree-section">
            <div v-if="siyuan.loadingDirectory.value" class="siyuan-loading-hint">正在从思源加载目录...</div>
            <el-empty
              v-if="!siyuan.directory.value.length && !siyuan.loadingDirectory.value"
              description="暂无目录记录，先点击加载目录"
            />
            <el-tree
              v-else-if="siyuan.directory.value.length"
              :data="siyuan.directory.value"
              :props="siyuan.treeProps.value"
              node-key="id"
              :expand-on-click-node="false"
              class="siyuan-tree"
            >
              <template #default="{ data }">
                <div class="siyuan-tree-node siyuan-tree-node--preview">
                  <div class="siyuan-node-main">
                    <span class="siyuan-node-title">{{ data.name }}</span>
                    <span
                      v-if="isPmSiyuanNotebookDirectory(data)"
                      class="siyuan-node-badge"
                      :class="{ 'is-disabled': data.closed }"
                    >
                      {{ data.closed ? "已关闭" : `${data.docCount} 篇` }}
                    </span>
                  </div>
                </div>
              </template>
            </el-tree>
          </div>
        </div>
      </el-tab-pane>
    </el-tabs>

    <!-- SiYuan sub-dialogs (location picker & page selector) -->
    <el-dialog
      v-model="siyuan.locationDialogVisible.value"
      :title="siyuan.locationPickerTitle.value"
      width="720px"
      destroy-on-close
      append-to-body
    >
      <div class="pm-siyuan-dialog-body pm-siyuan-dialog-body--picker">
        <div class="pm-siyuan-picker-intro">选择笔记本根目录或某个父文档后，项目内新建思源页面会默认存放到这里。</div>
        <el-input v-model="siyuan.locationPickerSearch.value" class="pm-siyuan-picker-search" placeholder="搜索笔记本、文档标题或路径" clearable />
        <div v-if="siyuan.loadingDirectory.value && !siyuan.directory.value.length" class="pm-siyuan-empty-hint">正在同步思源目录，请稍候…</div>
        <div v-else-if="!siyuan.directory.value.length" class="pm-siyuan-empty-hint">尚未加载思源目录，请先测试连接并加载目录。</div>
        <template v-else>
          <div class="pm-siyuan-picker-tree-head">
            <span>{{ siyuan.locationPickerStatusText.value }}</span>
            <div class="pm-siyuan-inline-actions">
              <span v-if="siyuan.directoryFetchedAt.value">最后同步：{{ siyuan.directoryFetchedAt.value }}</span>
              <span v-if="siyuan.locationPickerSearchKeyword.value">清空搜索后会恢复默认折叠。</span>
              <el-button size="small" text :loading="siyuan.loadingDirectory.value" @click="siyuan.refreshLocationPickerDirectory()">刷新目录</el-button>
            </div>
          </div>
          <div class="pm-siyuan-picker-tree-shell">
            <el-empty v-if="siyuan.locationPickerTreeData.value.length === 0" description="未找到匹配位置，请换个关键词试试" />
            <el-tree
              v-else
              :key="siyuan.locationPickerTreeKey.value"
              :data="siyuan.locationPickerTreeData.value"
              :props="siyuan.treeProps.value"
              node-key="id"
              highlight-current
              :current-node-key="siyuan.locationPickerCurrentNodeKey.value"
              :default-expanded-keys="siyuan.locationPickerExpandedKeys.value"
              :expand-on-click-node="false"
              class="siyuan-tree pm-siyuan-picker-tree"
              @node-click="siyuan.handleLocationTreeNodeClick"
            >
              <template #default="{ data, node }">
                <div
                  class="siyuan-tree-node siyuan-tree-node--interactive"
                  :class="{
                    'is-selected': siyuan.isLocationPickerNodeSelected(data),
                    'is-disabled': siyuan.isLocationPickerNodeDisabled(data, node),
                  }"
                >
                  <div class="siyuan-node-main">
                    <span class="siyuan-node-title">{{ data.name }}</span>
                    <span v-if="isPmSiyuanNotebookDirectory(data)" class="siyuan-node-badge" :class="{ 'is-disabled': data.closed }">
                      {{ data.closed ? "已关闭" : `${data.docCount} 篇` }}
                    </span>
                  </div>
                </div>
              </template>
            </el-tree>
          </div>
        </template>
        <div class="pm-siyuan-picker-selection" :class="{ 'is-empty': !siyuan.locationPickerValue.value }">
          <div class="pm-siyuan-picker-selection-label">当前选择</div>
          <template v-if="siyuan.locationPickerValue.value">
            <div class="pm-siyuan-picker-selection-title">{{ siyuan.locationPickerSelectionTarget.value }}</div>
            <div class="pm-siyuan-picker-selection-meta">{{ siyuan.locationPickerValue.value.notebookName }}</div>
            <div class="pm-siyuan-picker-selection-path">{{ siyuan.locationPickerSelectionPath.value }}</div>
          </template>
          <div v-else class="pm-siyuan-picker-selection-empty">暂未选择位置，点击上方笔记本根目录或父文档即可。</div>
        </div>
      </div>
      <template #footer>
        <el-button @click="siyuan.locationDialogVisible.value = false">取消</el-button>
        <el-button @click="siyuan.clearLocationPicker()">清空</el-button>
        <el-button type="primary" @click="siyuan.applyLocationPicker()">确定</el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="siyuan.pageDialogVisible.value"
      :title="siyuan.pageDialogTitle.value"
      width="760px"
      append-to-body
    >
      <div class="pm-siyuan-dialog-body">
        <div class="pm-siyuan-search-toolbar">
          <el-input v-model="siyuan.pageFilterKeyword.value" :placeholder="siyuan.pageDialogInputPlaceholder.value" clearable />
          <el-button v-if="siyuan.pageShowReturnToLocation.value" @click="siyuan.restoreLocationResults()">返回当前位置列表</el-button>
          <el-button :loading="siyuan.pageSearchingAll.value" @click="siyuan.expandPagesToAll()">扩展到全库</el-button>
          <el-button type="success" :loading="siyuan.pageCreating.value" :disabled="!siyuan.pageCanCreateImmediately.value" @click="siyuan.createPageForItem()">立即新建</el-button>
        </div>
        <div class="pm-siyuan-dialog-hint">{{ siyuan.pageCurrentRangeText.value }}</div>
        <div v-if="siyuan.pageFilterSummary.value" class="pm-siyuan-dialog-hint">{{ siyuan.pageFilterSummary.value }}</div>
        <div class="pm-siyuan-dialog-hint">当前创建位置：{{ formatPmSiyuanLocationLabel(siyuan.itemEffectiveLocation.value) }}</div>
        <div class="pm-siyuan-dialog-hint">当前创建标题：{{ siyuan.pageCreateTitle.value || "请先填写工作项标题或输入想创建的页面标题" }}</div>
        <div v-if="siyuan.pageLocationRefreshError.value && siyuan.pageResultSource.value === 'location' && siyuan.pageLocationState.value !== 'load-error'" class="pm-siyuan-dialog-notice pm-siyuan-dialog-notice--warning">
          当前位置列表刷新失败，暂展示上一次结果。{{ siyuan.pageLocationRefreshError.value }}
        </div>
        <div v-if="siyuan.pageShowLocationLoading.value" class="pm-siyuan-empty-hint">正在加载当前位置文档列表...</div>
        <div v-else-if="siyuan.pageShowAllLoading.value" class="pm-siyuan-empty-hint">正在搜索全库...</div>
        <div v-else-if="siyuan.pageEmptyMessage.value" class="pm-siyuan-empty-hint">{{ siyuan.pageEmptyMessage.value }}</div>
        <div v-else class="pm-siyuan-page-list">
          <div v-for="page in siyuan.pageDisplayedResults.value" :key="page.docId" class="pm-siyuan-page-row">
            <div class="pm-siyuan-page-main">
              <div class="pm-siyuan-page-title">{{ page.docTitle }}</div>
              <div class="pm-siyuan-page-meta">{{ page.notebookName }} · {{ page.docHpath }}</div>
            </div>
            <div class="pm-siyuan-inline-actions">
              <el-button size="small" type="primary" link @click="siyuan.selectPageResult(page)">{{ siyuan.pageDialogMode.value === 'primary' ? '设为主页面' : '添加' }}</el-button>
              <el-button size="small" link @click="siyuan.openSiyuanPage(page)">打开</el-button>
            </div>
          </div>
        </div>
      </div>
      <template #footer>
        <el-button @click="siyuan.pageDialogVisible.value = false">关闭</el-button>
      </template>
    </el-dialog>
  </el-drawer>
</template>

<script setup lang="ts">
import { ref, computed, inject } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { Delete } from "@element-plus/icons-vue";
import { invokeToolByChannel } from "../../bridge/tauri";
import { getSettingJson, setSettingJson } from "../../composables/useSettings";
import { formatPmSiyuanLocationLabel, isPmSiyuanNotebookDirectory } from "../../utils/pmSiyuan";
import { PM_SIYUAN_KEY } from "../../composables/pmSiyuanKey";
import type {
  PmImportMapping,
  PmImportFilterRule,
  PmImportTemplate,
  PmImportPreview,
  PmImportResult,
} from "../../types/pm";

defineOptions({ name: "PmSettingsDrawer" });

const props = defineProps<{ modelValue: boolean }>();
const emit = defineEmits<{ (e: "update:modelValue", val: boolean): void; (e: "imported"): void }>();

const siyuan = inject(PM_SIYUAN_KEY)!;

const activeTab = ref("import");
const TEMPLATES_KEY = "pm:import-templates";

// ── Import state ──
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
const canSaveTemplate = computed(
  () => mapping.value.title !== "" && headers.value.length > 0,
);

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
    const preview = await invokeToolByChannel<PmImportPreview>("tool:pm:item-import-preview", { filePath: path });
    headers.value = preview.headers;
    sampleRows.value = preview.sampleRows;
    mapping.value = { title: "" };
  } catch (e) {
    ElMessage.error(String(e));
  }
}

function addFilter() {
  filterRules.value.push({ column: headers.value[0] || "", operator: "contains", value: "" });
}

function applyTemplate(id: string | null) {
  if (!id) return;
  const tpl = templates.value.find(t => t.id === id);
  if (!tpl) return;
  mapping.value = { ...tpl.mapping };
  filterRules.value = tpl.filters.map(f => ({ ...f }));
}

function saveTemplate() {
  const name = prompt("输入模板名称");
  if (!name) return;
  const id = Date.now().toString(36);
  templates.value.push({ id, name, mapping: { ...mapping.value }, filters: filterRules.value.map(f => ({ ...f })) });
  templateId.value = id;
  persistTemplates();
  ElMessage.success("模板已保存");
}

function deleteTemplate() {
  if (!templateId.value) return;
  templates.value = templates.value.filter(t => t.id !== templateId.value);
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

<style>
.pm-settings-drawer .el-drawer__body {
  padding: 0;
}
.pm-settings-drawer .el-tabs__header {
  padding: 0 16px;
  margin-bottom: 0;
}
.pm-settings-drawer .el-tabs__content {
  padding: 0;
}
.pm-settings-tabs .el-tab-pane {
  overflow-y: auto;
  max-height: calc(100vh - 120px);
}
</style>

<style scoped>
.pm-settings-body {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.import-section { margin-bottom: 4px; }
.import-section-title { font-weight: 600; margin-bottom: 8px; font-size: 14px; }
.import-section-hint { font-weight: 400; font-size: 12px; color: var(--el-text-color-secondary); }
.import-file-row { display: flex; gap: 8px; }
.import-mapping-row { display: flex; gap: 8px; align-items: center; margin-bottom: 12px; }
.import-mapping-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 8px 16px; }
.import-mapping-field { display: flex; flex-direction: column; gap: 4px; }
.import-mapping-label { font-size: 12px; color: var(--el-text-color-regular); }
.import-filter-row { display: flex; gap: 8px; align-items: center; margin-bottom: 8px; }
.import-preview-table-wrap { overflow-x: auto; border: 1px solid var(--el-border-color-light); border-radius: 4px; }
.import-preview-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.import-preview-table th,
.import-preview-table td { padding: 4px 8px; border-bottom: 1px solid var(--el-border-color-lighter); text-align: left; white-space: nowrap; max-width: 160px; overflow: hidden; text-overflow: ellipsis; }
.import-preview-table th { background: var(--el-fill-color-light); font-weight: 600; }
.import-result { margin-top: 4px; }
.import-actions { padding-top: 8px; }

/* SiYuan config cards */
.pm-siyuan-config-card {
  border-radius: 14px;
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(244, 247, 251, 0.82));
  border: 1px solid rgba(219, 229, 241, 0.88);
}
.pm-siyuan-config-summary { font-size: 13px; color: var(--pm-text-muted); line-height: 1.55; }
.pm-siyuan-inline-actions { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
.pm-siyuan-link-title { font-size: 14px; font-weight: 600; color: var(--pm-text-main); }
.siyuan-actions { display: flex; gap: 10px; flex-wrap: wrap; }
.siyuan-status { display: flex; flex-direction: column; gap: 8px; }
.siyuan-fetch-hint { font-size: 13px; color: var(--pm-text-muted); }
.siyuan-loading-hint { font-size: 13px; color: var(--pm-text-muted); }
.siyuan-tree-section { display: flex; flex-direction: column; gap: 10px; }
.siyuan-tree { max-height: 360px; overflow-y: auto; }
.siyuan-tree-node { display: flex; align-items: center; justify-content: space-between; gap: 10px; width: 100%; }
.siyuan-tree-node--preview { cursor: default; }
.siyuan-tree-node--interactive { cursor: pointer; padding: 4px 8px; border-radius: 6px; transition: background 0.15s; }
.siyuan-tree-node--interactive.is-selected { background: rgba(14, 165, 233, 0.08); }
.siyuan-tree-node--interactive.is-disabled { opacity: 0.45; cursor: not-allowed; }
.siyuan-node-main { display: flex; align-items: center; gap: 8px; min-width: 0; flex: 1; }
.siyuan-node-title { font-size: 14px; color: var(--pm-text-main); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.siyuan-node-badge { font-size: 12px; color: var(--pm-text-muted); flex-shrink: 0; }
.siyuan-node-badge.is-disabled { opacity: 0.6; }
.siyuan-error-alert { padding: 8px 10px; }

/* SiYuan dialogs */
.pm-siyuan-dialog-body { display: flex; flex-direction: column; gap: 14px; }
.pm-siyuan-dialog-hint { font-size: 13px; color: var(--pm-text-muted); line-height: 1.55; }
.pm-siyuan-dialog-notice { border-radius: 8px; padding: 10px 12px; font-size: 13px; color: var(--pm-text-muted); line-height: 1.55; }
.pm-siyuan-dialog-notice--warning { background: rgba(230, 162, 60, 0.08); border: 1px solid rgba(230, 162, 60, 0.18); }
.pm-siyuan-picker-intro { font-size: 13px; color: var(--pm-text-muted); line-height: 1.55; }
.pm-siyuan-search-toolbar { display: flex; gap: 10px; align-items: center; }
.pm-siyuan-search-toolbar .el-input { flex: 1; }
.pm-siyuan-dialog-body--picker { gap: 16px; }
.pm-siyuan-empty-hint { padding: 10px 12px; border-radius: 8px; border: 1px dashed rgba(219, 229, 241, 0.9); font-size: 13px; color: var(--pm-text-muted); text-align: center; }
.pm-siyuan-page-list { display: flex; flex-direction: column; gap: 8px; }
.pm-siyuan-page-row { display: flex; align-items: flex-start; justify-content: space-between; gap: 10px; padding: 12px; border-radius: 10px; background: rgba(244, 247, 251, 0.76); border: 1px solid rgba(219, 229, 241, 0.88); }
.pm-siyuan-page-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.pm-siyuan-page-title { font-size: 14px; font-weight: 600; color: var(--pm-text-main); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.pm-siyuan-page-meta { font-size: 12px; color: var(--pm-text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.pm-siyuan-picker-selection { border: 1px solid rgba(219, 229, 241, 0.9); border-radius: 12px; padding: 12px 14px; display: flex; flex-direction: column; gap: 6px; background: rgba(244, 247, 251, 0.6); }
.pm-siyuan-picker-selection.is-empty { background: rgba(244, 247, 251, 0.3); }
.pm-siyuan-picker-selection-label { font-size: 12px; font-weight: 600; color: var(--pm-text-muted); text-transform: uppercase; letter-spacing: 0.05em; }
.pm-siyuan-picker-selection-title { font-size: 14px; font-weight: 600; color: var(--pm-text-main); }
.pm-siyuan-picker-selection-meta { font-size: 13px; color: var(--pm-text-muted); }
.pm-siyuan-picker-selection-path { font-size: 12px; color: var(--pm-text-muted); }
.pm-siyuan-picker-selection-empty { font-size: 13px; color: var(--pm-text-muted); }
.pm-siyuan-picker-tree-head { display: flex; align-items: center; justify-content: space-between; gap: 10px; flex-wrap: wrap; }
.pm-siyuan-picker-tree-shell { border-radius: 16px; border: 1px solid rgba(219, 229, 241, 0.88); padding: 10px; max-height: 320px; overflow-y: auto; }

/* Tree depth overrides */
:deep(.siyuan-tree .el-tree-node__content) { height: 34px; border-radius: 6px; }
:deep(.siyuan-tree .el-tree-node__content:hover) { background: rgba(14, 165, 233, 0.04); }
:deep(.pm-siyuan-picker-tree .el-tree-node.is-current > .el-tree-node__content) { background: rgba(14, 165, 233, 0.08); }
:deep(.siyuan-tree .el-tree-node__expand-icon) { color: var(--pm-text-muted); }
:deep(.siyuan-tree .el-tree-node__expand-icon.expanded) { color: var(--pm-text-muted); }
:deep(.siyuan-tree .el-tree-node__label) { width: 100%; }
</style>
