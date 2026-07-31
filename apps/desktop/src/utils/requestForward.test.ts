import { describe, expect, expectTypeOf, it } from "vitest";

import type {
  RequestForwardError,
  RequestForwardLogOutcome,
  RequestForwardLogPage,
  RequestForwardLogQuery,
  RequestForwardLogRow,
  RequestForwardRuleBundle,
  RequestForwardRestoreResult,
  RequestForwardRule,
  RequestForwardRuleForm,
  RequestForwardRuleWriteInput,
} from "../types/request-forward";
import {
  applyRequestForwardMutationResult,
  buildRequestForwardLogExportFileName,
  buildRequestForwardLogCommandExamples,
  buildRequestForwardRuleBundleFileName,
  buildRequestForwardLogQuery,
  clampRequestForwardInspectorWidth,
  clampRequestForwardRuleListWidth,
  captureRequestForwardMutationIntent,
  DEFAULT_REQUEST_FORWARD_FORM,
  duplicateRequestForwardRuleForm,
  exportRequestForwardLogsCsv,
  exportRequestForwardLogsJson,
  formatRequestForwardLogBody,
  formatRequestForwardLogHeaders,
  formatRequestForwardEndpoint,
  formatRequestForwardRuleSummary,
  getDefaultRequestForwardForm,
  getDefaultRequestForwardLogTimeRange,
  getForwardEventLabel,
  getRequestForwardBatchMessage,
  getRequestForwardBatchScope,
  filterRequestForwardRules,
  getRequestForwardCommandExamples,
  getRequestForwardLocalEndpoint,
  getRequestForwardLocalUrl,
  getRequestForwardLogProbeLimit,
  getRequestForwardLogTargetCount,
  getRequestForwardLogTone,
  getRequestForwardLogCopyText,
  getRequestForwardRecoveryActions,
  isExposedForwardBindHost,
  isRequestForwardRuleReadonly,
  normalizeRequestForwardRuleForm,
  parseRequestForwardError,
  parseRequestForwardRuleBundleText,
  parseRequestForwardLogTimestamp,
  retainRequestForwardSelectedLogId,
  sanitizeRequestForwardLogFileName,
  serializeRequestForwardRuleBundle,
  toRequestForwardRuleWriteInput,
  validateRequestForwardRuleForm,
} from "./requestForward";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function expectNoLoneSurrogate(value: string) {
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      expect(next).toBeGreaterThanOrEqual(0xdc00);
      expect(next).toBeLessThanOrEqual(0xdfff);
      index += 1;
      continue;
    }
    expect(unit < 0xdc00 || unit > 0xdfff).toBe(true);
  }
}

const baseForm: RequestForwardRuleForm = {
  name: "本地 API",
  protocol: "http",
  bindHost: "127.0.0.1",
  listenPort: 8080,
  targetUrl: "http://127.0.0.1:3000/api",
  targetHost: null,
  targetPort: null,
  captureHttpHeaders: true,
  captureHttpBody: false,
};

const baseLog: RequestForwardLogRow = {
  id: 31,
  ruleId: 7,
  protocol: "http",
  clientAddr: "127.0.0.1:53000",
  targetAddr: "127.0.0.1:3000",
  method: "POST",
  path: "/api/items",
  statusCode: 201,
  durationMs: 24,
  uploadBytes: 18,
  downloadBytes: 26,
  requestHeaders: [["Content-Type", "application/json; charset=utf-8"]],
  responseHeaders: [["content-type", "application/problem+json"]],
  requestBodyPreview: '{"name":"demo"}',
  responseBodyPreview: '{"ok":true}',
  requestBodyTruncated: false,
  responseBodyTruncated: false,
  error: null,
  createdAt: "2026-07-19 08:09:10.123",
};

describe("request forward utilities", () => {
  it("parses marked runtime error envelopes from strings and Error.message", () => {
    const payload = JSON.stringify({
      marker: "lazycat.request_forward.error",
      version: 1,
      code: "listener_in_use",
      message: "HTTP 监听绑定失败: os error 10048",
      state: "failed",
    });
    const expected: RequestForwardError = {
      code: "listener_in_use",
      message: "HTTP 监听绑定失败: os error 10048",
      state: "failed",
    };

    expect(parseRequestForwardError(payload, "stopped")).toEqual(expected);
    expect(parseRequestForwardError(new Error(payload), "stopped")).toEqual(expected);
    expect(parseRequestForwardError(new Error(`invoke failed: ${payload}`), "stopped")).toEqual(
      expected,
    );
  });

  it("falls back safely for legacy text, unrelated JSON and malformed envelopes", () => {
    for (const input of [
      "历史纯文本错误",
      '{"code":"dns_failed","message":"not marked","state":"failed"}',
      '{"marker":"lazycat.request_forward.error","version":1,"code":"dns_failed","message":"bad state","state":"broken"}',
      '{"marker":"lazycat.request_forward.error","version":2,"code":"dns_failed","message":"bad version","state":"failed"}',
      '{"marker":"lazycat.request_forward.error","version":1,"code":"not_a_code","message":"bad code","state":"failed"}',
      '{"marker":"lazycat.request_forward.error",',
    ]) {
      expect(parseRequestForwardError(input, "failed")).toEqual({
        code: "unknown",
        message: input,
        state: "failed",
      });
    }
  });

  it("maps reusable recovery actions and gates suggested ports on actual preflight output", () => {
    const listenerError: RequestForwardError = {
      code: "listener_in_use",
      message: "端口被占用",
      state: "failed",
    };
    expect(getRequestForwardRecoveryActions(listenerError, null)).toEqual([
      "restart",
      "edit",
      "check_target",
    ]);
    expect(getRequestForwardRecoveryActions(listenerError, 18080)).toEqual([
      "restart",
      "edit",
      "check_target",
      "use_suggested_port",
    ]);
    expect(
      getRequestForwardRecoveryActions(
        { code: "dns_failed", message: "DNS 失败", state: "failed" },
        18080,
      ),
    ).toEqual(["restart", "edit", "check_target"]);
    expect(
      getRequestForwardRecoveryActions(
        { code: "invalid_config", message: "配置错误", state: "stopped" },
        null,
      ),
    ).toEqual(["edit"]);
    expect(
      getRequestForwardRecoveryActions(
        { code: "self_forward", message: "目标指向自身", state: "failed" },
        null,
      ),
    ).toEqual(["edit"]);
  });
  it("clamps the preferred rule list width without consuming the workbench", () => {
    expect(clampRequestForwardRuleListWidth(undefined, 1200)).toBe(260);
    expect(clampRequestForwardRuleListWidth("oops", 1200)).toBe(260);
    expect(clampRequestForwardRuleListWidth(180, 1200)).toBe(220);
    expect(clampRequestForwardRuleListWidth(500, 1200)).toBe(420);
    expect(clampRequestForwardRuleListWidth(400, 800)).toBe(320);
  });

  it("clamps the preferred inspector width to the current workspace", () => {
    expect(clampRequestForwardInspectorWidth(undefined, 1200)).toBe(420);
    expect(clampRequestForwardInspectorWidth("oops", 1200)).toBe(420);
    expect(clampRequestForwardInspectorWidth(200, 1200)).toBe(320);
    expect(clampRequestForwardInspectorWidth(900, 1200)).toBe(600);
    expect(clampRequestForwardInspectorWidth(480, 800)).toBe(400);
  });

  it("retains a selected log only while its stable id is present", () => {
    expect(retainRequestForwardSelectedLogId(7, [{ id: 9 }, { id: 7 }])).toBe(7);
    expect(retainRequestForwardSelectedLogId(7, [{ id: 9 }])).toBeNull();
    expect(retainRequestForwardSelectedLogId(null, [{ id: 9 }])).toBeNull();
  });

  it("keeps background log refreshes on a continuous bounded window", () => {
    expect(getRequestForwardLogProbeLimit(30)).toBe(60);
    expect(getRequestForwardLogProbeLimit(990)).toBe(1000);

    expect(
      getRequestForwardLogTargetCount({
        loadedCount: 60,
        previousTotal: 100,
        nextTotal: 105,
      }),
    ).toBe(65);
    expect(
      getRequestForwardLogTargetCount({
        loadedCount: 60,
        previousTotal: 100,
        nextTotal: 200,
      }),
    ).toBe(160);
    expect(
      getRequestForwardLogTargetCount({
        loadedCount: 60,
        previousTotal: 100,
        nextTotal: 20,
      }),
    ).toBe(20);
    expect(
      getRequestForwardLogTargetCount({
        loadedCount: 990,
        previousTotal: 1000,
        nextTotal: 1000,
      }),
    ).toBe(990);
  });

  it("does not let a late mutation response overwrite a newer selection or edit", async () => {
    let current = { selectionToken: 7, selectedId: 1, draft: false };
    let visibleForm: RequestForwardRuleForm;
    const intent = captureRequestForwardMutationIntent(current, 1);
    const response = deferred<RequestForwardRuleForm>();
    const mutation = applyRequestForwardMutationResult(
      response.promise,
      intent,
      () => current,
      (saved) => {
        visibleForm = saved;
      },
    );

    current = { selectionToken: 8, selectedId: 2, draft: false };
    visibleForm = { ...baseForm, name: "规则 B 的本地编辑" };
    response.resolve({ ...baseForm, name: "规则 A 的晚响应" });

    const result = await mutation;
    expect(result.applied).toBe(false);
    expect(result.value.name).toBe("规则 A 的晚响应");
    expect(visibleForm.name).toBe("规则 B 的本地编辑");
  });

  it("applies a mutation response while the captured selection intent is current", async () => {
    const current = { selectionToken: 3, selectedId: 1, draft: false };
    const intent = captureRequestForwardMutationIntent(current, 1);
    let visibleName = "旧名称";

    const result = await applyRequestForwardMutationResult(
      Promise.resolve("新名称"),
      intent,
      () => current,
      (name) => {
        visibleName = name;
      },
    );

    expect(result.applied).toBe(true);
    expect(visibleName).toBe("新名称");
  });

  it("returns a safe default form", () => {
    expect(getDefaultRequestForwardForm()).toEqual(DEFAULT_REQUEST_FORWARD_FORM);
    expect(getDefaultRequestForwardForm()).not.toBe(DEFAULT_REQUEST_FORWARD_FORM);
    expect(getDefaultRequestForwardForm()).toMatchObject({
      bindHost: "127.0.0.1",
      listenPort: 8080,
      protocol: "http",
    });
  });

  it("builds a duplicate HTTP rule form with a caller-provided listen port", () => {
    const source: RequestForwardRule & {
      state: "running";
      lastError: string;
    } = {
      ...baseForm,
      id: 12,
      autoStart: true,
      createdAt: "2026-07-18T08:00:00Z",
      updatedAt: "2026-07-19T08:00:00Z",
      state: "running",
      lastError: "旧运行错误",
    };

    const duplicate = duplicateRequestForwardRuleForm(source, 18_080);

    expect(duplicate).toEqual({
      name: "本地 API 副本",
      protocol: "http",
      bindHost: "127.0.0.1",
      listenPort: 18_080,
      targetUrl: "http://127.0.0.1:3000/api",
      targetHost: null,
      targetPort: null,
      captureHttpHeaders: true,
      captureHttpBody: false,
    });
    expect(duplicate).not.toHaveProperty("id");
    expect(duplicate).not.toHaveProperty("autoStart");
    expect(duplicate).not.toHaveProperty("createdAt");
    expect(duplicate).not.toHaveProperty("updatedAt");
    expect(duplicate).not.toHaveProperty("state");
    expect(duplicate).not.toHaveProperty("lastError");
  });

  it("preserves socket targets and capture settings when duplicating", () => {
    const source: RequestForwardRule = {
      ...baseForm,
      id: 13,
      name: "UDP DNS",
      protocol: "udp",
      listenPort: 5353,
      targetUrl: null,
      targetHost: "2001:db8::53",
      targetPort: 53,
      captureHttpHeaders: false,
      captureHttpBody: true,
      autoStart: false,
      createdAt: "2026-07-18T08:00:00Z",
      updatedAt: "2026-07-19T08:00:00Z",
    };

    expect(duplicateRequestForwardRuleForm(source, 15_353)).toEqual({
      name: "UDP DNS 副本",
      protocol: "udp",
      bindHost: "127.0.0.1",
      listenPort: 15_353,
      targetUrl: null,
      targetHost: "2001:db8::53",
      targetPort: 53,
      captureHttpHeaders: false,
      captureHttpBody: true,
    });
  });

  it("keeps duplicate Chinese names within the 80-character form limit", () => {
    const source: RequestForwardRule = {
      ...baseForm,
      id: 14,
      name: "转".repeat(80),
      autoStart: false,
      createdAt: "2026-07-18T08:00:00Z",
      updatedAt: "2026-07-19T08:00:00Z",
    };

    const duplicate = duplicateRequestForwardRuleForm(source, 8081);

    expect(duplicate.name).toBe(`${"转".repeat(77)} 副本`);
    expect(duplicate.name).toHaveLength(80);
  });

  it("keeps a 40-emoji duplicate name within 80 UTF-16 code units", () => {
    const source: RequestForwardRule = {
      ...baseForm,
      id: 15,
      name: "😀".repeat(40),
      autoStart: false,
      createdAt: "2026-07-18T08:00:00Z",
      updatedAt: "2026-07-19T08:00:00Z",
    };

    const duplicate = duplicateRequestForwardRuleForm(source, 8081);

    expect(duplicate.name).toBe(`${"😀".repeat(38)} 副本`);
    expect(duplicate.name.length).toBeLessThanOrEqual(80);
    expectNoLoneSurrogate(duplicate.name);
  });

  it("truncates mixed BMP and astral names without leaving a lone surrogate", () => {
    const source: RequestForwardRule = {
      ...baseForm,
      id: 16,
      name: `${"甲".repeat(74)}😀${"乙".repeat(10)}`,
      autoStart: false,
      createdAt: "2026-07-18T08:00:00Z",
      updatedAt: "2026-07-19T08:00:00Z",
    };

    const duplicate = duplicateRequestForwardRuleForm(source, 8081);

    expect(duplicate.name).toBe(`${"甲".repeat(74)}😀乙 副本`);
    expect(duplicate.name).toHaveLength(80);
    expectNoLoneSurrogate(duplicate.name);
  });

  it("validates protocol-specific required fields", () => {
    expect(validateRequestForwardRuleForm({ ...baseForm, targetUrl: "" })).toContain("targetUrl");
    expect(
      validateRequestForwardRuleForm({
        ...baseForm,
        protocol: "tcp",
        targetUrl: null,
        targetHost: "",
        targetPort: null,
      }),
    ).toEqual(expect.arrayContaining(["targetHost", "targetPort"]));
    expect(
      validateRequestForwardRuleForm({
        ...baseForm,
        protocol: "udp",
        targetUrl: null,
        targetHost: "127.0.0.1",
        targetPort: 53,
      }),
    ).toEqual([]);
    expect(
      validateRequestForwardRuleForm({
        ...baseForm,
        protocol: "tcp",
        targetUrl: null,
        targetHost: "db.internal",
        targetPort: 1.5,
      }),
    ).toContain("targetPort");
  });

  it("validates bind IP literals and HTTP target URLs", () => {
    expect(validateRequestForwardRuleForm({ ...baseForm, bindHost: "localhost" })).toContain(
      "bindHost",
    );
    for (const targetUrl of [
      "ftp://example.com/api",
      "https://example.com/api?q=1",
      "http://example.com/api?",
      "https://example.com/api#part",
      "http://example.com/api#",
      "http://example.com:0/api",
    ]) {
      expect(validateRequestForwardRuleForm({ ...baseForm, targetUrl })).toContain("targetUrl");
    }
    expect(
      validateRequestForwardRuleForm({
        ...baseForm,
        bindHost: "::1",
        targetUrl: "https://example.com/api",
      }),
    ).toEqual([]);
  });

  it("normalizes protocol fields before writing", () => {
    const normalized = normalizeRequestForwardRuleForm({
      ...baseForm,
      name: "  API  ",
      bindHost: "  0.0.0.0  ",
      targetUrl: "  http://127.0.0.1:3000  ",
    });
    expect(normalized.name).toBe("API");
    expect(normalized.bindHost).toBe("0.0.0.0");
    expect(normalized.targetUrl).toBe("http://127.0.0.1:3000");
    expect(
      normalizeRequestForwardRuleForm({
        ...baseForm,
        protocol: "tcp",
        targetUrl: "  ",
      }),
    ).toMatchObject({ targetUrl: null });
  });

  it("builds write payload without autoStart", () => {
    const payload: RequestForwardRuleWriteInput = toRequestForwardRuleWriteInput(baseForm);
    expect(payload).toEqual({
      name: "本地 API",
      protocol: "http",
      bindHost: "127.0.0.1",
      listenPort: 8080,
      targetUrl: "http://127.0.0.1:3000/api",
      targetHost: null,
      targetPort: null,
      captureHttpHeaders: true,
      captureHttpBody: false,
    });
    expect(payload).not.toHaveProperty("autoStart");
  });

  it("keeps running forms readonly and failed forms editable", () => {
    expect(isRequestForwardRuleReadonly("starting")).toBe(true);
    expect(isRequestForwardRuleReadonly("running")).toBe(true);
    expect(isRequestForwardRuleReadonly("stopping")).toBe(true);
    expect(isRequestForwardRuleReadonly("stopped")).toBe(false);
    expect(isRequestForwardRuleReadonly("failed")).toBe(false);
  });

  it("detects exposed bind addresses including IPv6 wildcard", () => {
    expect(isExposedForwardBindHost("127.0.0.1")).toBe(false);
    expect(isExposedForwardBindHost("127.0.0.2")).toBe(false);
    expect(isExposedForwardBindHost("localhost")).toBe(false);
    expect(isExposedForwardBindHost("not-an-ip")).toBe(false);
    expect(isExposedForwardBindHost("::1")).toBe(false);
    expect(isExposedForwardBindHost("0.0.0.0")).toBe(true);
    expect(isExposedForwardBindHost("::")).toBe(true);
    expect(isExposedForwardBindHost("192.168.1.20")).toBe(true);
    expect(isExposedForwardBindHost("2001:db8::1")).toBe(true);
  });

  it("formats IPv6 endpoints with brackets", () => {
    expect(formatRequestForwardEndpoint("127.0.0.1", 8080)).toBe("127.0.0.1:8080");
    expect(formatRequestForwardEndpoint("2001:db8::1", 443)).toBe("[2001:db8::1]:443");
    expect(formatRequestForwardEndpoint(undefined, undefined)).toBe("—");
    expect(formatRequestForwardEndpoint("127.0.0.1", 0)).toBe("—");
  });

  it("summarizes rule endpoints by protocol", () => {
    expect(formatRequestForwardRuleSummary(baseForm)).toBe(
      "127.0.0.1:8080 → http://127.0.0.1:3000/api",
    );
    expect(
      formatRequestForwardRuleSummary({
        ...baseForm,
        protocol: "tcp",
        targetUrl: null,
        targetHost: "2001:db8::2",
        targetPort: 5432,
      }),
    ).toBe("127.0.0.1:8080 → [2001:db8::2]:5432");
    expect(
      formatRequestForwardRuleSummary({
        ...baseForm,
        protocol: "udp",
        targetUrl: null,
        targetHost: null,
        targetPort: null,
      }),
    ).toBe("127.0.0.1:8080 → —");
  });

  it("builds protocol-specific local endpoints and HTTP browser URLs", () => {
    expect(getRequestForwardLocalEndpoint(baseForm)).toBe("127.0.0.1:8080");
    expect(getRequestForwardLocalUrl(baseForm)).toBe("http://127.0.0.1:8080");

    const ipv6Http = { ...baseForm, bindHost: "::", listenPort: 9090 };
    expect(getRequestForwardLocalEndpoint(ipv6Http)).toBe("[::]:9090");
    expect(getRequestForwardLocalUrl(ipv6Http)).toBe("http://[::1]:9090");
    expect(getRequestForwardLocalUrl({ ...baseForm, bindHost: "0.0.0.0" })).toBe(
      "http://127.0.0.1:8080",
    );

    const tcpRule = {
      ...baseForm,
      protocol: "tcp" as const,
      bindHost: "2001:db8::1",
      targetUrl: null,
      targetHost: "2001:db8::2",
      targetPort: 5432,
    };
    expect(getRequestForwardLocalEndpoint(tcpRule)).toBe("[2001:db8::1]:8080");
    expect(getRequestForwardLocalUrl(tcpRule)).toBeNull();
    expect(getRequestForwardLocalEndpoint({ ...tcpRule, protocol: "udp", listenPort: 5353 })).toBe(
      "[2001:db8::1]:5353",
    );
  });

  it("builds copyable PowerShell and curl commands for the local HTTP URL", () => {
    expect(getRequestForwardCommandExamples(baseForm)).toEqual({
      powershell: "Invoke-WebRequest -UseBasicParsing -Uri 'http://127.0.0.1:8080'",
      curl: "curl --url 'http://127.0.0.1:8080'",
    });
    expect(getRequestForwardCommandExamples({ ...baseForm, bindHost: "::" })).toEqual({
      powershell: "Invoke-WebRequest -UseBasicParsing -Uri 'http://[::1]:8080'",
      curl: "curl --url 'http://[::1]:8080'",
    });

    const quotedHost = { ...baseForm, bindHost: "127.0.0.1'quoted" };
    expect(getRequestForwardCommandExamples(quotedHost)).toEqual({
      powershell: "Invoke-WebRequest -UseBasicParsing -Uri 'http://127.0.0.1''quoted:8080'",
      curl: `curl --url 'http://127.0.0.1'"'"'quoted:8080'`,
    });
    expect(
      getRequestForwardCommandExamples({
        ...baseForm,
        protocol: "udp",
        targetUrl: null,
        targetHost: "127.0.0.1",
        targetPort: 53,
      }),
    ).toBeNull();
  });

  it("builds reproducible commands from an HTTP log without removing sensitive headers", () => {
    const log: RequestForwardLogRow = {
      id: 20,
      ruleId: 7,
      protocol: "http",
      clientAddr: "127.0.0.1:50000",
      targetAddr: "example.com:80",
      method: "POST",
      path: "/items?q=one",
      statusCode: 201,
      durationMs: 12,
      uploadBytes: 18,
      downloadBytes: 2,
      requestHeaders: [
        ["Authorization", "Bearer top-secret"],
        ["Cookie", "session=private"],
        ["Content-Type", "application/json"],
      ],
      responseHeaders: null,
      requestBodyPreview: '{"name":"O\'Reilly"}',
      responseBodyPreview: null,
      requestBodyTruncated: false,
      responseBodyTruncated: false,
      error: null,
      createdAt: "2026-07-31 08:00:00",
    };

    expect(buildRequestForwardLogCommandExamples(baseForm, log)).toEqual({
      curl: `curl --request 'POST' --url 'http://127.0.0.1:8080/items?q=one' --header 'Authorization: Bearer top-secret' --header 'Cookie: session=private' --header 'Content-Type: application/json' --data-raw '{"name":"O'"'"'Reilly"}'`,
      powershell: `Invoke-WebRequest -UseBasicParsing -Method 'POST' -Uri 'http://127.0.0.1:8080/items?q=one' -Headers @{ 'Authorization' = 'Bearer top-secret'; 'Cookie' = 'session=private'; 'Content-Type' = 'application/json' } -Body '{"name":"O''Reilly"}'`,
      warnings: [],
    });
  });

  it("marks generated requests when captured data is incomplete", () => {
    const result = buildRequestForwardLogCommandExamples(baseForm, {
      ...baseLog,
      method: "PUT",
      path: "/upload",
      uploadBytes: 70_000,
      requestHeaders: null,
      requestBodyPreview: "partial",
      requestBodyTruncated: true,
    });

    expect(result?.warnings).toEqual([
      "本条日志未采集请求头",
      "请求体预览已截断，命令仅包含已采集部分",
    ]);
  });

  it("serializes and parses the versioned request-forward rule bundle", () => {
    const bundle: RequestForwardRuleBundle = {
      format: "lazycat.request-forward.rules",
      version: 1,
      exportedAt: "2026-07-31T08:00:00Z",
      rules: [toRequestForwardRuleWriteInput(baseForm)],
    };
    const text = serializeRequestForwardRuleBundle(bundle);

    expect(parseRequestForwardRuleBundleText(text)).toEqual(bundle);
    expect(text).not.toContain("autoStart");
    expect(() =>
      parseRequestForwardRuleBundleText('{"format":"other","version":1,"rules":[]}'),
    ).toThrow("不是 LazyCat 请求转发规则包");
    expect(() =>
      parseRequestForwardRuleBundleText(
        '{"format":"lazycat.request-forward.rules","version":2,"rules":[]}',
      ),
    ).toThrow("不支持的请求转发规则包版本");
    expect(() =>
      parseRequestForwardRuleBundleText(
        JSON.stringify({
          ...bundle,
          rules: Array.from({ length: 501 }, () => bundle.rules[0]),
        }),
      ),
    ).toThrow("单次最多导入 500 条");
    expect(buildRequestForwardRuleBundleFileName(new Date(2026, 6, 31, 8, 9, 5))).toBe(
      "request-forward-rules-20260731-080905.json",
    );
  });

  it("formats protocol-specific event labels", () => {
    expect(getForwardEventLabel("http")).toBe("请求数");
    expect(getForwardEventLabel("tcp")).toBe("连接数");
    expect(getForwardEventLabel("udp")).toBe("数据报数");
  });

  it("formats batch result messages", () => {
    expect(getRequestForwardBatchMessage("start", { requested: 3, succeeded: 3, failed: 0 })).toBe(
      "已启动 3 条规则",
    );
    expect(getRequestForwardBatchMessage("start", { requested: 3, succeeded: 2, failed: 1 })).toBe(
      "已启动 2 条规则，1 条失败",
    );
    expect(getRequestForwardBatchMessage("start", { requested: 0, succeeded: 0, failed: 0 })).toBe(
      "没有可启动的规则",
    );
    expect(getRequestForwardBatchMessage("stop", { requested: 3, succeeded: 2, failed: 1 })).toBe(
      "已停止 2 条规则，1 条失败",
    );
    expect(getRequestForwardBatchMessage("stop", { requested: 0, succeeded: 0, failed: 0 })).toBe(
      "没有可停止的规则",
    );
  });

  it("filters rules and resolves a stable batch scope", () => {
    const rules: RequestForwardRule[] = [
      { ...baseForm, id: 1, name: "API", autoStart: false, createdAt: "", updatedAt: "" },
      {
        ...baseForm,
        id: 2,
        name: "数据库",
        protocol: "tcp",
        autoStart: false,
        createdAt: "",
        updatedAt: "",
      },
      { ...baseForm, id: 3, name: "失败 API", autoStart: false, createdAt: "", updatedAt: "" },
    ];
    const statuses = [
      { ruleId: 1, state: "running", lastError: null, lastObservabilityError: null },
      { ruleId: 2, state: "stopped", lastError: null, lastObservabilityError: null },
      { ruleId: 3, state: "failed", lastError: "bad", lastObservabilityError: null },
    ] as const;

    expect(filterRequestForwardRules(rules, statuses, "api", "all").map((rule) => rule.id)).toEqual(
      [1, 3],
    );
    expect(filterRequestForwardRules(rules, statuses, "", "failed").map((rule) => rule.id)).toEqual(
      [3],
    );
    expect(getRequestForwardBatchScope(rules, [rules[0], rules[2]], [3, 3, 99], true)).toEqual({
      kind: "selected",
      ids: [3],
      label: "选中 1 条",
    });
    expect(getRequestForwardBatchScope(rules, [rules[0], rules[2]], [], true)).toEqual({
      kind: "filtered",
      ids: [1, 3],
      label: "当前筛选 2 条",
    });
    expect(getRequestForwardBatchScope(rules, rules, [], false)).toEqual({
      kind: "all",
      ids: [1, 2, 3],
      label: "全部 3 条",
    });
  });

  it("maps log outcome to success or danger tone", () => {
    expect(getRequestForwardLogTone("success")).toBe("success");
    expect(getRequestForwardLogTone("error")).toBe("danger");
    expectTypeOf(getRequestForwardLogTone).parameter(0).toEqualTypeOf<RequestForwardLogOutcome>();
  });

  it("matches log query, page and restore result contracts", () => {
    const query: RequestForwardLogQuery = {
      id: 7,
      keyword: "timeout",
      mode: "error",
      offset: 20,
      limit: 50,
    };
    const page: RequestForwardLogPage = { items: [], total: 0 };
    const restore: RequestForwardRestoreResult = {
      ruleId: 7,
      ok: false,
      error: "bind failed",
      state: "failed",
    };
    expect(query).not.toHaveProperty("ruleId");
    expect(page.total).toBe(0);
    expect(restore.state).toBe("failed");
  });

  it("normalizes all log filters into an explicit backend query", () => {
    expect(
      buildRequestForwardLogQuery({
        id: 7,
        keyword: "  timeout ",
        mode: "all",
        method: " post ",
        statusCode: 503,
        startedAt: "2026-07-19 08:00:00",
        endedAt: "2026-07-19 09:00:00",
        offset: 30,
        limit: 30,
      }),
    ).toEqual({
      id: 7,
      keyword: "timeout",
      mode: null,
      method: "post",
      statusCode: 503,
      startedAt: "2026-07-19 08:00:00",
      endedAt: "2026-07-19 09:00:00",
      offset: 30,
      limit: 30,
    });
    expect(buildRequestForwardLogQuery({ id: 7, keyword: " ", method: " " })).toEqual({
      id: 7,
      keyword: null,
      mode: null,
      method: null,
      statusCode: null,
      startedAt: null,
      endedAt: null,
      offset: undefined,
      limit: undefined,
    });
  });

  it("defaults the log time range to one hour ago through local end of day", () => {
    expect(getDefaultRequestForwardLogTimeRange(new Date(2026, 6, 20, 10, 15, 30))).toEqual([
      "2026-07-20T09:15:30",
      "2026-07-20T23:59:59",
    ]);
    expect(getDefaultRequestForwardLogTimeRange(new Date(2026, 6, 20, 0, 30, 0))).toEqual([
      "2026-07-19T23:30:00",
      "2026-07-20T23:59:59",
    ]);
  });

  it("pretty prints only valid JSON bodies with a JSON content type", () => {
    expect(
      formatRequestForwardLogBody('{"name":"demo","items":[1,2]}', [
        ["Content-Type", "application/json; charset=utf-8"],
      ]),
    ).toBe(JSON.stringify({ name: "demo", items: [1, 2] }, null, 2));
    expect(formatRequestForwardLogBody('{"name":"demo"}', [["Content-Type", "text/plain"]])).toBe(
      '{"name":"demo"}',
    );
    expect(
      formatRequestForwardLogBody('{"name":"demo"}', [["Content-Type", "notapplication/jsonx"]]),
    ).toBe('{"name":"demo"}');
    expect(
      formatRequestForwardLogBody("{not json}", [["Content-Type", "application/problem+json"]]),
    ).toBe("{not json}");
    expect(formatRequestForwardLogBody(null, null)).toBeNull();
  });

  it("parses database log timestamps as UTC instead of local time", () => {
    expect(parseRequestForwardLogTimestamp("2026-07-19 08:09:10.123")?.getTime()).toBe(
      Date.UTC(2026, 6, 19, 8, 9, 10, 123),
    );
    expect(parseRequestForwardLogTimestamp("2026-07-19T08:09:10+08:00")?.getTime()).toBe(
      Date.UTC(2026, 6, 19, 0, 9, 10),
    );
    expect(parseRequestForwardLogTimestamp("invalid")).toBeNull();
  });

  it("builds copy text for headers, body, errors and full logs", () => {
    expect(formatRequestForwardLogHeaders(baseLog.requestHeaders)).toBe(
      "Content-Type: application/json; charset=utf-8",
    );
    expect(getRequestForwardLogCopyText({ ...baseLog, error: "连接超时" }, "error")).toBe(
      "连接超时",
    );
    expect(getRequestForwardLogCopyText(baseLog, "requestBody")).toContain('"name": "demo"');
    expect(getRequestForwardLogCopyText(baseLog, "responseHeaders")).toBe(
      "content-type: application/problem+json",
    );
    expect(getRequestForwardLogCopyText(baseLog, "full")).toContain('"statusCode": 201');
  });

  it("exports filtered logs with metadata and a hard 1000-row cap", () => {
    const items = Array.from({ length: 1001 }, (_, index) => ({
      ...baseLog,
      id: index + 1,
    }));
    const filters = {
      keyword: "demo",
      mode: "all" as const,
      method: "POST",
      statusCode: null,
      startedAt: null,
      endedAt: null,
    };
    const json = exportRequestForwardLogsJson({ items, total: 1200, filters });
    const parsed = JSON.parse(json.content) as {
      total: number;
      exported: number;
      truncated: boolean;
      filters: typeof filters;
      items: RequestForwardLogRow[];
    };
    expect(json.exported).toBe(1000);
    expect(json.truncated).toBe(true);
    expect(parsed).toMatchObject({ total: 1200, exported: 1000, truncated: true, filters });
    expect(parsed.items).toHaveLength(1000);

    const csv = exportRequestForwardLogsCsv({ items: items.slice(0, 1), total: 1, filters });
    expect(csv.exported).toBe(1);
    expect(csv.truncated).toBe(false);
    expect(csv.content.split(/\r\n/)).toHaveLength(2);
    expect(csv.content).toContain('"POST"');
    expect(csv.content).toContain("application/json; charset=utf-8");
  });

  it("sanitizes Windows filenames and includes a local timestamp", () => {
    expect(sanitizeRequestForwardLogFileName(' API:/v1* "测试"  ')).toBe("API__v1_ _测试_");
    expect(sanitizeRequestForwardLogFileName("...   ")).toBe("request-forward");
    expect(
      buildRequestForwardLogExportFileName("API:/v1", "json", new Date(2026, 6, 19, 8, 9, 10)),
    ).toBe("API__v1-logs-20260719-080910.json");
  });
});
