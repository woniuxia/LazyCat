import { describe, expect, it } from "vitest";
import {
  resolveApiWorkbenchTemplate,
  summarizeApiWorkbenchVariables,
} from "./apiWorkbenchVariables";

describe("apiWorkbenchVariables", () => {
  it("summarizes variables from actual send path with source precedence", () => {
    const usage = summarizeApiWorkbenchVariables({
      draft: {
        method: "POST",
        url: "{{BASE_URL}}/users/{{USER_ID}}",
        query: [{ enabled: true, key: "token", value: "{{TOKEN}}" }],
        headers: [{ enabled: true, key: "Authorization", value: "Bearer {{TOKEN}}" }],
        bodyType: "json",
        body: "{\"org\":\"{{ORG_ID}}\"}",
        form: [{ enabled: true, key: "hidden", value: "{{FORM_ONLY}}" }],
        timeoutMs: 10000,
      },
      environmentVariables: [
        { name: "BASE_URL", value: "http://127.0.0.1:8080" },
        { name: "TOKEN", value: "env-token" },
      ],
      globalVariables: [
        { name: "TOKEN", value: "global-token" },
        { name: "ORG_ID", value: "org-1" },
      ],
    });

    expect(usage).toEqual([
      { name: "BASE_URL", source: "environment" },
      { name: "USER_ID", source: "missing" },
      { name: "TOKEN", source: "environment" },
      { name: "ORG_ID", source: "global" },
    ]);
  });

  it("resolves templates with environment variables", () => {
    expect(
      resolveApiWorkbenchTemplate("{{BASE_URL}}/users", [
        [{ name: "BASE_URL", value: "http://a" }],
        [],
      ]),
    ).toEqual({ text: "http://a/users", missing: [] });
  });

  it("prefers higher priority variable groups", () => {
    expect(
      resolveApiWorkbenchTemplate("{{TOKEN}}", [
        [{ name: "TOKEN", value: "env" }],
        [{ name: "TOKEN", value: "global" }],
      ]),
    ).toEqual({ text: "env", missing: [] });
    expect(
      resolveApiWorkbenchTemplate("{{TOKEN}}", [[], [{ name: "TOKEN", value: "global" }]]),
    ).toEqual({ text: "global", missing: [] });
  });

  it("keeps missing variables verbatim and reports them once", () => {
    expect(
      resolveApiWorkbenchTemplate("/a/{{MISSING}}/b/{{ MISSING }}", [[], []]),
    ).toEqual({ text: "/a/{{MISSING}}/b/{{ MISSING }}", missing: ["MISSING"] });
  });

  it("returns text without variables as-is", () => {
    expect(resolveApiWorkbenchTemplate("/plain/path", [[], []])).toEqual({
      text: "/plain/path",
      missing: [],
    });
  });
});
