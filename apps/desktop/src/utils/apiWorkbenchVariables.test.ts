import { describe, expect, it } from "vitest";
import { summarizeApiWorkbenchVariables } from "./apiWorkbenchVariables";

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
});
