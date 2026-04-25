<template>
  <el-dialog
    :model-value="visible"
    :title="editingItem ? '编辑工作项' : '新建工作项'"
    width="860px"
    @close="handleClose"
  >
    <el-form :model="form" label-position="top" size="default" class="pm-item-dialog-form">
      <!-- 所属项目卡片 -->
      <div
        class="pm-item-card pm-item-project-card"
        :class="{
          'pm-item-project-card--switchable': projectSwitcherEnabled,
          'pm-item-project-card--switch-open': projectSwitchMenuVisible,
        }"
      >
        <div class="pm-item-project-card-head">
          <div class="pm-item-project-card-body">
            <div class="pm-item-section-eyebrow">所属项目</div>
            <div class="pm-item-project-card-main">
              <span class="pm-item-project-card-dot" :style="{ backgroundColor: projectDisplayColor }" />
              <span class="pm-item-project-card-name">{{ projectDisplayName }}</span>
            </div>
          </div>
          <div v-if="projectSwitcherEnabled" class="pm-item-project-card-actions">
            <el-dropdown
              trigger="click"
              placement="bottom-end"
              @command="handleProjectSwitchCommand"
              @visible-change="handleProjectSwitchVisibleChange"
            >
              <button type="button" class="pm-item-project-switch-trigger">
                切换项目
              </button>
              <template #dropdown>
                <el-dropdown-menu class="pm-item-project-switch-menu">
                  <el-dropdown-item
                    v-for="p in projectOptions"
                    :key="p.id"
                    :command="p.id"
                    :disabled="p.id === dialogProjectId"
                  >
                    <div class="pm-item-project-switch-item">
                      <span class="pm-item-project-switch-item-name">{{ p.name }}</span>
                      <span v-if="p.id === dialogProjectId" class="pm-item-project-switch-item-meta">
                        当前项目
                      </span>
                      <span
                        v-else-if="p.status === 'archived'"
                        class="pm-item-project-switch-item-meta"
                      >
                        已归档
                      </span>
                    </div>
                  </el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>
          </div>
        </div>
      </div>

      <!-- 核心信息 -->
      <div class="pm-item-section">
        <div class="pm-item-section-title">核心信息</div>
        <el-form-item label="标题">
          <el-input v-model="form.title" placeholder="工作项标题" />
        </el-form-item>
        <el-form-item label="编号">
          <el-input v-model="form.refCode" placeholder="可选，用于外部系统关联和去重" clearable />
        </el-form-item>
        <div class="pm-item-dialog-core-grid">
          <el-form-item label="类型" class="pm-item-dialog-inline-field">
            <el-select v-model="form.itemType">
              <el-option v-for="(meta, key) in PM_ITEM_TYPE_MAP" :key="key" :label="meta.label" :value="key" />
            </el-select>
          </el-form-item>
          <el-form-item label="优先级" class="pm-item-dialog-inline-field">
            <el-select v-model="form.priority">
              <template #prefix>
                <span class="priority-dot" :style="{ backgroundColor: PM_PRIORITY_MAP[form.priority]?.color }" />
              </template>
              <el-option v-for="(meta, key) in PM_PRIORITY_MAP" :key="key" :label="meta.label" :value="key">
                <span class="priority-dot" :style="{ backgroundColor: meta.color }" />
                {{ meta.label }}
              </el-option>
            </el-select>
          </el-form-item>
          <el-form-item label="状态" class="pm-item-dialog-inline-field">
            <el-select v-model="form.status">
              <el-option v-for="col in PM_STATUS_COLUMNS" :key="col.key" :label="col.label" :value="col.key" />
            </el-select>
          </el-form-item>
        </div>
      </div>

      <!-- 时间安排 -->
      <div class="pm-item-card pm-item-dialog-inline-row">
        <div class="pm-item-dialog-inline-label">时间安排</div>
        <div class="pm-item-dialog-inline-content">
          <el-form-item class="pm-item-dialog-inline-form-item">
            <el-date-picker
              v-model="formDateRange"
              type="daterange"
              value-format="YYYY-MM-DD"
              range-separator="~"
              start-placeholder="开始日期"
              end-placeholder="截止日期"
              clearable
              class="pm-item-dialog-inline-picker"
            />
          </el-form-item>
        </div>
      </div>

      <!-- 链接 -->
      <div class="pm-item-card pm-item-dialog-inline-row pm-item-dialog-inline-row--link">
          <div class="pm-item-dialog-inline-label">链接</div>
          <div class="pm-item-dialog-inline-content pm-item-dialog-inline-content--link">
            <div class="pm-item-dialog-link-main">
              <el-form-item class="pm-item-dialog-inline-form-item pm-item-dialog-inline-form-item--grow">
                <el-input
                  v-model="form.linkUrl"
                  placeholder="https://example.com 或 localhost:3000"
                />
              </el-form-item>
              <div class="pm-item-dialog-link-actions">
                <el-button
                  plain
                  class="pm-item-dialog-link-clear-btn"
                  :disabled="!form.linkUrl"
                  @click="form.linkUrl = ''"
                >
                  清空
                </el-button>
                <el-button
                  type="primary"
                  plain
                  class="pm-item-dialog-link-open-btn"
                  :disabled="!formOpenableLink"
                  @click="openFormLink"
                >
                  打开
                </el-button>
              </div>
            </div>
          </div>
      </div>

      <!-- 描述 -->
      <div class="pm-item-section">
        <el-form-item label="描述" class="pm-form-item-top">
          <RichDescriptionEditor
            v-if="dataDirReady"
            ref="editorRef"
            v-model="form.description"
            owner-type="pm_item"
            :owner-id="editingItem?.id ?? null"
            placeholder="工作项描述（支持粘贴图片）"
          />
          <div v-else class="pm-item-editor-placeholder">加载编辑器...</div>
        </el-form-item>
      </div>

      <!-- 思源关联 -->
      <div class="pm-item-card pm-siyuan-link-card">
        <div class="pm-siyuan-link-header">
          <div class="pm-siyuan-link-heading">
            <div class="pm-siyuan-link-title">思源关联</div>
            <div class="pm-siyuan-link-subtitle">{{ siyuanLocationSummary }}</div>
          </div>
          <div class="pm-siyuan-inline-actions">
            <el-button size="small" type="primary" plain @click="$emit('open-siyuan-link-picker')">关联页面</el-button>
          </div>
        </div>
        <div v-if="linkedPages.length > 0" class="pm-siyuan-page-list">
          <div v-for="row in linkedPages" :key="row.page.docId" class="pm-siyuan-page-row">
            <div class="pm-siyuan-page-main">
              <div class="pm-siyuan-page-row-head">
                <div class="pm-siyuan-page-title">{{ row.page.docTitle }}</div>
                <el-tag v-if="row.kind === 'primary'" size="small" effect="plain" type="primary">主页面</el-tag>
              </div>
              <div class="pm-siyuan-page-meta">{{ row.page.notebookName }} · {{ row.page.docHpath }}</div>
            </div>
            <div class="pm-siyuan-page-actions">
              <el-button size="small" link @click="$emit('open-siyuan-page', row.page)">打开</el-button>
              <el-dropdown trigger="click" @command="(command) => $emit('siyuan-page-command', row, command)">
                <el-button size="small" link class="pm-siyuan-more-trigger">更多</el-button>
                <template #dropdown>
                  <el-dropdown-menu>
                    <el-dropdown-item v-if="row.kind === 'primary'" command="replace-primary">更换主页面</el-dropdown-item>
                    <el-dropdown-item v-else command="promote-primary">设为主页面</el-dropdown-item>
                    <el-dropdown-item command="remove">移除</el-dropdown-item>
                  </el-dropdown-menu>
                </template>
              </el-dropdown>
            </div>
          </div>
        </div>
        <div v-else class="pm-siyuan-empty-inline">尚未关联页面，点击右上角开始关联。</div>
      </div>

      <!-- 流转记录（仅编辑模式） -->
      <div v-if="editingItem" class="pm-item-card">
        <div class="pm-item-section-title">流转记录</div>
        <div class="pm-item-dialog-core-grid">
          <el-form-item label="开始执行" class="pm-item-dialog-inline-field">
            <el-date-picker v-model="form.startedAt" type="datetime" clearable placeholder="自动记录，可手动修改" style="width: 100%" />
          </el-form-item>
          <el-form-item label="开始测试" class="pm-item-dialog-inline-field">
            <el-date-picker v-model="form.testingAt" type="datetime" clearable placeholder="自动记录，可手动修改" style="width: 100%" />
          </el-form-item>
          <el-form-item label="完成时间" class="pm-item-dialog-inline-field">
            <el-date-picker v-model="form.completedAt" type="datetime" clearable placeholder="自动记录，可手动修改" style="width: 100%" />
          </el-form-item>
        </div>
      </div>

      <!-- 执行任务 - 编辑模式 -->
      <slot name="todo-edit-mode" />

      <!-- 执行任务 - 新建模式 -->
      <slot name="todo-create-mode" />
    </el-form>
    <template #footer>
      <el-button @click="handleClose">取消</el-button>
      <el-button type="primary" @click="handleSubmit">确定</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from "vue";
import type { PmProject, PmItem, PmItemType, PmPriority, PmItemStatus, PmSiyuanPageRef, PmSiyuanLocation } from "../types/pm";
import { PM_STATUS_COLUMNS, PM_ITEM_TYPE_MAP, PM_PRIORITY_MAP } from "../types/pm";
import { getPmDateRangeValue, normalizePmDateRangeForDraft } from "../utils/pmDate";
import { formatPmSiyuanLocationLabel, resolvePmSiyuanEffectiveLocation } from "../utils/pmSiyuan";
import RichDescriptionEditor from "./RichDescriptionEditor.vue";
import { ensureDataDir } from "../rich/data-dir";
import {
  useRichDescriptionLifecycle,
  type RichEditorExposed,
} from "../composables/useRichDescriptionLifecycle";

interface ItemSiyuanLinkedRow {
  page: PmSiyuanPageRef;
  kind: "primary" | "extra";
}

interface Props {
  visible: boolean;
  editingItem: PmItem | null;
  projects: PmProject[];
  selectedProjectId: number | "overview" | null;
  isOverview: boolean;
  formProjectId: number | null;
  primaryPage: PmSiyuanPageRef | null;
  extraPages: PmSiyuanPageRef[];
  globalSiyuanLocation: PmSiyuanLocation | null;
  siyuanConfigReady: boolean;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  "update:visible": [value: boolean];
  "update:formProjectId": [value: number | null];
  "submit": [form: typeof form.value];
  "open-siyuan-link-picker": [];
  "open-siyuan-page": [page: PmSiyuanPageRef];
  "siyuan-page-command": [row: ItemSiyuanLinkedRow, command: string | number | object];
  "open-item-link": [url: string];
}>();

// ── State ────────────────────────────────────────────────

const form = ref({
  title: "",
  refCode: "" as string,
  itemType: "task" as PmItemType,
  priority: "P2" as PmPriority,
  status: "todo" as PmItemStatus,
  startAt: null as string | null,
  endAt: null as string | null,
  linkUrl: "",
  description: "",
  startedAt: null as string | null,
  testingAt: null as string | null,
  completedAt: null as string | null,
});

const projectSwitchMenuVisible = ref(false);

// ── Computed ─────────────────────────────────────────────

const activeProjects = computed(() => props.projects.filter((p) => p.status === "active"));

const dialogProjectId = computed<number | null>(() => {
  if (props.formProjectId !== null) {
    return props.formProjectId;
  }
  if (props.editingItem) {
    return props.editingItem.projectId;
  }
  return typeof props.selectedProjectId === "number" ? props.selectedProjectId : null;
});

const projectOptions = computed(() => {
  const active = [...activeProjects.value];
  const currentProjectId = props.formProjectId ?? props.editingItem?.projectId ?? null;
  if (currentProjectId === null || active.some((project) => project.id === currentProjectId)) {
    return active;
  }

  const currentProject = props.projects.find((project) => project.id === currentProjectId);
  return currentProject ? [...active, currentProject] : active;
});

const projectSwitcherEnabled = computed(
  () => (props.isOverview || Boolean(props.editingItem)) && projectOptions.value.length > 1,
);

const dialogProject = computed(() =>
  props.projects.find((project) => project.id === dialogProjectId.value) ?? null,
);

const projectDisplayName = computed(() => {
  if (dialogProject.value) {
    return dialogProject.value.name;
  }
  if (typeof props.selectedProjectId === "number") {
    const selectedProject = props.projects.find((p) => p.id === props.selectedProjectId);
    return selectedProject?.name ?? "未选择项目";
  }
  return "未选择项目";
});

const projectDisplayColor = computed(() => {
  if (dialogProject.value?.color) {
    return dialogProject.value.color;
  }
  if (typeof props.selectedProjectId === "number") {
    const selectedProject = props.projects.find((p) => p.id === props.selectedProjectId);
    return selectedProject?.color ?? "#4d7df2";
  }
  return "#4d7df2";
});

const effectiveLocation = computed(() =>
  resolvePmSiyuanEffectiveLocation(dialogProject.value?.siyuanLocationOverride, props.globalSiyuanLocation),
);

const effectiveLocationSource = computed(() => {
  if (dialogProject.value?.siyuanLocationOverride) {
    return "项目专属位置";
  }
  if (props.globalSiyuanLocation) {
    return "全局默认位置";
  }
  return "未配置";
});

const siyuanLocationSummary = computed(() => {
  if (!effectiveLocation.value) {
    return "未配置默认位置";
  }
  return `${effectiveLocationSource.value} · ${formatPmSiyuanLocationLabel(effectiveLocation.value)}`;
});

const linkedPages = computed<ItemSiyuanLinkedRow[]>(() => [
  ...(props.primaryPage ? [{ page: props.primaryPage, kind: "primary" as const }] : []),
  ...props.extraPages.map((page) => ({ page, kind: "extra" as const })),
]);

const formDateRange = computed<[string, string] | null>({
  get() {
    return getPmDateRangeValue(form.value.startAt, form.value.endAt);
  },
  set(value) {
    if (!value || value.length < 2) {
      form.value.startAt = null;
      form.value.endAt = null;
      return;
    }
    const normalizedRange = normalizePmDateRangeForDraft(value[0], value[1]);
    form.value.startAt = normalizedRange.startAt;
    form.value.endAt = normalizedRange.endAt;
  },
});

const formOpenableLink = computed(() => Boolean(normalizeItemLinkUrl(form.value.linkUrl)));

// ── Methods ──────────────────────────────────────────────

function normalizeItemLinkUrl(value: string | null | undefined): string {
  let url = (value ?? "").trim();
  if (!url) return "";
  if (/^https?:\/\//i.test(url)) {
    // Already has http/https scheme, keep as-is
  } else if (url.includes("://")) {
    // Non-http scheme (e.g. ftp://), reject like backend does
    return "";
  } else {
    url = `http://${url}`;
  }
  return url;
}

function openFormLink() {
  emit("open-item-link", form.value.linkUrl);
}

function handleProjectSwitchVisibleChange(visible: boolean) {
  projectSwitchMenuVisible.value = visible;
}

function handleProjectSwitchCommand(command: string | number) {
  const nextProjectId = Number(command);
  if (Number.isNaN(nextProjectId)) {
    return;
  }
  emit("update:formProjectId", nextProjectId);
}

function handleClose() {
  emit("update:visible", false);
}

function handleSubmit() {
  emit("submit", form.value);
}

// ── Watch ────────────────────────────────────────────────

watch(() => props.editingItem, (item) => {
  if (item) {
    const normalizedDateRange = normalizePmDateRangeForDraft(item.startAt, item.endAt);
    form.value = {
      title: item.title,
      refCode: item.refCode ?? "",
      itemType: item.itemType,
      priority: item.priority,
      status: item.status,
      startAt: normalizedDateRange.startAt,
      endAt: normalizedDateRange.endAt,
      linkUrl: item.linkUrl ?? "",
      description: item.description,
      startedAt: item.startedAt ?? null,
      testingAt: item.testingAt ?? null,
      completedAt: item.completedAt ?? null,
    };
  } else {
    form.value = {
      title: "",
      refCode: "",
      itemType: "task",
      priority: "P2",
      status: "todo",
      startAt: null,
      endAt: null,
      linkUrl: "",
      description: "",
      startedAt: null,
      testingAt: null,
      completedAt: null,
    };
  }
}, { immediate: true });

// ── Rich description lifecycle ────────────────────────────

const editorRef = ref<RichEditorExposed | null>(null);
const dataDirReady = ref(false);
void ensureDataDir().then(() => { dataDirReady.value = true; });
const submittedThisRound = ref(false);
const richLifecycle = useRichDescriptionLifecycle({
  ownerType: "pm_item",
  editorRef,
  getRealId: () => props.editingItem?.id ?? null,
});

watch(
  () => props.visible,
  async (v, prev) => {
    if (v && !prev) {
      // 每次打开对话框：预热 dataDir 并重置 submit 标记
      void ensureDataDir();
      submittedThisRound.value = false;
      // 新建模式下 editingItem 始终为 null，watch 不会触发，需要在此重置表单
      if (!props.editingItem) {
        form.value = {
          title: "",
          itemType: "task",
          priority: "P2",
          status: "todo",
          startAt: null,
          endAt: null,
          linkUrl: "",
          description: "",
          startedAt: null,
          testingAt: null,
          completedAt: null,
        };
      }
      queueMicrotask(() => {
        const descCurrent = form.value.description ?? "";
        (editorRef.value as any)?.reset?.(descCurrent);
      });
    }
    if (!v && prev && !submittedThisRound.value) {
      // 用户取消/关闭：清理新建场景残留的 tmp 附件（编辑场景 onCancel 会因非 tmp 跳过）
      try {
        await richLifecycle.onCancel();
      } catch (e) {
        console.warn("PmItemDialog cleanup cancel failed:", e);
      }
    }
  }
);

defineExpose({
  form,
  /** 新建场景：调用方拿到 realId 后调用，将 tmp owner 改写为 realId */
  async runAfterSubmit(realId: number) {
    submittedThisRound.value = true;
    await richLifecycle.afterSubmit(realId);
  },
  /** 编辑场景：调用 update 之前先按当前 doc 的 attIds 清理用户删除的附件 */
  async runBeforeSubmit() {
    submittedThisRound.value = true;
    await richLifecycle.beforeCloseEdit();
  },
});
</script>

<style scoped>
/* Item dialog styles */
.pm-item-dialog-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.pm-item-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-bottom: 4px;
}

.pm-item-card {
  border: 1px solid rgba(77, 125, 242, 0.12);
  border-radius: 8px;
  padding: 12px;
  background: rgba(77, 125, 242, 0.04);
}

.pm-item-section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  margin-bottom: 4px;
}

.pm-item-section-subtitle {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  margin-bottom: 8px;
}

.pm-item-section-eyebrow {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 0;
  flex-shrink: 0;
}

.pm-item-project-card {
  border-color: rgba(77, 125, 242, 0.2);
  transition: border-color 0.2s;
}

.pm-item-project-card--switchable {
  cursor: pointer;
}

.pm-item-project-card--switchable:hover {
  border-color: var(--el-color-primary-light-5);
}

.pm-item-project-card--switch-open {
  border-color: var(--el-color-primary);
}

.pm-item-project-card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.pm-item-project-card-body {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 12px;
}

.pm-item-project-card-main {
  display: flex;
  align-items: center;
  gap: 8px;
}

.pm-item-project-card-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
}

.pm-item-project-card-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.pm-item-project-switch-trigger {
  padding: 6px 12px;
  border: 1px solid var(--el-border-color);
  border-radius: 6px;
  background: var(--el-bg-color);
  color: var(--el-text-color-primary);
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s;
}

.pm-item-project-switch-trigger:hover {
  border-color: var(--el-color-primary);
  color: var(--el-color-primary);
}

.pm-item-project-switch-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.pm-item-project-switch-item-name {
  flex: 1;
}

.pm-item-project-switch-item-meta {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.pm-item-dialog-core-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 12px;
}

.pm-item-dialog-inline-field {
  margin-bottom: 0;
}

.pm-item-dialog-inline-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.pm-item-dialog-inline-row--link {
  align-items: flex-start;
}

.pm-item-dialog-inline-label {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  min-width: 80px;
  flex-shrink: 0;
}

.pm-item-dialog-inline-content {
  flex: 1;
  min-width: 0;
}

.pm-item-dialog-inline-content--link {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.pm-item-dialog-link-main {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.pm-item-dialog-inline-form-item {
  margin-bottom: 0;
}

.pm-item-dialog-inline-form-item--grow {
  flex: 1;
  min-width: 0;
}

.pm-item-dialog-link-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.pm-item-dialog-inline-picker {
  width: 100%;
}

.priority-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 6px;
  vertical-align: middle;
  flex-shrink: 0;
}

/* Siyuan styles */
.pm-siyuan-link-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
}

.pm-siyuan-link-heading {
  flex: 1;
  min-width: 0;
}

.pm-siyuan-link-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  margin-bottom: 4px;
}

.pm-siyuan-link-subtitle {
  font-size: 13px;
  color: var(--el-text-color-secondary);
}

.pm-siyuan-inline-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.pm-siyuan-page-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.pm-siyuan-page-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  background: var(--el-bg-color);
}

.pm-siyuan-page-main {
  flex: 1;
  min-width: 0;
}

.pm-siyuan-page-row-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.pm-siyuan-page-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--el-text-color-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-siyuan-page-meta {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-siyuan-page-actions {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
}

.pm-siyuan-empty-inline {
  padding: 16px;
  text-align: center;
  font-size: 13px;
  color: var(--el-text-color-secondary);
  border: 1px dashed var(--el-border-color);
  border-radius: 6px;
  background: var(--el-bg-color);
}

.pm-form-item-top :deep(.el-form-item__label) {
  margin-bottom: 8px;
}

.pm-item-editor-placeholder {
  width: 100%;
  min-height: 180px;
  border: 1px solid var(--el-border-color);
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--el-text-color-secondary);
  font-size: 13px;
  background: var(--el-fill-color-lighter);
}
</style>
