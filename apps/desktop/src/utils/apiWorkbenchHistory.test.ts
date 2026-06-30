import { describe, expect, it } from "vitest";
import type { ApiWorkbenchHistoryDetail, ApiWorkbenchHistoryItem } from "../types/api-workbench";
import {
  buildApiWorkbenchDraftFromHistory,
  canReplayApiWorkbenchHistory,
  defaultApiWorkbenchHistoryDisplayName,
} from "./apiWorkbenchHistory";

function history(overrides: Partial<ApiWorkbenchHistoryItem> = {}): ApiWorkbenchHistoryItem {
  return {
    id: 1,
    collectionId: null,
    environmentId: null,
    requestId: null,
    replayedFromHistoryId: null,
    name: "",
    note: "",
    pinned: false,
    method: "POST",
    url: "/api/login",
    finalUrl: "http://127.0.0.1:8080/api/login?debug=1",
    status: 200,
    durationMs: 12,
    ok: true,
    error: null,
    contentType: "application/json",
    bodySize: 2,
    bodyPreview: "{}",
    bodyTruncated: false,
    hasRequestSnapshot: false,
    hasExecutedRequestSnapshot: false,
    createdAt: "2026-06-30 10:00:00",
    ...overrides,
  };
}

describe("apiWorkbenchHistory utils", () => {
  it("allows replay only when executed snapshot exists", () => {
    expect(canReplayApiWorkbenchHistory(history({ hasExecutedRequestSnapshot: true }))).toBe(true);
    expect(canReplayApiWorkbenchHistory(history({ hasExecutedRequestSnapshot: false }))).toBe(false);
  });

  it("builds draft from request snapshot", () => {
    const detail: ApiWorkbenchHistoryDetail = {
      ...history({ hasRequestSnapshot: true }),
      requestSnapshot: {
        method: "PATCH",
        url: "/users/1",
        query: [{ enabled: true, key: "expand", value: "roles" }],
        headers: [{ enabled: true, key: "X-Token", value: "{{TOKEN}}" }],
        bodyType: "json",
        body: "{\"name\":\"demo\"}",
        form: [],
        timeoutMs: 12000,
      },
    };
    const result = buildApiWorkbenchDraftFromHistory(detail);
    expect(result.degraded).toBe(false);
    expect(result.draft.method).toBe("PATCH");
    expect(result.draft.headers[0].value).toBe("{{TOKEN}}");
    expect(result.draft.timeoutMs).toBe(12000);
  });

  it("degrades old history to method and url", () => {
    const detail: ApiWorkbenchHistoryDetail = { ...history(), requestSnapshot: null };
    const result = buildApiWorkbenchDraftFromHistory(detail);
    expect(result.degraded).toBe(true);
    expect(result.draft).toMatchObject({ method: "POST", url: "/api/login", bodyType: "none" });
    expect(result.draft.headers).toEqual([]);
    expect(result.draft.query).toEqual([]);
  });

  it("builds stable default display names", () => {
    expect(defaultApiWorkbenchHistoryDisplayName(history({ name: "  Login  " }))).toBe("Login");
    expect(
      defaultApiWorkbenchHistoryDisplayName(
        history({ name: "", url: "http://x.test/api/users?debug=1", method: "GET" }),
      ),
    ).toBe("GET /api/users");
    expect(
      defaultApiWorkbenchHistoryDisplayName(history({ name: "", url: "not a url", method: "DELETE" })),
    ).toBe("DELETE not a url");
  });
});
