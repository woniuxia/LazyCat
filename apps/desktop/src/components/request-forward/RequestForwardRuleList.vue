<script setup lang="ts">
import { computed, ref } from "vue";
import { Delete, Edit, MoreFilled, Plus, Search, VideoPause, VideoPlay } from "@element-plus/icons-vue";
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
  busy: boolean;
}>();

const emit = defineEmits<{
  add: [];
  select: [id: number];
  start: [id: number];
  stop: [id: number];
  edit: [id: number];
  delete: [id: number];
  "start-all": [];
  "stop-all": [];
}>();

type RuleMenuHandle = { handleOpen: () => void };

const keyword = ref("");
const menuRefs = new Map<number, RuleMenuHandle>();

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
const runningCount = computed(
  () => props.statuses.filter((status) => status.state === "running").length,
);

function setMenuRef(ruleId: number, value: unknown) {
  if (value) menuRefs.set(ruleId, value as RuleMenuHandle);
  else menuRefs.delete(ruleId);
}

function openMenu(ruleId: number) {
  menuRefs.get(ruleId)?.handleOpen();
}

function handleCommand(command: "edit" | "delete", ruleId: number) {
  emit(command, ruleId);
}

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
        <h2>转发规则</h2>
        <span>{{ rules.length }} 条 · {{ runningCount }} 条运行中</span>
      </div>
      <el-tooltip content="新建规则" placement="bottom">
        <el-button
          type="primary"
          circle
          :icon="Plus"
          :disabled="busy"
          aria-label="新建规则"
          @click="emit('add')"
        />
      </el-tooltip>
    </div>

    <el-input
      v-model="keyword"
      clearable
      :prefix-icon="Search"
      placeholder="搜索名称、协议或端点"
      aria-label="搜索规则"
    />

    <div class="rule-list__batch" aria-label="批量操作">
      <el-button size="small" :disabled="busy || !rules.length" @click="emit('start-all')">
        全部启动
      </el-button>
      <el-button size="small" :disabled="busy || !rules.length" @click="emit('stop-all')">
        全部停止
      </el-button>
      <span>{{ filteredRules.length }} / {{ rules.length }}</span>
    </div>

    <div v-loading="loading" class="rule-list__scroll">
      <el-dropdown
        v-for="rule in filteredRules"
        :key="rule.id"
        :ref="(value: unknown) => setMenuRef(rule.id, value)"
        class="rule-menu"
        trigger="contextmenu"
        @command="(command: 'edit' | 'delete') => handleCommand(command, rule.id)"
      >
        <div
          class="rule-row"
          :class="{ 'is-selected': rule.id === selectedId }"
          :aria-current="rule.id === selectedId ? 'true' : undefined"
        >
          <button
            type="button"
            class="rule-row__select"
            :disabled="busy"
            :title="`${rule.name}，左键查看日志，右键编辑规则`"
            @click="emit('select', rule.id)"
          >
            <span class="rule-row__topline">
              <strong>{{ rule.name }}</strong>
              <span class="state-label" :class="`is-${stateOf(rule.id)}`">
                {{ stateLabel(stateOf(rule.id)) }}
              </span>
            </span>
            <span class="rule-row__summary">
              <b>{{ rule.protocol.toUpperCase() }}</b>
              <span>{{ formatRequestForwardRuleSummary(rule) }}</span>
            </span>
          </button>

          <div class="rule-row__actions">
            <el-tooltip :content="canStart(stateOf(rule.id)) ? '启动规则' : '停止规则'" placement="bottom">
              <el-button
                v-if="canStart(stateOf(rule.id))"
                text
                circle
                size="small"
                :icon="VideoPlay"
                :disabled="busy"
                aria-label="启动规则"
                @click="emit('start', rule.id)"
              />
              <el-button
                v-else
                text
                circle
                size="small"
                :icon="VideoPause"
                :disabled="busy || !canStop(stateOf(rule.id))"
                aria-label="停止规则"
                @click="emit('stop', rule.id)"
              />
            </el-tooltip>
            <el-tooltip content="规则菜单" placement="bottom">
              <el-button
                text
                circle
                size="small"
                :icon="MoreFilled"
                :disabled="busy"
                aria-label="打开规则菜单"
                @click="openMenu(rule.id)"
              />
            </el-tooltip>
          </div>
        </div>

        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item command="edit" :icon="Edit">编辑规则</el-dropdown-item>
            <el-dropdown-item command="delete" :icon="Delete" divided>删除规则</el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>

      <div v-if="!loading && rules.length === 0" class="rule-list__empty">
        <strong>还没有转发规则</strong>
        <span>新建一条规则，配置本地监听端点与目标服务。</span>
        <el-button type="primary" plain :disabled="busy" @click="emit('add')">
          新建第一条规则
        </el-button>
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
  width: 220px;
  min-width: 220px;
  min-height: 0;
  flex-direction: column;
  gap: 8px;
  padding: 12px 10px;
  border-right: 1px solid var(--border-color, #dfe3e8);
  background: #f7f8fa;
}

.rule-list__header,
.rule-list__batch,
.rule-row__topline,
.rule-row__summary,
.rule-row__actions {
  display: flex;
  align-items: center;
}

.rule-list__header {
  min-height: 34px;
  justify-content: space-between;
  gap: 10px;
}

.rule-list__header h2 {
  margin: 0;
  color: var(--text-primary, #1f2937);
  font-size: 15px;
}

.rule-list__header span {
  display: block;
  margin-top: 3px;
  color: var(--text-secondary, #64748b);
  font-size: 10px;
}

.rule-list__batch { gap: 5px; }
.rule-list__batch span { margin-left: auto; color: #718095; font-size: 10px; }

.rule-list__scroll {
  display: flex;
  min-height: 140px;
  flex: 1;
  flex-direction: column;
  overflow-y: auto;
  border-top: 1px solid #e2e7ec;
}

.rule-menu { display: block; width: 100%; }

.rule-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  width: 100%;
  min-height: 50px;
  border-bottom: 1px solid #e2e7ec;
  background: transparent;
  transition: background-color 160ms ease, box-shadow 160ms ease;
}

.rule-row:hover { background: #f0f3f6; }
.rule-row.is-selected { background: #eaf2f7; box-shadow: inset 3px 0 0 var(--el-color-primary, #409eff); }

.rule-row__select {
  display: grid;
  min-width: 0;
  gap: 5px;
  border: 0;
  padding: 7px 5px 7px 9px;
  background: transparent;
  color: inherit;
  cursor: pointer;
  text-align: left;
}

.rule-row__select:disabled { cursor: not-allowed; opacity: .68; }
.rule-row__select:focus-visible { outline: 2px solid var(--el-color-primary, #409eff); outline-offset: -2px; }
.rule-row__topline { min-width: 0; justify-content: space-between; gap: 7px; }
.rule-row__topline strong { overflow: hidden; color: #273548; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.rule-row__summary { min-width: 0; gap: 6px; color: #6d7a8d; font-size: 10px; }
.rule-row__summary b { flex: none; color: #45627b; font-size: 9px; }
.rule-row__summary span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.rule-row__actions { align-self: center; gap: 0; padding-right: 3px; }
.rule-row__actions :deep(.el-button) { width: 26px; height: 26px; margin: 0; }

.state-label { flex: none; font-size: 10px; font-weight: 600; }
.state-label::before { display: inline-block; width: 5px; height: 5px; margin-right: 4px; border-radius: 50%; background: currentColor; content: ""; vertical-align: 1px; }
.state-label.is-running { color: #168357; }
.state-label.is-starting,
.state-label.is-stopping { color: #a86608; }
.state-label.is-failed { color: #c23b35; }
.state-label.is-stopped { color: #6b7280; }

.rule-list__empty {
  display: grid;
  min-height: 180px;
  place-items: center;
  align-content: center;
  gap: 7px;
  padding: 18px 10px;
  color: var(--text-secondary, #64748b);
  text-align: center;
}
.rule-list__empty strong { color: var(--text-primary, #1f2937); font-size: 13px; }
.rule-list__empty span { font-size: 11px; line-height: 1.5; }
.rule-list__empty.compact { min-height: 120px; }

@media (max-width: 780px) {
  .rule-list {
    width: 100%;
    min-width: 0;
    max-height: 38vh;
    border-right: 0;
    border-bottom: 1px solid var(--border-color, #dfe3e8);
  }
}
</style>
