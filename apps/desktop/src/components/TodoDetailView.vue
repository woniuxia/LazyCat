<template>
  <div class="detail-view" :key="item.id">
    <!-- Header -->
    <div class="detail-pane-header detail-pane-header--view">
      <div class="detail-title-group">
        <div class="detail-eyebrow">事项详情</div>
        <div class="detail-title-row">
          <h3 class="detail-title detail-title--copyable" title="点击复制标题" @click="$emit('copyTitle', item.title)">{{ item.title }}</h3>
          <div class="detail-badges">
            <span class="detail-badge pinned" v-if="item.pinned">
              <el-icon :size="12"><Top /></el-icon> 置顶
            </span>
            <span class="detail-badge repeat" v-if="hasRepeatRule(item)">
              <el-icon :size="12"><Refresh /></el-icon> 重复
            </span>
            <span class="detail-badge overdue" v-if="isItemOverdue(item)">
              <el-icon :size="12"><AlarmClock /></el-icon> 逾期
            </span>
          </div>
        </div>
      </div>
      <div class="detail-header-actions">
        <el-button
          size="small"
          link
          class="detail-edit-btn"
          @click="$emit('edit', item)"
        >编辑</el-button>
        <el-button
          v-if="canPinItem(item)"
          size="small"
          link
          @click="$emit('togglePin', item.id)"
        >
          {{ item.pinned ? "取消置顶" : "置顶" }}
        </el-button>
        <el-button
          size="small"
          link
          :type="isDoneItem(item) ? '' : 'success'"
          @click="$emit('changeStatus', item.id, isDoneItem(item) ? 'pending' : 'completed')"
        >
          {{ isDoneItem(item) ? "恢复" : "完成" }}
        </el-button>
        <el-button size="small" link type="danger" @click="$emit('delete', item)"
        >删除</el-button>
      </div>
    </div>

    <!-- Content -->
    <div class="detail-scroll">
      <div class="detail-content">
        <div v-if="!hasDetailCards" class="detail-card detail-card--empty">
          <div class="detail-card-header">
            <div class="detail-card-icon primary">
              <el-icon><Document /></el-icon>
            </div>
            <span class="detail-card-title">暂无可展示详情</span>
          </div>
          <div class="detail-card-body">
            <div class="detail-empty-info">
              <div class="detail-empty-info-text">当前事项还没有补充任何可展示信息。</div>
              <div class="detail-empty-info-hint">
                你可以在编辑中添加：到期时间、提醒、分类、执行人或详细描述。
              </div>
              <div class="detail-empty-info-actions">
                <el-button size="small" type="primary" @click="$emit('edit', item)">
                  去完善
                </el-button>
              </div>
            </div>
          </div>
        </div>

        <!-- Status Card -->
        <div
          v-if="item.status !== 'pending' || item.priority !== 'P2'"
          class="detail-card"
        >
          <div class="detail-card-header">
            <div class="detail-card-icon" :class="priorityCardClass(item.priority)">
              <el-icon><Flag /></el-icon>
            </div>
            <span class="detail-card-title">状态与优先级</span>
          </div>
          <div class="detail-card-body">
            <div class="detail-grid">
              <div v-if="item.status !== 'pending'" class="detail-field">
                <div class="detail-label">当前状态</div>
                <div class="detail-value">
                  <el-tag size="small" effect="plain" round>
                    {{ formatStatusLabel(item.status) }}
                  </el-tag>
                </div>
              </div>
              <div v-if="item.priority !== 'P2'" class="detail-field">
                <div class="detail-label">优先级</div>
                <div class="detail-value">
                  <span class="priority-with-dot">
                    <span
                      class="priority-dot"
                      :class="'priority-' + item.priority.toLowerCase()"
                    />
                    {{ item.priority }} - {{ priorityLabel(item.priority) }}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Schedule Card -->
        <div
          v-if="
            item.eventAt ||
            effectiveReminderPresets(item.reminderPresets).length > 0
          "
          class="detail-card"
        >
          <div class="detail-card-header">
            <div class="detail-card-icon primary">
              <el-icon><Calendar /></el-icon>
            </div>
            <span class="detail-card-title">时间安排</span>
          </div>
          <div class="detail-card-body">
            <div class="detail-grid" :class="{ 'is-stacked': !item.eventAt }">
              <div v-if="item.eventAt" class="detail-field">
                <div class="detail-label">
                  <el-icon :size="12"><Clock /></el-icon> 任务时间
                </div>
                <div class="detail-value">
                  {{ formatDate(item.eventAt) }}
                </div>
                <div v-if="relativeTimeLabel(item)" class="detail-hint">
                  {{ relativeTimeLabel(item) }}
                </div>
              </div>
              <div
                v-if="effectiveReminderPresets(item.reminderPresets).length > 0"
                class="detail-field"
              >
                <div class="detail-label">
                  <el-icon :size="12"><Bell /></el-icon> 提醒设置
                </div>
                <div class="detail-value">
                  {{ formatReminderDescription(item) }}
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Info Card -->
        <div
          v-if="item.typeName || item.assignees.length > 0"
          class="detail-card"
        >
          <div class="detail-card-header">
            <div class="detail-card-icon success">
              <el-icon><User /></el-icon>
            </div>
            <span class="detail-card-title">分类与执行人</span>
          </div>
          <div class="detail-card-body">
            <div class="detail-grid">
              <div v-if="item.typeName" class="detail-field">
                <div class="detail-label">分类</div>
                <div class="detail-value">
                  <span class="type-with-color">
                    <span
                      class="color-dot-sm"
                      :style="{ backgroundColor: item.typeColor || '#909399' }"
                    />
                    {{ item.typeName }}
                  </span>
                </div>
              </div>
              <div v-if="item.assignees.length > 0" class="detail-field">
                <div class="detail-label">执行人</div>
                <div class="detail-value">
                  <div class="assignee-list">
                    <span
                      v-for="assignee in item.assignees"
                      :key="assignee.id"
                      class="assignee-tag"
                    >
                      {{ assignee.name }}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Project & PM Item Card -->
        <div
          v-if="item.projectId"
          class="detail-card project-unified-card"
        >
          <div class="detail-card-header">
            <div class="detail-card-icon warning">
              <el-icon><Briefcase /></el-icon>
            </div>
            <span class="detail-card-title">项目归属</span>
            <span class="project-inline">
              <span
                class="project-section-dot"
                :style="{ backgroundColor: item.projectColor || '#909399' }"
              />
              <span class="project-section-name">
                {{ item.projectName || `项目 #${item.projectId}` }}
              </span>
            </span>
          </div>
          <div v-if="item.pmItemId" class="pm-section">
            <div class="pm-section-badge">
              <el-icon><Link /></el-icon>
            </div>
            <span class="pm-section-title">
              {{ item.pmItemTitle || `#${item.pmItemId}` }}
            </span>
            <el-tag
              size="small"
              effect="plain"
              round
              class="pm-section-status"
              :style="{
                backgroundColor: pmStatusColor(item.pmItemStatus) + '15',
                borderColor: pmStatusColor(item.pmItemStatus) + '40',
                color: pmStatusColor(item.pmItemStatus),
              }"
            >
              {{ pmStatusLabel(item.pmItemStatus) }}
            </el-tag>
            <el-button
              class="pm-section-jump"
              size="small"
              link
              type="primary"
              @click="$emit('navigateToPm', item.pmItemId, item.pmItemProjectId)"
            >
              跳转 &rarr;
            </el-button>
          </div>
        </div>

        <!-- Recurrence Card -->
        <div v-if="hasRepeatRule(item)" class="detail-card">
          <div class="detail-card-header">
            <div class="detail-card-icon warning">
              <el-icon><Refresh /></el-icon>
            </div>
            <span class="detail-card-title">重复规则</span>
          </div>
          <div class="detail-card-body">
            <div class="detail-grid is-stacked">
              <div class="detail-field detail-field--full">
                <div class="detail-label">规则描述</div>
                <div class="detail-value">
                  {{ formatRecurrenceDescription(item) }}
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Description Card -->
        <div v-if="item.description" class="detail-card">
          <div class="detail-card-header">
            <div class="detail-card-icon primary">
              <el-icon><Document /></el-icon>
            </div>
            <span class="detail-card-title">详细描述</span>
          </div>
          <div class="detail-card-body">
            <div class="detail-description-card" :key="'desc-' + item.id">
              <div
                class="detail-description md-rendered"
                v-html="renderedDescription"
              ></div>
            </div>
          </div>
        </div>

        <!-- Links Card -->
        <div v-if="item.links?.length" class="detail-card">
          <div class="detail-card-header">
            <div class="detail-card-icon primary">
              <el-icon><Link /></el-icon>
            </div>
            <span class="detail-card-title">关联链接</span>
          </div>
          <div class="detail-card-body">
            <div class="detail-links-list">
              <div
                v-for="link in item.links"
                :key="link.id"
                class="detail-link-item"
                @click="$emit('openLink', link.url)"
              >
                <el-icon class="detail-link-icon"><Link /></el-icon>
                <span class="detail-link-text">{{ link.title || link.url }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Footer -->
    <div class="detail-pane-footer detail-pane-footer--meta">
      <div class="meta-timestamps">
        <span><span class="meta-label">创建：</span>{{ formatDate(item.createdAt) }}</span>
        <span class="meta-divider">·</span>
        <span><span class="meta-label">更新：</span>{{ formatDate(item.updatedAt) }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import {
  AlarmClock,
  Bell,
  Briefcase,
  Calendar,
  Clock,
  Document,
  Flag,
  Link,
  Refresh,
  Top,
  User,
} from "@element-plus/icons-vue";
import { effectiveReminderPresets } from "../composables/useTodoItem";
import type { TodoItem, TodoPriority, TodoStatus } from "../types";
import { formatTodoRelativeDateTimeLabel } from "../utils/todoRelativeDate";
import { renderMarkdown } from "../utils/renderMarkdown";

const props = defineProps<{
  item: TodoItem;
}>();

const emit = defineEmits<{
  edit: [item: TodoItem];
  togglePin: [id: number];
  changeStatus: [id: number, status: string];
  delete: [item: TodoItem];
  copyTitle: [title: string];
  openLink: [url: string];
  navigateToPm: [pmItemId: number, pmProjectId: number | null];
}>();

// --- Formatter helpers ---

function pmStatusColor(status: string | null | undefined): string {
  if (!status) return "#909399";
  const map: Record<string, string> = {
    todo: "#909399",
    in_progress: "#409eff",
    done: "#67c23a",
    cancelled: "#f56c6c",
  };
  return map[status] || "#909399";
}

function pmStatusLabel(status: string | null | undefined): string {
  if (!status) return "未知";
  const map: Record<string, string> = {
    todo: "待办",
    in_progress: "进行中",
    done: "已完成",
    cancelled: "已取消",
  };
  return map[status] || status;
}

function formatStatusLabel(status: TodoStatus | null) {
  if (!status) return "";
  const map: Record<string, string> = {
    pending: "待办",
    in_progress: "进行中",
    completed: "已完成",
  };
  return map[status] || status;
}

function priorityCardClass(priority: TodoPriority): "danger" | "warning" | "primary" | "" {
  if (priority === "P0" || priority === "P1") return "danger";
  if (priority === "P2") return "warning";
  return "";
}

function priorityLabel(priority: TodoPriority): string {
  const map: Record<TodoPriority, string> = { P0: "紧急", P1: "高", P2: "中", P3: "低" };
  return map[priority] || priority;
}

function formatDate(value?: string | null) {
  if (!value) return "-";
  try {
    const date = new Date(value);
    if (isNaN(date.getTime())) return value;
    const y = date.getFullYear();
    const m = String(date.getMonth() + 1).padStart(2, "0");
    const d = String(date.getDate()).padStart(2, "0");
    const h = String(date.getHours()).padStart(2, "0");
    const min = String(date.getMinutes()).padStart(2, "0");
    return `${y}-${m}-${d} ${h}:${min}`;
  } catch {
    return value;
  }
}

function isDoneItem(item: TodoItem) {
  return item.status === "completed";
}

function canPinItem(item: TodoItem) {
  return item.status !== "completed";
}

function hasRepeatRule(item: TodoItem): boolean {
  return item.kind === "recurring" && item.recurrence !== null;
}

function isItemOverdue(item: TodoItem): boolean {
  if (item.status === "completed") return false;
  if (!item.eventAt) return false;
  return new Date(item.eventAt).getTime() < Date.now();
}

function relativeTimeLabel(item: TodoItem): string {
  return formatTodoRelativeDateTimeLabel(item.eventAt);
}

function formatReminderDescription(item: TodoItem) {
  const presets = effectiveReminderPresets(item.reminderPresets);
  if (presets.length === 0) return "";
  const labels: Record<string, string> = {
    "0m": "任务开始时",
    "5m": "提前 5 分钟",
    "10m": "提前 10 分钟",
    "30m": "提前 30 分钟",
    "1h": "提前 1 小时",
    "1d": "提前 1 天",
    "2d": "提前 2 天",
  };
  return presets.map((p) => labels[p] || p).join("、");
}

function formatWeekdayList(days: number[] = []) {
  const names = ["", "周一", "周二", "周三", "周四", "周五", "周六", "周日"];
  return days.map((d) => names[d] || `周${d}`).join("、");
}

function formatRecurrenceDescription(item: TodoItem) {
  if (!item.recurrence) return "";
  const rule = item.recurrence.rule;
  if (item.recurrence.ruleMode === "cron") {
    return `Cron: ${item.recurrence.cronExpression || (typeof rule === "object" && "expression" in rule ? rule.expression : "")}`;
  }
  if ("frequency" in rule) {
    const freqMap: Record<string, string> = { daily: "每天", weekly: "每周", monthly: "每月" };
    let desc = freqMap[rule.frequency] || rule.frequency;
    if (rule.interval > 1) desc += `，间隔 ${rule.interval}`;
    if ("weekdays" in rule && rule.weekdays.length > 0) desc += `，${formatWeekdayList(rule.weekdays)}`;
    if ("dayOfMonth" in rule) desc += `，${rule.dayOfMonth} 号`;
    if (rule.time) desc += `，${rule.time}`;
    return desc;
  }
  return "";
}

// --- Computed ---

const hasDetailCards = computed(() => {
  const item = props.item;
  const hasSchedule = !!item.eventAt || effectiveReminderPresets(item.reminderPresets).length > 0;
  return (
    item.status !== "pending" ||
    item.priority !== "P2" ||
    hasSchedule ||
    !!item.typeName ||
    item.assignees.length > 0 ||
    item.description ||
    (item.links?.length ?? 0) > 0 ||
    hasRepeatRule(item) ||
    !!item.projectId
  );
});

const renderedDescription = computed(() => {
  const desc = props.item.description;
  if (!desc) return "";
  return renderMarkdown(desc);
});
</script>

<style scoped>
/* Detail view layout */
.detail-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  animation: slideInFromRight 0.3s var(--lc-ease-out, ease);
}
.detail-pane-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid var(--lc-border);
  background: linear-gradient(180deg, var(--lc-surface-0), var(--lc-surface-1));
  gap: 12px;
  flex-shrink: 0;
}
.detail-pane-header--view {
  padding-bottom: 12px;
}
.detail-title-group {
  min-width: 0;
  flex: 1;
}
.detail-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.detail-eyebrow {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.8px;
  color: var(--lc-accent);
  margin-bottom: 8px;
}
.detail-title {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  line-height: 1.3;
  color: var(--lc-text);
  word-break: break-word;
  flex: 1;
  min-width: 0;
}
.detail-title--copyable {
  cursor: pointer;
  transition: color 0.15s;
}
.detail-title--copyable:hover {
  color: var(--el-color-primary);
}
.detail-title--copyable:active {
  opacity: 0.7;
}
.detail-badges {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
  flex-wrap: wrap;
}
.detail-header-actions {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
}
.detail-edit-btn {
  font-weight: 500;
}
.detail-edit-btn:hover {
  color: var(--el-color-primary);
}

/* Detail scroll and content */
.detail-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 16px 20px;
}
.detail-content {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* Detail cards */
.detail-card {
  background: var(--lc-surface-0);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md, 8px);
  overflow: hidden;
  transition: box-shadow 0.25s var(--lc-ease, ease);
  animation: cardFadeIn 0.35s var(--lc-ease-out, ease) backwards;
}
.detail-card:nth-child(1) { animation-delay: 0ms; }
.detail-card:nth-child(2) { animation-delay: 40ms; }
.detail-card:nth-child(3) { animation-delay: 80ms; }
.detail-card:nth-child(4) { animation-delay: 120ms; }
.detail-card:nth-child(5) { animation-delay: 160ms; }
.detail-card:hover {
  box-shadow: var(--lc-shadow-sm);
}
.detail-card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  background: var(--lc-surface-1);
  border-bottom: 1px solid var(--lc-border);
}
.detail-card-icon {
  width: 24px;
  height: 24px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--lc-surface-1);
  color: var(--el-text-color-secondary);
  font-size: 13px;
  flex-shrink: 0;
}
.detail-card-icon.primary {
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}
.detail-card-icon.success {
  background: var(--el-color-success-light-9);
  color: var(--el-color-success);
}
.detail-card-icon.warning {
  background: var(--el-color-warning-light-9);
  color: var(--el-color-warning);
}
.detail-card-icon.danger {
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
}
.detail-card-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}
.project-inline {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
  font-size: 12px;
  color: var(--el-text-color-regular);
}
.project-section-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.project-section-name {
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pm-section {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  border-top: 1px solid var(--lc-border);
  background: var(--lc-surface-1);
}
.pm-section-badge {
  color: var(--el-text-color-secondary);
  font-size: 14px;
}
.pm-section-title {
  font-size: 12px;
  color: var(--el-text-color-regular);
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pm-section-status {
  flex-shrink: 0;
}
.pm-section-jump {
  flex-shrink: 0;
}
.detail-card-body {
  padding: 12px 14px;
}
.detail-empty-info {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 16px 0;
  text-align: center;
}
.detail-empty-info-text {
  font-size: 13px;
  color: var(--el-text-color-regular);
}
.detail-empty-info-hint {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}
.detail-empty-info-actions {
  display: flex;
  gap: 8px;
  margin-top: 4px;
}
.detail-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px 16px;
}
.detail-grid.is-stacked {
  grid-template-columns: 1fr;
}
.detail-field {
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.detail-field:hover {
  background: var(--lc-surface-1);
  border-radius: 4px;
  margin: -3px -6px;
  padding: 3px 6px;
}
.detail-field--full {
  grid-column: 1 / -1;
}
.detail-label {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  display: flex;
  align-items: center;
  gap: 4px;
}
.detail-value {
  font-size: 13px;
  color: var(--el-text-color-primary);
}
.detail-hint {
  font-size: 11px;
  color: var(--el-text-color-placeholder);
  margin-top: 2px;
}
.detail-description-card {
  margin-top: 4px;
}
.detail-description {
  font-size: 13px;
  line-height: 1.7;
  color: var(--el-text-color-regular);
}
.detail-description:not(.md-rendered) {
  white-space: pre-wrap;
  word-break: break-word;
}
.detail-badges {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}
.detail-badge {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 2px 8px;
  border-radius: 10px;
  font-size: 11px;
  font-weight: 500;
}
.detail-badge.pinned {
  background: var(--el-color-warning-light-9);
  color: var(--el-color-warning);
}
.detail-badge.repeat {
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}
.detail-badge.overdue {
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
}
.priority-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.priority-p0 { background-color: var(--lc-danger); }
.priority-p1 { background-color: var(--lc-warning); }
.priority-p2 { background-color: var(--lc-accent); }
.priority-p3 { background-color: var(--lc-text-muted); }
.priority-with-dot {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.color-dot-sm {
  display: inline-block;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}
.type-with-color {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.assignee-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.assignee-tag {
  display: inline-block;
  padding: 2px 10px;
  border-radius: 10px;
  font-size: 12px;
  background: var(--lc-surface-1);
  color: var(--el-text-color-regular);
}
.meta-timestamps {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--el-text-color-placeholder);
}
.meta-label {
  color: var(--lc-text-muted);
}
.meta-divider {
  color: var(--lc-border);
}
.detail-pane-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 20px;
  border-top: 1px solid var(--lc-border);
  flex-shrink: 0;
}
.detail-pane-footer--meta {
  justify-content: flex-start;
}
.detail-links-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.detail-link-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.15s;
}
.detail-link-item:hover {
  background: var(--lc-surface-1);
}
.detail-link-icon {
  color: var(--el-text-color-secondary);
  font-size: 14px;
}
.detail-link-text {
  font-size: 13px;
  color: var(--el-color-primary);
  word-break: break-all;
}

/* --- Animations --- */
@keyframes slideInFromRight {
  from {
    opacity: 0;
    transform: translateX(20px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}
@keyframes cardFadeIn {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* --- Markdown rendered styles --- */
.md-rendered :deep(h1) {
  font-size: 1.4em;
  margin: 0.4em 0;
  border-bottom: 1px solid var(--el-border-color);
  padding-bottom: 0.2em;
}
.md-rendered :deep(h2) {
  font-size: 1.2em;
  margin: 0.4em 0;
}
.md-rendered :deep(h3) {
  font-size: 1.05em;
  margin: 0.3em 0;
}
.md-rendered :deep(p) {
  margin: 0.3em 0;
}
.md-rendered :deep(pre) {
  background: var(--el-fill-color);
  padding: 8px 12px;
  border-radius: 6px;
  overflow-x: auto;
  margin: 0.4em 0;
}
.md-rendered :deep(code) {
  font-family: monospace;
  font-size: 0.9em;
}
.md-rendered :deep(p code) {
  background: var(--el-fill-color);
  padding: 1px 4px;
  border-radius: 3px;
}
.md-rendered :deep(ul) {
  padding-left: 1.5em;
  margin: 0.3em 0;
}
.md-rendered :deep(a) {
  color: var(--el-color-primary);
  text-decoration: none;
}
.md-rendered :deep(a:hover) {
  text-decoration: underline;
}
.md-rendered :deep(strong) {
  font-weight: 600;
}

/* --- Custom scrollbar --- */
.detail-scroll::-webkit-scrollbar {
  width: 4px;
}
.detail-scroll::-webkit-scrollbar-thumb {
  background: var(--lc-border);
  border-radius: 2px;
}
.detail-scroll::-webkit-scrollbar-thumb:hover {
  background: var(--lc-border-hover);
}
.detail-scroll::-webkit-scrollbar-track {
  background: transparent;
}

/* --- Responsive --- */
@media (max-width: 640px) {
  .detail-pane-header,
  .detail-scroll {
    padding: 14px;
  }
}
</style>
