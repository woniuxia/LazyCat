<template>
  <div class="log-list">
    <div class="log-toolbar">
      <span class="log-status" :class="`status-${status}`">
        <i class="status-dot" />
        {{ statusLabel }}
      </span>
      <el-button size="small" text :disabled="logs.length === 0" @click="emit('clear')"
        >清空</el-button
      >
    </div>

    <div v-if="logs.length === 0" class="api-mock-empty">暂无运行期请求日志。</div>
    <div
      v-for="log in logs"
      :key="log.id"
      class="log-row"
      :class="{
        alert: getMockLogRowTone(log) === 'alert',
        clickable: log.routeId !== null,
      }"
      :title="log.routeId !== null ? '点击定位命中的路由' : ''"
      @click="log.routeId !== null && emit('jump', log)"
    >
      <span class="method">{{ log.method }}</span>
      <strong>{{ log.path }}</strong>
      <span>{{ log.status }}</span>
      <span>{{ log.durationMs }} ms</span>
      <small>{{ log.routeName || log.error || "未命中" }}</small>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import type { ApiMockRequestLog } from "../../types/api-mock";
import { getMockLogRowTone } from "../../utils/apiMock";

const props = defineProps<{
  logs: ApiMockRequestLog[];
  status: "active" | "stopped" | "paused";
}>();

const emit = defineEmits<{
  (event: "clear"): void;
  (event: "jump", log: ApiMockRequestLog): void;
}>();

const statusLabel = computed(() => {
  if (props.status === "active") return "自动刷新中";
  if (props.status === "paused") return "自动刷新已暂停";
  return "服务未运行";
});
</script>

<style scoped>
.log-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.log-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.log-status {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #64748b;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #cbd5e1;
}

.log-status.status-active .status-dot {
  background: #16a34a;
}

.log-status.status-paused .status-dot {
  background: #f59e0b;
}

.log-row {
  display: grid;
  grid-template-columns: 64px minmax(0, 1fr) 60px 80px minmax(120px, 0.8fr);
  gap: 10px;
  align-items: center;
  padding: 9px 10px;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  background: #f8fafc;
  font-size: 13px;
}

.log-row.alert {
  border-color: #fecaca;
  border-left: 3px solid #ef4444;
  background: #fef2f2;
}

.log-row.clickable {
  cursor: pointer;
}

.log-row.clickable:hover {
  border-color: #93c5fd;
}

.log-row strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.log-row small {
  font-size: 12px;
  color: #64748b;
}

.method {
  font-size: 12px;
  font-weight: 700;
  color: #0f766e;
}

.api-mock-empty {
  padding: 20px 10px;
  font-size: 13px;
  line-height: 1.6;
  color: #64748b;
}

@media (max-width: 860px) {
  .log-row {
    grid-template-columns: 1fr;
  }
}
</style>
