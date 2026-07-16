<script setup lang="ts">
import { computed, ref } from "vue";
import type {
  RequestForwardRule,
  RequestForwardRuntimeState,
  RequestForwardRuntimeStatus,
} from "../../types/request-forward";
import { formatRequestForwardRuleSummary } from "../../utils/requestForward";

const props = defineProps<{
  rules: RequestForwardRule[];
  statuses: RequestForwardRuntimeStatus[];
  selectedId: number | null;
  loading?: boolean;
}>();

const emit = defineEmits<{
  add: [];
  select: [id: number];
  start: [id: number];
  stop: [id: number];
  "start-all": [];
  "stop-all": [];
}>();

const keyword = ref("");

const statusById = computed(
  () => new Map(props.statuses.map((status) => [status.ruleId, status])),
);
const filteredRules = computed(() => {
  const query = keyword.value.trim().toLowerCase();
  if (!query) return props.rules;
  return props.rules.filter((rule) =>
    `${rule.name} ${rule.protocol} ${formatRequestForwardRuleSummary(rule)}`
      .toLowerCase()
      .includes(query),
  );
});

function stateOf(ruleId: number): RequestForwardRuntimeState {
  return statusById.value.get(ruleId)?.state ?? "stopped";
}

function stateLabel(state: RequestForwardRuntimeState): string {
  return {
    stopped: "已停止",
    starting: "启动中",
    running: "运行中",
    stopping: "停止中",
    failed: "失败",
  }[state];
}

function canStart(state: RequestForwardRuntimeState): boolean {
  return state === "stopped" || state === "failed";
}

function canStop(state: RequestForwardRuntimeState): boolean {
  return state === "starting" || state === "running";
}
</script>

<template>
  <aside class="rule-list" aria-label="转发规则列表">
    <div class="rule-list__header">
      <div>
        <p class="rule-list__eyebrow">FORWARD RULES</p>
        <h2>转发规则</h2>
      </div>
      <el-button type="primary" @click="emit('add')">新建规则</el-button>
    </div>

    <el-input v-model="keyword" clearable placeholder="搜索名称、协议或端点" aria-label="搜索规则" />

    <div class="rule-list__batch" aria-label="批量操作">
      <el-button size="small" :disabled="!rules.length" @click="emit('start-all')">
        全部启动
      </el-button>
      <el-button size="small" :disabled="!rules.length" @click="emit('stop-all')">
        全部停止
      </el-button>
      <span>{{ filteredRules.length }} / {{ rules.length }}</span>
    </div>

    <div v-loading="loading" class="rule-list__scroll">
      <button
        v-for="rule in filteredRules"
        :key="rule.id"
        type="button"
        class="rule-card"
        :class="{ 'is-selected': rule.id === selectedId }"
        :aria-current="rule.id === selectedId ? 'true' : undefined"
        @click="emit('select', rule.id)"
      >
        <span class="rule-card__topline">
          <strong>{{ rule.name }}</strong>
          <span class="state-label" :class="`is-${stateOf(rule.id)}`">
            {{ stateLabel(stateOf(rule.id)) }}
          </span>
        </span>
        <span class="rule-card__protocol">{{ rule.protocol.toUpperCase() }}</span>
        <span class="rule-card__summary">{{ formatRequestForwardRuleSummary(rule) }}</span>
        <span class="rule-card__actions">
          <el-button
            v-if="canStart(stateOf(rule.id))"
            text
            size="small"
            @click.stop="emit('start', rule.id)"
          >
            启动
          </el-button>
          <el-button
            v-else
            text
            size="small"
            :disabled="!canStop(stateOf(rule.id))"
            @click.stop="emit('stop', rule.id)"
          >
            停止
          </el-button>
        </span>
      </button>

      <div v-if="!loading && rules.length === 0" class="rule-list__empty">
        <strong>还没有转发规则</strong>
        <span>新建一条规则，配置本地监听端口与目标服务。</span>
        <el-button type="primary" plain @click="emit('add')">新建第一条规则</el-button>
      </div>
      <div v-else-if="!loading && filteredRules.length === 0" class="rule-list__empty compact">
        <strong>没有匹配规则</strong>
        <span>尝试缩短关键词或清空搜索。</span>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.rule-list {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  gap: 12px;
  padding: 18px;
  border-right: 1px solid var(--border-color, #dfe3e8);
  background: #f7f8fa;
}

.rule-list__header,
.rule-list__batch,
.rule-card__topline,
.rule-card__actions {
  display: flex;
  align-items: center;
}

.rule-list__header {
  justify-content: space-between;
  gap: 12px;
}

.rule-list__header h2 {
  margin: 2px 0 0;
  color: var(--text-primary, #1f2937);
  font-size: 18px;
}

.rule-list__eyebrow {
  margin: 0;
  color: var(--text-secondary, #64748b);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.12em;
}

.rule-list__batch {
  gap: 8px;
}

.rule-list__batch span {
  margin-left: auto;
  color: var(--text-secondary, #64748b);
  font-size: 12px;
}

.rule-list__scroll {
  display: flex;
  min-height: 160px;
  flex: 1;
  flex-direction: column;
  gap: 8px;
  overflow-y: auto;
}

.rule-card {
  position: relative;
  display: grid;
  width: 100%;
  min-width: 0;
  gap: 7px;
  padding: 12px;
  border: 1px solid #dfe3e8;
  border-radius: 7px;
  background: #fff;
  color: inherit;
  text-align: left;
  cursor: pointer;
  transition: border-color 160ms ease, background-color 160ms ease, box-shadow 160ms ease;
}

.rule-card:hover {
  border-color: #aeb8c5;
  background: #fbfcfd;
}

.rule-card:focus-visible {
  outline: 2px solid var(--el-color-primary, #409eff);
  outline-offset: 2px;
}

.rule-card.is-selected {
  border-color: var(--el-color-primary, #409eff);
  box-shadow: inset 3px 0 0 var(--el-color-primary, #409eff);
}

.rule-card__topline {
  min-width: 0;
  justify-content: space-between;
  gap: 8px;
}

.rule-card__topline strong {
  overflow: hidden;
  color: var(--text-primary, #1f2937);
  font-size: 14px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rule-card__protocol {
  width: fit-content;
  padding: 2px 5px;
  border: 1px solid #d7dde5;
  border-radius: 3px;
  color: #455468;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.06em;
}

.rule-card__summary {
  overflow: hidden;
  color: var(--text-secondary, #64748b);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rule-card__actions {
  min-height: 24px;
  justify-content: flex-end;
}

.state-label {
  flex: none;
  font-size: 12px;
  font-weight: 600;
}

.state-label::before {
  display: inline-block;
  width: 6px;
  height: 6px;
  margin-right: 5px;
  border-radius: 50%;
  background: currentColor;
  content: "";
  vertical-align: 1px;
}

.state-label.is-running { color: #168357; }
.state-label.is-starting,
.state-label.is-stopping { color: #a86608; }
.state-label.is-failed { color: #c23b35; }
.state-label.is-stopped { color: #6b7280; }

.rule-list__empty {
  display: grid;
  place-items: center;
  gap: 8px;
  min-height: 220px;
  padding: 24px;
  border: 1px dashed #cbd3dd;
  border-radius: 7px;
  color: var(--text-secondary, #64748b);
  text-align: center;
}

.rule-list__empty strong { color: var(--text-primary, #1f2937); }
.rule-list__empty span { max-width: 260px; font-size: 13px; line-height: 1.55; }
.rule-list__empty.compact { min-height: 140px; }

@media (max-width: 780px) {
  .rule-list {
    max-height: 42vh;
    border-right: 0;
    border-bottom: 1px solid var(--border-color, #dfe3e8);
  }
}
</style>
