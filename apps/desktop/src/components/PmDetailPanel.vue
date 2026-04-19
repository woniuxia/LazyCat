<template>
  <Transition name="pm-detail-slide">
    <aside v-if="item" class="pm-detail">
      <div class="detail-header">
        <div>
          <div class="detail-header-eyebrow">工作项详情</div>
          <span class="detail-title">详情</span>
        </div>
        <el-button size="small" link @click="emit('close')">
          <el-icon><Close /></el-icon>
        </el-button>
      </div>
      <div class="detail-form">
        <div class="detail-hero">
          <div class="detail-hero-head">
            <div class="detail-project-chip">
              <span class="detail-project-dot" :style="{ backgroundColor: project?.color ?? item.projectColor ?? '#4d7df2' }" />
              <span class="detail-project-name">{{ project?.name ?? item.projectName ?? "-" }}</span>
              <el-tag v-if="project?.status === 'archived'" size="small" effect="plain" class="detail-project-archived-tag">
                已归档
              </el-tag>
            </div>
            <el-tag v-if="isOverdue(item)" size="small" effect="light" type="danger">已逾期</el-tag>
          </div>
          <div class="detail-item-title">{{ item.title }}</div>
          <div class="detail-field-inline">
            <el-tag
              size="small"
              effect="light"
              round
              :style="getPmLightTagStyle(PM_ITEM_TYPE_MAP[item.itemType]?.color)"
            >
              {{ PM_ITEM_TYPE_MAP[item.itemType]?.label ?? item.itemType }}
            </el-tag>
            <el-tag size="small" effect="light" round>
              <span class="priority-dot" :style="{ backgroundColor: PM_PRIORITY_MAP[item.priority]?.color }" />
              {{ PM_PRIORITY_MAP[item.priority]?.label ?? item.priority }}
            </el-tag>
            <el-tag size="small" :type="item.status === 'done' ? 'success' : item.status === 'in_progress' ? 'primary' : item.status === 'testing' ? 'warning' : 'info'" effect="light" round>
              {{ PM_STATUS_COLUMNS.find(c => c.key === item.status)?.label ?? item.status }}
            </el-tag>
            <el-tag v-for="tag in item.tags" :key="tag" size="small" type="info">{{ tag }}</el-tag>
          </div>
        </div>

        <div class="detail-section">
          <div class="detail-section-head">
            <span class="detail-section-title">时间轨迹</span>
            <span class="detail-section-subtitle">按状态推进记录</span>
          </div>
          <div class="detail-timeline-grid">
            <div class="detail-timeline-card">
              <span class="detail-label">时间安排</span>
              <span class="detail-value" :class="{ 'is-overdue-date': isOverdue(item) }">
                {{ formatPmDateRangeForDisplay(item.startAt, item.endAt) }}
              </span>
            </div>
            <div class="detail-timeline-card">
              <span class="detail-label">创建时间</span>
              <span class="detail-value">{{ formatDateTime(item.createdAt) }}</span>
            </div>
            <div class="detail-timeline-card">
              <span class="detail-label">开始执行</span>
              <span class="detail-value">{{ item.startedAt ? formatDateTime(item.startedAt) : "-" }}</span>
            </div>
            <div class="detail-timeline-card">
              <span class="detail-label">开始测试</span>
              <span class="detail-value">{{ item.testingAt ? formatDateTime(item.testingAt) : "-" }}</span>
            </div>
            <div v-if="item.completedAt" class="detail-timeline-card">
              <span class="detail-label">完成时间</span>
              <span class="detail-value">{{ formatDateTime(item.completedAt) }}</span>
            </div>
          </div>
        </div>

        <div v-if="descriptionText" class="detail-section">
          <div class="detail-section-head">
            <span class="detail-section-title">描述</span>
            <span class="detail-section-subtitle">保留换行</span>
          </div>
          <pre class="detail-value detail-description">{{ descriptionText }}</pre>
        </div>

        <!-- 执行任务区块 -->
        <div class="detail-section">
          <div class="detail-section-head">
            <span class="detail-section-title">执行任务</span>
          </div>
          <InlineTodoList
            :pm-item-id="() => item?.id"
            :items="pmTodo.items"
            :summary="pmTodo.summary"
            :loading="pmTodo.loading"
            mode="edit"
            :candidates="pmTodo.candidates"
            :candidates-loading="pmTodo.candidateLoading"
            @create="pmTodo.quickCreate"
            @toggle="pmTodo.toggleCompleteById"
            @unlink="pmTodo.unlink"
            @link="pmTodo.linkBatch"
            @search-candidates="pmTodo.searchCandidates"
          />
        </div>

        <div class="detail-section">
          <div class="detail-section-head">
            <span class="detail-section-title">资源关联</span>
            <span class="detail-section-subtitle">链接与思源页面</span>
          </div>
          <div class="detail-resource-list">
            <div class="detail-resource-card">
              <div class="detail-resource-main">
                <span class="detail-label">链接</span>
                <span class="detail-value detail-link-text">{{ item.linkUrl || "-" }}</span>
              </div>
              <el-button v-if="item.linkUrl" size="small" link @click="openItemLink(item.linkUrl)">
                打开
              </el-button>
            </div>
            <div class="detail-resource-card">
              <template v-if="item.siyuanPrimaryPage">
                <div class="detail-siyuan-page-main">
                  <span class="detail-label">思源主页面</span>
                  <span class="detail-siyuan-page-title">{{ item.siyuanPrimaryPage.docTitle }}</span>
                  <span class="detail-siyuan-page-meta">
                    {{ item.siyuanPrimaryPage.notebookName }} · {{ item.siyuanPrimaryPage.docHpath }}
                  </span>
                </div>
                <el-button size="small" link @click="siyuan.openSiyuanPage(item.siyuanPrimaryPage)">打开</el-button>
              </template>
              <template v-else>
                <div class="detail-resource-main">
                  <span class="detail-label">思源主页面</span>
                  <span class="detail-value">-</span>
                </div>
              </template>
            </div>
            <div v-for="page in item.siyuanExtraPages" :key="page.docId" class="detail-resource-card">
              <div class="detail-siyuan-page-main">
                <span class="detail-label">附加页面</span>
                <span class="detail-siyuan-page-title">{{ page.docTitle }}</span>
                <span class="detail-siyuan-page-meta">{{ page.notebookName }} · {{ page.docHpath }}</span>
              </div>
              <el-button size="small" link @click="siyuan.openSiyuanPage(page)">打开</el-button>
            </div>
          </div>
        </div>

        <div v-if="!descriptionText" class="detail-section detail-section--muted">
          <div class="detail-section-head">
            <span class="detail-section-title">描述</span>
            <span class="detail-section-subtitle">暂无内容</span>
          </div>
          <div class="detail-empty-text">暂无描述</div>
        </div>

        <div class="detail-actions">
          <el-button size="small" @click="emit('toggle-pin', item)">{{ item.pinned ? '取消置顶' : '置顶' }}</el-button>
          <el-button v-if="item.status !== 'done'" size="small" type="primary" plain @click="emit('advance-status', item)">
            推进状态
          </el-button>
          <el-button size="small" type="danger" plain @click="emit('delete', item)">删除</el-button>
        </div>
      </div>
    </aside>
  </Transition>
</template>

<script setup lang="ts">
import { computed, reactive, watch, inject } from "vue";
import { ElMessage } from "element-plus";
import { Close } from "@element-plus/icons-vue";
import { useToolInvoke } from "../composables/useToolInvoke";
import type { PmProject, PmItem } from "../types/pm";
import { PM_STATUS_COLUMNS, PM_ITEM_TYPE_MAP, PM_PRIORITY_MAP } from "../types/pm";
import { isPmItemOverdue } from "../utils/pmDate";
import { formatPmDateRangeForDisplay } from "../utils/pmDate";
import InlineTodoList from "./InlineTodoList.vue";
import { usePmTodoLinking } from "../composables/usePmTodoLinking";
import { PM_SIYUAN_KEY } from "../composables/pmSiyuanKey";

const props = defineProps<{
  project: PmProject | null;
  item: PmItem | null;
}>();

const emit = defineEmits<{
  close: [];
  "toggle-pin": [item: PmItem];
  "advance-status": [item: PmItem];
  delete: [item: PmItem];
}>();

const { invoke } = useToolInvoke();
const siyuan = inject(PM_SIYUAN_KEY)!;

const pmTodo = reactive(usePmTodoLinking(() => props.item?.id));

const descriptionText = computed(() => props.item?.description?.trim() ?? "");

watch(() => props.item?.id, (id) => {
  if (id != null) {
    pmTodo.loadItems(id);
  } else {
    pmTodo.reset();
  }
});

function isOverdue(item: PmItem): boolean {
  return isPmItemOverdue(item);
}

function nextStatusLabel(item: PmItem): string {
  const idx = PM_STATUS_COLUMNS.findIndex((c) => c.key === item.status);
  return idx >= 0 && idx < PM_STATUS_COLUMNS.length - 1 ? PM_STATUS_COLUMNS[idx + 1].label : "";
}

function getPmLightTagStyle(color?: string | null) {
  const resolvedColor = color ?? "#409eff";
  return {
    "--el-tag-bg-color": `${resolvedColor}14`,
    "--el-tag-border-color": `${resolvedColor}33`,
    "--el-tag-text-color": resolvedColor,
  };
}

function formatDateTime(dateStr: string): string {
  if (!dateStr) return "";
  const d = new Date(dateStr);
  return d.toLocaleString("zh-CN");
}

function normalizeItemLinkUrl(value: string | null | undefined): string {
  let url = (value ?? "").trim();
  if (!url) return "";
  if (/^https?:\/\//i.test(url)) {
    // Already has http/https scheme, keep as-is
  } else if (url.includes("://")) {
    return "";
  } else {
    url = `http://${url}`;
  }
  return url;
}

async function openItemLink(url: string | null | undefined) {
  const normalized = normalizeItemLinkUrl(url);
  if (!normalized) return;
  try {
    await invoke("tool:pm:open-link", { url: normalized });
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}
</script>

<style scoped>
/* Detail panel (floating overlay) */
.pm-detail {
  position: absolute;
  top: 0;
  right: 0;
  width: 320px;
  height: 100%;
  border-left: 1px solid var(--el-border-color-lighter);
  padding: 12px;
  overflow-y: auto;
  background: var(--el-bg-color);
  box-shadow: -4px 0 12px rgba(0, 0, 0, 0.08);
  z-index: 10;
}
.detail-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}
.detail-title {
  font-weight: 600;
  font-size: 16px;
}
.detail-form {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.detail-section {
  background: var(--el-fill-color-lighter);
  border-radius: 8px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.detail-field-inline {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}
.detail-item-title {
  font-size: 16px;
  font-weight: 600;
  line-height: 1.4;
}
.detail-field {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.detail-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  font-weight: 500;
}
.detail-value {
  font-size: 14px;
  color: var(--el-text-color-primary);
  word-break: break-word;
}
.detail-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.detail-link-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
}
.detail-link-text {
  flex: 1;
  min-width: 0;
  word-break: break-all;
}
.detail-description {
  margin: 0;
  font-family: inherit;
  white-space: pre-wrap;
  background: var(--el-fill-color-lighter);
  border-radius: 4px;
  padding: 6px 8px;
  font-size: 13px;
  color: var(--el-text-color-regular);
}
.detail-readonly {
  font-size: 14px;
  color: var(--el-text-color-secondary);
}
.detail-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 12px;
}
/* Detail panel transition */
.pm-detail-slide-enter-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}
.pm-detail-slide-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}
.pm-detail-slide-enter-from,
.pm-detail-slide-leave-to {
  opacity: 0;
  transform: translateX(20px);
}

/* PM visual unification */
.pm-detail {
  width: 364px;
  padding: 16px;
  border-left: 1px solid var(--pm-edge);
  background: linear-gradient(180deg, rgba(249, 251, 255, 0.97), rgba(242, 246, 252, 0.98));
  box-shadow: -16px 0 30px rgba(34, 48, 66, 0.08);
}

.detail-header {
  margin-bottom: 14px;
  padding-bottom: 12px;
  border-bottom: 1px solid rgba(219, 229, 241, 0.9);
}

.detail-header-eyebrow {
  margin-bottom: 4px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.08em;
  color: var(--pm-text-muted);
}

.detail-title {
  font-size: 18px;
  font-weight: 700;
  color: var(--pm-text-main);
}

.detail-form {
  min-height: calc(100% - 58px);
  gap: 12px;
}

.detail-hero,
.detail-section {
  padding: 14px 16px;
  border: 1px solid var(--pm-edge-soft);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.92);
  box-shadow: 0 10px 24px rgba(34, 48, 66, 0.05);
}

.detail-hero {
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(244, 247, 251, 0.9));
}

.detail-hero-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  flex-wrap: wrap;
}

.detail-project-chip {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  max-width: 100%;
  padding: 6px 10px;
  border: 1px solid rgba(77, 125, 242, 0.14);
  border-radius: 999px;
  background: rgba(77, 125, 242, 0.08);
}

.detail-project-dot {
  width: 10px;
  height: 10px;
  border-radius: 999px;
  box-shadow: 0 0 0 3px rgba(77, 125, 242, 0.12);
  flex-shrink: 0;
}

.detail-project-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  font-weight: 600;
  color: var(--pm-text-main);
}

.detail-item-title {
  font-size: 18px;
  font-weight: 700;
  color: var(--pm-text-main);
}

.detail-field-inline {
  gap: 8px;
}

.detail-field-inline :deep(.el-tag) {
  border-radius: 999px;
}

.detail-label {
  color: var(--pm-text-muted);
}

.detail-value {
  color: var(--pm-text-main);
}

.detail-section--muted {
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.84), rgba(244, 247, 251, 0.82));
}

.detail-section-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.detail-section-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--pm-text-main);
}

.detail-section-subtitle {
  font-size: 12px;
  color: var(--pm-text-muted);
}

.detail-timeline-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.detail-timeline-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 76px;
  padding: 12px;
  border: 1px solid rgba(219, 229, 241, 0.95);
  border-radius: 12px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(244, 247, 251, 0.84));
}

.detail-resource-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.detail-resource-card {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  padding: 12px;
  border: 1px solid var(--pm-edge-soft);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.98);
  box-shadow: 0 2px 6px rgba(34, 48, 66, 0.03);
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.detail-resource-card:hover {
  border-color: rgba(77, 125, 242, 0.22);
  box-shadow: 0 6px 14px rgba(77, 125, 242, 0.06);
}

.detail-resource-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.detail-link-text {
  line-height: 1.55;
}

.detail-description {
  padding: 10px 12px;
  border: 1px solid var(--pm-edge-soft);
  border-radius: 12px;
  background: rgba(244, 247, 251, 0.88);
  line-height: 1.7;
  color: var(--pm-text-main);
}

.detail-empty-text {
  color: var(--pm-text-muted);
  font-size: 13px;
  line-height: 1.6;
}

.detail-actions {
  position: sticky;
  bottom: -16px;
  z-index: 1;
  gap: 8px;
  margin-top: auto;
  padding: 14px 0 2px;
  background: linear-gradient(180deg, rgba(242, 246, 252, 0), rgba(242, 246, 252, 0.94) 32%, rgba(242, 246, 252, 1));
}

.detail-actions :deep(.el-button) {
  flex: 1;
}

@media (max-width: 1280px) {
  .pm-detail {
    width: 350px;
  }
}
</style>

<style>
/* Detail panel siyuan page styles (unscoped for element-plus penetration) */
.detail-siyuan-pages {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.detail-siyuan-page {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  padding: 10px 12px;
  border: 1px solid var(--el-border-color-extra-light);
  border-radius: 10px;
  background: var(--el-fill-color-blank);
}

.detail-siyuan-page-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
</style>
