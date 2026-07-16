<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useToolInvoke } from "../composables/useToolInvoke";
import type {
  RequestForwardBatchOperationResult,
  RequestForwardLogOutcome,
  RequestForwardLogPage,
  RequestForwardLogRow,
  RequestForwardRule,
  RequestForwardRuleForm,
  RequestForwardRuntimeState,
  RequestForwardRuntimeStatus,
  RequestForwardStats,
} from "../types/request-forward";
import {
  applyRequestForwardMutationResult,
  captureRequestForwardMutationIntent,
  getDefaultRequestForwardForm,
  getRequestForwardBatchMessage,
  getRequestForwardLogProbeLimit,
  getRequestForwardLogTargetCount,
  isRequestForwardRuleReadonly,
  toRequestForwardRuleWriteInput,
  validateRequestForwardRuleForm,
} from "../utils/requestForward";
import RequestForwardRuleFormEditor from "./request-forward/RequestForwardRuleForm.vue";
import RequestForwardLogList from "./request-forward/RequestForwardLogList.vue";
import RequestForwardRuleList from "./request-forward/RequestForwardRuleList.vue";

type RuleListEnvelope = { items: RequestForwardRule[] };
type StatusListEnvelope = { items: RequestForwardRuntimeStatus[] };
type RuleEnvelope = { item: RequestForwardRule };
type StatusEnvelope = { item: RequestForwardRuntimeStatus };
type BatchEnvelope = { results: RequestForwardBatchOperationResult[] };
type StatsEnvelope = { item: RequestForwardStats };
type WorkbenchTab = "config" | "observability";
type LogQueryContext = {
  ruleId: number;
  intentToken: number;
  keyword: string;
  mode: "all" | RequestForwardLogOutcome;
};

const LOG_PAGE_SIZE = 30;

const { loading, invoke } = useToolInvoke();
const rules = ref<RequestForwardRule[]>([]);
const statuses = ref<RequestForwardRuntimeStatus[]>([]);
const selectedId = ref<number | null>(null);
const draft = ref(false);
const activeWorkbenchTab = ref<WorkbenchTab>("config");
const form = ref<RequestForwardRuleForm>(getDefaultRequestForwardForm());
const formDirty = ref(false);
const fieldErrors = ref<Partial<Record<keyof RequestForwardRuleForm, string>>>({});
const saving = ref(false);
const operating = ref(false);
const stats = ref<RequestForwardStats | null>(null);
const statsLoading = ref(false);
const statsError = ref("");
const logItems = ref<RequestForwardLogRow[]>([]);
const logTotal = ref(0);
const logKeyword = ref("");
const logMode = ref<"all" | RequestForwardLogOutcome>("all");
const logsLoading = ref(false);
const loadingMore = ref(false);
const logError = ref("");
const logRefreshError = ref("");
const observabilityMutating = ref(false);

let refreshRequestToken = 0;
let selectionIntentToken = 0;
let pollTimer: ReturnType<typeof setTimeout> | undefined;
let pollGeneration = 0;
let pollInFlight = false;
let statsRequestToken = 0;
let logRequestToken = 0;
let logInFlight = false;
let logDebounceTimer: ReturnType<typeof setTimeout> | undefined;
let pendingLogRefresh: LogQueryContext | null = null;

const selectedRule = computed(
  () => rules.value.find((rule) => rule.id === selectedId.value) ?? null,
);
const selectedStatus = computed<RequestForwardRuntimeStatus | null>(
  () => statuses.value.find((status) => status.ruleId === selectedId.value) ?? null,
);
const selectedState = computed<RequestForwardRuntimeState>(
  () => selectedStatus.value?.state ?? "stopped",
);
const readonly = computed(
  () => Boolean(selectedRule.value) && isRequestForwardRuleReadonly(selectedState.value),
);
const interactionBusy = computed(
  () => operating.value || saving.value || observabilityMutating.value,
);
const hasActiveRuntimeRule = computed(() =>
  statuses.value.some((status) => isRequestForwardRuleReadonly(status.state)),
);
const hasEditor = computed(() => draft.value || Boolean(selectedRule.value));
const hasMoreLogs = computed(() => logItems.value.length < logTotal.value);
const eventLabel = computed(() => {
  if (selectedRule.value?.protocol === "tcp") return "连接数";
  if (selectedRule.value?.protocol === "udp") return "数据报数";
  return "请求数";
});

const stateCopy = computed(() => {
  const labels: Record<RequestForwardRuntimeState, string> = {
    stopped: "已停止",
    starting: "启动中",
    running: "运行中",
    stopping: "停止中",
    failed: "运行失败",
  };
  return labels[selectedState.value];
});

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function currentSelectionIntent() {
  return {
    selectionToken: selectionIntentToken,
    selectedId: selectedId.value,
    draft: draft.value,
  };
}

function syncFormFromSelection() {
  if (!draft.value && !formDirty.value && selectedRule.value) {
    form.value = { ...selectedRule.value };
  }
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function clearLogDebounce() {
  if (logDebounceTimer) clearTimeout(logDebounceTimer);
  logDebounceTimer = undefined;
}

function captureLogQueryContext(
  ruleId = selectedId.value,
  intentToken = selectionIntentToken,
): LogQueryContext | null {
  if (ruleId == null || draft.value) return null;
  return {
    ruleId,
    intentToken,
    keyword: logKeyword.value.trim(),
    mode: logMode.value,
  };
}

function isLogQueryContextCurrent(context: LogQueryContext): boolean {
  return (
    selectionIntentToken === context.intentToken &&
    selectedId.value === context.ruleId &&
    !draft.value &&
    logKeyword.value.trim() === context.keyword &&
    logMode.value === context.mode
  );
}

function queryLogs(
  context: LogQueryContext,
  offset: number,
  limit: number,
): Promise<RequestForwardLogPage> {
  return invoke<RequestForwardLogPage>("tool:request-forward:log-list", {
    id: context.ruleId,
    keyword: context.keyword || null,
    mode: context.mode === "all" ? null : context.mode,
    offset,
    limit,
  });
}

function resetObservabilityState() {
  statsRequestToken += 1;
  logRequestToken += 1;
  clearLogDebounce();
  stats.value = null;
  statsLoading.value = false;
  statsError.value = "";
  logItems.value = [];
  logTotal.value = 0;
  logsLoading.value = false;
  loadingMore.value = false;
  logError.value = "";
  logRefreshError.value = "";
  logInFlight = false;
  pendingLogRefresh = null;
}

async function loadStats(
  ruleId = selectedId.value,
  intentToken = selectionIntentToken,
) {
  if (ruleId == null || draft.value || observabilityMutating.value) return;
  const requestToken = ++statsRequestToken;
  statsLoading.value = true;
  statsError.value = "";
  try {
    const result = await invoke<StatsEnvelope>("tool:request-forward:stats-get", { id: ruleId });
    if (
      requestToken !== statsRequestToken ||
      selectionIntentToken !== intentToken ||
      selectedId.value !== ruleId ||
      draft.value
    ) return;
    stats.value = result.item;
  } catch (error) {
    if (requestToken === statsRequestToken && selectedId.value === ruleId) {
      statsError.value = `加载统计失败：${errorMessage(error)}`;
    }
  } finally {
    if (requestToken === statsRequestToken) statsLoading.value = false;
  }
}

async function loadLogs(
  append = false,
  ruleId = selectedId.value,
  intentToken = selectionIntentToken,
) {
  const context = captureLogQueryContext(ruleId, intentToken);
  if (!context || observabilityMutating.value) return;
  if (append && (loadingMore.value || logInFlight)) return;
  const requestToken = ++logRequestToken;
  const offset = append ? logItems.value.length : 0;
  logInFlight = true;
  append ? (loadingMore.value = true) : (logsLoading.value = true);
  if (!append) {
    logItems.value = [];
    logTotal.value = 0;
  }
  logError.value = "";
  logRefreshError.value = "";
  try {
    const result = await queryLogs(context, offset, LOG_PAGE_SIZE);
    if (requestToken !== logRequestToken || !isLogQueryContextCurrent(context)) return;
    const knownIds = new Set(logItems.value.map((item) => item.id));
    logItems.value = append
      ? [...logItems.value, ...result.items.filter((item) => !knownIds.has(item.id))]
      : result.items;
    logTotal.value = result.total;
    logRefreshError.value = "";
  } catch (error) {
    if (requestToken === logRequestToken && isLogQueryContextCurrent(context)) {
      logError.value = `加载日志失败：${errorMessage(error)}`;
    }
  } finally {
    if (requestToken === logRequestToken) {
      logInFlight = false;
      logsLoading.value = false;
      loadingMore.value = false;
    }
    flushPendingLogRefresh();
  }
}

async function refreshLogsInBackground(
  context = captureLogQueryContext(),
): Promise<void> {
  if (
    !context ||
    activeWorkbenchTab.value !== "observability" ||
    !isLogQueryContextCurrent(context)
  ) return;
  if (observabilityMutating.value || logInFlight || loadingMore.value || logDebounceTimer) {
    pendingLogRefresh = context;
    return;
  }

  pendingLogRefresh = null;
  const requestToken = ++logRequestToken;
  const loadedCount = logItems.value.length;
  const previousTotal = logTotal.value;
  logInFlight = true;
  try {
    const probe = await queryLogs(
      context,
      0,
      getRequestForwardLogProbeLimit(loadedCount),
    );
    if (requestToken !== logRequestToken || !isLogQueryContextCurrent(context)) return;

    const targetCount = getRequestForwardLogTargetCount({
      loadedCount,
      previousTotal,
      nextTotal: probe.total,
    });
    const page = probe.items.length >= targetCount
      ? { ...probe, items: probe.items.slice(0, targetCount) }
      : await queryLogs(context, 0, targetCount);
    if (requestToken !== logRequestToken || !isLogQueryContextCurrent(context)) return;

    logItems.value = page.items;
    logTotal.value = page.total;
    logRefreshError.value = "";
  } catch (error) {
    if (requestToken === logRequestToken && isLogQueryContextCurrent(context)) {
      logRefreshError.value = `日志自动刷新失败：${errorMessage(error)}`;
    }
  } finally {
    if (requestToken === logRequestToken) logInFlight = false;
    flushPendingLogRefresh();
  }
}

function flushPendingLogRefresh() {
  const pending = pendingLogRefresh;
  if (!pending) return;
  if (
    activeWorkbenchTab.value !== "observability" ||
    !isLogQueryContextCurrent(pending)
  ) {
    pendingLogRefresh = null;
    return;
  }
  if (observabilityMutating.value || logInFlight || loadingMore.value || logDebounceTimer) return;
  pendingLogRefresh = null;
  void refreshLogsInBackground(pending);
}

function scheduleLogReload() {
  clearLogDebounce();
  logRequestToken += 1;
  logItems.value = [];
  logTotal.value = 0;
  logDebounceTimer = setTimeout(() => {
    logDebounceTimer = undefined;
    void loadLogs(false);
  }, 300);
}

function loadMoreLogs() {
  if (loadingMore.value || logInFlight) return;
  void loadLogs(true);
}

function reloadCurrentObservability() {
  const intentToken = selectionIntentToken;
  const ruleId = selectedId.value;
  if (draft.value || ruleId == null || selectedRule.value?.id !== ruleId) return;
  void Promise.all([
    loadStats(ruleId, intentToken),
    loadLogs(false, ruleId, intentToken),
  ]);
}

async function refreshRules(options: { showLoading?: boolean } = {}) {
  const requestToken = ++refreshRequestToken;
  const intentToken = selectionIntentToken;
  const previousSelectedId = selectedId.value;
  try {
    const request = Promise.all([
      invoke<RuleListEnvelope>("tool:request-forward:list", {}),
      invoke<StatusListEnvelope>("tool:request-forward:status", {}),
    ]);
    if (options.showLoading) loading.value = true;
    const [ruleResult, statusResult] = await request;
    if (requestToken !== refreshRequestToken) return;
    rules.value = ruleResult.items;
    statuses.value = statusResult.items;
    if (selectionIntentToken !== intentToken || draft.value) return;
    const retained = rules.value.some((rule) => rule.id === selectedId.value);
    const removedSelectedRule = selectedId.value != null && !retained;
    selectedId.value = retained ? selectedId.value : rules.value[0]?.id ?? null;
    if (removedSelectedRule) {
      formDirty.value = false;
      fieldErrors.value = {};
      ElMessage.warning("当前编辑的规则已被删除，已切换到可用规则");
    }
    syncFormFromSelection();
    if (selectedId.value != null && selectedId.value === previousSelectedId) {
      await loadStats(selectedId.value);
    }
  } catch (error) {
    if (options.showLoading) ElMessage.error(`加载转发规则失败：${errorMessage(error)}`);
  } finally {
    if (options.showLoading && requestToken === refreshRequestToken) loading.value = false;
  }
}

function selectRule(id: number) {
  if (interactionBusy.value) return;
  selectionIntentToken += 1;
  draft.value = false;
  selectedId.value = id;
  formDirty.value = false;
  fieldErrors.value = {};
  syncFormFromSelection();
}

function createDraft() {
  if (interactionBusy.value) return;
  selectionIntentToken += 1;
  draft.value = true;
  selectedId.value = null;
  activeWorkbenchTab.value = "config";
  form.value = getDefaultRequestForwardForm();
  formDirty.value = false;
  fieldErrors.value = {};
}

function handleFormUpdate(value: RequestForwardRuleForm) {
  if (interactionBusy.value || readonly.value) return;
  form.value = value;
  formDirty.value = true;
}

function validateForm(): boolean {
  const labels: Partial<Record<keyof RequestForwardRuleForm, string>> = {
    name: "请输入规则名称",
    bindHost: "请输入有效的 IPv4 或 IPv6 地址",
    listenPort: "监听端口必须为 1 到 65535 的整数",
    targetUrl: "请输入不含查询参数和片段的 HTTP/HTTPS 目标地址",
    targetHost: "请输入目标主机",
    targetPort: "目标端口必须为 1 到 65535 的整数",
  };
  const invalidFields = validateRequestForwardRuleForm(form.value);
  fieldErrors.value = Object.fromEntries(
    invalidFields.map((field) => [field, labels[field as keyof RequestForwardRuleForm] ?? "字段无效"]),
  );
  if (invalidFields.length) {
    ElMessage.error("请修正表单中的错误后再保存");
    return false;
  }
  return true;
}

async function saveRule(): Promise<RequestForwardRule | null> {
  if (interactionBusy.value) return null;
  if (readonly.value) {
    ElMessage.warning("运行中的规则不能修改，请先停止规则");
    return null;
  }
  if (!validateForm()) return null;
  const isDraft = draft.value;
  const targetId = isDraft ? null : selectedRule.value?.id ?? null;
  if (!isDraft && targetId == null) return null;
  const intent = captureRequestForwardMutationIntent(currentSelectionIntent(), targetId);
  const payload = toRequestForwardRuleWriteInput(form.value);
  saving.value = true;
  try {
    const operation = isDraft
      ? invoke<RuleEnvelope>("tool:request-forward:create", payload)
      : invoke<RuleEnvelope>("tool:request-forward:update", { id: targetId, ...payload });
    const { value: result } = await applyRequestForwardMutationResult(
      operation,
      intent,
      currentSelectionIntent,
      (completed) => {
        draft.value = false;
        selectedId.value = completed.item.id;
        selectionIntentToken += 1;
        form.value = { ...completed.item };
        formDirty.value = false;
      },
    );
    await refreshRules();
    ElMessage.success("规则已保存");
    return result.item;
  } catch (error) {
    ElMessage.error(`保存规则失败：${errorMessage(error)}`);
    return null;
  } finally {
    saving.value = false;
  }
}

async function saveAndStart() {
  const saved = await saveRule();
  if (!saved) return;
  try {
    await startRule(saved.id, false);
    ElMessage.success("规则已保存并启动");
  } catch (error) {
    ElMessage.error(`规则已保存，但启动失败：${errorMessage(error)}`);
  }
}

async function startRule(id: number, feedback = true) {
  if (interactionBusy.value) return;
  const intent = captureRequestForwardMutationIntent(currentSelectionIntent(), id);
  operating.value = true;
  try {
    await applyRequestForwardMutationResult(
      invoke<StatusEnvelope>("tool:request-forward:start", { id: intent.targetId }),
      intent,
      currentSelectionIntent,
      (result) => {
        statuses.value = upsertStatus(statuses.value, result.item);
      },
    );
    await refreshRules();
    if (feedback) ElMessage.success("规则已启动");
  } catch (error) {
    if (feedback) ElMessage.error(`启动规则失败：${errorMessage(error)}`);
    if (!feedback) throw error;
  } finally {
    operating.value = false;
  }
}

async function stopRule(id: number, feedback = true) {
  if (interactionBusy.value) return;
  const intent = captureRequestForwardMutationIntent(currentSelectionIntent(), id);
  operating.value = true;
  try {
    await applyRequestForwardMutationResult(
      invoke<StatusEnvelope>("tool:request-forward:stop", { id: intent.targetId }),
      intent,
      currentSelectionIntent,
      (result) => {
        statuses.value = upsertStatus(statuses.value, result.item);
      },
    );
    await refreshRules();
    if (feedback) ElMessage.success("规则已停止");
  } catch (error) {
    if (feedback) ElMessage.error(`停止规则失败：${errorMessage(error)}`);
    if (!feedback) throw error;
  } finally {
    operating.value = false;
  }
}

async function handleStopAndEdit() {
  if (!selectedRule.value) return;
  try {
    await stopRule(selectedRule.value.id, false);
    ElMessage.success("规则已停止，可以编辑");
  } catch (error) {
    ElMessage.error(`停止失败，规则仍保持只读：${errorMessage(error)}`);
  }
}

async function runBatch(operation: "start" | "stop") {
  operating.value = true;
  try {
    const channel = `tool:request-forward:${operation}-all`;
    const result = await invoke<BatchEnvelope>(channel, {});
    const summary = {
      requested: result.results.length,
      succeeded: result.results.filter((item) => item.ok).length,
      failed: result.results.filter((item) => !item.ok).length,
    };
    const message = getRequestForwardBatchMessage(operation, summary);
    summary.failed ? ElMessage.warning(message) : ElMessage.success(message);
    await refreshRules();
  } catch (error) {
    ElMessage.error(`${operation === "start" ? "全部启动" : "全部停止"}失败：${errorMessage(error)}`);
  } finally {
    operating.value = false;
  }
}

async function deleteSelected() {
  const rule = selectedRule.value;
  if (!rule || interactionBusy.value) return;
  const intent = captureRequestForwardMutationIntent(currentSelectionIntent(), rule.id);
  if (isRequestForwardRuleReadonly(selectedState.value)) {
    ElMessage.warning("运行中的规则不能删除，请先停止规则");
    return;
  }
  try {
    await ElMessageBox.confirm(
      `确定删除规则“${rule.name}”吗？删除后无法恢复。`,
      "删除转发规则",
      { type: "warning", confirmButtonText: "删除", cancelButtonText: "取消" },
    );
  } catch {
    return;
  }
  operating.value = true;
  try {
    await applyRequestForwardMutationResult(
      invoke<{ ok: boolean }>("tool:request-forward:delete", { id: intent.targetId }),
      intent,
      currentSelectionIntent,
      () => {
        selectionIntentToken += 1;
        selectedId.value = null;
        formDirty.value = false;
      },
    );
    await refreshRules();
    ElMessage.success("规则已删除");
  } catch (error) {
    ElMessage.error(`删除规则失败：${errorMessage(error)}`);
  } finally {
    operating.value = false;
  }
}

async function clearLogs() {
  const rule = selectedRule.value;
  if (!rule || observabilityMutating.value) return;
  if (pollInFlight) {
    ElMessage.warning("状态刷新中，请稍后再清空日志");
    return;
  }
  try {
    await ElMessageBox.confirm(
      `确定清空规则“${rule.name}”的全部转发日志吗？统计数据不会被重置。`,
      "清空转发日志",
      {
        type: "warning",
        confirmButtonText: "清空日志",
        cancelButtonText: "取消",
        customClass: "request-forward-observability-confirm",
      },
    );
  } catch {
    return;
  }
  const intentToken = selectionIntentToken;
  observabilityMutating.value = true;
  logRequestToken += 1;
  pendingLogRefresh = null;
  try {
    await invoke<{ ok: boolean }>("tool:request-forward:log-clear", { id: rule.id });
    if (selectionIntentToken !== intentToken || selectedId.value !== rule.id || draft.value) return;
    ElMessage.success("转发日志已清空");
  } catch (error) {
    ElMessage.error(`清空日志失败：${errorMessage(error)}`);
  } finally {
    observabilityMutating.value = false;
    reloadCurrentObservability();
    if (hasActiveRuntimeRule.value) syncPolling(true);
  }
}

async function resetStats() {
  const rule = selectedRule.value;
  if (!rule || observabilityMutating.value) return;
  if (pollInFlight) {
    ElMessage.warning("状态刷新中，请稍后再重置统计");
    return;
  }
  try {
    await ElMessageBox.confirm(
      `确定重置规则“${rule.name}”的转发统计吗？历史日志不会被清空。`,
      "重置转发统计",
      {
        type: "warning",
        confirmButtonText: "重置统计",
        cancelButtonText: "取消",
        customClass: "request-forward-observability-confirm",
      },
    );
  } catch {
    return;
  }
  const intentToken = selectionIntentToken;
  observabilityMutating.value = true;
  statsRequestToken += 1;
  try {
    const result = await invoke<StatsEnvelope>("tool:request-forward:stats-reset", { id: rule.id });
    if (selectionIntentToken !== intentToken || selectedId.value !== rule.id || draft.value) return;
    stats.value = result.item;
    statsError.value = "";
    ElMessage.success("转发统计已重置");
  } catch (error) {
    ElMessage.error(`重置统计失败：${errorMessage(error)}`);
  } finally {
    observabilityMutating.value = false;
    reloadCurrentObservability();
    if (hasActiveRuntimeRule.value) syncPolling(true);
  }
}

function upsertStatus(
  current: RequestForwardRuntimeStatus[],
  next: RequestForwardRuntimeStatus,
): RequestForwardRuntimeStatus[] {
  return [...current.filter((item) => item.ruleId !== next.ruleId), next];
}

function clearPolling() {
  pollGeneration += 1;
  if (pollTimer) clearTimeout(pollTimer);
  pollTimer = undefined;
}

function schedulePolling(generation: number) {
  pollTimer = setTimeout(() => void runPoll(generation), 2_000);
}

async function runPoll(generation: number) {
  if (
    generation !== pollGeneration ||
    pollInFlight ||
    observabilityMutating.value ||
    !hasActiveRuntimeRule.value
  ) {
    return;
  }
  pollTimer = undefined;
  pollInFlight = true;
  try {
    await refreshRules();
    await refreshLogsInBackground();
  } finally {
    pollInFlight = false;
    if (generation === pollGeneration && hasActiveRuntimeRule.value) {
      schedulePolling(generation);
    }
  }
}

function syncPolling(active: boolean) {
  clearPolling();
  if (active) schedulePolling(pollGeneration);
}

watch([selectedId, draft], ([ruleId, isDraft]) => {
  resetObservabilityState();
  if (ruleId != null && !isDraft && activeWorkbenchTab.value === "observability") {
    void Promise.all([loadStats(ruleId), loadLogs(false, ruleId)]);
  }
});
watch(activeWorkbenchTab, (tab) => {
  if (tab === "observability") {
    reloadCurrentObservability();
  } else {
    pendingLogRefresh = null;
  }
});
watch(logKeyword, scheduleLogReload);
watch(logMode, () => {
  clearLogDebounce();
  logRequestToken += 1;
  logItems.value = [];
  logTotal.value = 0;
  void loadLogs(false);
});
watch(hasActiveRuntimeRule, syncPolling, { immediate: true });
onMounted(() => void refreshRules({ showLoading: true }));
onUnmounted(() => {
  refreshRequestToken += 1;
  statsRequestToken += 1;
  logRequestToken += 1;
  pendingLogRefresh = null;
  clearLogDebounce();
  clearPolling();
});
</script>

<template>
  <div class="request-forward-panel">
    <RequestForwardRuleList
      :rules="rules"
      :statuses="statuses"
      :selected-id="selectedId"
      :loading="loading"
      :busy="interactionBusy"
      @add="createDraft"
      @select="selectRule"
      @start="startRule"
      @stop="stopRule"
      @start-all="runBatch('start')"
      @stop-all="runBatch('stop')"
    />

    <main class="rule-workbench">
      <template v-if="hasEditor">
        <header class="workbench-header">
          <div>
            <p class="workbench-header__eyebrow">{{ draft ? "NEW RULE" : `RULE #${selectedRule?.id}` }}</p>
            <h1>{{ draft ? "新建转发规则" : selectedRule?.name }}</h1>
            <p>{{ draft ? "新规则默认保持停止，保存后可按需启动。" : "配置本地监听端点与转发目标。" }}</p>
          </div>
          <div v-if="!draft" class="runtime-state" :class="`is-${selectedState}`">
            <span>{{ stateCopy }}</span>
            <small v-if="selectedStatus?.lastError">{{ selectedStatus.lastError }}</small>
          </div>
        </header>

        <el-tabs
          v-model="activeWorkbenchTab"
          class="workbench-tabs"
          :class="{ 'is-draft': draft }"
        >
          <el-tab-pane label="规则配置" name="config">
            <div class="workbench-pane">
              <div v-if="readonly" class="readonly-banner" role="status">
                <div>
                  <strong>规则正在运行，配置已锁定</strong>
                  <span>停止成功后才会解除只读，避免运行配置与持久化配置不一致。</span>
                </div>
                <el-button
                  :disabled="interactionBusy"
                  :loading="operating"
                  @click="handleStopAndEdit"
                >停止并编辑</el-button>
              </div>

              <div class="workbench-scroll">
                <RequestForwardRuleFormEditor
                  :model-value="form"
                  :readonly="readonly"
                  :disabled="interactionBusy"
                  :persisted="!draft"
                  :errors="fieldErrors"
                  @update:model-value="handleFormUpdate"
                />
              </div>

              <footer class="workbench-actions">
                <el-button
                  v-if="!draft"
                  type="danger"
                  plain
                  :disabled="readonly || interactionBusy"
                  @click="deleteSelected"
                >
                  删除规则
                </el-button>
                <span class="workbench-actions__spacer" />
                <el-button
                  :disabled="readonly || interactionBusy"
                  :loading="saving"
                  @click="saveRule"
                >
                  仅保存
                </el-button>
                <el-button
                  type="primary"
                  :disabled="readonly || interactionBusy"
                  :loading="saving"
                  @click="saveAndStart"
                >
                  保存并启动
                </el-button>
              </footer>
            </div>
          </el-tab-pane>

          <el-tab-pane v-if="!draft" label="运行观测" name="observability">
            <div class="workbench-pane">
              <div class="workbench-scroll">
                <section class="observability" aria-labelledby="observability-title">
                  <div
                    v-if="selectedStatus?.lastObservabilityError"
                    class="observability-warning"
                    role="status"
                  >
                    <strong>观测数据暂不可用</strong>
                    <span>{{ selectedStatus.lastObservabilityError }}</span>
                  </div>

                  <header class="section-header">
                    <div>
                      <p class="section-header__eyebrow">OBSERVABILITY</p>
                      <h2 id="observability-title">转发统计</h2>
                    </div>
                    <el-button
                      size="small"
                      :disabled="statsLoading || observabilityMutating"
                      :loading="observabilityMutating"
                      @click="resetStats"
                    >
                      重置统计
                    </el-button>
                  </header>

                  <div v-if="statsError" class="stats-error" role="alert">
                    <span>{{ statsError }}</span>
                    <el-button size="small" @click="loadStats()">重新加载</el-button>
                  </div>
                  <div v-else class="stats-grid" :aria-busy="statsLoading">
                    <article class="stat-card">
                      <span>{{ eventLabel }}</span>
                      <strong>{{ stats?.eventCount ?? (statsLoading ? "…" : 0) }}</strong>
                    </article>
                    <article class="stat-card">
                      <span>上传</span>
                      <strong>{{ stats ? formatBytes(stats.uploadBytes) : statsLoading ? "…" : "0 B" }}</strong>
                    </article>
                    <article class="stat-card">
                      <span>下载</span>
                      <strong>{{ stats ? formatBytes(stats.downloadBytes) : statsLoading ? "…" : "0 B" }}</strong>
                    </article>
                    <article class="stat-card is-error">
                      <span>错误数</span>
                      <strong>{{ stats?.errorCount ?? (statsLoading ? "…" : 0) }}</strong>
                    </article>
                  </div>

                  <header class="section-header log-header">
                    <div>
                      <p class="section-header__eyebrow">RECENT ACTIVITY</p>
                      <h2>转发日志</h2>
                    </div>
                    <el-button
                      size="small"
                      :disabled="!selectedRule || logsLoading || observabilityMutating"
                      :loading="observabilityMutating"
                      @click="clearLogs"
                    >
                      清空全部日志
                    </el-button>
                  </header>

                  <div class="log-toolbar">
                    <label class="log-search">
                      <span>日志关键字</span>
                      <el-input
                        v-model="logKeyword"
                        clearable
                        placeholder="客户端、目标、路径或错误信息"
                      />
                    </label>
                    <div class="log-mode" aria-label="日志结果筛选">
                      <button
                        type="button"
                        aria-label="全部"
                        :class="{ 'is-active': logMode === 'all' }"
                        @click="logMode = 'all'"
                      >全部</button>
                      <button
                        type="button"
                        aria-label="成功"
                        :class="{ 'is-active': logMode === 'success' }"
                        @click="logMode = 'success'"
                      >成功</button>
                      <button
                        type="button"
                        aria-label="失败"
                        :class="{ 'is-active': logMode === 'error' }"
                        @click="logMode = 'error'"
                      >失败</button>
                    </div>
                  </div>

                  <div v-if="logRefreshError" class="log-refresh-warning" role="status">
                    <span>{{ logRefreshError }}</span>
                    <el-button size="small" @click="refreshLogsInBackground()">重试</el-button>
                  </div>

                  <RequestForwardLogList
                    :items="logItems"
                    :loading="logsLoading"
                    :loading-more="loadingMore"
                    :error="logError"
                    :has-more="hasMoreLogs"
                    @retry="loadLogs(false)"
                    @load-more="loadMoreLogs"
                  />
                </section>
              </div>
            </div>
          </el-tab-pane>
        </el-tabs>
      </template>

      <div v-else class="workbench-empty">
        <div class="workbench-empty__mark">RF</div>
        <h1>选择或新建转发规则</h1>
        <p>在左侧选择已有规则查看配置，或新建 HTTP、TCP、UDP 转发规则。</p>
        <el-button type="primary" :disabled="interactionBusy" @click="createDraft">
          新建规则
        </el-button>
      </div>
    </main>
  </div>
</template>

<style scoped>
.request-forward-panel {
  display: grid;
  grid-template-columns: minmax(280px, 34%) minmax(0, 1fr);
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: #fff;
}

.rule-workbench {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  background: #fdfdfd;
}

.workbench-tabs {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex: 1;
  flex-direction: column;
}

.workbench-tabs :deep(.el-tabs__header) {
  flex: none;
  margin: 0;
  padding: 0 20px;
  background: #fff;
}

.workbench-tabs :deep(.el-tabs__item) {
  height: 38px;
  padding-inline: 14px;
  font-size: 13px;
}

.workbench-tabs :deep(.el-tabs__content) {
  min-height: 0;
  flex: 1;
}

.workbench-tabs :deep(.el-tab-pane) {
  height: 100%;
  min-height: 0;
}

.workbench-tabs.is-draft :deep(.el-tabs__header) { display: none; }

.workbench-pane {
  display: flex;
  height: 100%;
  min-height: 0;
  flex-direction: column;
}

.workbench-header {
  display: flex;
  flex: none;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
  padding: 22px 26px 18px;
  border-bottom: 1px solid #e4e7eb;
}

.workbench-header h1,
.workbench-empty h1 {
  margin: 3px 0 5px;
  color: var(--text-primary, #1f2937);
  font-size: 20px;
}

.workbench-header p,
.workbench-empty p {
  margin: 0;
  color: var(--text-secondary, #64748b);
  font-size: 13px;
  line-height: 1.55;
}

.workbench-header__eyebrow {
  color: var(--el-color-primary, #409eff) !important;
  font-size: 10px !important;
  font-weight: 800;
  letter-spacing: 0.12em;
}

.runtime-state {
  display: grid;
  max-width: 300px;
  justify-items: end;
  gap: 4px;
  color: #6b7280;
  text-align: right;
}

.runtime-state span { font-size: 13px; font-weight: 700; }
.runtime-state span::before { content: "●"; margin-right: 6px; font-size: 9px; }
.runtime-state small { color: #c23b35; line-height: 1.45; }
.runtime-state.is-running { color: #168357; }
.runtime-state.is-starting,
.runtime-state.is-stopping { color: #a86608; }
.runtime-state.is-failed { color: #c23b35; }

.readonly-banner {
  display: flex;
  flex: none;
  align-items: center;
  gap: 14px;
  margin: 12px 20px 0;
  padding: 9px 10px;
  border: 1px solid #ecd6a9;
  border-radius: 6px;
  background: #fffaf0;
}

.readonly-banner > div { display: grid; min-width: 0; gap: 3px; margin-right: auto; }
.readonly-banner strong { color: #65450d; font-size: 13px; }
.readonly-banner span { color: #85672f; font-size: 12px; line-height: 1.45; }

.workbench-scroll {
  min-width: 0;
  min-height: 0;
  flex: 1;
  overflow-y: auto;
  padding: 14px 20px 18px;
}

.observability { min-width: 0; }
.observability-warning {
  display: grid;
  gap: 3px;
  margin-bottom: 10px;
  padding: 8px 10px;
  border: 1px solid #ecd6a9;
  border-radius: 6px;
  background: #fffaf0;
}
.observability-warning strong { color: #65450d; font-size: 12px; }
.observability-warning span { color: #85672f; font-size: 12px; line-height: 1.45; overflow-wrap: anywhere; }
.section-header { display: flex; align-items: center; justify-content: space-between; gap: 14px; margin-bottom: 8px; }
.section-header h2 { margin: 2px 0 0; color: var(--text-primary, #1f2937); font-size: 15px; }
.section-header__eyebrow { margin: 0; color: #778494; font-size: 9px; font-weight: 800; letter-spacing: .12em; }
.stats-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 7px; }
.stat-card { display: grid; gap: 3px; min-width: 0; padding: 9px 10px; border: 1px solid #e2e6eb; border-radius: 6px; background: #fff; }
.stat-card span { color: #7b8795; font-size: 11px; }
.stat-card strong { overflow: hidden; color: #26364a; font-size: 16px; text-overflow: ellipsis; white-space: nowrap; }
.stat-card.is-error strong { color: #aa3933; }
.stats-error { display: flex; min-height: 64px; align-items: center; justify-content: center; gap: 10px; border: 1px solid #efc8c5; border-radius: 6px; background: #fff8f7; color: #a9332d; font-size: 12px; }
.log-header { margin-top: 16px; }
.log-toolbar { display: flex; align-items: flex-end; gap: 10px; margin-bottom: 10px; }
.log-search { display: grid; min-width: 220px; flex: 1; gap: 4px; }
.log-search > span { color: #657386; font-size: 11px; font-weight: 600; }
.log-mode { display: inline-flex; flex: none; padding: 2px; border: 1px solid #d7dce3; border-radius: 6px; background: #f4f6f8; }
.log-mode button { min-height: 26px; border: 0; border-radius: 4px; padding: 0 10px; background: transparent; color: #637083; cursor: pointer; font: inherit; font-size: 12px; }
.log-mode button:hover { color: #2f5f86; }
.log-mode button.is-active { background: #fff; color: #245b83; box-shadow: 0 1px 2px rgb(31 41 55 / 12%); font-weight: 700; }
.log-mode button:focus-visible { outline: 2px solid var(--el-color-primary, #409eff); outline-offset: 2px; }

.log-refresh-warning {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 8px;
  padding: 7px 9px;
  border: 1px solid #ecd6a9;
  border-radius: 6px;
  background: #fffaf0;
  color: #85672f;
  font-size: 12px;
}

.workbench-actions {
  display: flex;
  flex: none;
  gap: 8px;
  padding: 12px 20px;
  border-top: 1px solid #e4e7eb;
  background: #fff;
}

.workbench-actions__spacer { flex: 1; }

.workbench-empty {
  display: grid;
  max-width: 440px;
  margin: auto;
  justify-items: center;
  padding: 30px;
  text-align: center;
}

.workbench-empty__mark {
  display: grid;
  width: 54px;
  height: 54px;
  margin-bottom: 12px;
  place-items: center;
  border: 1px solid #cfd6df;
  border-radius: 8px;
  background: #f6f8fa;
  color: #4c5d72;
  font-size: 15px;
  font-weight: 800;
  letter-spacing: 0.08em;
}

.workbench-empty .el-button { margin-top: 18px; }

@media (max-width: 780px) {
  .request-forward-panel {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr);
    overflow: hidden;
  }
  .workbench-header { padding: 18px; }
  .workbench-tabs :deep(.el-tabs__header) { padding-inline: 16px; }
  .readonly-banner { margin-inline: 16px; align-items: flex-start; }
  .workbench-scroll { padding-inline: 16px; }
  .workbench-actions { padding-inline: 16px; flex-wrap: wrap; }
  .stats-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .log-toolbar { align-items: stretch; flex-direction: column; }
  .log-search { min-width: 0; }
  .log-mode { align-self: flex-start; }
}

@media (prefers-reduced-motion: reduce) {
  :deep(*) { scroll-behavior: auto !important; transition-duration: 0.01ms !important; }
}
</style>
