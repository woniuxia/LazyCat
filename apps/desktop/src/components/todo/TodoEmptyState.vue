<template>
  <div class="detail-empty-pane">
    <div class="detail-empty-visual">
      <div class="empty-illustration">
        <svg class="empty-svg" viewBox="0 0 200 160" fill="none">
          <circle cx="160" cy="40" r="20" fill="var(--lc-accent)" opacity="0.08" />
          <circle cx="30" cy="120" r="15" fill="var(--lc-success)" opacity="0.06" />
          <circle cx="170" cy="130" r="10" fill="var(--lc-warning)" opacity="0.08" />
          <rect
            x="50"
            y="20"
            width="100"
            height="120"
            rx="12"
            fill="var(--lc-surface-1)"
            stroke="var(--lc-border)"
            stroke-width="2"
          />
          <rect x="65" y="45" width="70" height="6" rx="3" fill="var(--lc-border)" opacity="0.6" />
          <rect x="65" y="60" width="50" height="6" rx="3" fill="var(--lc-border)" opacity="0.4" />
          <rect x="65" y="75" width="60" height="6" rx="3" fill="var(--lc-border)" opacity="0.4" />
          <rect x="65" y="90" width="40" height="6" rx="3" fill="var(--lc-border)" opacity="0.4" />
          <circle
            cx="140"
            cy="115"
            r="22"
            fill="var(--lc-surface-0)"
            stroke="var(--lc-accent)"
            stroke-width="2.5"
          />
          <path
            d="M130 115L137 122L152 107"
            stroke="var(--lc-accent)"
            stroke-width="3"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
          <circle cx="60" cy="30" r="4" fill="var(--lc-danger)" opacity="0.6" />
          <circle cx="75" cy="30" r="4" fill="var(--lc-warning)" opacity="0.6" />
          <circle cx="90" cy="30" r="4" fill="var(--lc-success)" opacity="0.6" />
        </svg>
        <div class="empty-glow"></div>
      </div>
    </div>
    <div class="detail-empty-content">
      <div class="detail-empty-title">选择事项查看详情</div>
      <div class="detail-empty-text">在列表中点击任意任务，或快速创建新任务开始管理您的工作。</div>
    </div>
    <div class="detail-empty-actions">
      <el-button type="primary" size="large" @click="emit('create')">
        <el-icon class="empty-btn-icon"><Plus /></el-icon>
        新建任务
      </el-button>
      <el-button size="large" @click="emit('refresh')">
        <el-icon class="empty-btn-icon"><Refresh /></el-icon>
        刷新列表
      </el-button>
    </div>
    <div class="detail-empty-divider">
      <span>今日概览</span>
    </div>
    <div class="detail-empty-stats">
      <div class="detail-empty-stat" :class="{ 'is-active': todayDueCount > 0 }">
        <div class="stat-icon today">
          <el-icon><Calendar /></el-icon>
        </div>
        <div class="stat-info">
          <span class="stat-label">今日到期</span>
          <strong class="stat-value">{{ todayDueCount }}</strong>
        </div>
      </div>
      <div class="detail-empty-stat" :class="{ 'is-alert': overdueCount > 0 }">
        <div class="stat-icon overdue">
          <el-icon><AlarmClock /></el-icon>
        </div>
        <div class="stat-info">
          <span class="stat-label">逾期事项</span>
          <strong class="stat-value">{{ overdueCount }}</strong>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { AlarmClock, Calendar, Plus, Refresh } from "@element-plus/icons-vue";

defineProps<{
  todayDueCount: number;
  overdueCount: number;
}>();

const emit = defineEmits<{
  create: [];
  refresh: [];
}>();
</script>

<style scoped>
.detail-empty-pane {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 20px;
  padding: 32px 24px;
  text-align: center;
  animation: fadeIn 0.4s var(--lc-ease);
}
.detail-empty-visual {
  position: relative;
}
.empty-illustration {
  position: relative;
  width: 180px;
  height: 144px;
}
.empty-svg {
  width: 100%;
  height: 100%;
  animation: float 6s ease-in-out infinite;
}
.empty-glow {
  position: absolute;
  inset: 20%;
  background: radial-gradient(circle, var(--lc-accent) 0%, transparent 70%);
  opacity: 0.1;
  filter: blur(20px);
  animation: pulse 4s ease-in-out infinite;
}
@keyframes float {
  0%,
  100% {
    transform: translateY(0px);
  }
  50% {
    transform: translateY(-8px);
  }
}
@keyframes pulse {
  0%,
  100% {
    opacity: 0.08;
    transform: scale(1);
  }
  50% {
    opacity: 0.15;
    transform: scale(1.1);
  }
}
.detail-empty-content {
  max-width: 280px;
}
.detail-empty-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--lc-text);
  margin-bottom: 8px;
}
.detail-empty-text {
  font-size: 13px;
  line-height: 1.7;
  color: var(--lc-text-muted);
}
.detail-empty-actions {
  display: flex;
  gap: 12px;
  margin-top: 4px;
}
.empty-btn-icon {
  margin-right: 4px;
}
.detail-empty-divider {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  max-width: 320px;
  color: var(--lc-text-muted);
  font-size: 12px;
  margin-top: 8px;
}
.detail-empty-divider::before,
.detail-empty-divider::after {
  content: "";
  flex: 1;
  height: 1px;
  background: var(--lc-border);
}
.detail-empty-stats {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
  width: 100%;
  max-width: 320px;
}
.detail-empty-stat {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px;
  border-radius: 12px;
  background: var(--lc-surface-1);
  border: 1px solid var(--lc-border);
  transition: all 0.25s var(--lc-ease);
}
.detail-empty-stat:hover {
  transform: translateY(-2px);
  box-shadow: var(--lc-shadow-sm);
}
.detail-empty-stat.is-active {
  background: linear-gradient(135deg, rgba(56, 189, 248, 0.08), var(--lc-surface-1));
  border-color: rgba(56, 189, 248, 0.3);
}
.detail-empty-stat.is-alert {
  background: linear-gradient(135deg, rgba(248, 113, 113, 0.08), var(--lc-surface-1));
  border-color: rgba(248, 113, 113, 0.3);
}
.stat-icon {
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 10px;
  font-size: 18px;
  flex-shrink: 0;
}
.stat-icon.today {
  color: var(--lc-accent);
  background: rgba(56, 189, 248, 0.12);
}
.stat-icon.overdue {
  color: var(--lc-danger);
  background: rgba(248, 113, 113, 0.12);
}
.stat-info {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
}
.stat-label {
  font-size: 12px;
  color: var(--lc-text-muted);
  white-space: nowrap;
}
.stat-value {
  font-size: 20px;
  font-weight: 700;
  color: var(--lc-text);
  white-space: nowrap;
}
.detail-empty-stat.is-alert .stat-value {
  color: var(--lc-danger);
}
@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
