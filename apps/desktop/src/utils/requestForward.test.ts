import { describe, expect, it } from "vitest";

import type { RequestForwardRuleForm, RequestForwardRuleWriteInput } from "../types/request-forward";
import {
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
    expect(isExposedForwardBindHost("localhost")).toBe(false);
    expect(isExposedForwardBindHost("::1")).toBe(false);
    expect(isExposedForwardBindHost("0.0.0.0")).toBe(true);
    expect(isExposedForwardBindHost("::")).toBe(true);
  });

  it("formats IPv6 endpoints with brackets", () => {
    expect(formatRequestForwardEndpoint("127.0.0.1", 8080)).toBe("127.0.0.1:8080");
    expect(formatRequestForwardEndpoint("2001:db8::1", 443)).toBe("[2001:db8::1]:443");
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
  });

  it("formats protocol-specific event labels", () => {
    expect(getForwardEventLabel("http")).toBe("请求数");
    expect(getForwardEventLabel("tcp")).toBe("连接数");
    expect(getForwardEventLabel("udp")).toBe("数据报数");
  });

  it("formats batch result messages", () => {
    expect(getRequestForwardBatchMessage({ requested: 3, succeeded: 3, failed: 0 })).toBe(
      "已启动 3 条规则",
    );
    expect(getRequestForwardBatchMessage({ requested: 3, succeeded: 2, failed: 1 })).toBe(
      "已启动 2 条规则，1 条失败",
    );
    expect(getRequestForwardBatchMessage({ requested: 0, succeeded: 0, failed: 0 })).toBe(
      "没有可处理的规则",
    );
  });

  it("maps log status to a stable tone", () => {
    expect(getRequestForwardLogTone("success")).toBe("success");
    expect(getRequestForwardLogTone("error")).toBe("danger");
    expect(getRequestForwardLogTone("warn")).toBe("warning");
    expect(getRequestForwardLogTone("info")).toBe("info");
  });
});
