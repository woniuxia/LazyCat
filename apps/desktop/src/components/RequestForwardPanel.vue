<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useRequestForwardPreflight } from "../composables/useRequestForwardPreflight";
import { getSetting, setSetting } from "../composables/useSettings";
import { useToolInvoke } from "../composables/useToolInvoke";
import type {
  RequestForwardBatchOperationResult,
  RequestForwardLogOutcome,
  RequestForwardLogPage,
  RequestForwardLogRow,
  RequestForwardPreflightResult,
  RequestForwardRule,
  RequestForwardRuleForm,
  RequestForwardRuntimeState,
  RequestForwardRuntimeStatus,
  RequestForwardStats,
} from "../types/request-forward";
import {
  applyRequestForwardMutationResult,
  clampRequestForwardInspectorWidth,
  clampRequestForwardRuleListWidth,
  captureRequestForwardMutationIntent,
  DEFAULT_REQUEST_FORWARD_INSPECTOR_WIDTH,
  DEFAULT_REQUEST_FORWARD_RULE_LIST_WIDTH,
  duplicateRequestForwardRuleForm,
  formatRequestForwardEndpoint,
  getDefaultRequestForwardForm,
  getRequestForwardBatchMessage,
  getRequestForwardCommandExamples,
  getRequestForwardLocalEndpoint,
  getRequestForwardLocalUrl,
  getRequestForwardLogProbeLimit,
  getRequestForwardLogTargetCount,
  getRequestForwardErrorSummary,
  getRequestForwardRecoveryActions,
  isRequestForwardRuleReadonly,
  MIN_REQUEST_FORWARD_INSPECTOR_WIDTH,
  MIN_REQUEST_FORWARD_RULE_LIST_WIDTH,
  parseRequestForwardError,
  retainRequestForwardSelectedLogId,
  toRequestForwardRuleWriteInput,
  validateRequestForwardRuleForm,
} from "../utils/requestForward";
import type { RequestForwardCommandExamples } from "../utils/requestForward";
import RequestForwardEndpointActions from "./request-forward/RequestForwardEndpointActions.vue";
import RequestForwardLogList from "./request-forward/RequestForwardLogList.vue";
import RequestForwardLogInspector from "./request-forward/RequestForwardLogInspector.vue";
import RequestForwardRuleDialog from "./request-forward/RequestForwardRuleDialog.vue";
import RequestForwardRuleList from "./request-forward/RequestForwardRuleList.vue";

type RuleListEnvelope = { items: RequestForwardRule[] };
type StatusListEnvelope = { items: RequestForwardRuntimeStatus[] };
type RuleEnvelope = { item: RequestForwardRule };
type StatusEnvelope = { item: RequestForwardRuntimeStatus };
type BatchEnvelope = { results: RequestForwardBatchOperationResult[] };
type StatsEnvelope = { item: RequestForwardStats };
type LogQueryContext = {
  ruleId: number;
  intentToken: number;
  keyword: string;
  mode: "all" | RequestForwardLogOutcome;
};

const LOG_PAGE_SIZE = 30;
const INSPECTOR_WIDTH_SETTING = "request-forward:inspector-width";
const RULE_LIST_WIDTH_SETTING = "request-forward:rule-list-width";
const RESIZER_WIDTH = 6;

const { loading, invoke } = useToolInvoke();
const rules = ref<RequestForwardRule[]>([]);
const workspaceRef = ref<HTMLElement | null>(null);
const workspaceWidth = ref(1200);
const preferredRuleListWidth = ref(
  Number(getSetting(RULE_LIST_WIDTH_SETTING)) || DEFAULT_REQUEST_FORWARD_RULE_LIST_WIDTH,
);
const preferredInspectorWidth = ref(
  Number(getSetting(INSPECTOR_WIDTH_SETTING)) || DEFAULT_REQUEST_FORWARD_INSPECTOR_WIDTH,
);
const statuses = ref<RequestForwardRuntimeStatus[]>([]);
const selectedId = ref<number | null>(null);
const editorMode = ref<"create" | "edit" | null>(null);
const editorRuleId = ref<number | null>(null);
const form = ref<RequestForwardRuleForm>(getDefaultRequestForwardForm());
const formDirty = ref(false);
const fieldErrors = ref<Partial<Record<keyof RequestForwardRuleForm, string>>>({});
const saving = ref(false);
const operating = ref(false);
const recoveryPreflightResult = ref<RequestForwardPreflightResult | null>(null);
const recoveryPreflightRuleId = ref<number | null>(null);
const recoveryPreflighting = ref(false);
const {
  result: preflightResult,
  loading: preflighting,
  run: executePreflight,
  invalidate: invalidatePreflight,
  isAcceptedCurrent: isAcceptedPreflightCurrent,
} = useRequestForwardPreflight({
  currentContext: () => ({
    intent: currentEditorIntent(),
    payload: toRequestForwardRuleWriteInput(form.value),
  }),
  execute: (payload) =>
    invoke<RequestForwardPreflightResult>("tool:request-forward:preflight", payload),
  onError: (error) => {
    ElMessage.error(`检测配置失败：${errorMessage(error)}`);
  },
});
const stats = ref<RequestForwardStats | null>(null);
const statsLoading = ref(false);
const statsError = ref("");
const logItems = ref<RequestForwardLogRow[]>([]);
const selectedLogId = ref<number | null>(null);
const logTotal = ref(0);
const logKeyword = ref("");
const logMode = ref<"all" | RequestForwardLogOutcome>("all");
const logsLoading = ref(false);
const loadingMore = ref(false);
const logError = ref("");
const logRefreshError = ref("");
const observabilityMutating = ref(false);

let refreshRequestToken = 0;
let recoveryPreflightRequestToken = 0;
let selectionIntentToken = 0;
let editorIntentToken = 0;
let pollTimer: ReturnType<typeof setTimeout> | undefined;
let pollGeneration = 0;
let pollInFlight = false;
let statsRequestToken = 0;
let logRequestToken = 0;
let logInFlight = false;
let logDebounceTimer: ReturnType<typeof setTimeout> | undefined;
let pendingLogRefresh: LogQueryContext | null = null;
let workspaceResizeObserver: ResizeObserver | null = null;
let inspectorResizeStartX = 0;
let inspectorResizeStartWidth = 0;
let inspectorResizeActive = false;
let ruleListResizeStartX = 0;
let ruleListResizeStartWidth = 0;
let ruleListResizeActive = false;

const selectedRule = computed(
  () => rules.value.find((rule) => rule.id === selectedId.value) ?? null,
);
const selectedStatus = computed<RequestForwardRuntimeStatus | null>(
  () => statuses.value.find((status) => status.ruleId === selectedId.value) ?? null,
);
const selectedState = computed<RequestForwardRuntimeState>(
  () => selectedStatus.value?.state ?? "stopped",
);
const selectedRuntimeError = computed(() => {
  const lastError = selectedStatus.value?.lastError;
  return lastError ? parseRequestForwardError(lastError, selectedState.value) : null;
});
const selectedSuggestedListenPort = computed(() => {
  if (
    selectedRuntimeError.value?.code !== "listener_in_use" ||
    recoveryPreflightRuleId.value !== selectedId.value
  ) return null;
  return recoveryPreflightResult.value?.suggestedListenPort ?? null;
});
const selectedRecoveryActions = computed(() =>
  selectedRuntimeError.value
    ? getRequestForwardRecoveryActions(
        selectedRuntimeError.value,
        selectedSuggestedListenPort.value,
      )
    : [],
);
const editorRule = computed(
  () => rules.value.find((rule) => rule.id === editorRuleId.value) ?? null,
);
const editorStatus = computed<RequestForwardRuntimeStatus | null>(
  () => statuses.value.find((status) => status.ruleId === editorRuleId.value) ?? null,
);
const readonly = computed(
  () => Boolean(editorRule.value) &&
    isRequestForwardRuleReadonly(editorStatus.value?.state ?? "stopped"),
);
const interactionBusy = computed(
  () =>
    operating.value ||
    saving.value ||
    preflighting.value ||
    recoveryPreflighting.value ||
    observabilityMutating.value,
);
const hasActiveRuntimeRule = computed(() =>
  statuses.value.some((status) => isRequestForwardRuleReadonly(status.state)),
);
const hasMoreLogs = computed(() => logItems.value.length < logTotal.value);
const selectedLog = computed(
  () => logItems.value.find((item) => item.id === selectedLogId.value) ?? null,
);
const ruleListWidth = computed(() =>
  clampRequestForwardRuleListWidth(
    preferredRuleListWidth.value,
    workspaceWidth.value,
  ),
);
const ruleListMaximum = computed(() =>
  clampRequestForwardRuleListWidth(Number.MAX_SAFE_INTEGER, workspaceWidth.value),
);
const inspectorAvailableWidth = computed(() =>
  Math.max(0, workspaceWidth.value - ruleListWidth.value - RESIZER_WIDTH),
);
const inspectorWidth = computed(() =>
  clampRequestForwardInspectorWidth(
    preferredInspectorWidth.value,
    inspectorAvailableWidth.value,
  ),
);
const inspectorMaximum = computed(() =>
  Math.max(
    MIN_REQUEST_FORWARD_INSPECTOR_WIDTH,
    Math.floor(inspectorAvailableWidth.value * 0.5),
  ),
);
const workspaceStyle = computed(() => ({
  "--request-forward-rule-list-width": `${ruleListWidth.value}px`,
  "--request-forward-inspector-width": `${inspectorWidth.value}px`,
}));
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
  return parseRequestForwardError(error, "failed").message;
}

async function copyEndpointValue(value: string, label: string) {
  try {
    await navigator.clipboard.writeText(value);
    ElMessage.success(`已复制${label}`);
  } catch (error) {
    ElMessage.error(`复制${label}失败：${errorMessage(error)}`);
  }
}

function copyListenEndpoint() {
  const rule = selectedRule.value;
  if (!rule) return;
  void copyEndpointValue(getRequestForwardLocalEndpoint(rule), "监听地址");
}

function copyTargetEndpoint() {
  const rule = selectedRule.value;
  if (!rule) return;
  const target = rule.protocol === "http"
    ? rule.targetUrl?.trim() ?? ""
    : formatRequestForwardEndpoint(rule.targetHost, rule.targetPort);
  void copyEndpointValue(target, "目标地址");
}

async function openLocalEndpoint() {
  const rule = selectedRule.value;
  const url = rule ? getRequestForwardLocalUrl(rule) : null;
  if (!url) {
    ElMessage.error("当前规则没有可打开的 HTTP 监听地址");
    return;
  }
  try {
    await invoke<{ ok: boolean }>("tool:system:open-external", { url });
  } catch (error) {
    ElMessage.error(`浏览器打开失败：${errorMessage(error)}`);
  }
}

function copyEndpointCommand(command: keyof RequestForwardCommandExamples) {
  const rule = selectedRule.value;
  const examples = rule ? getRequestForwardCommandExamples(rule) : null;
  if (!examples) {
    ElMessage.error("当前规则没有可复制的 HTTP 命令");
    return;
  }
  const label = command === "powershell" ? "PowerShell 命令" : "curl 命令";
  void copyEndpointValue(examples[command], label);
}

function currentSelectionIntent() {
  return {
    selectionToken: selectionIntentToken,
    selectedId: selectedId.value,
    draft: false,
  };
}

function currentEditorIntent() {
  return {
    selectionToken: editorIntentToken,
    selectedId: editorRuleId.value,
    draft: editorMode.value === "create",
  };
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
  if (ruleId == null) return null;
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
  selectedLogId.value = null;
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
  if (ruleId == null || observabilityMutating.value) return;
  const requestToken = ++statsRequestToken;
  statsLoading.value = true;
  statsError.value = "";
  try {
    const result = await invoke<StatsEnvelope>("tool:request-forward:stats-get", { id: ruleId });
    if (
      requestToken !== statsRequestToken ||
      selectionIntentToken !== intentToken ||
      selectedId.value !== ruleId
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
    selectedLogId.value = retainRequestForwardSelectedLogId(
      selectedLogId.value,
      logItems.value,
    );
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
    selectedLogId.value = retainRequestForwardSelectedLogId(
      selectedLogId.value,
      logItems.value,
    );
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
  selectedLogId.value = null;
  logDebounceTimer = setTimeout(() => {
    logDebounceTimer = undefined;
    void loadLogs(false);
  }, 300);
}

function loadMoreLogs() {
  if (loadingMore.value || logInFlight) return;
  void loadLogs(true);
}

function selectLog(id: number) {
  selectedLogId.value = id;
}

function persistRuleListWidth() {
  setSetting(
    RULE_LIST_WIDTH_SETTING,
    String(Math.round(preferredRuleListWidth.value)),
  );
}

function handleRuleListPointerMove(event: PointerEvent) {
  if (!ruleListResizeActive) return;
  preferredRuleListWidth.value = clampRequestForwardRuleListWidth(
    ruleListResizeStartWidth + event.clientX - ruleListResizeStartX,
    workspaceWidth.value,
  );
}

function stopRuleListResize(persist = true) {
  if (!ruleListResizeActive) return;
  ruleListResizeActive = false;
  window.removeEventListener("pointermove", handleRuleListPointerMove);
  window.removeEventListener("pointerup", handleRuleListPointerUp);
  document.body.classList.remove("request-forward-is-resizing");
  if (persist) persistRuleListWidth();
}

function handleRuleListPointerUp() {
  stopRuleListResize();
}

function startRuleListResize(event: PointerEvent) {
  if (event.button !== 0) return;
  event.preventDefault();
  ruleListResizeStartX = event.clientX;
  ruleListResizeStartWidth = ruleListWidth.value;
  ruleListResizeActive = true;
  document.body.classList.add("request-forward-is-resizing");
  window.addEventListener("pointermove", handleRuleListPointerMove);
  window.addEventListener("pointerup", handleRuleListPointerUp);
}

function adjustRuleListWidth(delta: number) {
  preferredRuleListWidth.value = clampRequestForwardRuleListWidth(
    ruleListWidth.value + delta,
    workspaceWidth.value,
  );
  persistRuleListWidth();
}

function persistInspectorWidth() {
  setSetting(
    INSPECTOR_WIDTH_SETTING,
    String(Math.round(preferredInspectorWidth.value)),
  );
}

function handleInspectorPointerMove(event: PointerEvent) {
  if (!inspectorResizeActive) return;
  preferredInspectorWidth.value = clampRequestForwardInspectorWidth(
    inspectorResizeStartWidth + inspectorResizeStartX - event.clientX,
    inspectorAvailableWidth.value,
  );
}

function stopInspectorResize(persist = true) {
  if (!inspectorResizeActive) return;
  inspectorResizeActive = false;
  window.removeEventListener("pointermove", handleInspectorPointerMove);
  window.removeEventListener("pointerup", handleInspectorPointerUp);
  document.body.classList.remove("request-forward-is-resizing");
  if (persist) persistInspectorWidth();
}

function handleInspectorPointerUp() {
  stopInspectorResize();
}

function startInspectorResize(event: PointerEvent) {
  if (event.button !== 0) return;
  event.preventDefault();
  inspectorResizeStartX = event.clientX;
  inspectorResizeStartWidth = inspectorWidth.value;
  inspectorResizeActive = true;
  document.body.classList.add("request-forward-is-resizing");
  window.addEventListener("pointermove", handleInspectorPointerMove);
  window.addEventListener("pointerup", handleInspectorPointerUp);
}

function adjustInspectorWidth(delta: number) {
  preferredInspectorWidth.value = clampRequestForwardInspectorWidth(
    inspectorWidth.value + delta,
    inspectorAvailableWidth.value,
  );
  persistInspectorWidth();
}

function reloadCurrentObservability() {
  const intentToken = selectionIntentToken;
  const ruleId = selectedId.value;
  if (ruleId == null || selectedRule.value?.id !== ruleId) return;
  void Promise.all([
    loadStats(ruleId, intentToken),
    loadLogs(false, ruleId, intentToken),
  ]);
}

async function refreshRules(options: { showLoading?: boolean } = {}) {
  const requestToken = ++refreshRequestToken;
  const intentToken = selectionIntentToken;
  const capturedEditorToken = editorIntentToken;
  const previousSelectedId = selectedId.value;
  const previousEditorId = editorRuleId.value;
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
    const removedEditorRule =
      editorMode.value === "edit" &&
      previousEditorId != null &&
      capturedEditorToken === editorIntentToken &&
      !rules.value.some((rule) => rule.id === previousEditorId);
    if (removedEditorRule) {
      closeEditor();
      ElMessage.warning("当前编辑的规则已被删除，编辑弹窗已关闭");
    }
    if (selectionIntentToken !== intentToken) return;
    const retained = rules.value.some((rule) => rule.id === selectedId.value);
    const removedSelectedRule = selectedId.value != null && !retained;
    selectedId.value = retained ? selectedId.value : rules.value[0]?.id ?? null;
    if (removedSelectedRule) {
      ElMessage.warning("当前查看的规则已被删除，已切换到可用规则");
    }
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
  invalidateRecoveryPreflight();
  selectionIntentToken += 1;
  selectedId.value = id;
}

function openCreateDialog() {
  if (interactionBusy.value) return;
  invalidateRecoveryPreflight();
  editorIntentToken += 1;
  invalidatePreflight();
  editorMode.value = "create";
  editorRuleId.value = null;
  form.value = getDefaultRequestForwardForm();
  formDirty.value = false;
  fieldErrors.value = {};
}

function openDuplicateDialog(id: number) {
  if (interactionBusy.value) return;
  invalidateRecoveryPreflight();
  const source = rules.value.find((item) => item.id === id);
  if (!source) {
    ElMessage.error("无法复制规则：源规则不存在或已被删除");
    return;
  }
  editorIntentToken += 1;
  invalidatePreflight();
  editorMode.value = "create";
  editorRuleId.value = null;
  form.value = duplicateRequestForwardRuleForm(source, source.listenPort);
  formDirty.value = false;
  fieldErrors.value = {};
  void runPreflight();
}

function openEditDialog(id: number) {
  if (interactionBusy.value) return;
  const rule = rules.value.find((item) => item.id === id);
  if (!rule) return;
  invalidateRecoveryPreflight();
  editorIntentToken += 1;
  invalidatePreflight();
  editorMode.value = "edit";
  editorRuleId.value = id;
  form.value = { ...rule };
  formDirty.value = false;
  fieldErrors.value = {};
}

function closeEditor() {
  editorIntentToken += 1;
  invalidatePreflight();
  editorMode.value = null;
  editorRuleId.value = null;
  formDirty.value = false;
  fieldErrors.value = {};
}

async function requestEditorClose() {
  if (interactionBusy.value) return;
  if (formDirty.value) {
    try {
      await ElMessageBox.confirm(
        "关闭后将丢失未保存的修改。",
        "未保存的修改",
        {
          type: "warning",
          confirmButtonText: "放弃修改",
          cancelButtonText: "继续编辑",
        },
      );
    } catch {
      return;
    }
  }
  closeEditor();
}

function handleFormUpdate(value: RequestForwardRuleForm) {
  if (interactionBusy.value || readonly.value) return;
  invalidatePreflight();
  form.value = value;
  formDirty.value = true;
}

function applySuggestedListenPort(port: number) {
  if (
    interactionBusy.value ||
    readonly.value ||
    !Number.isInteger(port) ||
    port < 1 ||
    port > 65535
  ) return;
  form.value = { ...form.value, listenPort: port };
  formDirty.value = true;
  invalidatePreflight();
}

function restartSelectedRule() {
  if (!selectedRule.value || interactionBusy.value) return;
  invalidateRecoveryPreflight();
  void startRule(selectedRule.value.id);
}

function editSelectedRule() {
  if (!selectedRule.value || interactionBusy.value) return;
  openEditDialog(selectedRule.value.id);
}

function invalidateRecoveryPreflight() {
  recoveryPreflightRequestToken += 1;
  recoveryPreflightResult.value = null;
  recoveryPreflightRuleId.value = null;
}

async function checkSelectedTarget() {
  const rule = selectedRule.value;
  if (!rule || interactionBusy.value) return;
  const ruleId = rule.id;
  const intentToken = selectionIntentToken;
  const requestToken = ++recoveryPreflightRequestToken;
  recoveryPreflighting.value = true;
  try {
    const result = await invoke<RequestForwardPreflightResult>(
      "tool:request-forward:preflight",
      toRequestForwardRuleWriteInput(rule),
    );
    if (
      requestToken !== recoveryPreflightRequestToken ||
      intentToken !== selectionIntentToken ||
      selectedId.value !== ruleId
    ) return;
    recoveryPreflightResult.value = result;
    recoveryPreflightRuleId.value = ruleId;
    result.ready
      ? ElMessage.success("目标检测通过")
      : ElMessage.warning("目标检测未通过，请查看检测结果或编辑规则");
  } catch (error) {
    if (requestToken === recoveryPreflightRequestToken) {
      ElMessage.error(`检测目标失败：${errorMessage(error)}`);
    }
  } finally {
    if (requestToken === recoveryPreflightRequestToken) recoveryPreflighting.value = false;
  }
}

function useSelectedSuggestedPort() {
  const ruleId = selectedRule.value?.id;
  const port = selectedSuggestedListenPort.value;
  if (ruleId == null || port == null || interactionBusy.value) return;
  openEditDialog(ruleId);
  if (editorMode.value === "edit" && editorRuleId.value === ruleId) applySuggestedListenPort(port);
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
    ElMessage.error("请修正表单中的错误后再继续");
    return false;
  }
  return true;
}

async function runPreflight(): Promise<RequestForwardPreflightResult | null> {
  if (interactionBusy.value || readonly.value || editorMode.value == null) return null;
  if (!validateForm()) return null;
  return executePreflight();
}

async function preflightAndStart() {
  const result = await runPreflight();
  if (!result?.ready) {
    if (result) ElMessage.warning("配置预检未通过，请先处理阻断项");
    return;
  }
  if (!isAcceptedPreflightCurrent()) return;
  await saveAndStart();
}

async function saveRule(): Promise<RequestForwardRule | null> {
  if (interactionBusy.value) return null;
  if (readonly.value) {
    ElMessage.warning("运行中的规则不能修改，请先停止规则");
    return null;
  }
  if (!validateForm()) return null;
  const isDraft = editorMode.value === "create";
  const targetId = isDraft ? null : editorRuleId.value;
  if (!isDraft && targetId == null) return null;
  const intent = captureRequestForwardMutationIntent(currentEditorIntent(), targetId);
  const payload = toRequestForwardRuleWriteInput(form.value);
  saving.value = true;
  try {
    const operation = isDraft
      ? invoke<RuleEnvelope>("tool:request-forward:create", payload)
      : invoke<RuleEnvelope>("tool:request-forward:update", { id: targetId, ...payload });
    const { value: result } = await applyRequestForwardMutationResult(
      operation,
      intent,
      currentEditorIntent,
      (completed) => {
        if (isDraft) {
          selectionIntentToken += 1;
          selectedId.value = completed.item.id;
        }
        closeEditor();
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
    await refreshRules();
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

async function handleEditorStopAndEdit() {
  if (!editorRule.value) return;
  try {
    await stopRule(editorRule.value.id, false);
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

async function deleteRule(id: number) {
  const rule = rules.value.find((item) => item.id === id) ?? null;
  if (!rule || interactionBusy.value) return;
  const intent = captureRequestForwardMutationIntent(currentSelectionIntent(), rule.id);
  const state = statuses.value.find((status) => status.ruleId === id)?.state ?? "stopped";
  if (isRequestForwardRuleReadonly(state)) {
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
        if (selectedId.value === id) {
          selectionIntentToken += 1;
          selectedId.value = null;
        }
      },
    );
    if (editorRuleId.value === id) closeEditor();
    await refreshRules();
    ElMessage.success("规则已删除");
  } catch (error) {
    ElMessage.error(`删除规则失败：${errorMessage(error)}`);
  } finally {
    operating.value = false;
  }
}

function deleteEditorRule() {
  if (editorRuleId.value != null) void deleteRule(editorRuleId.value);
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
    if (selectionIntentToken !== intentToken || selectedId.value !== rule.id) return;
    ElMessage.success("转发日志已清空");
    selectedLogId.value = null;
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
    if (selectionIntentToken !== intentToken || selectedId.value !== rule.id) return;
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

watch(selectedId, (ruleId) => {
  resetObservabilityState();
  if (ruleId != null) {
    void Promise.all([loadStats(ruleId), loadLogs(false, ruleId)]);
  }
});
watch(logKeyword, scheduleLogReload);
watch(logMode, () => {
  clearLogDebounce();
  logRequestToken += 1;
  logItems.value = [];
  logTotal.value = 0;
  selectedLogId.value = null;
  void loadLogs(false);
});
watch(hasActiveRuntimeRule, syncPolling, { immediate: true });
onMounted(() => {
  const savedRuleListWidth = Number(getSetting(RULE_LIST_WIDTH_SETTING));
  if (Number.isFinite(savedRuleListWidth) && savedRuleListWidth > 0) {
    preferredRuleListWidth.value = savedRuleListWidth;
  }
  const savedWidth = Number(getSetting(INSPECTOR_WIDTH_SETTING));
  if (Number.isFinite(savedWidth) && savedWidth > 0) {
    preferredInspectorWidth.value = savedWidth;
  }
  if (workspaceRef.value) {
    workspaceResizeObserver = new ResizeObserver(([entry]) => {
      workspaceWidth.value = Math.max(0, entry.contentRect.width);
    });
    workspaceResizeObserver.observe(workspaceRef.value);
  }
  void refreshRules({ showLoading: true });
});
onUnmounted(() => {
  refreshRequestToken += 1;
  statsRequestToken += 1;
  logRequestToken += 1;
  invalidatePreflight();
  invalidateRecoveryPreflight();
  pendingLogRefresh = null;
  clearLogDebounce();
  clearPolling();
  workspaceResizeObserver?.disconnect();
  workspaceResizeObserver = null;
  stopRuleListResize(false);
  stopInspectorResize(false);
});
</script>

<template>
  <div
    ref="workspaceRef"
    class="request-forward-panel request-forward-workspace"
    :style="workspaceStyle"
  >
    <RequestForwardRuleList
      :rules="rules"
      :statuses="statuses"
      :selected-id="selectedId"
      :loading="loading"
      :busy="interactionBusy"
      @add="openCreateDialog"
      @select="selectRule"
      @start="startRule"
      @stop="stopRule"
      @edit="openEditDialog"
      @duplicate="openDuplicateDialog"
      @delete="deleteRule"
      @start-all="runBatch('start')"
      @stop-all="runBatch('stop')"
    />

    <div
      class="rule-list-resizer"
      role="separator"
      aria-label="调整规则列表宽度"
      aria-orientation="vertical"
      :aria-valuemin="MIN_REQUEST_FORWARD_RULE_LIST_WIDTH"
      :aria-valuemax="ruleListMaximum"
      :aria-valuenow="ruleListWidth"
      tabindex="0"
      @pointerdown="startRuleListResize"
      @keydown.left.prevent="adjustRuleListWidth(-16)"
      @keydown.right.prevent="adjustRuleListWidth(16)"
    />

    <main class="rule-workbench">
      <template v-if="selectedRule">
        <header class="workbench-header">
          <div>
            <p class="workbench-header__eyebrow">RULE #{{ selectedRule.id }}</p>
            <h1>{{ selectedRule.name }}</h1>
            <p>查看当前规则的运行统计与转发日志。</p>
          </div>
          <div class="workbench-header__aside">
            <div class="runtime-state" :class="`is-${selectedState}`">
              <span>{{ stateCopy }}</span>
              <template v-if="selectedRuntimeError">
                <small>{{ getRequestForwardErrorSummary(selectedRuntimeError.code) }}</small>
                <div class="runtime-state__actions">
                  <el-button
                    v-if="selectedRecoveryActions.includes('restart')"
                    size="small"
                    :disabled="interactionBusy"
                    @click="restartSelectedRule"
                  >重新启动</el-button>
                  <el-button
                    v-if="selectedRecoveryActions.includes('edit')"
                    size="small"
                    :disabled="interactionBusy"
                    @click="editSelectedRule"
                  >编辑规则</el-button>
                  <el-button
                    v-if="selectedRecoveryActions.includes('check_target')"
                    size="small"
                    :disabled="interactionBusy"
                    @click="checkSelectedTarget"
                  >检测目标</el-button>
                  <el-button
                    v-if="selectedRecoveryActions.includes('use_suggested_port')"
                    size="small"
                    :disabled="interactionBusy"
                    @click="useSelectedSuggestedPort"
                  >使用建议端口</el-button>
                </div>
                <div
                  v-if="recoveryPreflightResult && recoveryPreflightRuleId === selectedRule.id"
                  class="runtime-state__preflight"
                  role="status"
                >
                  <strong>检测结果</strong>
                  <ul>
                    <li
                      v-for="check in recoveryPreflightResult.checks"
                      :key="check.kind"
                      :class="`is-${check.state}`"
                    >{{ check.message }}</li>
                  </ul>
                </div>
                <details class="runtime-state__details">
                  <summary>查看技术详情</summary>
                  <dl>
                    <div><dt>错误码</dt><dd>{{ selectedRuntimeError.code }}</dd></div>
                    <div><dt>状态</dt><dd>{{ selectedRuntimeError.state }}</dd></div>
                    <div><dt>原始信息</dt><dd>{{ selectedRuntimeError.message }}</dd></div>
                  </dl>
                </details>
              </template>
            </div>
            <RequestForwardEndpointActions
              :protocol="selectedRule.protocol"
              @copy-listen="copyListenEndpoint"
              @copy-target="copyTargetEndpoint"
              @open-local="openLocalEndpoint"
              @copy-command="copyEndpointCommand"
            />
          </div>
        </header>

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
                    :selected-id="selectedLogId"
                    :loading="logsLoading"
                    :loading-more="loadingMore"
                    :error="logError"
                    :has-more="hasMoreLogs"
                    @select="selectLog"
                    @retry="loadLogs(false)"
                    @load-more="loadMoreLogs"
                  />
                </section>
              </div>
            </div>
      </template>

      <div v-else class="workbench-empty">
        <div class="workbench-empty__mark">RF</div>
        <h1>选择或新建转发规则</h1>
        <p>在左侧选择规则查看日志，或新建 HTTP、TCP、UDP 转发规则。</p>
        <el-button type="primary" :disabled="interactionBusy" @click="openCreateDialog">
          新建规则
        </el-button>
      </div>
    </main>

    <div
      class="inspector-resizer"
      role="separator"
      aria-label="调整日志详情宽度"
      aria-orientation="vertical"
      :aria-valuemin="MIN_REQUEST_FORWARD_INSPECTOR_WIDTH"
      :aria-valuemax="inspectorMaximum"
      :aria-valuenow="inspectorWidth"
      tabindex="0"
      @pointerdown="startInspectorResize"
      @keydown.left.prevent="adjustInspectorWidth(16)"
      @keydown.right.prevent="adjustInspectorWidth(-16)"
    />

    <aside
      class="log-inspector-shell"
      :class="{ 'is-inspector-open': selectedLog }"
    >
      <RequestForwardLogInspector
        :log="selectedLog"
        @close="selectedLogId = null"
      />
    </aside>

    <RequestForwardRuleDialog
      :visible="editorMode !== null"
      :mode="editorMode"
      :form="form"
      :errors="fieldErrors"
      :readonly="readonly"
      :persisted="editorMode === 'edit'"
      :disabled="interactionBusy"
      :saving="saving"
      :operating="operating"
      :preflight-result="preflightResult"
      :preflighting="preflighting"
      @update:form="handleFormUpdate"
      @request-close="requestEditorClose"
      @save="saveRule"
      @save-and-start="saveAndStart"
      @preflight="runPreflight"
      @preflight-and-start="preflightAndStart"
      @apply-suggested-port="applySuggestedListenPort"
      @stop-and-edit="handleEditorStopAndEdit"
      @delete="deleteEditorRule"
    />
  </div>
</template>

<style scoped>
.request-forward-panel {
  display: grid;
  position: relative;
  grid-template-columns: var(--request-forward-rule-list-width) 6px minmax(0, 1fr) 6px var(--request-forward-inspector-width);
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: #fff;
  font-size: 16px;
}
.request-forward-panel :deep(.el-button),
.request-forward-panel :deep(.el-input__inner),
.request-forward-panel :deep(.el-input__wrapper),
.request-forward-panel :deep(.el-select__placeholder) { font-size: 16px; }

.rule-list-resizer,
.inspector-resizer {
  position: relative;
  z-index: 2;
  width: 6px;
  min-width: 6px;
  border: 0;
  padding: 0;
  background: #eef1f4;
  cursor: col-resize;
  transition: background-color 150ms ease;
}
.rule-list-resizer::after,
.inspector-resizer::after { position: absolute; top: 50%; left: 2px; width: 2px; height: 34px; border-radius: 2px; background: #b8c2cd; content: ""; transform: translateY(-50%); }
.rule-list-resizer:hover,
.inspector-resizer:hover { background: #dfe8ee; }
.rule-list-resizer:focus-visible,
.inspector-resizer:focus-visible { outline: 2px solid var(--el-color-primary, #409eff); outline-offset: -2px; }
.log-inspector-shell { display: flex; min-width: 0; min-height: 0; border-left: 1px solid #dfe4e9; background: #fbfcfd; }

.rule-workbench {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  background: #fdfdfd;
}

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
  font-size: 24px;
}

.workbench-header p,
.workbench-empty p {
  margin: 0;
  color: var(--text-secondary, #64748b);
  font-size: 16px;
  line-height: 1.55;
}

.workbench-header__eyebrow {
  color: var(--el-color-primary, #409eff) !important;
  font-size: 12px !important;
  font-weight: 800;
  letter-spacing: 0.12em;
}

.workbench-header__aside {
  display: grid;
  max-width: 560px;
  justify-items: end;
  gap: 10px;
}

.runtime-state {
  display: grid;
  max-width: 300px;
  justify-items: end;
  gap: 4px;
  color: #6b7280;
  text-align: right;
}

.runtime-state span { font-size: 16px; font-weight: 700; }
.runtime-state span::before { content: "●"; margin-right: 6px; font-size: 10px; }
.runtime-state small { color: #c23b35; line-height: 1.45; }
.runtime-state__actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 6px; }
.runtime-state__actions :deep(.el-button + .el-button) { margin-left: 0; }
.runtime-state__preflight { max-width: 420px; text-align: left; }
.runtime-state__preflight strong { font-size: 13px; }
.runtime-state__preflight ul { display: grid; gap: 3px; margin: 4px 0 0; padding-left: 18px; }
.runtime-state__preflight li { color: #6b7280; font-size: 13px; overflow-wrap: anywhere; }
.runtime-state__preflight li.is-failed { color: #c23b35; }
.runtime-state__preflight li.is-warning { color: #a86608; }
.runtime-state__details { max-width: 420px; color: #6b4f4c; font-size: 13px; }
.runtime-state__details summary { cursor: pointer; }
.runtime-state__details dl { display: grid; gap: 4px; margin: 8px 0 0; }
.runtime-state__details dl div { display: grid; grid-template-columns: 64px minmax(0, 1fr); gap: 8px; }
.runtime-state__details dt { font-weight: 700; }
.runtime-state__details dd { margin: 0; overflow-wrap: anywhere; }
.runtime-state.is-running { color: #168357; }
.runtime-state.is-starting,
.runtime-state.is-stopping { color: #a86608; }
.runtime-state.is-failed { color: #c23b35; }

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
.observability-warning strong { color: #65450d; font-size: 14px; }
.observability-warning span { color: #85672f; font-size: 14px; line-height: 1.5; overflow-wrap: anywhere; }
.section-header { display: flex; align-items: center; justify-content: space-between; gap: 14px; margin-bottom: 8px; }
.section-header h2 { margin: 2px 0 0; color: var(--text-primary, #1f2937); font-size: 18px; }
.section-header__eyebrow { margin: 0; color: #667486; font-size: 12px; font-weight: 800; letter-spacing: .12em; }
.stats-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 7px; }
.stat-card { display: grid; gap: 3px; min-width: 0; padding: 9px 10px; border: 1px solid #e2e6eb; border-radius: 6px; background: #fff; }
.stat-card span { color: #667486; font-size: 14px; }
.stat-card strong { overflow: hidden; color: #26364a; font-size: 20px; text-overflow: ellipsis; white-space: nowrap; }
.stat-card.is-error strong { color: #aa3933; }
.stats-error { display: flex; min-height: 72px; align-items: center; justify-content: center; gap: 10px; border: 1px solid #efc8c5; border-radius: 6px; background: #fff8f7; color: #a9332d; font-size: 14px; }
.log-header { margin-top: 16px; }
.log-toolbar { display: flex; align-items: flex-end; gap: 10px; margin-bottom: 10px; }
.log-search { display: grid; min-width: 220px; flex: 1; gap: 4px; }
.log-search > span { color: #56667a; font-size: 14px; font-weight: 600; }
.log-mode { display: inline-flex; flex: none; padding: 2px; border: 1px solid #d7dce3; border-radius: 6px; background: #f4f6f8; }
.log-mode button { min-height: 32px; border: 0; border-radius: 4px; padding: 0 12px; background: transparent; color: #56667a; cursor: pointer; font: inherit; font-size: 14px; }
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
  font-size: 14px;
}

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
  font-size: 16px;
  font-weight: 800;
  letter-spacing: 0.08em;
}

.workbench-empty .el-button { margin-top: 18px; }

@media (max-width: 1100px) {
  .request-forward-panel {
    grid-template-columns: var(--request-forward-rule-list-width) 6px minmax(0, 1fr);
  }
  .inspector-resizer { display: none; }
  .log-inspector-shell {
    position: absolute;
    z-index: 20;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(92%, 520px);
    border-left: 1px solid #cfd8e1;
    box-shadow: -12px 0 28px rgb(31 41 55 / 16%);
    transform: translateX(102%);
    transition: transform 180ms ease;
  }
  .log-inspector-shell.is-inspector-open { transform: translateX(0); }
}

@media (max-width: 780px) {
  .request-forward-panel {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr);
    overflow: hidden;
  }
  .rule-list-resizer,
  .inspector-resizer { display: none; }
  .workbench-header { align-items: stretch; flex-direction: column; padding: 18px; }
  .workbench-header__aside { max-width: none; justify-items: start; }
  .runtime-state { max-width: none; justify-items: start; text-align: left; }
  .workbench-scroll { padding-inline: 16px; }
  .stats-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .log-toolbar { align-items: stretch; flex-direction: column; }
  .log-search { min-width: 0; }
  .log-mode { align-self: flex-start; }
}

@media (prefers-reduced-motion: reduce) {
  :deep(*) { scroll-behavior: auto !important; transition-duration: 0.01ms !important; }
}
</style>
