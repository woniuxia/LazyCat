<template>
  <div class="pm-matrix-view">
    <div class="matrix-toolbar">
      <div class="toolbar-left">
        <span class="toolbar-label">紧急阈值</span>
        <div class="segmented">
          <button
            v-for="opt in THRESHOLD_OPTIONS"
            :key="opt"
            type="button"
            class="seg-btn"
            :class="{ 'is-active': threshold === opt }"
            @click="setThreshold(opt)"
          >
            {{ opt }} 天
          </button>
        </div>
      </div>
      <div class="toolbar-right">
        <el-checkbox :model-value="hideCompleted" @change="(v: boolean | string | number) => setHideCompleted(Boolean(v))">
          隐藏已完成
        </el-checkbox>
      </div>
    </div>

    <div v-if="loading && !hasData" class="matrix-loading">加载中…</div>

    <div class="matrix-container">
      <div class="matrix-vaxis">
        <div class="vaxis-half">
          <span class="vaxis-text important">▲ 重要</span>
        </div>
        <div class="vaxis-half">
          <span class="vaxis-text not-important">不重要 ▼</span>
        </div>
      </div>
      <div class="matrix-main">
        <div class="matrix-haxis">
          <span class="haxis-text">◀ 紧急</span>
          <span class="haxis-text">不紧急 ▶</span>
        </div>
        <div class="matrix-grid">
          <MatrixQuadrant
            :title="'立即做'"
            :roman="'I'"
            accent-color="#f56c6c"
            :items="q1"
            :selected-item-id="selectedItemId"
            empty-text="此象限暂无任务"
            @select="emitSelect"
            @edit="emitEdit"
            @context="emitContext"
          />
          <MatrixQuadrant
            :title="'计划做'"
            :roman="'II'"
            accent-color="#409eff"
            :items="q2"
            :selected-item-id="selectedItemId"
            empty-text="此象限暂无任务"
            @select="emitSelect"
            @edit="emitEdit"
            @context="emitContext"
          />
          <MatrixQuadrant
            :title="'快速处理'"
            :roman="'III'"
            accent-color="#e6a23c"
            :items="q3"
            :selected-item-id="selectedItemId"
            empty-text="此象限暂无任务"
            @select="emitSelect"
            @edit="emitEdit"
            @context="emitContext"
          />
          <MatrixQuadrant
            :title="'少做 / 推迟'"
            :roman="'IV'"
            accent-color="#909399"
            :items="q4"
            :selected-item-id="selectedItemId"
            empty-text="此象限暂无任务"
            @select="emitSelect"
            @edit="emitEdit"
            @context="emitContext"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import type { PmItem } from "../types/pm";
import { useToolInvoke } from "../composables/useToolInvoke";
import { getSetting, setSetting } from "../composables/useSettings";
import MatrixQuadrant from "./PmMatrixQuadrant.vue";

const THRESHOLD_OPTIONS = [3, 7, 14];
const KEY_THRESHOLD = "pm:view:matrix:urgentThreshold";
const KEY_HIDE_COMPLETED = "pm:view:matrix:hideCompleted";

interface BucketResponse {
  q1: PmItem[];
  q2: PmItem[];
  q3: PmItem[];
  q4: PmItem[];
}

const props = defineProps<{
  selectedProjectId: number | "overview" | null;
  selectedItemId: number | null;
}>();

const emit = defineEmits<{
  (e: "select", item: PmItem): void;
  (e: "edit", item: PmItem): void;
  (e: "item-context", event: MouseEvent, item: PmItem): void;
  (e: "items-changed"): void;
}>();

const { invoke } = useToolInvoke();

const q1 = ref<PmItem[]>([]);
const q2 = ref<PmItem[]>([]);
const q3 = ref<PmItem[]>([]);
const q4 = ref<PmItem[]>([]);
const loading = ref(false);

const threshold = ref<number>(readThreshold());
const hideCompleted = ref<boolean>(readHideCompleted());

function readThreshold(): number {
  const raw = Number(getSetting(KEY_THRESHOLD));
  if (THRESHOLD_OPTIONS.includes(raw)) return raw;
  return 3;
}

function readHideCompleted(): boolean {
  const raw = getSetting(KEY_HIDE_COMPLETED);
  if (raw === undefined) return true;
  return raw === "true";
}

function setThreshold(v: number) {
  threshold.value = v;
  setSetting(KEY_THRESHOLD, String(v));
  void load();
}

function setHideCompleted(v: boolean) {
  hideCompleted.value = v;
  setSetting(KEY_HIDE_COMPLETED, v ? "true" : "false");
  void load();
}

const hasData = computed(
  () => q1.value.length + q2.value.length + q3.value.length + q4.value.length > 0,
);

function formatLocalDate(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

async function load() {
  if (props.selectedProjectId === null) {
    q1.value = q2.value = q3.value = q4.value = [];
    return;
  }
  loading.value = true;
  try {
    const payload: Record<string, unknown> = {
      todayDate: formatLocalDate(new Date()),
      urgentThresholdDays: threshold.value,
      hideCompleted: hideCompleted.value,
    };
    const pid = props.selectedProjectId;
    if (typeof pid === "number") payload.projectId = pid;
    const result = (await invoke<BucketResponse>("tool:pm:item-matrix-bucket", payload)) ?? {
      q1: [],
      q2: [],
      q3: [],
      q4: [],
    };
    q1.value = result.q1 ?? [];
    q2.value = result.q2 ?? [];
    q3.value = result.q3 ?? [];
    q4.value = result.q4 ?? [];
  } catch (e) {
    ElMessage.error((e as Error).message);
  } finally {
    loading.value = false;
  }
}

function emitSelect(item: PmItem) {
  emit("select", item);
}
function emitEdit(item: PmItem) {
  emit("edit", item);
}
function emitContext(event: MouseEvent, item: PmItem) {
  emit("item-context", event, item);
}

watch(
  () => props.selectedProjectId,
  () => {
    void load();
  },
  { immediate: true },
);

onMounted(() => {
  void load();
});

defineExpose({ refresh: load });
</script>

<style scoped>
.pm-matrix-view {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 12px 20px 16px;
  gap: 12px;
  background: var(--lc-surface-1);
  border-radius: var(--lc-radius-lg);
}

.matrix-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}
.toolbar-left,
.toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}
.toolbar-label {
  font-size: 13px;
  color: var(--el-text-color-regular);
}
.segmented {
  display: inline-flex;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  overflow: hidden;
  background: var(--el-fill-color-light);
}
.seg-btn {
  appearance: none;
  background: transparent;
  border: 0;
  padding: 4px 12px;
  font-size: 13px;
  color: var(--el-text-color-regular);
  cursor: pointer;
  line-height: 22px;
}
.seg-btn.is-active {
  background: var(--el-color-primary);
  color: #fff;
}

.matrix-loading {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.matrix-container {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 36px 1fr;
  gap: 6px;
}

.matrix-vaxis {
  display: flex;
  flex-direction: column;
}
.vaxis-half {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 0;
}
.vaxis-text {
  writing-mode: vertical-rl;
  text-orientation: upright;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 2px;
  color: var(--el-text-color-regular);
  font-family: "Segoe UI", "Microsoft YaHei", sans-serif;
}
.vaxis-text.important {
  color: var(--el-color-primary);
}
.vaxis-text.not-important {
  color: var(--el-text-color-secondary);
}

.matrix-main {
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.matrix-haxis {
  display: flex;
  justify-content: space-between;
  padding: 0 12px 6px;
}
.haxis-text {
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 1px;
  color: var(--el-text-color-regular);
}

.matrix-grid {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 1fr 1fr;
  grid-template-rows: 1fr 1fr;
  gap: 10px;
}
</style>
