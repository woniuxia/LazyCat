<template>
  <div class="pm-today-view">
    <div class="pm-today-scroll">
      <div class="pm-today-stats">
        <div
          v-for="stat in statsList"
          :key="stat.key"
          class="pm-today-stat"
          :class="[`is-${stat.key}`]"
        >
          <div class="pm-today-stat-value">{{ stat.value }}</div>
          <div class="pm-today-stat-label">{{ stat.label }}</div>
          <div v-if="stat.subText" class="pm-today-stat-sub">{{ stat.subText }}</div>
        </div>
      </div>

      <PmTodaySection
        key-name="overdue"
        icon="!"
        title="逾期"
        accent-color="#f56c6c"
        empty-text="暂无逾期任务，保持住"
        :items="overdue"
        :selected-item-id="selectedItemId"
        :loading="loading"
        :collapsible="true"
        @select="handleSelect"
        @edit="handleEdit"
        @item-context="handleContext"
        @start="handleStart"
        @postpone="handlePostpone"
        @complete="handleComplete"
      />

      <PmTodaySection
        key-name="dueToday"
        icon="◷"
        title="今日到期"
        accent-color="#e6a23c"
        empty-text="今日暂无到期任务"
        :items="dueToday"
        :selected-item-id="selectedItemId"
        :loading="loading"
        :collapsible="true"
        @select="handleSelect"
        @edit="handleEdit"
        @item-context="handleContext"
        @start="handleStart"
        @postpone="handlePostpone"
        @complete="handleComplete"
      />

      <PmTodaySection
        key-name="inProgress"
        icon="▶"
        title="进行中"
        accent-color="#409eff"
        empty-text="当前没有正在进行的任务"
        :items="inProgress"
        :selected-item-id="selectedItemId"
        :loading="loading"
        :collapsible="true"
        @select="handleSelect"
        @edit="handleEdit"
        @item-context="handleContext"
        @start="handleStart"
        @postpone="handlePostpone"
        @complete="handleComplete"
      />

      <PmTodaySection
        key-name="completedToday"
        icon="✓"
        title="今日已完成"
        accent-color="#67c23a"
        empty-text="今天还没有完成的任务"
        :items="completedToday"
        :selected-item-id="selectedItemId"
        :loading="loading"
        :collapsible="true"
        :default-collapsed="true"
        @select="handleSelect"
        @edit="handleEdit"
        @item-context="handleContext"
        @start="handleStart"
        @postpone="handlePostpone"
        @complete="handleComplete"
      />

      <div v-if="!loading && allEmpty" class="pm-today-all-empty">
        <el-empty description="今天没有需要处理的任务" :image-size="80" />
      </div>

      <PmTodaySection
        v-if="!loading && unscheduled.length > 0"
        key-name="unscheduled"
        icon="?"
        title="未排期"
        accent-color="#909399"
        empty-text="暂无未排期任务"
        :items="unscheduled"
        :selected-item-id="selectedItemId"
        :loading="loading"
        :collapsible="true"
        :default-collapsed="true"
        @select="handleSelect"
        @edit="handleEdit"
        @item-context="handleContext"
        @start="handleStart"
        @postpone="handlePostpone"
        @complete="handleComplete"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import type { PmItem } from "../types/pm";
import { useToolInvoke } from "../composables/useToolInvoke";
import PmTodaySection from "./PmTodaySection.vue";

interface TodayListResponse {
  overdue: PmItem[];
  dueToday: PmItem[];
  inProgress: PmItem[];
  completedToday: PmItem[];
  unscheduled?: PmItem[];
  unscheduledCount?: number;
}

const props = defineProps<{
  selectedProjectId: number | "overview" | null;
  selectedItemId: number | null;
  refreshSignal?: number;
}>();

const emit = defineEmits<{
  (e: "select", item: PmItem): void;
  (e: "edit", item: PmItem): void;
  (e: "item-context", event: MouseEvent, item: PmItem): void;
  (e: "items-changed"): void;
}>();

const { invoke } = useToolInvoke();

const overdue = ref<PmItem[]>([]);
const dueToday = ref<PmItem[]>([]);
const inProgress = ref<PmItem[]>([]);
const completedToday = ref<PmItem[]>([]);
const unscheduled = ref<PmItem[]>([]);
const loading = ref(false);

const allEmpty = computed(
  () =>
    overdue.value.length === 0 &&
    dueToday.value.length === 0 &&
    inProgress.value.length === 0 &&
    completedToday.value.length === 0,
);

const statsList = computed(() => [
  {
    key: "overdue",
    label: "逾期",
    value: overdue.value.length,
    subText: overdueSubText.value,
  },
  {
    key: "dueToday",
    label: "今日到期",
    value: dueToday.value.length,
    subText: dueTodaySubText.value,
  },
  {
    key: "inProgress",
    label: "进行中",
    value: inProgress.value.length,
    subText: inProgressSubText.value,
  },
  {
    key: "completedToday",
    label: "今日完成",
    value: completedToday.value.length,
    subText: completedSubText.value,
  },
]);

const isOverview = computed(() => props.selectedProjectId === "overview");

const overdueSubText = computed(() => {
  if (overdue.value.length === 0) return "";
  const todayStr = formatLocalDate(new Date());
  let maxDays = 0;
  for (const item of overdue.value) {
    const end = (item.endAt ?? "").slice(0, 10);
    if (!end || end >= todayStr) continue;
    const endDate = new Date(end + "T00:00:00");
    const today = new Date(todayStr + "T00:00:00");
    const diff = Math.round((today.getTime() - endDate.getTime()) / 86400000);
    if (diff > maxDays) maxDays = diff;
  }
  return maxDays > 0 ? `最长逾期 ${maxDays} 天` : "";
});

const dueTodaySubText = computed(() => {
  if (dueToday.value.length === 0) return "";
  let p0 = 0;
  let p1 = 0;
  for (const item of dueToday.value) {
    if (item.priority === "P0") p0 += 1;
    else if (item.priority === "P1") p1 += 1;
  }
  const parts: string[] = [];
  if (p0 > 0) parts.push(`P0 × ${p0}`);
  if (p1 > 0) parts.push(`P1 × ${p1}`);
  return parts.join(" · ");
});

const inProgressSubText = computed(() => {
  if (inProgress.value.length === 0) return "";
  if (!isOverview.value) return "";
  const projectIds = new Set<number>();
  for (const item of inProgress.value) {
    if (typeof item.projectId === "number") projectIds.add(item.projectId);
  }
  return projectIds.size > 0 ? `跨 ${projectIds.size} 个项目` : "";
});

const completedSubText = computed(() => {
  const n = completedToday.value.length;
  if (n === 0) return "今天刚开始，加油";
  if (n >= 5) return "今天收获满满";
  if (n >= 3) return "保持节奏";
  return "已有进展";
});

function formatLocalDate(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

function resolveProjectId(): number | null {
  const id = props.selectedProjectId;
  if (typeof id === "number") return id;
  return null;
}

async function load() {
  if (props.selectedProjectId === null) {
    overdue.value = [];
    dueToday.value = [];
    inProgress.value = [];
    completedToday.value = [];
    unscheduled.value = [];
    return;
  }
  loading.value = true;
  try {
    const payload: Record<string, unknown> = {
      todayDate: formatLocalDate(new Date()),
    };
    const pid = resolveProjectId();
    if (pid !== null) payload.projectId = pid;
    const data = (await invoke<TodayListResponse>("tool:pm:item-today-list", payload)) ?? {
      overdue: [],
      dueToday: [],
      inProgress: [],
      completedToday: [],
      unscheduled: [],
    };
    overdue.value = data.overdue ?? [];
    dueToday.value = data.dueToday ?? [];
    inProgress.value = data.inProgress ?? [];
    completedToday.value = data.completedToday ?? [];
    unscheduled.value = data.unscheduled ?? [];
  } catch (e) {
    ElMessage.error((e as Error).message);
  } finally {
    loading.value = false;
  }
}

watch(
  () => props.selectedProjectId,
  () => {
    void load();
  },
  { immediate: true },
);

watch(
  () => props.refreshSignal ?? 0,
  () => {
    void load();
  },
);

function handleSelect(item: PmItem) {
  emit("select", item);
}

function handleEdit(item: PmItem) {
  emit("edit", item);
}

function handleContext(event: MouseEvent, item: PmItem) {
  emit("item-context", event, item);
}

async function handleStart(item: PmItem) {
  try {
    await invoke("tool:pm:item-change-status", { id: item.id, status: "in_progress" });
    ElMessage.success({ message: "已开始", duration: 1500 });
    await load();
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

function shiftDateOneDay(value: string | null): string | null {
  if (!value) return null;
  const prefix = value.length >= 10 ? value.slice(0, 10) : value;
  const parts = prefix.split("-");
  if (parts.length !== 3) return null;
  const y = Number(parts[0]);
  const m = Number(parts[1]);
  const d = Number(parts[2]);
  if (!Number.isInteger(y) || !Number.isInteger(m) || !Number.isInteger(d)) return null;
  const date = new Date(y, m - 1, d);
  date.setDate(date.getDate() + 1);
  return formatLocalDate(date);
}

async function handlePostpone(item: PmItem) {
  const currentEnd = item.endAt ?? formatLocalDate(new Date());
  const nextEnd = shiftDateOneDay(currentEnd);
  if (!nextEnd) {
    ElMessage.error("截止日期格式异常，无法推迟");
    return;
  }
  const nextStart = item.startAt && item.startAt > nextEnd ? nextEnd : item.startAt;
  try {
    await invoke("tool:pm:item-update", {
      id: item.id,
      startAt: nextStart,
      endAt: nextEnd,
    });
    ElMessage.success({ message: "已推到明天", duration: 1500 });
    await load();
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function handleComplete(item: PmItem) {
  try {
    await invoke("tool:pm:item-change-status", { id: item.id, status: "done" });
    ElMessage.success({ message: "已标记完成", duration: 1500 });
    await load();
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

defineExpose({ refresh: load });
</script>

<style scoped>
.pm-today-view {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 0;
}

.pm-today-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 16px 20px 32px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.pm-today-stats {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
}

.pm-today-stat {
  background: var(--el-bg-color, #fff);
  border: 1px solid var(--pm-edge-soft, #e4e7ed);
  border-radius: 10px;
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  transition: border-color 0.18s;
}

.pm-today-stat-value {
  font-size: 24px;
  font-weight: 600;
  line-height: 1.1;
  color: var(--el-text-color-primary, #303133);
}

.pm-today-stat-label {
  font-size: 12px;
  color: var(--el-text-color-secondary, #606266);
}

.pm-today-stat-sub {
  font-size: 11px;
  color: var(--el-text-color-placeholder, #a8abb2);
  line-height: 1.4;
  margin-top: 2px;
  min-height: 15px;
}

.pm-today-stat.is-overdue {
  border-color: rgba(245, 108, 108, 0.4);
}
.pm-today-stat.is-overdue .pm-today-stat-value {
  color: #f56c6c;
}
.pm-today-stat.is-dueToday {
  border-color: rgba(230, 162, 60, 0.4);
}
.pm-today-stat.is-dueToday .pm-today-stat-value {
  color: #e6a23c;
}
.pm-today-stat.is-inProgress {
  border-color: rgba(64, 158, 255, 0.4);
}
.pm-today-stat.is-inProgress .pm-today-stat-value {
  color: #409eff;
}
.pm-today-stat.is-completedToday {
  border-color: rgba(103, 194, 58, 0.4);
}
.pm-today-stat.is-completedToday .pm-today-stat-value {
  color: #67c23a;
}

.pm-today-all-empty {
  display: flex;
  justify-content: center;
  padding: 24px 0;
}

@media (max-width: 900px) {
  .pm-today-stats {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
