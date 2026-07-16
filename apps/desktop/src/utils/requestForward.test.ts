import { describe, expect, expectTypeOf, it } from "vitest";

import type {
  RequestForwardLogOutcome,
  RequestForwardLogPage,
  RequestForwardLogQuery,
  RequestForwardRestoreResult,
  RequestForwardRuleForm,
  RequestForwardRuleWriteInput,
} from "../types/request-forward";
import {
  applyRequestForwardMutationResult,
  captureRequestForwardMutationIntent,
  DEFAULT_REQUEST_FORWARD_FORM,
  formatRequestForwardEndpoint,
  formatRequestForwardRuleSummary,
  getDefaultRequestForwardForm,
  getForwardEventLabel,
  getRequestForwardBatchMessage,
  getRequestForwardLogTone,
  isExposedForwardBindHost,
  isRequestForwardRuleReadonly,
  normalizeRequestForwardRuleForm,
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

describe("request forward utilities", () => {
  it("does not let a late mutation response overwrite a newer selection or edit", async () => {
    let current = { selectionToken: 7, selectedId: 1, draft: false };
    let visibleForm = { ...baseForm, name: "规则 A" };
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

  it("validates protocol-specific required fields", () => {
    expect(validateRequestForwardRuleForm({ ...baseForm, targetUrl: "" })).toContain(
      "targetUrl",
    );
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

  it("maps log outcome to success or danger tone", () => {
    expect(getRequestForwardLogTone("success")).toBe("success");
    expect(getRequestForwardLogTone("error")).toBe("danger");
    expectTypeOf(getRequestForwardLogTone)
      .parameter(0)
      .toEqualTypeOf<RequestForwardLogOutcome>();
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
});
