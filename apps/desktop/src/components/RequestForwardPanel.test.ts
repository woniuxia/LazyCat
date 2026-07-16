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

describe("RequestForwardPanel source structure", () => {
  it("keeps running rules readonly and exposes stop-and-edit", () => {
    expect(source).toContain("停止并编辑");
    expect(source).toContain("handleStopAndEdit");
    expect(formSource).toContain(':disabled="readonly"');
  });

  it("separates save from save-and-start", () => {
    expect(source).toContain("仅保存");
    expect(source).toContain("保存并启动");
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
    expect(formSource).toContain(':disabled="persisted"');
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

  it("keeps the card selection button separate from action buttons", () => {
    expect(listSource).not.toMatch(/<button[^>]*class="rule-card"/);
    expect(listSource).toContain('class="rule-card__select"');
    expect(listSource).toContain('class="rule-card__actions"');
    expect(listSource).toMatch(/<\/button>\s*<span class="rule-card__actions">/);
  });

  it("does not overwrite dirty forms during background refresh", () => {
    expect(source).toContain("const formDirty");
    expect(source).toContain("!formDirty.value");
    expect(source).toContain(':model-value="form"');
    expect(source).toContain('@update:model-value="handleFormUpdate"');
    expect(source).toContain("formDirty.value = false");
  });

  it("disables list actions while panel operations are busy", () => {
    expect(listSource).toContain("busy: boolean");
    expect(source).toContain(':busy="operating || saving"');
    expect(listSource.match(/:disabled="busy/g)?.length ?? 0).toBeGreaterThanOrEqual(4);
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
    expect(source).toMatch(/id:\s*ruleId/);
    expect(source).toContain("keyword: normalizedKeyword || null");
    expect(source).toContain("mode: logMode.value === \"all\" ? null : logMode.value");
    expect(source).toContain("300");
    expect(source).toContain("logRequestToken");
    expect(source).toContain("selectionIntentToken");
  });

  it("keeps log pagination stable and guards concurrent load-more", () => {
    expect(source).toContain("offset: append ? logItems.value.length : 0");
    expect(source).toContain("limit: LOG_PAGE_SIZE");
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
    expect(source).toContain("await loadLogs(false)");
    const clearLogsBody = source.match(/async function clearLogs\(\)[\s\S]*?\n}\n\nasync function resetStats/)?.[0] ?? "";
    const resetStatsBody = source.match(/async function resetStats\(\)[\s\S]*?\n}\n\nfunction upsertStatus/)?.[0] ?? "";
    expect(clearLogsBody).not.toContain("tool:request-forward:stats-reset");
    expect(resetStatsBody).not.toContain("tool:request-forward:log-clear");
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
    expect(logListSource).toMatch(/log\.protocol === ["']http["']/);
    expect(logListSource).toContain("requestHeaders");
    expect(logListSource).toContain("responseHeaders");
    expect(logListSource).toContain("requestBodyPreview");
    expect(logListSource).toContain("responseBodyPreview");
    expect(logListSource).toContain("内容已截断");
  });

  it("never renders TCP or UDP payload details", () => {
    expect(logListSource).not.toMatch(/payload/i);
    expect(logListSource).toContain('v-if="log.protocol === \'http\'"');
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
