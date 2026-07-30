import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const source = readFileSync(
  fileURLToPath(new URL("./RequestForwardPanel.vue", import.meta.url)),
  "utf8",
);
const bridgeSource = readFileSync(
  fileURLToPath(new URL("../bridge/tauri.ts", import.meta.url)),
  "utf8",
);
const tauriMainSource = readFileSync(
  fileURLToPath(new URL("../../src-tauri/src/main.rs", import.meta.url)),
  "utf8",
);
const typesSource = readFileSync(
  fileURLToPath(new URL("../types/request-forward.ts", import.meta.url)),
  "utf8",
);
const listSource = readFileSync(
  fileURLToPath(
    new URL("./request-forward/RequestForwardRuleList.vue", import.meta.url),
  ),
  "utf8",
);
const formSource = readFileSync(
  fileURLToPath(
    new URL("./request-forward/RequestForwardRuleForm.vue", import.meta.url),
  ),
  "utf8",
);
const logListUrl = new URL(
  "./request-forward/RequestForwardLogList.vue",
  import.meta.url,
);
const logListSource = existsSync(fileURLToPath(logListUrl))
  ? readFileSync(fileURLToPath(logListUrl), "utf8")
  : "";
const dialogUrl = new URL(
  "./request-forward/RequestForwardRuleDialog.vue",
  import.meta.url,
);
const dialogSource = existsSync(fileURLToPath(dialogUrl))
  ? readFileSync(fileURLToPath(dialogUrl), "utf8")
  : "";
const inspectorUrl = new URL(
  "./request-forward/RequestForwardLogInspector.vue",
  import.meta.url,
);
const inspectorSource = existsSync(fileURLToPath(inspectorUrl))
  ? readFileSync(fileURLToPath(inspectorUrl), "utf8")
  : "";
const preflightResultUrl = new URL(
  "./request-forward/RequestForwardPreflightResult.vue",
  import.meta.url,
);
const preflightResultSource = existsSync(fileURLToPath(preflightResultUrl))
  ? readFileSync(fileURLToPath(preflightResultUrl), "utf8")
  : "";
const endpointActionsUrl = new URL(
  "./request-forward/RequestForwardEndpointActions.vue",
  import.meta.url,
);
const endpointActionsSource = existsSync(fileURLToPath(endpointActionsUrl))
  ? readFileSync(fileURLToPath(endpointActionsUrl), "utf8")
  : "";
const batchResultDialogUrl = new URL(
  "./request-forward/RequestForwardBatchResultDialog.vue",
  import.meta.url,
);
const batchResultDialogSource = existsSync(fileURLToPath(batchResultDialogUrl))
  ? readFileSync(fileURLToPath(batchResultDialogUrl), "utf8")
  : "";

describe("RequestForwardPanel source structure", () => {
  it("renders structured runtime recovery without bypassing panel orchestration", () => {
    expect(source).toContain("parseRequestForwardError");
    expect(source).toContain("getRequestForwardRecoveryActions");
    expect(source).toContain("查看技术详情");
    expect(source).toContain("重新启动");
    expect(source).toContain("编辑规则");
    expect(source).toContain("检测目标");
    expect(source).toContain("使用建议端口");
    expect(source).toMatch(/async function checkSelectedTarget\([\s\S]*?tool:request-forward:preflight/);
    expect(source).toMatch(/function checkSelectedTarget[\s\S]*?selectionIntentToken/);
    expect(source).toContain("recoveryPreflightRequestToken");
    expect(source).toContain("recoveryPreflightResult.checks");
    expect(source).toContain("检测结果");
    expect(source).toMatch(/function useSelectedSuggestedPort\([\s\S]*?openEditDialog[\s\S]*?applySuggestedListenPort/);
    expect(source).toContain(':disabled="interactionBusy"');
  });
  it("registers the preflight channel and exact result contract", () => {
    expect(bridgeSource).toContain(
      '"tool:request-forward:preflight": { domain: "request_forward", action: "preflight" }',
    );
    expect(typesSource).toContain(
      'RequestForwardPreflightCheckKind = "listener" | "dns" | "connect" | "tls"',
    );
    expect(typesSource).toContain(
      'RequestForwardPreflightCheckState = "passed" | "failed" | "warning"',
    );
    expect(typesSource).toContain("interface RequestForwardPreflightCheck");
    expect(typesSource).toContain("interface RequestForwardPreflightResult");
    expect(typesSource).toContain("suggestedListenPort: number | null");
  });

  it("routes preflight through a dedicated blocking-safe async command", () => {
    expect(tauriMainSource).toContain("async fn request_forward_preflight");
    expect(tauriMainSource).toContain("tauri::async_runtime::spawn_blocking");
    expect(tauriMainSource).toMatch(
      /tauri::generate_handler!\[[\s\S]*?request_forward_preflight/,
    );
    expect(bridgeSource).toContain(
      'channel === "tool:request-forward:preflight"',
    );
    expect(bridgeSource).toContain(
      'invoke<unknown>("request_forward_preflight", { payload })',
    );
    expect(tauriMainSource).toContain(
      "tools::request_forward::encode_preflight_task_error",
    );
    expect(tauriMainSource).toMatch(
      /spawn_blocking[\s\S]*?\.await[\s\S]*?encode_preflight_task_error/,
    );
  });

  it("renders accessible preflight stages and applies suggestions only by explicit click", () => {
    expect(existsSync(fileURLToPath(preflightResultUrl))).toBe(true);
    expect(preflightResultSource).toContain("PreflightCheck");
    expect(preflightResultSource).toContain('role="status"');
    expect(preflightResultSource).toContain('role="alert"');
    expect(preflightResultSource).toContain("使用建议端口");
    expect(preflightResultSource).toMatch(/apply-suggested-port/);
    expect(preflightResultSource).toMatch(/@click=.*apply-suggested-port/);
    expect(preflightResultSource).toContain("disabled: boolean");
    expect(preflightResultSource).toContain(':disabled="disabled"');
    expect(dialogSource).toMatch(
      /RequestForwardPreflightResultView[\s\S]*?:disabled="disabled"/,
    );
  });

  it("keeps preflight, preflight-and-start and legacy save paths discoverable", () => {
    expect(dialogSource).toContain("preflightResult");
    expect(dialogSource).toContain("preflighting");
    expect(dialogSource).toMatch(/preflight: \[\]/);
    expect(dialogSource).toMatch(/"preflight-and-start": \[autoStart: boolean\]/);
    expect(dialogSource).toMatch(/"apply-suggested-port": \[port: number\]/);
    expect(dialogSource).toContain("检测配置");
    expect(dialogSource).toContain("检测并启动");
    expect(dialogSource).toContain("仅保存");
    expect(dialogSource).toContain("保存并启动");
    expect(dialogSource).toContain("RequestForwardPreflightResult");
  });

  it("delegates preflight concurrency and acceptance to the composable", () => {
    expect(source).toContain("useRequestForwardPreflight");
    expect(source).toContain("result: preflightResult");
    expect(source).toContain("loading: preflighting");
    expect(source).toContain("run: executePreflight");
    expect(source).toContain("invalidate: invalidatePreflight");
    expect(source).toContain("isAcceptedCurrent: isAcceptedPreflightCurrent");
    expect(source).toContain("tool:request-forward:preflight");
    expect(source).toContain("toRequestForwardRuleWriteInput(form.value)");
    expect(source).not.toContain("preflightRequestToken");
    expect(source).not.toContain("preflightPayloadSnapshot");
    expect(source).not.toContain("preflightEditorIntentToken");
    expect(source).toMatch(/function handleFormUpdate[\s\S]*?invalidatePreflight\(\)/);
    expect(source).toMatch(/function openCreateDialog[\s\S]*?invalidatePreflight\(\)/);
    expect(source).toMatch(/function openEditDialog[\s\S]*?invalidatePreflight\(\)/);
    expect(source).toMatch(/function closeEditor[\s\S]*?invalidatePreflight\(\)/);
  });

  it("starts from the tested snapshot only when the backend result is ready", () => {
    const body = source.match(
      /async function preflightAndStart\(autoStart\?: boolean\)[\s\S]*?\n}\n\nasync function/,
    )?.[0] ?? "";
    expect(body).toContain("await runPreflight()");
    expect(body).toMatch(/!result\?\.ready|!result\.ready/);
    expect(body).toContain("isAcceptedPreflightCurrent");
    expect(body).toContain("await saveAndStart(autoStart)");
    expect(source).toMatch(/const interactionBusy = computed\([\s\S]*?preflighting\.value/);
  });

  it("applies suggested ports by updating the draft, marking dirty and clearing the result", () => {
    const body = source.match(
      /function applySuggestedListenPort\(port: number\)[\s\S]*?\n}/,
    )?.[0] ?? "";
    expect(body).toContain("listenPort: port");
    expect(body).toContain("formDirty.value = true");
    expect(body).toContain("invalidatePreflight()");
    expect(source).toContain('@apply-suggested-port="applySuggestedListenPort"');
  });

  it("keeps running rules readonly and exposes stop-and-edit", () => {
    expect(dialogSource).toContain("停止并编辑");
    expect(source).toContain("handleEditorStopAndEdit");
    expect(formSource).toContain(':disabled="readonly || disabled"');
  });

  it("separates save from save-and-start", () => {
    expect(dialogSource).toContain("仅保存");
    expect(dialogSource).toContain("保存并启动");
    expect(source).toContain("saveRule");
    expect(source).toContain("startRule");
  });

  it("provides single-rule and batch start-stop controls", () => {
    expect(listSource).toMatch(/emit\(["']start["']/);
    expect(listSource).toMatch(/emit\(["']stop["']/);
    expect(listSource).toContain('"batch-start"');
    expect(listSource).toContain('"batch-stop"');
    expect(listSource).toContain("启动{{ batchScope.label }}");
    expect(listSource).toContain("停止{{ batchScope.label }}");
  });

  it("uses visible multi-selection as the explicit batch range", () => {
    expect(listSource).toContain("filterRequestForwardRules");
    expect(listSource).toContain("getRequestForwardBatchScope");
    expect(listSource).toContain("stateFilter");
    expect(listSource).toContain("selectedIds");
    expect(listSource).toContain("全选当前");
    expect(listSource).toContain("clearSelection");
    expect(listSource).toContain("visibleIds.has(id)");
    expect(listSource).toContain("@click.stop");
  });

  it("submits explicit ids, confirms stops and renders per-rule batch results", () => {
    expect(source).toMatch(/async function runBatch\([\s\S]*?\{ ids \}/);
    expect(source).toContain("确认批量停止");
    expect(source).toContain("batchDialogVisible.value = true");
    expect(source).toContain("RequestForwardBatchResultDialog");
    expect(batchResultDialogSource).toContain("parseRequestForwardError");
    expect(batchResultDialogSource).toContain("定位规则");
    expect(batchResultDialogSource).toContain("重试");
    expect(batchResultDialogSource).toContain("编辑规则");
    expect(batchResultDialogSource).toContain("row.details.code");
    expect(batchResultDialogSource).toContain("row.details.message");
  });

  it("warns when a listener is exposed beyond loopback", () => {
    expect(formSource).toContain("当前监听地址可被其他设备访问");
    expect(formSource).toContain("isExposedForwardBindHost");
  });

  it("confirms deletion and keeps persisted protocols immutable", () => {
    expect(source).toContain("ElMessageBox.confirm");
    expect(source).toContain("删除后无法恢复");
    expect(formSource).toContain(':disabled="persisted || readonly || disabled"');
  });

  it("polls serially with timeout guards and clears the timer", () => {
    expect(source).toContain("hasActiveRuntimeRule");
    expect(source).not.toContain("setInterval");
    expect(source).toContain("setTimeout");
    expect(source).toContain("2_000");
    expect(source).toContain("pollGeneration");
    expect(source).toContain("pollInFlight");
    expect(source).toContain("onUnmounted");
    expect(source).toContain("clearTimeout");
  });

  it("invalidates pending preflight responses when unmounted", () => {
    const body = source.match(/onUnmounted\(\(\) => \{[\s\S]*?\n}\);/)?.[0] ?? "";
    expect(body).toContain("invalidatePreflight()");
  });

  it("uses observability as the default workspace without tabs", () => {
    expect(source).not.toContain("activeWorkbenchTab");
    expect(source).not.toContain("<el-tabs");
    expect(source).not.toContain("<el-tab-pane");
    expect(source).toContain('class="observability"');
  });

  it("moves create and edit into one controlled rule dialog", () => {
    expect(source).toContain("RequestForwardRuleDialog");
    expect(source).toContain('ref<"create" | "edit" | null>');
    expect(source).toContain("function openCreateDialog()");
    expect(source).toContain("function openEditDialog(id: number)");
    expect(source).toContain('@edit="openEditDialog"');
    expect(source).not.toContain("<el-tabs");
    expect(source).not.toContain("<el-tab-pane");
    expect(dialogSource).toContain("<el-dialog");
    expect(dialogSource).toContain("RequestForwardRuleFormEditor");
    expect(dialogSource).toContain("停止并编辑");
    expect(dialogSource).toContain("仅保存");
    expect(dialogSource).toContain("保存并启动");
  });

  it("does not replace the selected log context when editing another rule", () => {
    expect(source).toContain("editorRuleId");
    expect(source).toContain("currentEditorIntent");
    expect(source).toMatch(/function openEditDialog\(id: number\)[\s\S]*?editorRuleId\.value = id/);
    expect(source).not.toMatch(/function openEditDialog\(id: number\)[\s\S]*?selectedId\.value = id/);
  });

  it("opens create without replacing the selected observability rule", () => {
    const openCreateBody = source.match(/function openCreateDialog\(\)[\s\S]*?\n}/)?.[0] ?? "";
    expect(openCreateBody).toContain('editorMode.value = "create"');
    expect(openCreateBody).not.toContain("selectedId.value");
  });

  it("queues one background log refresh from the existing serial poll", () => {
    expect(source).toContain("pendingLogRefresh");
    expect(source).toContain("refreshLogsInBackground");
    expect(source).toContain("flushPendingLogRefresh");
    expect(source).toMatch(/await refreshRules\(\)[\s\S]*?refreshLogsInBackground/);
    expect(source).not.toContain("setInterval");
  });

  it("keeps background refresh errors non-blocking", () => {
    expect(source).toContain("logRefreshError");
    expect(source).toContain("日志自动刷新失败");
    expect(source).toMatch(/logRefreshError[\s\S]*?RequestForwardLogList/);
  });

  it("keeps rule selection separate from runtime controls and the row menu", () => {
    expect(listSource).not.toMatch(/<button[^>]*class="rule-row"/);
    expect(listSource).toContain('class="rule-row__select"');
    expect(listSource).toContain('class="rule-row__controls"');
    expect(listSource).toContain('class="rule-row__menu"');
    expect(listSource).toMatch(/<\/button>\s*<div class="rule-row__controls"/);
  });

  it("uses a compact rule navigation with context editing", () => {
    expect(listSource).toContain('trigger="contextmenu"');
    expect(listSource).toMatch(/edit: \[id: number\]/);
    expect(listSource).toMatch(/delete: \[id: number\]/);
    expect(listSource).toContain('command="edit"');
    expect(listSource).toContain('command="delete"');
    expect(listSource).toContain("MoreFilled");
    expect(listSource).toContain('class="rule-row"');
    expect(listSource).not.toContain('class="rule-card"');
  });

  it("offers duplicate as a rule menu action without selecting the source rule", () => {
    expect(listSource).toMatch(/duplicate: \[id: number\]/);
    expect(listSource).toContain('command="duplicate"');
    expect(listSource).toContain("复制规则");
    expect(source).toContain('@duplicate="openDuplicateDialog"');

    const body = source.match(
      /function openDuplicateDialog\(id: number\)[\s\S]*?\n}/,
    )?.[0] ?? "";
    expect(body).toContain("rules.value.find");
    expect(body).toContain("duplicateRequestForwardRuleForm");
    expect(body).toContain('editorMode.value = "create"');
    expect(body).toContain("editorRuleId.value = null");
    expect(body).toContain("editorIntentToken += 1");
    expect(body).toContain("ElMessage.error");
    expect(body).not.toContain("selectedId.value");
  });

  it("automatically preflights a duplicate draft without applying its suggested port", () => {
    const body = source.match(
      /function openDuplicateDialog\(id: number\)[\s\S]*?\n}/,
    )?.[0] ?? "";
    expect(body).toContain("source.listenPort");
    expect(body).toContain("void runPreflight()");
    expect(body).not.toContain("applySuggestedListenPort");
    expect(body).not.toContain("suggestedListenPort");
    expect(preflightResultSource).toContain("使用建议端口");
    expect(source).toContain('@apply-suggested-port="applySuggestedListenPort"');
  });

  it("keeps duplicate drafts on the existing create save actions", () => {
    expect(source).toMatch(/function openDuplicateDialog[\s\S]*?editorMode\.value = "create"/);
    expect(source).toMatch(/const operation = isDraft[\s\S]*?tool:request-forward:create/);
    expect(dialogSource).toContain("仅保存");
    expect(dialogSource).toContain("保存并启动");
    expect(dialogSource).toContain("检测配置");
    expect(dialogSource).toContain("检测并启动");
  });

  it("separates current runtime controls from the application startup policy", () => {
    expect(listSource).toMatch(/emit\(["']start["'], rule\.id\)/);
    expect(listSource).toMatch(/emit\(["']stop["'], rule\.id\)/);
    expect(listSource).toContain("应用启动时");
    expect(listSource).toContain('active-text="开"');
    expect(listSource).toContain('inactive-text="关"');
    expect(source).toContain('@start="startRule"');
    expect(source).not.toContain('@start="startRuleWithPrompt"');
    expect(listSource).not.toContain('command="start-once"');
    expect(listSource).not.toContain('command="start-auto"');
    expect(listSource).not.toContain('command="stop-cancel-auto"');
  });

  it("exposes explicit auto-start intent controls", () => {
    expect(source).toContain("auto-start-update");
    expect(source).toContain("仅本次启动");
    expect(source).toContain("启动并自动恢复");
    expect(source).toContain("停止并取消自动恢复");
    expect(listSource).toContain("应用启动时");
  });

  it("does not overwrite dirty forms during background refresh", () => {
    expect(source).toContain("const formDirty");
    expect(source).toContain("requestEditorClose");
    expect(source).toContain('formDirty.value = true');
    expect(dialogSource).toContain(':model-value="form"');
    expect(dialogSource).toContain('@update:model-value');
    expect(source).toContain("formDirty.value = false");
  });

  it("disables list actions while panel operations are busy", () => {
    expect(listSource).toContain("busy: boolean");
    expect(source).toContain(':busy="interactionBusy"');
    expect(listSource.match(/:disabled="busy/g)?.length ?? 0).toBeGreaterThanOrEqual(4);
  });

  it("locks rule selection and every form field during mutations", () => {
    expect(source).toContain("const interactionBusy = computed");
    expect(source).toContain(':busy="interactionBusy"');
    expect(source).toContain(':disabled="interactionBusy"');
    expect(listSource).toMatch(/class="rule-row__select"[\s\S]*?:disabled="busy"/);
    expect(formSource).toContain("disabled: boolean");
    expect(formSource.match(/readonly \|\| disabled/g)?.length ?? 0).toBeGreaterThanOrEqual(8);
  });

  it("guards mutation responses with captured target and selection intent", () => {
    expect(source).toContain("captureRequestForwardMutationIntent");
    expect(source).toContain("applyRequestForwardMutationResult");
    expect(source).toContain("currentSelectionIntent");
    expect(source.match(/applyRequestForwardMutationResult/g)?.length ?? 0).toBeGreaterThanOrEqual(4);
  });

  it("separates externally removed view and editor targets", () => {
    expect(source).toContain("当前查看的规则已被删除");
    expect(source).toContain("当前编辑的规则已被删除");
    expect(source).toContain("removedEditorRule");
    expect(source).toMatch(/removedEditorRule[\s\S]*?closeEditor\(\)/);
  });

  it("keeps rule selection and actions busy during observability mutations", () => {
    expect(source).toContain(':busy="interactionBusy"');
    expect(source).toContain("function reloadCurrentObservability");
    expect(source).toContain("const intentToken = selectionIntentToken");
    expect(source).toContain("const ruleId = selectedId.value");
    expect(source).toMatch(/finally \{\s*observabilityMutating\.value = false;\s*reloadCurrentObservability\(\);/);
  });

  it("loads selected-rule stats with protocol-specific event labels", () => {
    expect(source).toContain("tool:request-forward:stats-get");
    expect(source).toContain("请求数");
    expect(source).toContain("连接数");
    expect(source).toContain("数据报数");
    expect(source).toContain("eventCount");
    expect(source).toContain("uploadBytes");
    expect(source).toContain("downloadBytes");
    expect(source).toContain("errorCount");
  });

  it("queries logs by rule id with debounced filters and stale-response guards", () => {
    expect(source).toContain("tool:request-forward:log-list");
    expect(source).toContain("buildRequestForwardLogQuery");
    expect(source).toContain("id: context.ruleId");
    expect(source).toContain("keyword: context.keyword");
    expect(source).toContain("method: context.method");
    expect(source).toContain("statusCode: context.statusCode");
    expect(source).toContain("logTimeForQuery(context.startedAt)");
    expect(source).toContain("logTimeForQuery(context.endedAt)");
    expect(source).toContain("300");
    expect(source).toContain("logRequestToken");
    expect(source).toContain("selectionIntentToken");
    expect(source).toContain("isLogQueryContextCurrent");
  });

  it("keeps log pagination stable and guards concurrent load-more", () => {
    expect(source).toContain("const offset = append ? logItems.value.length : 0");
    expect(source).toContain("queryLogs(context, offset, LOG_PAGE_SIZE)");
    expect(source).toContain("loadingMore");
    expect(source).toContain("logInFlight");
    expect(source).toContain("if (loadingMore.value || logInFlight)");
  });

  it("separates confirmed log clearing from stats reset", () => {
    expect(source).toContain("tool:request-forward:log-clear");
    expect(source).toContain("tool:request-forward:stats-reset");
    expect(source).toContain("清空转发日志");
    expect(source).toContain("重置转发统计");
    expect(source).toContain("request-forward-observability-confirm");
    expect(source).toContain("loadLogs(false, ruleId, intentToken)");
    const clearLogsBody = source.match(/async function clearLogs\(\)[\s\S]*?\n}\n\nasync function resetStats/)?.[0] ?? "";
    const resetStatsBody = source.match(/async function resetStats\(\)[\s\S]*?\n}\n\nfunction upsertStatus/)?.[0] ?? "";
    expect(clearLogsBody).not.toContain("tool:request-forward:stats-reset");
    expect(resetStatsBody).not.toContain("tool:request-forward:log-clear");
  });

  it("allows clearing all rule logs even when the active filter is empty", () => {
    const clearButton = source.match(/<el-button[\s\S]*?@click="clearLogs"[\s\S]*?<\/el-button>/)?.[0] ?? "";
    expect(clearButton).toContain("全部日志");
    expect(clearButton).toContain("selectedRule");
    expect(clearButton).not.toContain("!logItems.length");
  });

  it("shows observability warnings without changing runtime state", () => {
    expect(source).toContain("lastObservabilityError");
    expect(source).toContain("观测数据暂不可用");
    expect(source).toContain("selectedStatus.value?.state");
  });

  it("renders HTTP-only expandable masked details and summary rows", () => {
    expect(logListSource).toContain("clientAddr");
    expect(logListSource).toContain("targetAddr");
    expect(logListSource).toContain("statusCode");
    expect(logListSource).toContain("error");
    expect(logListSource).toContain("uploadBytes");
    expect(logListSource).toContain("downloadBytes");
    expect(logListSource).toContain("durationMs");
    expect(inspectorSource).toMatch(/log\.protocol === ["']http["']/);
    expect(inspectorSource).toContain("requestHeaders");
    expect(inspectorSource).toContain("responseHeaders");
    expect(inspectorSource).toContain("requestBodyPreview");
    expect(inspectorSource).toContain("responseBodyPreview");
    expect(inspectorSource).toContain("内容已截断");
  });

  it("uses a direct date-time range picker with a local default window", () => {
    expect(source).toContain("getDefaultRequestForwardLogTimeRange");
    expect(source).toContain('type="datetimerange"');
    expect(source).toContain('value-format="YYYY-MM-DDTHH:mm:ss"');
    expect(source).toContain(':default-time="logRangeDefaultTime"');
    expect(source).not.toContain('type="datetime-local"');
  });

  it("keeps rule titles and HTTP methods in dedicated rows and columns", () => {
    expect(listSource).toContain('class="rule-row__title"');
    expect(listSource).toMatch(/class="rule-row__title"[\s\S]*?<strong>\{\{ rule\.name \}\}<\/strong>[\s\S]*?<\/span>[\s\S]*?class="rule-row__meta"/);
    expect(logListSource).toContain('role="columnheader">请求方式');
    expect(logListSource).toContain('class="method-cell"');
    expect(logListSource).toContain(':aria-colcount="9"');
  });

  it("renders selectable dense log rows and a separate inspector", () => {
    expect(logListSource).toContain("selectedId: number | null");
    expect(logListSource).toMatch(/select: \[id: number\]/);
    expect(logListSource).toContain('class="log-table"');
    expect(logListSource).toContain('class="log-table__row"');
    expect(logListSource).not.toContain('class="http-details"');
    expect(source).toContain("selectedLogId");
    expect(source).toContain("RequestForwardLogInspector");
    expect(inspectorSource).toContain("请求头");
    expect(inspectorSource).toContain("响应头");
    expect(inspectorSource).toContain("请求体预览");
    expect(inspectorSource).toContain("响应体预览");
  });

  it("keeps the inspector header to two full-width rows and reveals the full request on hover", () => {
    expect(inspectorSource).toContain('class="log-inspector__header-top"');
    expect(inspectorSource).toContain('<h2 :title="requestTitle(log)">');
    expect(inspectorSource).toMatch(
      /\.log-inspector__header h2\s*\{[^}]*overflow:\s*hidden;[^}]*text-overflow:\s*ellipsis;[^}]*white-space:\s*nowrap;/s,
    );
  });

  it("adapts log columns to the log region instead of the viewport", () => {
    expect(logListSource).toContain("container-type: inline-size");
    expect(logListSource).toContain("@container forward-log-list");
    expect(logListSource).not.toMatch(/\.log-table\s*\{[^}]*min-width:\s*(?:720|570|430)px/s);
  });

  it("uses a stable toolbar grid for wide and constrained workbench sizes", () => {
    expect(source).toMatch(/\.log-toolbar\s*\{[^}]*display:\s*grid/s);
    expect(source).toContain("container-name: request-forward-observability");
    expect(source).toContain("@container request-forward-observability");
    expect(source).toMatch(
      /@container request-forward-observability \(max-width: 780px\)[\s\S]*?grid-template-columns: repeat\(2, minmax\(0, 1fr\)\)/,
    );
    expect(source).toMatch(/\.log-filter :deep\(\.el-input-number\)[^}]*width:\s*100%/s);
    expect(source).toContain("@container request-forward-observability (max-width: 480px)");
  });

  it("splits listening and forwarding details without reserving an action column", () => {
    expect(listSource).toContain(':title="formatRequestForwardRuleSummary(rule)"');
    expect(listSource).toContain("<b>监听</b>");
    expect(listSource).toContain("<b>转发</b>");
    expect(listSource).toContain("listenEndpoint(rule)");
    expect(listSource).toContain("targetEndpoint(rule)");
    expect(listSource).toMatch(/\.rule-row\s*\{[^}]*position:\s*relative/s);
    expect(listSource).toMatch(/\.rule-row__menu\s*\{[^}]*position:\s*absolute/s);
    expect(listSource).toMatch(/\.rule-row__summary-line span\s*\{[^}]*white-space:\s*normal/s);
  });

  it("uses persistent keyboard-accessible resizers for both side panes", () => {
    expect(source).toContain('class="request-forward-panel request-forward-workspace"');
    expect(source).toContain('class="rule-list-resizer"');
    expect(source).toContain('class="inspector-resizer"');
    expect(source).toContain('role="separator"');
    expect(source).toContain('aria-orientation="vertical"');
    expect(source).toContain('request-forward:rule-list-width');
    expect(source).toContain('request-forward:inspector-width');
    expect(source).toContain("getSetting");
    expect(source).toContain("setSetting");
    expect(source).toContain("@pointerdown");
    expect(source).toContain("@keydown.left");
    expect(source).toContain("@keydown.right");
    expect(source).toContain("ResizeObserver");
  });

  it("moves field guidance into hover and focus tooltips", () => {
    expect(formSource).toContain("QuestionFilled");
    expect(formSource).toContain('class="field-tip"');
    expect(formSource).toContain("<el-tooltip");
    expect(formSource).not.toContain('class="field-hint"');
    expect(formSource).toContain('aria-label="目标 URL 提示"');
  });

  it("uses an unnumbered compact rule form with side-by-side endpoints", () => {
    expect(formSource).toContain('class="form-identity"');
    expect(formSource).toContain('class="form-endpoints"');
    expect(formSource).toContain('class="form-group__title">本地监听');
    expect(formSource).toContain('class="form-group__title">转发目标');
    expect(formSource).toContain('class="form-group__title">采集选项');
    expect(formSource).not.toContain('class="form-section__heading"');
    expect(formSource).not.toMatch(/<span>0[1-4]<\/span>/);
    expect(formSource).toMatch(
      /\.form-endpoints\s*\{[^}]*grid-template-columns:\s*repeat\(2, minmax\(0, 1fr\)\)/s,
    );
    expect(formSource).toMatch(
      /@media \(max-width: 680px\)[\s\S]*?\.form-endpoints\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\)/,
    );
  });

  it("distinguishes the local HTTP listener from HTTP or HTTPS targets", () => {
    expect(formSource).toContain('<el-option label="HTTP" value="http" />');
    expect(formSource).not.toContain('label="HTTP / HTTPS"');
    expect(formSource).toContain("HTTP 规则支持普通 HTTP 请求和 WebSocket Upgrade，目标可为 HTTP 或 HTTPS。");
    expect(formSource).not.toContain("HTTP 规则的本地监听使用 HTTP，目标 URL 支持 HTTP/HTTPS。");
    expect(formSource).toContain("支持 HTTP/HTTPS 基础地址及 WebSocket Upgrade");
  });

  it("delegates endpoint action rendering to a semantic event-only component", () => {
    expect(source).toContain("RequestForwardEndpointActions");
    expect(source).toContain(':protocol="selectedRule.protocol"');
    expect(source).toContain('@copy-listen="copyListenEndpoint"');
    expect(source).toContain('@copy-target="copyTargetEndpoint"');
    expect(source).toContain('@open-local="openLocalEndpoint"');
    expect(source).toContain('@copy-command="copyEndpointCommand"');

    expect(endpointActionsSource).toContain("复制监听地址");
    expect(endpointActionsSource).toContain("复制目标地址");
    expect(endpointActionsSource).toContain("浏览器打开");
    expect(endpointActionsSource).toContain("命令示例");
    expect(endpointActionsSource).toMatch(/protocol === ["']http["']/);
    expect(endpointActionsSource).toContain('command="powershell"');
    expect(endpointActionsSource).toContain('command="curl"');
    expect(endpointActionsSource).toMatch(/["']copy-listen["']: \[\]/);
    expect(endpointActionsSource).toMatch(/["']copy-target["']: \[\]/);
    expect(endpointActionsSource).toMatch(/["']open-local["']: \[\]/);
    expect(endpointActionsSource).toMatch(
      /["']copy-command["']: \[command: ["']powershell["'] \| ["']curl["']\]/,
    );
    expect(endpointActionsSource).not.toContain("navigator.clipboard");
    expect(endpointActionsSource).not.toContain("tool:system:open-external");
  });

  it("keeps endpoint side effects in the panel and reports every failure", () => {
    expect(source).toContain("getRequestForwardLocalEndpoint");
    expect(source).toContain("getRequestForwardLocalUrl");
    expect(source).toContain("getRequestForwardCommandExamples");
    expect(source).toContain("formatRequestForwardEndpoint");
    expect(source).toContain("await navigator.clipboard.writeText(value)");
    expect(source).toContain('invoke<{ ok: boolean }>("tool:system:open-external", { url })');
    expect(source).toContain("复制${label}失败：${errorMessage(error)}");
    expect(source).toContain("浏览器打开失败：${errorMessage(error)}");
  });

  it("uses a readable typography baseline across the workspace", () => {
    expect(source).toMatch(/\.request-forward-panel\s*\{[^}]*font-size:\s*16px/s);
    expect(source).toMatch(/\.workbench-header h1,[\s\S]*?font-size:\s*24px/s);
    expect(listSource).toMatch(/\.rule-row__summary\s*\{[^}]*font-size:\s*14px/s);
    expect(logListSource).toMatch(/\.log-table__row\s*\{[^}]*font-size:\s*14px/s);
    expect(formSource).toContain(".rule-form :deep(.el-form-item__label)");
    expect(formSource).toMatch(/\.rule-form :deep\(\.el-checkbox__label\)\s*\{\s*font-size:\s*16px/s);
  });

  it("keeps the inspector as an overlay on narrow layouts", () => {
    expect(source).toContain("is-inspector-open");
    expect(source).toMatch(/@media \(max-width: 1100px\)/);
    expect(source).toContain("position: absolute");
  });

  it("never renders TCP or UDP payload details", () => {
    expect(inspectorSource).not.toMatch(/payload/i);
    expect(inspectorSource).toContain('v-if="log.protocol === \'http\'"');
  });

  it("provides complete log filters and loading states", () => {
    expect(source).toContain("<span>关键字</span>");
    expect(source).toContain("<span>Method</span>");
    expect(source).toContain("<span>状态码</span>");
    expect(source).toContain("<span>时间范围</span>");
    expect(source).toContain('start-placeholder="开始时间"');
    expect(source).toContain('end-placeholder="结束时间"');
    expect(source).toContain("clearLogFilters");
    expect(source).toContain('label="全部"');
    expect(source).toContain('label="成功"');
    expect(source).toContain('label="失败"');
    expect(logListSource).toContain("加载更多");
    expect(logListSource).toContain("重新加载");
    expect(logListSource).toContain("暂无转发日志");
  });

  it("defaults to paused and starts real-time collection only on explicit resume", () => {
    expect(typesSource).toContain("logCaptureEnabled: boolean");
    expect(bridgeSource).toContain(
      '"tool:request-forward:log-capture-update": { domain: "request_forward", action: "log_capture_update" }',
    );
    expect(source).toContain("selectedStatus.value?.logCaptureEnabled ?? false");
    expect(source).toMatch(/async function refreshLogsInBackground[\s\S]*?if \(!logLive\.value\) return/);
    expect(source).not.toContain("probePausedLogs");
    expect(source).not.toContain("pausedNewCount");
    expect(source).toMatch(
      /async function setLogLive\(live: boolean\)[\s\S]*?tool:request-forward:log-capture-update/,
    );
    expect(source).toContain("statuses.value = upsertStatus(statuses.value, result.item)");
    expect(source).toContain("开启实时采集");
  });

  it("exports the current filtered query with an explicit 1000-row cap", () => {
    expect(source).toContain('import { save } from "@tauri-apps/plugin-dialog"');
    expect(source).toContain("buildRequestForwardLogExportFileName");
    expect(source).toContain("queryLogs(context, 0, 1000)");
    expect(source).toContain("exportRequestForwardLogsJson");
    expect(source).toContain("exportRequestForwardLogsCsv");
    expect(source).toContain('"tool:file:write-text"');
    expect(source).toContain("已截断，最多导出 1000 条");
  });

  it("supports keyboard log navigation and detail copy actions", () => {
    expect(logListSource).toContain("@keydown.down.prevent");
    expect(logListSource).toContain("@keydown.up.prevent");
    expect(logListSource).toContain("@keydown.home.prevent");
    expect(logListSource).toContain("@keydown.end.prevent");
    expect(logListSource).toContain('role="grid"');
    expect(inspectorSource).toContain("formatRequestForwardLogBody");
    expect(inspectorSource).toContain("getRequestForwardLogCopyText");
    expect(inspectorSource).toContain("复制完整日志");
  });
});
