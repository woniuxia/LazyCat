<template>
  <!-- B1 Spike: PoC canvas mock，360×800，模拟 living-wallpaper 仪表盘 -->
  <div class="poc-canvas">
    <!-- 概览块 -->
    <section class="block overview">
      <div class="ring">
        <svg viewBox="0 0 100 100" class="ring-svg">
          <circle cx="50" cy="50" r="42" class="ring-bg" />
          <circle
            cx="50"
            cy="50"
            r="42"
            class="ring-fg"
            :stroke-dasharray="`${ringValue} 999`"
          />
        </svg>
        <div class="ring-text">
          <div class="ring-percent">33%</div>
          <div class="ring-count">2/6 件</div>
        </div>
      </div>
      <div class="alarms">
        <div class="alarm warn">⚠ P0×1</div>
        <div class="alarm time">⏰ 9h 截止</div>
      </div>
    </section>

    <!-- 待办列表 -->
    <section class="block todo-list">
      <div v-for="item in todos" :key="item.id" class="todo-row">
        <span class="dot" :class="`p${item.priority}`"></span>
        <span v-if="item.pinned" class="pin">📌</span>
        <span class="title">{{ item.title }}</span>
        <span class="deadline" :class="{ overdue: item.overdue }">
          {{ item.deadline }}
        </span>
      </div>
      <div class="more">+3 件</div>
    </section>

    <!-- 扩展位（PoC 留空） -->
    <section class="block extension">
      <div class="ext-placeholder">扩展位</div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";

const ringValue = ref(33 * 2.64); // 33% of 264 (圆周)

const todos = ref([
  { id: 1, priority: 0, pinned: true, title: "完成桌面壁纸 PoC 验证", deadline: "今天", overdue: false },
  { id: 2, priority: 1, pinned: false, title: "修复 PM 面板渲染闪烁", deadline: "今天", overdue: false },
  { id: 3, priority: 1, pinned: false, title: "回 #42 设计评审", deadline: "明天", overdue: false },
  { id: 4, priority: 2, pinned: false, title: "整理周报文档", deadline: "5月7日", overdue: false },
  { id: 5, priority: 2, pinned: false, title: "写 Todo Drag 单元测试", deadline: "5月8日", overdue: false },
  { id: 6, priority: 3, pinned: false, title: "更新依赖版本", deadline: "已逾期 2 天", overdue: true },
]);
</script>

<style scoped>
.poc-canvas {
  width: 360px;
  height: 800px;
  background: linear-gradient(135deg, #1e293b 0%, #334155 100%);
  color: #f1f5f9;
  font-family: "Segoe UI", "Microsoft YaHei", sans-serif;
  padding: 16px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 12px;
  border-radius: 16px;
  backdrop-filter: blur(8px);
}

.block {
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  padding: 12px;
}

.overview {
  display: flex;
  align-items: center;
  gap: 16px;
  height: 200px;
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
  stroke: rgba(255, 255, 255, 0.1);
  stroke-width: 8;
}

.ring-fg {
  fill: none;
  stroke: #38bdf8;
  stroke-width: 8;
  stroke-linecap: round;
  transition: stroke-dasharray 0.6s ease;
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
  color: #f1f5f9;
}

.ring-count {
  font-size: 12px;
  color: #94a3b8;
  margin-top: 2px;
}

.alarms {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.alarm {
  font-size: 13px;
  padding: 6px 10px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.08);
}

.alarm.warn {
  color: #fbbf24;
}

.alarm.time {
  color: #f87171;
}

.todo-list {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
  overflow: hidden;
}

.todo-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 4px;
  font-size: 13px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.dot.p0 {
  background: #ef4444;
}
.dot.p1 {
  background: #f59e0b;
}
.dot.p2 {
  background: #3b82f6;
}
.dot.p3 {
  background: #6b7280;
}

.pin {
  font-size: 12px;
}

.title {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.deadline {
  font-size: 11px;
  color: #94a3b8;
  flex-shrink: 0;
}

.deadline.overdue {
  color: #ef4444;
  font-weight: 500;
}

.more {
  font-size: 12px;
  color: #94a3b8;
  text-align: center;
  padding-top: 6px;
}

.extension {
  height: 80px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.ext-placeholder {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.3);
  font-style: italic;
}
</style>
