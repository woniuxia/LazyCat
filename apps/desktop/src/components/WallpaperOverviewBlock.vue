<template>
  <!-- 概览块（design §5.1）：左进度环 + 右警戒栏，高度 ~220 px -->
  <section class="overview-block" :class="{ alert: isAlert }">
    <div class="ring">
      <svg viewBox="0 0 100 100" class="ring-svg">
        <circle cx="50" cy="50" r="42" class="ring-bg" />
        <circle
          cx="50"
          cy="50"
          r="42"
          class="ring-fg"
          :class="{ 'ring-alert': isAlert }"
          :stroke-dasharray="`${ringValue} 999`"
        />
      </svg>
      <div class="ring-text">
        <div class="ring-percent">{{ percentText }}</div>
        <div class="ring-count">{{ overview.completedToday }}/{{ overview.totalToday }} 件</div>
      </div>
    </div>
    <div class="alarms">
      <div v-if="overview.p0Pending > 0" class="alarm warn" :class="{ strong: isAlert }">
        ⚠ P0×{{ overview.p0Pending }}
      </div>
      <div v-if="nearestText" class="alarm time" :class="{ strong: isAlert }">
        ⏰ {{ nearestText }}
      </div>
      <div v-if="!overview.p0Pending && !nearestText" class="alarm idle">无紧急事项</div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from "vue";
import type { WallpaperOverview } from "@/types/wallpaper";

const props = defineProps<{ overview: WallpaperOverview }>();

// 圆周长 ≈ 264（2π·42）；保留 1 位小数避免 stroke 抖动
const RING_CIRCUMFERENCE = 264;

const percent = computed(() => {
  const { completedToday, totalToday } = props.overview;
  if (totalToday <= 0) return 0;
  return Math.min(1, completedToday / totalToday);
});

const percentText = computed(() => `${Math.round(percent.value * 100)}%`);
const ringValue = computed(() =>
  (percent.value * RING_CIRCUMFERENCE).toFixed(1),
);

const nearestText = computed(() => {
  const h = props.overview.nearestDeadlineHours;
  if (h === null || h === undefined) return null;
  if (h <= 0) return "已逾期";
  if (h < 1) return `${Math.round(h * 60)}min`;
  if (h < 24) return `${Math.round(h)}h`;
  return `${Math.round(h / 24)}d`;
});

// 告警态：P0 ≥ 3 或最近截止 ≤ 1h
const isAlert = computed(() => {
  if (props.overview.p0Pending >= 3) return true;
  const h = props.overview.nearestDeadlineHours;
  return h !== null && h !== undefined && h <= 1;
});
</script>

<style scoped>
.overview-block {
  height: 200px;
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px;
  border-radius: 12px;
  background: var(--wc-block-bg);
  border: 1px solid var(--wc-block-border);
  flex-shrink: 0;
}

.ring {
  position: relative;
  width: 120px;
  height: 120px;
  flex-shrink: 0;
}

.ring-svg {
  width: 100%;
  height: 100%;
  transform: rotate(-90deg);
}

.ring-bg {
  fill: none;
  stroke: var(--wc-block-border);
  stroke-width: 8;
}

.ring-fg {
  fill: none;
  stroke: #38bdf8;
  stroke-width: 8;
  stroke-linecap: round;
  transition: stroke-dasharray 0.6s ease;
}

.ring-fg.ring-alert {
  stroke: #ef4444;
}

.ring-text {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

.ring-percent {
  font-size: 26px;
  font-weight: 600;
  color: var(--wc-text-strong);
}

.ring-count {
  font-size: 12px;
  color: var(--wc-text-muted);
  margin-top: 2px;
}

.alarms {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
}

.alarm {
  font-size: 13px;
  padding: 6px 10px;
  border-radius: 8px;
  background: var(--wc-block-bg);
}

.alarm.warn {
  color: #f59e0b;
}

.alarm.time {
  color: #f87171;
}

.alarm.idle {
  color: var(--wc-text-muted);
  font-size: 12px;
}

.alarm.strong {
  font-weight: 600;
}
</style>
