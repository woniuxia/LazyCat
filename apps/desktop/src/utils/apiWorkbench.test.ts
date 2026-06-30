import { describe, expect, it } from "vitest";
import {
  buildApiWorkbenchPreviewUrl,
  buildApiWorkbenchNewRequestState,
  buildApiWorkbenchSelectionState,
  draftApiWorkbenchEnvironmentRows,
  extractApiWorkbenchVariables,
  formatApiWorkbenchResponseBody,
  normalizeApiWorkbenchDraft,
  serializeApiWorkbenchEnvironmentRows,
  validateApiWorkbenchVariableName,
} from "./apiWorkbench";

describe("apiWorkbench utils", () => {
  it("validates variable names", () => {
    expect(validateApiWorkbenchVariableName("TOKEN")).toBe(true);
    expect(validateApiWorkbenchVariableName("org_id")).toBe(true);
    expect(validateApiWorkbenchVariableName("x-api-key")).toBe(true);
    expect(validateApiWorkbenchVariableName("")).toBe(false);
    expect(validateApiWorkbenchVariableName("a.b")).toBe(false);
  });

  it("extracts unique variables", () => {
    expect(extractApiWorkbenchVariables("{{TOKEN}} {{TOKEN}} {{ORG_ID}}")).toEqual([
      "TOKEN",
      "ORG_ID",
    ]);
  });

  it("builds preview url from base and relative path", () => {
    expect(
      buildApiWorkbenchPreviewUrl("http://127.0.0.1:8080/", "api/users", [
        { enabled: true, key: "page", value: "1" },
      ]),
    ).toBe("http://127.0.0.1:8080/api/users?page=1");
  });

  it("normalizes invalid draft values", () => {
    const draft = normalizeApiWorkbenchDraft({
      method: "bad",
      url: " /api ",
      query: [{ enabled: true, key: " q ", value: "1" }],
      headers: [],
      bodyType: "bad",
      body: "",
      form: [],
      timeoutMs: 999999,
    });
    expect(draft.method).toBe("GET");
    expect(draft.url).toBe("/api");
    expect(draft.bodyType).toBe("none");
    expect(draft.timeoutMs).toBe(120000);
    expect(draft.query[0].key).toBe("q");
  });

  it("formats json response bodies", () => {
    expect(formatApiWorkbenchResponseBody("{\"ok\":true}", "application/json")).toBe(
      "{\n  \"ok\": true\n}",
    );
    expect(formatApiWorkbenchResponseBody("plain", "text/plain")).toBe("plain");
  });

  it("resets editor state when switching collections", () => {
    const state = buildApiWorkbenchSelectionState({
      nextCollection: {
        id: 2,
        activeEnvironmentId: 20,
      },
    });

    expect(state.selectedCollectionId).toBe(2);
    expect(state.selectedEnvironmentId).toBe(20);
    expect(state.selectedRequestId).toBeNull();
    expect(state.requestName).toBe("");
    expect(state.response).toBeNull();
    expect(state.draft).toEqual(normalizeApiWorkbenchDraft({}));
  });

  it("creates a blank request draft for the target folder", () => {
    const state = buildApiWorkbenchNewRequestState({ folderId: 12 });

    expect(state.selectedRequestId).toBeNull();
    expect(state.selectedRequestFolderId).toBe(12);
    expect(state.requestName).toBe("");
    expect(state.requestDescription).toBe("");
    expect(state.response).toBeNull();
    expect(state.draft).toEqual(normalizeApiWorkbenchDraft({}));
  });

  it("converts environment variables to editable rows and back", () => {
    const rows = draftApiWorkbenchEnvironmentRows([
      { name: "BASE_URL", value: "http://127.0.0.1:8080", isSecret: false },
      { name: "TOKEN", value: "abc", isSecret: true },
    ]);

    expect(rows).toEqual([
      { enabled: true, key: "BASE_URL", value: "http://127.0.0.1:8080" },
      { enabled: true, key: "TOKEN", value: "abc" },
    ]);
    expect(serializeApiWorkbenchEnvironmentRows(rows)).toEqual([
      { name: "BASE_URL", value: "http://127.0.0.1:8080", isSecret: false },
      { name: "TOKEN", value: "abc", isSecret: false },
    ]);
  });
});
