import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const source = readFileSync(
  fileURLToPath(new URL("./RequestForwardPanel.vue", import.meta.url)),
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

describe("RequestForwardPanel source structure", () => {
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
    expect(listSource).toMatch(/emit\(["']start-all["']/);
    expect(listSource).toMatch(/emit\(["']stop-all["']/);
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

  it("keeps rule selection separate from row actions", () => {
    expect(listSource).not.toMatch(/<button[^>]*class="rule-row"/);
    expect(listSource).toContain('class="rule-row__select"');
    expect(listSource).toContain('class="rule-row__actions"');
    expect(listSource).toMatch(/<\/button>\s*<div class="rule-row__actions">/);
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

  it("keeps inline start and stop controls in the rule navigation", () => {
    expect(listSource).toMatch(/emit\(["']start["'], rule\.id\)/);
    expect(listSource).toMatch(/emit\(["']stop["'], rule\.id\)/);
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
    expect(source).toContain("id: context.ruleId");
    expect(source).toContain("keyword: context.keyword || null");
    expect(source).toContain('mode: context.mode === "all" ? null : context.mode');
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

  it("adapts log columns to the log region instead of the viewport", () => {
    expect(logListSource).toContain("container-type: inline-size");
    expect(logListSource).toContain("@container forward-log-list");
    expect(logListSource).not.toMatch(/\.log-table\s*\{[^}]*min-width:\s*(?:720|570|430)px/s);
  });

  it("splits listening and forwarding details without reserving an action column", () => {
    expect(listSource).toContain(':title="formatRequestForwardRuleSummary(rule)"');
    expect(listSource).toContain("<b>监听</b>");
    expect(listSource).toContain("<b>转发</b>");
    expect(listSource).toContain("listenEndpoint(rule)");
    expect(listSource).toContain("targetEndpoint(rule)");
    expect(listSource).toMatch(/\.rule-row\s*\{[^}]*position:\s*relative/s);
    expect(listSource).toMatch(/\.rule-row__actions\s*\{[^}]*position:\s*absolute/s);
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

  it("provides keyword, success/error filters and loading states", () => {
    expect(source).toContain("日志关键字");
    expect(source).toContain('label="全部"');
    expect(source).toContain('label="成功"');
    expect(source).toContain('label="失败"');
    expect(logListSource).toContain("加载更多");
    expect(logListSource).toContain("重新加载");
    expect(logListSource).toContain("暂无转发日志");
  });
});
