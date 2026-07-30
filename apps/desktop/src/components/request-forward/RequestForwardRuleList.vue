<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  CopyDocument,
  Delete,
  Edit,
  MoreFilled,
  Plus,
  Search,
  VideoPause,
  VideoPlay,
} from "@element-plus/icons-vue";
import type {
  RequestForwardRule,
  RequestForwardRuntimeState,
  RequestForwardRuntimeStatus,
} from "../../types/request-forward";
import {
  filterRequestForwardRules,
  formatRequestForwardEndpoint,
  formatRequestForwardRuleSummary,
  getRequestForwardBatchScope,
} from "../../utils/requestForward";
import type { RequestForwardRuleStateFilter } from "../../utils/requestForward";

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
  "auto-start-update": [id: number, enabled: boolean];
  edit: [id: number];
  duplicate: [id: number];
  delete: [id: number];
  "batch-start": [ids: number[], scopeLabel: string];
  "batch-stop": [ids: number[], scopeLabel: string];
}>();

type RuleMenuHandle = { handleOpen: () => void };

const keyword = ref("");
const stateFilter = ref<RequestForwardRuleStateFilter>("all");
const selectedIds = ref<number[]>([]);
const menuRefs = new Map<number, RuleMenuHandle>();

const statusById = computed(
  () => new Map(props.statuses.map((status) => [status.ruleId, status])),
);
const filteredRules = computed(() => {
  return filterRequestForwardRules(
    props.rules,
    props.statuses,
    keyword.value,
    stateFilter.value,
  );
});
const filterActive = computed(
  () => Boolean(keyword.value.trim()) || stateFilter.value !== "all",
);
const batchScope = computed(() =>
  getRequestForwardBatchScope(
    props.rules,
    filteredRules.value,
    selectedIds.value,
    filterActive.value,
  ),
);
const allFilteredSelected = computed(
  () => filteredRules.value.length > 0 && filteredRules.value.every((rule) => selectedIds.value.includes(rule.id)),
);
const someFilteredSelected = computed(
  () => filteredRules.value.some((rule) => selectedIds.value.includes(rule.id)) && !allFilteredSelected.value,
);
const runningCount = computed(
  () => props.statuses.filter((status) => status.state === "running").length,
);

watch(filteredRules, (visibleRules) => {
  const visibleIds = new Set(visibleRules.map((rule) => rule.id));
  selectedIds.value = selectedIds.value.filter((id) => visibleIds.has(id));
});

function toggleRuleSelection(ruleId: number, selected: boolean) {
  const next = new Set(selectedIds.value);
  if (selected) next.add(ruleId);
  else next.delete(ruleId);
  selectedIds.value = [...next];
}

function toggleFilteredSelection(selected: boolean) {
  selectedIds.value = selected ? filteredRules.value.map((rule) => rule.id) : [];
}

function clearSelection() {
  selectedIds.value = [];
}

function runBatch(operation: "start" | "stop") {
  const event = operation === "start" ? "batch-start" : "batch-stop";
  emit(event, batchScope.value.ids, batchScope.value.label);
}

function setMenuRef(ruleId: number, value: unknown) {
  if (value) menuRefs.set(ruleId, value as RuleMenuHandle);
  else menuRefs.delete(ruleId);
}

function openMenu(ruleId: number) {
  menuRefs.get(ruleId)?.handleOpen();
}

function handleCommand(
  command: "edit" | "duplicate" | "delete",
  ruleId: number,
) {
  emit(command, ruleId);
}

function autoStartTooltip(rule: RequestForwardRule): string {
  return rule.autoStart
    ? "已开启：下次启动应用时会自动运行此规则"
    : "已关闭：启动应用时不会自动运行此规则";
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

function listenEndpoint(rule: RequestForwardRule): string {
  return formatRequestForwardEndpoint(rule.bindHost, rule.listenPort);
}

function targetEndpoint(rule: RequestForwardRule): string {
  return rule.protocol === "http"
    ? rule.targetUrl?.trim() || "—"
    : formatRequestForwardEndpoint(rule.targetHost, rule.targetPort);
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

    <div class="rule-list__filters">
      <el-input
        v-model="keyword"
        clearable
        :prefix-icon="Search"
        placeholder="搜索规则"
        aria-label="搜索规则"
      />
      <el-select v-model="stateFilter" aria-label="按运行状态筛选">
        <el-option label="全部状态" value="all" />
        <el-option label="已停止" value="stopped" />
        <el-option label="运行中" value="running" />
        <el-option label="失败" value="failed" />
        <el-option label="启动中" value="starting" />
        <el-option label="停止中" value="stopping" />
      </el-select>
    </div>

    <div class="rule-list__batch" aria-label="批量操作">
      <div class="rule-list__batch-scope">
        <el-checkbox
          :model-value="allFilteredSelected"
          :indeterminate="someFilteredSelected"
          :disabled="busy || !filteredRules.length"
          @update:model-value="toggleFilteredSelection(Boolean($event))"
        >全选当前</el-checkbox>
        <el-button v-if="selectedIds.length" text size="small" :disabled="busy" @click="clearSelection">
          清除选择
        </el-button>
        <span>{{ batchScope.label }}</span>
      </div>
      <div class="rule-list__batch-actions">
        <el-button size="small" :disabled="busy || !batchScope.ids.length" @click="runBatch('start')">
          启动{{ batchScope.label }}
        </el-button>
        <el-button size="small" :disabled="busy || !batchScope.ids.length" @click="runBatch('stop')">
          停止{{ batchScope.label }}
        </el-button>
      </div>
    </div>

    <div v-loading="loading" class="rule-list__scroll">
      <el-dropdown
        v-for="rule in filteredRules"
        :key="rule.id"
        :ref="(value: unknown) => setMenuRef(rule.id, value)"
        class="rule-menu"
        trigger="contextmenu"
        @command="(command: 'edit' | 'duplicate' | 'delete') => handleCommand(command, rule.id)"
      >
        <div
          class="rule-row"
          :class="{ 'is-selected': rule.id === selectedId }"
          :aria-current="rule.id === selectedId ? 'true' : undefined"
        >
          <el-checkbox
            class="rule-row__check"
            :model-value="selectedIds.includes(rule.id)"
            :disabled="busy"
            :aria-label="`选择规则${rule.name}`"
            @click.stop
            @update:model-value="toggleRuleSelection(rule.id, Boolean($event))"
          />
          <button
            type="button"
            class="rule-row__select"
            :disabled="busy"
            :title="`${rule.name}，左键查看日志，右键打开更多操作`"
            @click="emit('select', rule.id)"
          >
            <span class="rule-row__title">
              <strong>{{ rule.name }}</strong>
            </span>
            <span class="rule-row__meta">
              <b class="protocol-label">{{ rule.protocol.toUpperCase() }}</b>
              <span class="state-label" :class="`is-${stateOf(rule.id)}`">
                当前：{{ stateLabel(stateOf(rule.id)) }}
              </span>
            </span>
            <span
              class="rule-row__summary"
              :title="formatRequestForwardRuleSummary(rule)"
            >
              <span class="rule-row__summary-line">
                <b>监听</b>
                <span>{{ listenEndpoint(rule) }}</span>
              </span>
              <span class="rule-row__summary-line">
                <b>转发</b>
                <span>{{ targetEndpoint(rule) }}</span>
              </span>
            </span>
          </button>

          <div class="rule-row__controls" role="group" aria-label="规则运行控制" @click.stop>
            <el-button
              v-if="canStart(stateOf(rule.id))"
              class="rule-row__runtime-action"
              type="primary"
              plain
              size="small"
              :icon="VideoPlay"
              :disabled="busy"
              :aria-label="`启动规则${rule.name}`"
              @click="emit('start', rule.id)"
            >启动</el-button>
            <el-button
              v-else
              class="rule-row__runtime-action"
              plain
              size="small"
              :icon="VideoPause"
              :disabled="busy || !canStop(stateOf(rule.id))"
              :aria-label="`停止规则${rule.name}`"
              @click="emit('stop', rule.id)"
            >{{ stateOf(rule.id) === "stopping" ? "停止中" : "停止" }}</el-button>
            <el-tooltip :content="autoStartTooltip(rule)" placement="bottom">
              <span class="rule-row__auto-start">
                <span>应用启动时</span>
                <el-switch
                  :model-value="rule.autoStart"
                  size="small"
                  inline-prompt
                  active-text="开"
                  inactive-text="关"
                  :disabled="busy"
                  :aria-label="`${rule.name}应用启动时自动运行`"
                  @update:model-value="emit('auto-start-update', rule.id, $event)"
                />
              </span>
            </el-tooltip>
          </div>

          <div class="rule-row__menu">
            <el-tooltip content="更多操作" placement="bottom">
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
            <el-dropdown-item command="duplicate" :icon="CopyDocument">复制规则</el-dropdown-item>
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
  width: 100%;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  gap: 8px;
  padding: 12px 10px;
  border-right: 1px solid var(--border-color, #dfe3e8);
  background: #f7f8fa;
}

.rule-list__header,
.rule-row__title,
.rule-row__summary,
.rule-row__summary-line,
.rule-row__meta,
.rule-row__controls,
.rule-row__auto-start,
.rule-row__menu {
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
  font-size: 18px;
}

.rule-list__header span {
  display: block;
  margin-top: 3px;
  color: var(--text-secondary, #64748b);
  font-size: 12px;
}

.rule-list__filters { display: grid; grid-template-columns: minmax(0, 1fr) 104px; gap: 6px; }
.rule-list__batch { display: grid; gap: 5px; }
.rule-list__batch-scope,
.rule-list__batch-actions { display: flex; align-items: center; gap: 5px; }
.rule-list__batch-scope span { margin-left: auto; color: #657386; font-size: 12px; white-space: nowrap; }
.rule-list__batch-scope :deep(.el-button) { margin-left: 0; padding-inline: 3px; }
.rule-list__batch-actions :deep(.el-button) { min-width: 0; flex: 1; margin-left: 0; }

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
  position: relative;
  display: block;
  width: 100%;
  min-height: 82px;
  border-bottom: 1px solid #e2e7ec;
  background: transparent;
  transition: background-color 160ms ease, box-shadow 160ms ease;
}

.rule-row:hover { background: #f0f3f6; }
.rule-row.is-selected { background: #eaf2f7; box-shadow: inset 3px 0 0 var(--el-color-primary, #409eff); }

.rule-row__select {
  display: grid;
  width: 100%;
  min-width: 0;
  box-sizing: border-box;
  gap: 6px;
  border: 0;
  padding: 9px 5px 9px 34px;
  background: transparent;
  color: inherit;
  cursor: pointer;
  text-align: left;
}

.rule-row__check { position: absolute; z-index: 2; top: 10px; left: 8px; }

.rule-row__select:disabled { cursor: not-allowed; opacity: .68; }
.rule-row__select:focus-visible { outline: 2px solid var(--el-color-primary, #409eff); outline-offset: -2px; }
.rule-row__title { min-width: 0; padding-right: 28px; }
.rule-row__title strong { overflow: hidden; color: #273548; font-size: 16px; text-overflow: ellipsis; white-space: nowrap; }
.rule-row__meta { min-width: 0; flex-wrap: wrap; gap: 7px; }
.protocol-label { color: #45627b; font-size: 12px; }
.rule-row__summary { display: grid; min-width: 0; gap: 3px; color: #56667a; font-size: 14px; line-height: 1.5; }
.rule-row__summary-line { min-width: 0; align-items: flex-start; gap: 7px; }
.rule-row__summary-line b { width: 34px; flex: none; color: #45627b; font-size: 12px; }
.rule-row__summary-line span { min-width: 0; overflow-wrap: anywhere; white-space: normal; }

.rule-row__controls { justify-content: space-between; gap: 8px; margin: 0 5px 9px 8px; padding-top: 7px; border-top: 1px solid #e2e7ec; }
.rule-row__controls :deep(.el-button) { margin: 0; }
.rule-row__runtime-action { width: 68px; flex: none; }
.rule-row__auto-start { min-width: 0; justify-content: flex-end; gap: 6px; color: #526275; font-size: 12px; white-space: nowrap; }
.rule-row__auto-start :deep(.el-switch) { flex: none; }
.rule-row__menu { position: absolute; z-index: 2; top: 5px; right: 3px; }
.rule-row__menu :deep(.el-button) { width: 26px; height: 26px; margin: 0; }

.state-label { flex: none; font-size: 12px; font-weight: 600; }
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
.rule-list__empty strong { color: var(--text-primary, #1f2937); font-size: 16px; }
.rule-list__empty span { font-size: 14px; line-height: 1.5; }
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
