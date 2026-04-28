<template>
  <div class="detail-view" :key="item.id">
    <!-- Header -->
    <div class="detail-pane-header detail-pane-header--view">
      <div class="detail-title-group">
        <div class="detail-eyebrow">{{ isPmItem ? '工作项详情' : '事项详情' }}</div>
        <div class="detail-title-row">
          <h3 class="detail-title detail-title--copyable" title="点击复制标题" @click="$emit('copyTitle', item.title)">{{ item.title }}</h3>
          <div class="detail-badges">
            <template v-if="isPmItem">
              <span
                v-if="PM_ITEM_TYPE_MAP[(item as UnifiedItem).itemType as string]"
                class="detail-badge pm-type-badge"
                :style="getPmLightTagStyle(PM_ITEM_TYPE_MAP[(item as UnifiedItem).itemType as string]?.color)"
              >
                {{ PM_ITEM_TYPE_MAP[(item as UnifiedItem).itemType as string]?.label ?? (item as UnifiedItem).itemType }}
              </span>
              <span class="detail-badge" :class="'priority-' + item.priority.toLowerCase()">
                <el-icon :size="12"><Flag /></el-icon>
                {{ item.priority }} {{ priorityLabel(item.priority as TodoPriority) }}
              </span>
              <span
                class="detail-badge pm-status-badge"
                :style="{ backgroundColor: pmItemStatusColor + '18', color: pmItemStatusColor }"
              >
                {{ pmItemStatusLabel }}
              </span>
              <span v-if="item.pinned" class="detail-badge pinned">
                <el-icon :size="12"><Top /></el-icon> 置顶
              </span>
              <span v-if="isItemOverdue(item)" class="detail-badge overdue">
                <el-icon :size="12"><AlarmClock /></el-icon> 逾期
              </span>
            </template>
            <template v-else>
              <span
                v-if="item.priority !== 'P2'"
                class="detail-badge"
                :class="'priority-' + item.priority.toLowerCase()"
              >
                <el-icon :size="12"><Flag /></el-icon>
                {{ item.priority }} {{ priorityLabel(item.priority) }}
              </span>
              <span class="detail-badge pinned" v-if="item.pinned">
                <el-icon :size="12"><Top /></el-icon> 置顶
              </span>
              <span class="detail-badge repeat" v-if="hasRepeatRule(item)">
                <el-icon :size="12"><Refresh /></el-icon> 重复
              </span>
              <span class="detail-badge overdue" v-if="isItemOverdue(item)">
                <el-icon :size="12"><AlarmClock /></el-icon> 逾期
              </span>
            </template>
          </div>
        </div>
      </div>
      <div class="detail-header-actions">
        <el-button size="small" link class="detail-edit-btn" @click="$emit('edit', item)">
          {{ isPmItem ? '在PM中编辑' : '编辑' }}
        </el-button>
        <el-button
          v-if="canPinItem(item)"
          size="small"
          link
          @click="$emit('togglePin', item.id)"
        >
          {{ item.pinned ? "取消置顶" : "置顶" }}
        </el-button>
        <el-button
          v-if="!isPmItem"
          size="small"
          link
          :type="isDoneItem(item) ? '' : 'success'"
          @click="$emit('changeStatus', item.id, isDoneItem(item) ? 'pending' : 'completed')"
        >
          {{ isDoneItem(item) ? "恢复" : "完成" }}
        </el-button>
        <el-button
          v-if="isPmItem && !isDoneItem(item)"
          size="small"
          link
          type="primary"
          @click="$emit('changeStatus', item.id, 'advance')"
        >
          推进状态
        </el-button>
        <el-button size="small" link type="danger" @click="$emit('delete', item)">删除</el-button>
      </div>
    </div>

    <!-- Content -->
    <div class="detail-scroll">
      <div class="detail-content">
        <!-- ===== PM Item Cards ===== -->
        <template v-if="isPmItem">
          <!-- PM Project & Type Card -->
          <div v-if="item.projectId || (item as UnifiedItem).itemType" class="detail-card">
            <div class="detail-card-header">
              <div class="detail-card-icon warning">
                <el-icon><Briefcase /></el-icon>
              </div>
              <span class="detail-card-title">工作项属性</span>
            </div>
            <div class="detail-card-body pm-attrs-body">
              <div v-if="item.projectId" class="pm-attr-row">
                <span class="pm-attr-label">所属项目</span>
                <span class="pm-attr-project">
                  <span class="project-section-dot" :style="{ backgroundColor: item.projectColor || '#909399' }" />
                  <span class="project-section-name">{{ item.projectName || '项目 #' + item.projectId }}</span>
                </span>
              </div>
              <div v-if="(item as UnifiedItem).itemType" class="pm-attr-row">
                <span class="pm-attr-label">工作类型</span>
                <el-tag size="small" effect="light" round :style="getPmLightTagStyle(PM_ITEM_TYPE_MAP[(item as UnifiedItem).itemType as string]?.color)">
                  {{ PM_ITEM_TYPE_MAP[(item as UnifiedItem).itemType as string]?.label ?? (item as UnifiedItem).itemType }}
                </el-tag>
              </div>
              <div v-if="(item as UnifiedItem).refCode" class="pm-attr-row">
                <span class="pm-attr-label">编号</span>
                <span class="detail-ref-code">{{ (item as UnifiedItem).refCode }}</span>
              </div>
              <div v-if="(item as UnifiedItem).tags && (item as UnifiedItem).tags!.length > 0" class="pm-attr-row">
                <span class="pm-attr-label">标签</span>
                <div class="pm-tags-list">
                  <el-tag v-for="tag in (item as UnifiedItem).tags" :key="tag" size="small" type="info" round>{{ tag }}</el-tag>
                </div>
              </div>
            </div>
          </div>

          <!-- PM Timeline Card -->
          <div class="detail-card">
            <div class="detail-card-header">
              <div class="detail-card-icon primary">
                <el-icon><Calendar /></el-icon>
              </div>
              <span class="detail-card-title">时间轨迹</span>
            </div>
            <div class="detail-card-body">
              <div class="pm-timeline-grid">
                <div class="pm-timeline-item">
                  <span class="detail-label">时间安排</span>
                  <span class="detail-value" :class="{ 'is-overdue-date': isItemOverdue(item) }">
                    {{ formatPmDateRangeForDisplay((item as UnifiedItem).startAt, (item as UnifiedItem).endAt) }}
                  </span>
                </div>
                <div class="pm-timeline-item">
                  <span class="detail-label">创建时间</span>
                  <span class="detail-value">{{ formatDateTime(item.createdAt) }}</span>
                </div>
                <div class="pm-timeline-item">
                  <span class="detail-label">开始执行</span>
                  <span class="detail-value">{{ formatDateTime((item as UnifiedItem).startedAt) }}</span>
                </div>
                <div class="pm-timeline-item">
                  <span class="detail-label">开始测试</span>
                  <span class="detail-value">{{ formatDateTime((item as UnifiedItem).testingAt) }}</span>
                </div>
                <div v-if="item.completedAt" class="pm-timeline-item">
                  <span class="detail-label">完成时间</span>
                  <span class="detail-value">{{ formatDateTime(item.completedAt) }}</span>
                </div>
              </div>
            </div>
          </div>

          <!-- PM Description Card -->
          <div v-if="item.description" class="detail-card">
            <div class="detail-card-header">
              <div class="detail-card-icon primary">
                <el-icon><Document /></el-icon>
              </div>
              <span class="detail-card-title">描述</span>
            </div>
            <div class="detail-card-body">
              <div class="detail-description-card" :key="'pm-desc-' + item.id">
                <RichDescriptionViewer :value="item.description" />
              </div>
            </div>
          </div>

          <!-- PM Exec Tasks Card -->
          <div class="detail-card">
            <div class="detail-card-header">
              <div class="detail-card-icon success">
                <el-icon><CircleCheck /></el-icon>
              </div>
              <span class="detail-card-title">执行任务</span>
            </div>
            <div class="detail-card-body pm-todo-body">
              <InlineTodoList
                :pm-item-id="() => item.id"
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
          </div>

          <!-- PM Resources Card -->
          <div class="detail-card">
            <div class="detail-card-header">
              <div class="detail-card-icon primary">
                <el-icon><Link /></el-icon>
              </div>
              <span class="detail-card-title">资源关联</span>
            </div>
            <div class="detail-card-body">
              <div class="pm-resource-list">
                <div class="pm-resource-item">
                  <span class="detail-label">链接</span>
                  <div class="pm-resource-link-row">
                    <span class="detail-value detail-link-text">{{ (item as UnifiedItem).linkUrl || "-" }}</span>
                    <el-button
                      v-if="(item as UnifiedItem).linkUrl"
                      size="small"
                      link
                      @click="openItemLink((item as UnifiedItem).linkUrl)"
                    >打开</el-button>
                  </div>
                </div>
                <div v-if="(item as UnifiedItem).siyuanPrimaryPage" class="pm-resource-item">
                  <span class="detail-label">思源主页面</span>
                  <div class="pm-resource-link-row">
                    <span class="detail-value">{{ (item as UnifiedItem).siyuanPrimaryPage!.docTitle }}</span>
                    <el-button size="small" link @click="openSiyuanPage((item as UnifiedItem).siyuanPrimaryPage!.docId)">打开</el-button>
                  </div>
                  <span class="pm-siyuan-meta">{{ (item as UnifiedItem).siyuanPrimaryPage!.notebookName }} · {{ (item as UnifiedItem).siyuanPrimaryPage!.docHpath }}</span>
                </div>
                <div
                  v-for="page in (item as UnifiedItem).siyuanExtraPages"
                  :key="page.docId"
                  class="pm-resource-item"
                >
                  <span class="detail-label">附加页面</span>
                  <div class="pm-resource-link-row">
                    <span class="detail-value">{{ page.docTitle }}</span>
                    <el-button size="small" link @click="openSiyuanPage(page.docId)">打开</el-button>
                  </div>
                  <span class="pm-siyuan-meta">{{ page.notebookName }} · {{ page.docHpath }}</span>
                </div>
              </div>
            </div>
          </div>
        </template>

        <!-- ===== Todo Item Cards ===== -->
        <template v-else>
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

        <!-- Schedule Card -->
        <div
          v-if="
            item.eventAt ||
            effectiveReminderPresets(item.reminderPresets).length > 0
          "
          class="detail-card schedule-card"
          :class="scheduleInsight ? `is-${scheduleInsight.urgencyKind}` : ''"
        >
          <div class="detail-card-header">
            <div class="detail-card-icon" :class="scheduleHeaderIconClass">
              <el-icon><Calendar /></el-icon>
            </div>
            <span class="detail-card-title">时间安排</span>
            <span
              v-if="scheduleInsight?.showBadge"
              class="schedule-urgency-badge"
              :class="`is-${scheduleInsight.badgeKind}`"
            >
              {{ scheduleInsight.badgeLabel }}
            </span>
          </div>
          <div class="detail-card-body schedule-card-body">
            <div v-if="scheduleInsight" class="schedule-hero">
              <div class="schedule-hero-left">
                <span
                  class="schedule-hero-icon"
                  :class="`is-${scheduleInsight.badgeKind}`"
                >
                  <el-icon :size="18">
                    <AlarmClock v-if="scheduleInsight.urgencyKind === 'overdue'" />
                    <CircleCheck v-else-if="scheduleInsight.urgencyKind === 'completed'" />
                    <Clock v-else />
                  </el-icon>
                </span>
                <div class="schedule-hero-text">
                  <div class="schedule-hero-headline">
                    {{ scheduleInsight.headline }}
                  </div>
                  <div v-if="scheduleInsight.headlineSub" class="schedule-hero-sub">
                    {{ scheduleInsight.headlineSub }}
                  </div>
                </div>
              </div>
              <div class="schedule-hero-right">
                <div class="schedule-date-main">{{ scheduleInsight.dateMain }}</div>
                <div v-if="scheduleInsight.dateSub" class="schedule-date-sub">
                  {{ scheduleInsight.dateSub }}
                </div>
              </div>
            </div>
            <div
              v-if="scheduleInsight && reminderChips.length > 0"
              class="schedule-divider"
            ></div>
            <div
              v-if="reminderChips.length > 0"
              class="schedule-reminders"
              :class="{ 'is-only': !scheduleInsight }"
            >
              <el-icon class="schedule-reminders-icon" :size="14"><Bell /></el-icon>
              <span class="schedule-reminders-label">提醒</span>
              <div class="schedule-reminder-chips">
                <span
                  v-for="chip in reminderChips"
                  :key="chip.value"
                  class="schedule-reminder-chip"
                >
                  {{ chip.label }}
                </span>
              </div>
            </div>
          </div>
        </div>

        <!-- Info Card -->
        <div
          v-if="item.typeName || (item.assignees?.length ?? 0) > 0"
          class="detail-card"
        >
          <div class="detail-card-header">
            <div class="detail-card-icon success">
              <el-icon><User /></el-icon>
            </div>
            <span class="detail-card-title">{{ infoCardTitle }}</span>
          </div>
          <div class="detail-card-body info-inline-body">
            <div v-if="item.typeName" class="info-inline-group">
              <el-icon class="info-inline-icon" :size="14"><CollectionTag /></el-icon>
              <span class="info-type-chip" :style="typeChipStyle">
                <span
                  class="color-dot-sm"
                  :style="{ backgroundColor: item.typeColor || '#909399' }"
                />
                {{ item.typeName }}
              </span>
            </div>
            <div v-if="(item.assignees?.length ?? 0) > 0" class="info-inline-group info-inline-group--assignees">
              <el-icon class="info-inline-icon" :size="14"><UserFilled /></el-icon>
              <div class="info-assignee-list">
                <span
                  v-for="assignee in item.assignees"
                  :key="assignee.id"
                  class="info-assignee-chip"
                >
                  {{ assignee.name }}
                </span>
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
              <RichDescriptionViewer :value="item.description" />
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
        </template>
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
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import {
  AlarmClock,
  Bell,
  Briefcase,
  Calendar,
  CircleCheck,
  Clock,
  CollectionTag,
  Document,
  Flag,
  Link,
  Refresh,
  Top,
  User,
  UserFilled,
} from "@element-plus/icons-vue";
import { effectiveReminderPresets } from "../composables/useTodoItem";
import { invokeToolByChannel } from "../bridge/tauri";
import type { TodoItem, TodoPriority } from "../types";
import type { PmItem } from "../types/pm";
import { PM_STATUS_COLUMNS, PM_ITEM_TYPE_MAP, PM_PRIORITY_MAP } from "../types/pm";
import { isPmItemOverdue } from "../utils/pmDate";
import { formatPmDateRangeForDisplay } from "../utils/pmDate";
import { formatTodoRelativeDateTimeLabel } from "../utils/todoRelativeDate";
import RichDescriptionViewer from "./RichDescriptionViewer.vue";
import InlineTodoList from "./InlineTodoList.vue";
import { usePmTodoLinking } from "../composables/usePmTodoLinking";
import type { UnifiedItem } from "../utils/todoBuckets";

const props = defineProps<{
  item: TodoItem | UnifiedItem;
}>();

const emit = defineEmits<{
  edit: [item: TodoItem | UnifiedItem];
  togglePin: [id: number];
  changeStatus: [id: number, status: string];
  delete: [item: TodoItem | UnifiedItem];
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

// --- Schedule insight ---

const WEEKDAY_LABELS = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"] as const;

const REMINDER_CHIP_LABELS: Record<string, string> = {
  "0m": "开始时",
  "5m": "-5 分钟",
  "10m": "-10 分钟",
  "30m": "-30 分钟",
  "1h": "-1 小时",
  "1d": "-1 天",
  "2d": "-2 天",
};

type ScheduleUrgencyKind =
  | "overdue"
  | "today"
  | "tomorrow"
  | "thisWeek"
  | "later"
  | "completed";

type ScheduleBadgeKind =
  | "danger"
  | "warning"
  | "primary"
  | "info"
  | "success"
  | "neutral";

interface ScheduleInsight {
  urgencyKind: ScheduleUrgencyKind;
  badgeKind: ScheduleBadgeKind;
  badgeLabel: string;
  showBadge: boolean;
  headline: string;
  headlineSub: string | null;
  dateMain: string;
  dateSub: string | null;
}

function startOfDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

function formatDurationBrief(ms: number): string {
  const totalSec = Math.max(1, Math.round(ms / 1000));
  if (totalSec < 60) return `${totalSec} 秒`;
  const min = Math.floor(totalSec / 60);
  if (min < 60) return `${min} 分钟`;
  const hour = Math.floor(min / 60);
  if (hour < 24) return `${hour} 小时`;
  const day = Math.floor(hour / 24);
  if (day < 30) return `${day} 天`;
  const month = Math.floor(day / 30);
  if (month < 12) return `${month} 个月`;
  const year = Math.floor(month / 12);
  return `${year} 年`;
}

const now = ref(new Date());
let scheduleTimer: ReturnType<typeof setInterval> | null = null;

onMounted(() => {
  scheduleTimer = setInterval(() => {
    now.value = new Date();
  }, 30_000);
});

onBeforeUnmount(() => {
  if (scheduleTimer) {
    clearInterval(scheduleTimer);
    scheduleTimer = null;
  }
});

// --- Computed ---

const scheduleInsight = computed<ScheduleInsight | null>(() => {
  const item = props.item;
  if (!item.eventAt) return null;
  const date = new Date(item.eventAt);
  if (Number.isNaN(date.getTime())) return null;

  const nowValue = now.value;
  const diffMs = date.getTime() - nowValue.getTime();
  const todayStartMs = startOfDay(nowValue).getTime();
  const itemDayStartMs = startOfDay(date).getTime();
  const dayDiff = Math.round((itemDayStartMs - todayStartMs) / 86_400_000);

  const friendly = formatTodoRelativeDateTimeLabel(item.eventAt, nowValue);
  const weekday = WEEKDAY_LABELS[date.getDay()];
  const absoluteDate = `${date.getMonth() + 1}月${date.getDate()}日`;
  const dateSub = friendly.includes(absoluteDate)
    ? weekday
    : `${absoluteDate} ${weekday}`;

  if (item.status === "completed") {
    return {
      urgencyKind: "completed",
      badgeKind: "success",
      badgeLabel: "已完成",
      showBadge: true,
      headline: "已完成",
      headlineSub: null,
      dateMain: friendly,
      dateSub,
    };
  }

  if (diffMs < 0) {
    const gap = formatDurationBrief(-diffMs);
    return {
      urgencyKind: "overdue",
      badgeKind: "danger",
      badgeLabel: `逾期 · 已过 ${gap}`,
      showBadge: true,
      headline: `已过 ${gap}`,
      headlineSub: "请尽快处理或调整时间",
      dateMain: friendly,
      dateSub,
    };
  }

  if (dayDiff === 0) {
    const minutes = Math.max(0, Math.round(diffMs / 60_000));
    let headline: string;
    if (minutes < 1) headline = "即刻开始";
    else if (minutes < 60) headline = `还有 ${minutes} 分钟`;
    else headline = `还有 ${Math.round(diffMs / 3_600_000)} 小时`;
    return {
      urgencyKind: "today",
      badgeKind: "warning",
      badgeLabel: "今天",
      showBadge: true,
      headline,
      headlineSub: null,
      dateMain: friendly,
      dateSub,
    };
  }

  if (dayDiff === 1) {
    return {
      urgencyKind: "tomorrow",
      badgeKind: "primary",
      badgeLabel: "",
      showBadge: false,
      headline: "明天",
      headlineSub: `约 ${Math.max(1, Math.round(diffMs / 3_600_000))} 小时后`,
      dateMain: friendly,
      dateSub,
    };
  }

  if (dayDiff <= 6) {
    return {
      urgencyKind: "thisWeek",
      badgeKind: "info",
      badgeLabel: "",
      showBadge: false,
      headline: `${dayDiff} 天后`,
      headlineSub: null,
      dateMain: friendly,
      dateSub,
    };
  }

  return {
    urgencyKind: "later",
    badgeKind: "neutral",
    badgeLabel: "",
    showBadge: false,
    headline: `${dayDiff} 天后`,
    headlineSub: null,
    dateMain: friendly,
    dateSub,
  };
});

const scheduleHeaderIconClass = computed(() => {
  const insight = scheduleInsight.value;
  if (!insight) return "primary";
  const map: Record<ScheduleBadgeKind, string> = {
    danger: "danger",
    warning: "warning",
    primary: "primary",
    info: "primary",
    success: "success",
    neutral: "primary",
  };
  return map[insight.badgeKind];
});

const reminderChips = computed(() => {
  const presets = effectiveReminderPresets(props.item.reminderPresets);
  if (presets.length === 0) return [];
  return presets.map((preset) => ({
    value: preset,
    label: REMINDER_CHIP_LABELS[preset] || preset,
  }));
});

const hasDetailCards = computed(() => {
  const item = props.item;
  const hasSchedule = !!item.eventAt || effectiveReminderPresets(item.reminderPresets).length > 0;
  return (
    hasSchedule ||
    !!item.typeName ||
    (item.assignees?.length ?? 0) > 0 ||
    item.description ||
    (item.links?.length ?? 0) > 0 ||
    hasRepeatRule(item) ||
    !!item.projectId
  );
});

function hexToRgba(hex: string, alpha: number): string {
  const clean = hex.replace(/^#/, "");
  if (clean.length !== 3 && clean.length !== 6) return hex;
  const full = clean.length === 3
    ? clean.split("").map((c) => c + c).join("")
    : clean;
  const r = parseInt(full.slice(0, 2), 16);
  const g = parseInt(full.slice(2, 4), 16);
  const b = parseInt(full.slice(4, 6), 16);
  if ([r, g, b].some(Number.isNaN)) return hex;
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

const typeChipStyle = computed(() => {
  const color = props.item.typeColor || "#909399";
  return {
    backgroundColor: hexToRgba(color, 0.1),
    borderColor: hexToRgba(color, 0.28),
    color,
  };
});

const infoCardTitle = computed(() => {
  const hasType = !!props.item.typeName;
  const hasAssignees = (props.item.assignees?.length ?? 0) > 0;
  if (hasType && hasAssignees) return "分类与执行人";
  if (hasType) return "分类";
  return "执行人";
});

// --- PM item support ---

const isPmItem = computed(() => (props.item as UnifiedItem).source === "pm");

function getPmField<T>(field: string, fallback: T): T {
  return ((props.item as Record<string, unknown>)[field] as T) ?? fallback;
}

const pmItemStatusColor = computed(() =>
  PM_STATUS_COLUMNS.find((c) => c.key === (props.item.status || "todo"))?.color ?? "#909399",
);

const pmItemStatusLabel = computed(() =>
  PM_STATUS_COLUMNS.find((c) => c.key === (props.item.status || "todo"))?.label ?? "待办",
);

function getPmLightTagStyle(color?: string | null) {
  const c = color ?? "#409eff";
  return {
    "--el-tag-bg-color": `${c}14`,
    "--el-tag-border-color": `${c}33`,
    "--el-tag-text-color": c,
  };
}

function formatDateTime(dateStr: string | null | undefined): string {
  if (!dateStr) return "-";
  const d = new Date(dateStr);
  if (isNaN(d.getTime())) return "-";
  return d.toLocaleString("zh-CN");
}

const pmTodo = reactive(usePmTodoLinking(() => (isPmItem.value ? props.item.id : null)));

watch(
  () => (isPmItem.value ? props.item.id : null),
  (id) => {
    if (id != null) {
      pmTodo.loadItems(id);
    } else {
      pmTodo.reset();
    }
  },
);

function isDoneItem(item: TodoItem | UnifiedItem): boolean {
  const src = (item as UnifiedItem).source;
  if (src === "pm") return item.status === "done";
  return item.status === "completed";
}

function canPinItem(item: TodoItem | UnifiedItem): boolean {
  const src = (item as UnifiedItem).source;
  if (src === "pm") return item.status !== "done";
  return item.status !== "completed";
}

function hasRepeatRule(item: TodoItem | UnifiedItem): boolean {
  if ((item as UnifiedItem).source === "pm") return false;
  return item.kind === "recurring" && item.recurrence !== null;
}

function isItemOverdue(item: TodoItem | UnifiedItem): boolean {
  const src = (item as UnifiedItem).source;
  if (src === "pm") {
    return isPmItemOverdue(item as unknown as PmItem);
  }
  if (item.status === "completed") return false;
  if (!item.eventAt) return false;
  return new Date(item.eventAt).getTime() < Date.now();
}

function normalizeItemLinkUrl(value: string | null | undefined): string {
  let url = (value ?? "").trim();
  if (!url) return "";
  if (/^https?:\/\//i.test(url)) return url;
  if (url.includes("://")) return "";
  return `http://${url}`;
}

async function openItemLink(url: string | null | undefined) {
  const normalized = normalizeItemLinkUrl(url);
  if (!normalized) return;
  try {
    await invokeToolByChannel("tool:pm:open-link", { url: normalized });
  } catch (e) {
    console.error("打开链接失败:", e);
  }
}

async function openSiyuanPage(docId: string) {
  try {
    await invokeToolByChannel("tool:pm:siyuan-open-page", { docId });
  } catch (e) {
    console.error("打开思源页面失败:", e);
  }
}
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
  flex-wrap: wrap;
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
.detail-badge.priority-p0 {
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
}
.detail-badge.priority-p1 {
  background: var(--el-color-warning-light-9);
  color: var(--el-color-warning);
}
.detail-badge.priority-p3 {
  background: var(--lc-surface-1);
  color: var(--el-text-color-secondary);
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
.info-inline-body {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px 20px;
  padding: 12px 14px;
}
.info-inline-group {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  max-width: 100%;
}
.info-inline-group--assignees {
  flex: 1 1 auto;
  min-width: 0;
}
.info-inline-icon {
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
}
.info-type-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 3px 10px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 500;
  line-height: 1.5;
  letter-spacing: 0.2px;
  border: 1px solid var(--lc-border);
  background: var(--lc-surface-1);
  color: var(--el-text-color-primary);
  white-space: nowrap;
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.info-assignee-list {
  display: inline-flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
.info-assignee-chip {
  display: inline-flex;
  align-items: center;
  padding: 2px 10px;
  border-radius: 999px;
  font-size: 12px;
  line-height: 1.5;
  background: var(--lc-surface-1);
  color: var(--el-text-color-regular);
  border: 1px solid var(--lc-border);
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

/* --- Schedule Card --- */
.schedule-card {
  position: relative;
}
.schedule-card.is-overdue::before,
.schedule-card.is-today::before,
.schedule-card.is-tomorrow::before,
.schedule-card.is-thisWeek::before,
.schedule-card.is-completed::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  pointer-events: none;
  z-index: 1;
}
.schedule-card.is-overdue::before { background: var(--el-color-danger); }
.schedule-card.is-today::before { background: var(--el-color-warning); }
.schedule-card.is-tomorrow::before { background: var(--el-color-primary); }
.schedule-card.is-thisWeek::before { background: var(--el-color-info, var(--el-color-primary)); }
.schedule-card.is-completed::before { background: var(--el-color-success); }

.schedule-urgency-badge {
  margin-left: auto;
  padding: 2px 10px;
  border-radius: 10px;
  font-size: 11px;
  font-weight: 500;
  white-space: nowrap;
  letter-spacing: 0.2px;
  line-height: 1.6;
}
.schedule-urgency-badge.is-danger {
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
}
.schedule-urgency-badge.is-warning {
  background: var(--el-color-warning-light-9);
  color: var(--el-color-warning);
}
.schedule-urgency-badge.is-primary {
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}
.schedule-urgency-badge.is-info {
  background: var(--el-color-info-light-9, var(--el-color-primary-light-9));
  color: var(--el-color-info, var(--el-color-primary));
}
.schedule-urgency-badge.is-success {
  background: var(--el-color-success-light-9);
  color: var(--el-color-success);
}
.schedule-urgency-badge.is-neutral {
  background: var(--lc-surface-1);
  color: var(--el-text-color-secondary);
}

.schedule-card-body {
  padding: 14px;
}
.schedule-hero {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}
.schedule-hero-left {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
  flex: 1;
}
.schedule-hero-icon {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}
.schedule-hero-icon.is-danger {
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
}
.schedule-hero-icon.is-warning {
  background: var(--el-color-warning-light-9);
  color: var(--el-color-warning);
}
.schedule-hero-icon.is-primary {
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}
.schedule-hero-icon.is-info {
  background: var(--el-color-info-light-9, var(--el-color-primary-light-9));
  color: var(--el-color-info, var(--el-color-primary));
}
.schedule-hero-icon.is-success {
  background: var(--el-color-success-light-9);
  color: var(--el-color-success);
}
.schedule-hero-icon.is-neutral {
  background: var(--lc-surface-1);
  color: var(--el-text-color-secondary);
}
.schedule-hero-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.schedule-hero-headline {
  font-size: 17px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  line-height: 1.25;
  letter-spacing: 0.2px;
  word-break: break-word;
}
.schedule-hero-sub {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  line-height: 1.4;
}
.schedule-hero-right {
  text-align: right;
  flex-shrink: 0;
  min-width: 0;
}
.schedule-date-main {
  font-size: 14px;
  font-weight: 500;
  color: var(--el-text-color-primary);
  line-height: 1.3;
  white-space: nowrap;
}
.schedule-date-sub {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  margin-top: 2px;
}

.schedule-divider {
  height: 1px;
  background: var(--lc-border);
  margin: 12px 0;
}

.schedule-reminders {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.schedule-reminders-icon {
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
}
.schedule-reminders-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
  letter-spacing: 0.2px;
}
.schedule-reminder-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.schedule-reminder-chip {
  display: inline-flex;
  align-items: center;
  padding: 2px 10px;
  border-radius: 10px;
  background: var(--lc-surface-1);
  color: var(--el-text-color-regular);
  font-size: 11px;
  border: 1px solid var(--lc-border);
  line-height: 1.5;
  letter-spacing: 0.2px;
}

@media (max-width: 520px) {
  .schedule-hero {
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
  }
  .schedule-hero-right {
    text-align: left;
    width: 100%;
  }
  .schedule-date-main {
    white-space: normal;
  }
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

/* --- PM item styles --- */
.pm-attrs-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px 14px;
}
.pm-attr-row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.pm-attr-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
  min-width: 56px;
}
.pm-attr-project {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.pm-tags-list {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.detail-ref-code {
  font-size: 12px;
  font-weight: 400;
  color: var(--el-text-color-secondary);
  font-family: monospace;
}
.pm-timeline-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}
.pm-timeline-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 64px;
  padding: 10px 12px;
  border: 1px solid var(--lc-border);
  border-radius: 10px;
  background: var(--lc-surface-1);
}
.is-overdue-date {
  color: var(--el-color-danger);
  font-weight: 600;
}
.pm-todo-body {
  padding: 0;
}
.pm-todo-body :deep(.inline-todo-list) {
  border: none;
  border-radius: 0;
}
.pm-resource-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.pm-resource-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.pm-resource-link-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.pm-siyuan-meta {
  font-size: 11px;
  color: var(--el-text-color-placeholder);
}
.pm-type-badge {
  border-radius: 10px;
  font-size: 11px;
  font-weight: 500;
  padding: 2px 8px;
}
.pm-status-badge {
  border-radius: 10px;
  font-size: 11px;
  font-weight: 500;
  padding: 2px 8px;
}
</style>
