<template>
  <div class="weekly-panel">
    <el-config-provider :locale="zhCn">
    <div class="weekly-header">
      <div class="header-left">
        <span class="header-kicker">近7天内的推进轨迹与完成情况</span>
        <div class="header-title-row">
          <h3>本周工作</h3>
          <el-date-picker
            v-model="dateRange"
            type="daterange"
            range-separator="~"
            start-placeholder="开始"
            end-placeholder="结束"
            size="small"
            :disabled-date="noop"
            @change="onDateChange"
          />
        </div>
      </div>
      <el-button size="small" @click="loadData" :loading="loading">刷新</el-button>
    </div>

    <div v-if="hasData" class="weekly-body">
      <div class="summary-grid">
        <div
          v-for="card in summaryCards"
          :key="card.key"
          class="summary-card"
          :class="{ 'is-alert': card.tone === 'alert' }"
        >
          <div class="summary-label">{{ card.label }}</div>
          <div class="summary-value">{{ card.value }}</div>
          <div class="summary-hint">{{ card.hint }}</div>
        </div>
      </div>

      <div class="group-list">
        <section v-for="group in groupedData" :key="group.key" class="work-group-card">
          <div class="group-header">
            <div class="group-title-wrap">
              <span class="project-dot" :style="{ backgroundColor: group.projectColor || 'var(--lc-accent)' }" />
              <div class="group-title-copy">
                <div class="group-title-row">
                  <span class="group-name">{{ group.projectName }}</span>
                  <el-tag v-if="group.projectArchived" size="small" type="info">已归档</el-tag>
                </div>
                <span class="group-summary">{{ formatGroupSummary(group) }}</span>
              </div>
            </div>
            <span class="group-count">{{ group.items.length }} 项</span>
          </div>

          <div class="group-timeline">
            <article
              v-for="item in group.items"
              :key="`${item.source}-${item.id}`"
              class="timeline-row"
            >
              <div class="timeline-date">{{ formatTimelineDate(item) }}</div>
              <div class="work-item-card" :class="{ 'is-risk': isRiskItem(item) }">
                <div class="item-head">
                  <span class="item-title">{{ item.title }}</span>
                  <span class="item-time">{{ formatItemTime(item) }}</span>
                </div>
                <div class="item-meta">
                  <span class="item-badge" :class="item.source === 'pm' ? 'is-pm' : 'is-todo'">
                    {{ item.source === "pm" ? "PM" : "Todo" }}
                  </span>
                  <span v-if="item.itemType" class="item-badge is-neutral">{{ itemTypeLabel(item.itemType) }}</span>
                  <span v-if="item.source === 'pm'" class="item-badge is-neutral">{{ statusLabel(item.status) }}</span>
                  <span class="item-badge is-priority" :style="{ color: priorityColor(item.priority) }">
                    {{ priorityLabel(item.priority) }}
                  </span>
                </div>
              </div>
            </article>
          </div>
        </section>
      </div>
    </div>

    <div v-else-if="!loading" class="weekly-empty">
      <el-empty description="近7天还没有命中的工作项" />
    </div>
    </el-config-provider>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useToolInvoke } from "../composables/useToolInvoke";
import type { WeeklyWorkItem, WeeklyWorkResult } from "../types/pm";
import { PM_ITEM_TYPE_MAP, PM_PRIORITY_MAP, PM_STATUS_COLUMNS } from "../types/pm";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import {
  formatPmDateForDisplay,
  formatPmDateRangeForDisplay,
  normalizePmDateRangeForDraft,
} from "../utils/pmDate";

const { invoke } = useToolInvoke();
const loading = ref(false);
const data = ref<WeeklyWorkResult | null>(null);

function pad(n: number) {
  return String(n).padStart(2, "0");
}

function todayStr() {
  const d = new Date();
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function daysAgoStr(n: number) {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

const dateRange = ref<[string, string]>([daysAgoStr(6), todayStr()]);

function noop(_d: Date) {
  return false;
}

function onDateChange() {
  loadData();
}

interface SummaryCard {
  key: string;
  label: string;
  value: number;
  hint: string;
  tone?: "default" | "alert";
}

interface WorkGroup {
  key: string;
  projectName: string;
  projectColor: string | null;
  projectArchived: boolean;
  items: WeeklyWorkItem[];
  latestSortAt: string;
  activeCount: number;
  doneCount: number;
  riskCount: number;
}

const allItems = computed<WeeklyWorkItem[]>(() => {
  if (!data.value) return [];
  return [...data.value.pmItems, ...data.value.todoItems];
});

const hasData = computed(() => allItems.value.length > 0);

const summaryCards = computed<SummaryCard[]>(() => {
  const items = allItems.value;
  const totalCount = items.length;
  const activeCount = items.filter(isActiveItem).length;
  const doneCount = items.filter(isDoneItem).length;
  const riskCount = items.filter(isRiskItem).length;

  return [
    {
      key: "total",
      label: "总量",
      value: totalCount,
      hint: "时间窗口内命中的全部事项",
    },
    {
      key: "active",
      label: "进行中",
      value: activeCount,
      hint: "进行中与测试中的事项",
    },
    {
      key: "done",
      label: "已完成",
      value: doneCount,
      hint: "时间窗口内已收口的事项",
    },
    {
      key: "risk",
      label: "风险项",
      value: riskCount,
      hint: "周期内未完成的PM事项",
      tone: "alert",
    },
  ];
});

const groupedData = computed<WorkGroup[]>(() => {
  const groups = new Map<string, WorkGroup>();

  for (const item of allItems.value) {
    const key = item.projectId ? `p-${item.projectId}` : "no-project";
    if (!groups.has(key)) {
      groups.set(key, {
        key,
        projectName: item.projectName ?? "未归项目",
        projectColor: item.projectColor ?? null,
        projectArchived: item.projectStatus === "archived",
        items: [],
        latestSortAt: getItemSortAt(item),
        activeCount: 0,
        doneCount: 0,
        riskCount: 0,
      });
    }

    const group = groups.get(key)!;
    group.items.push(item);
    if (isActiveItem(item)) {
      group.activeCount += 1;
    }
    if (isDoneItem(item)) {
      group.doneCount += 1;
    }
    if (isRiskItem(item)) {
      group.riskCount += 1;
    }

    const itemSortAt = getItemSortAt(item);
    if (itemSortAt > group.latestSortAt) {
      group.latestSortAt = itemSortAt;
    }
  }

  return Array.from(groups.values())
    .map((group) => ({
      ...group,
      items: [...group.items].sort((left, right) => getItemSortAt(right).localeCompare(getItemSortAt(left))),
    }))
    .sort((left, right) => right.latestSortAt.localeCompare(left.latestSortAt));
});

async function loadData() {
  loading.value = true;
  try {
    const [start, end] = dateRange.value;
    data.value = (await invoke<WeeklyWorkResult>("tool:pm:weekly-work", { windowStart: start, windowEnd: end })) ?? null;
  } catch (error) {
    console.error(error);
  } finally {
    loading.value = false;
  }
}

function itemTypeLabel(type: string): string {
  return PM_ITEM_TYPE_MAP[type as keyof typeof PM_ITEM_TYPE_MAP]?.label ?? type;
}

function priorityLabel(priority: string): string {
  return PM_PRIORITY_MAP[priority as keyof typeof PM_PRIORITY_MAP]?.label ?? priority;
}

function priorityColor(priority: string): string {
  return PM_PRIORITY_MAP[priority as keyof typeof PM_PRIORITY_MAP]?.color ?? "#909399";
}

function statusLabel(status: string): string {
  return PM_STATUS_COLUMNS.find((column) => column.key === status)?.label ?? status;
}

function isDoneItem(item: WeeklyWorkItem): boolean {
  if (item.source === "todo") {
    return item.status === "completed";
  }
  return item.status === "done";
}

function isActiveItem(item: WeeklyWorkItem): boolean {
  return item.source === "pm" && (item.status === "in_progress" || item.status === "testing");
}

function isRiskItem(item: WeeklyWorkItem): boolean {
  if (item.source !== "pm") return false;
  return item.status !== "done";
}

function formatDateOnly(value: string | null | undefined): string {
  if (!value) return "--";
  const pmDate = formatPmDateForDisplay(value, "short");
  if (pmDate) {
    return pmDate.replace("-", "/");
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "--";
  }

  return date.toLocaleDateString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
  });
}

function formatDateTime(value: string | null | undefined): string {
  if (!value) return "";

  const date = new Date(value);
  if (!Number.isNaN(date.getTime()) && value.includes("T")) {
    return date.toLocaleString("zh-CN", {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  const pmDate = formatPmDateForDisplay(value, "short");
  return pmDate ? pmDate.replace("-", "/") : "";
}

function getItemSortAt(item: WeeklyWorkItem): string {
  return item.sortAt ?? item.completedAt ?? item.endAt ?? item.startAt ?? item.createdAt;
}

function formatTimelineDate(item: WeeklyWorkItem): string {
  return formatDateOnly(getItemSortAt(item));
}

function formatItemTime(item: WeeklyWorkItem): string {
  if (item.source !== "pm") {
    return item.completedAt ? `完成于 ${formatDateTime(item.completedAt)}` : `创建于 ${formatDateTime(item.createdAt)}`;
  }

  const range = normalizePmDateRangeForDraft(item.startAt, item.endAt);
  if (range.startAt && range.endAt) {
    if (range.startAt === range.endAt) {
      return `计划 ${formatPmDateForDisplay(range.startAt, "short").replace("-", "/")}`;
    }

    return `计划 ${formatPmDateRangeForDisplay(range.startAt, range.endAt, {
      mode: "short",
      emptyText: "",
    }).replace(/-/g, "/")}`;
  }

  if (item.completedAt) {
    return `完成于 ${formatDateTime(item.completedAt)}`;
  }

  return `创建于 ${formatDateTime(item.createdAt)}`;
}

function formatGroupSummary(group: WorkGroup): string {
  const parts: string[] = [];
  if (group.activeCount > 0) {
    parts.push(`${group.activeCount} 项推进中`);
  }
  if (group.doneCount > 0) {
    parts.push(`${group.doneCount} 项已完成`);
  }
  if (group.riskCount > 0) {
    parts.push(`${group.riskCount} 项风险`);
  }

  if (parts.length === 0) {
    return "时间窗口内事项已全部收口";
  }

  return parts.join(" · ");
}

onMounted(loadData);
</script>

<style scoped>
.weekly-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.weekly-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 18px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.02), transparent),
    var(--el-bg-color);
  flex-shrink: 0;
}

.header-left {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.header-kicker {
  font-size: 12px;
  color: var(--lc-text-muted);
  letter-spacing: 0.04em;
}

.header-title-row {
  display: flex;
  align-items: baseline;
  gap: 12px;
  min-width: 0;
}

.header-title-row .el-date-editor {
  max-width: 260px;
}

.header-left h3 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--lc-text);
}

.weekly-body {
  flex: 1;
  overflow-y: auto;
  padding: 16px 18px 20px;
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
  margin-bottom: 16px;
}

.summary-card {
  position: relative;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 108px;
  padding: 12px 14px;
  border: 1px solid var(--lc-border);
  border-radius: 14px;
  background: linear-gradient(180deg, var(--lc-surface-1), rgba(255, 255, 255, 0.02));
  box-shadow: var(--lc-shadow-sm);
}

.summary-card::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  width: 100%;
  height: 3px;
  background: linear-gradient(90deg, var(--lc-accent), var(--lc-accent-light));
}

.summary-card.is-alert::before {
  background: linear-gradient(90deg, var(--lc-warning), #fb923c);
}

.summary-label {
  font-size: 11px;
  font-weight: 700;
  color: var(--lc-text-muted);
  letter-spacing: 0.05em;
}

.summary-value {
  font-family: var(--lc-font-display);
  font-size: 26px;
  font-weight: 700;
  line-height: 1;
  color: var(--lc-text);
}

.summary-card.is-alert .summary-value {
  color: var(--lc-warning);
}

.summary-hint {
  font-size: 12px;
  line-height: 1.5;
  color: var(--lc-text-secondary);
}

.group-list {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.work-group-card {
  border: 1px solid var(--lc-border);
  border-radius: 18px;
  background: linear-gradient(180deg, var(--lc-surface-1), rgba(255, 255, 255, 0.01));
  box-shadow: var(--lc-shadow-sm);
  overflow: hidden;
}

.group-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 16px 12px;
  border-bottom: 1px solid var(--el-border-color-extra-light);
}

.group-title-wrap {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  min-width: 0;
}

.group-title-copy {
  min-width: 0;
}

.group-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.project-dot {
  width: 12px;
  height: 12px;
  margin-top: 4px;
  border-radius: 3px;
  flex-shrink: 0;
  box-shadow: 0 0 0 4px rgba(56, 189, 248, 0.08);
}

.group-name {
  font-weight: 600;
  font-size: 15px;
  color: var(--lc-text);
}

.group-summary {
  display: block;
  margin-top: 4px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--lc-text-secondary);
}

.group-count {
  flex-shrink: 0;
  padding: 4px 8px;
  border-radius: 999px;
  background: var(--el-fill-color-light);
  font-size: 12px;
  color: var(--lc-text-secondary);
}

.group-timeline {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px 16px 16px;
}

.group-timeline::before {
  content: "";
  position: absolute;
  left: 72px;
  top: 18px;
  bottom: 18px;
  width: 2px;
  background: linear-gradient(180deg, rgba(56, 189, 248, 0.12), rgba(56, 189, 248, 0.04));
}

.timeline-row {
  position: relative;
  display: grid;
  grid-template-columns: 58px minmax(0, 1fr);
  gap: 16px;
  align-items: start;
}

.timeline-row::before {
  content: "";
  position: absolute;
  left: 67px;
  top: 9px;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--lc-accent);
  box-shadow: 0 0 0 4px rgba(56, 189, 248, 0.1);
}

.timeline-date {
  padding-top: 4px;
  font-size: 11px;
  font-weight: 600;
  color: var(--lc-text-muted);
  text-align: right;
}

.work-item-card {
  margin-left: 20px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px 14px;
  border: 1px solid var(--lc-border);
  border-radius: 14px;
  background: linear-gradient(180deg, var(--el-bg-color), var(--el-fill-color-extra-light));
  transition: border-color var(--lc-duration) var(--lc-ease), background var(--lc-duration) var(--lc-ease);
}

.work-item-card:hover {
  border-color: var(--lc-border-hover);
  background: var(--el-bg-color);
}

.work-item-card.is-risk {
  border-color: rgba(251, 191, 36, 0.28);
  box-shadow: inset 0 0 0 1px rgba(251, 191, 36, 0.12);
}

.item-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.item-title {
  min-width: 0;
  font-size: 14px;
  line-height: 1.45;
  font-weight: 600;
  color: var(--lc-text);
  display: -webkit-box;
  overflow: hidden;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.item-time {
  font-size: 12px;
  line-height: 1.5;
  color: var(--lc-text-secondary);
  text-align: right;
  flex-shrink: 0;
}

.item-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.item-badge {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
  background: var(--el-fill-color-light);
  color: var(--lc-text-secondary);
}

.item-badge.is-pm {
  background: rgba(56, 189, 248, 0.12);
  color: var(--lc-accent-light);
}

.item-badge.is-todo {
  background: rgba(52, 211, 153, 0.12);
  color: var(--lc-success);
}

.item-badge.is-neutral {
  background: var(--el-fill-color-light);
  color: var(--lc-text-secondary);
}

.item-badge.is-priority {
  background: rgba(251, 191, 36, 0.08);
}

.weekly-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

@media (max-width: 1080px) {
  .summary-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 820px) {
  .weekly-header {
    align-items: flex-start;
  }

  .timeline-row {
    grid-template-columns: 1fr;
    gap: 8px;
    padding-left: 24px;
  }

  .group-timeline::before {
    left: 22px;
  }

  .timeline-row::before {
    left: 17px;
  }

  .timeline-date {
    text-align: left;
    padding-top: 0;
  }

  .work-item-card {
    margin-left: 0;
  }

  .item-head {
    flex-direction: column;
    align-items: flex-start;
  }

  .item-time {
    text-align: left;
  }
}

@media (max-width: 560px) {
  .weekly-header {
    flex-direction: column;
    align-items: stretch;
  }

  .header-title-row {
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
  }

  .summary-grid {
    grid-template-columns: 1fr;
  }

  .group-header {
    flex-direction: column;
    align-items: flex-start;
  }

  .group-count {
    align-self: flex-start;
  }
}
</style>
