import { describe, expect, it } from "vitest";
import {
  API_WORKBENCH_ENVIRONMENT_MANAGER_VALUE,
  buildApiWorkbenchEnvironmentDraftSummary,
  buildApiWorkbenchPreviewUrl,
  buildApiWorkbenchNewRequestState,
  buildApiWorkbenchSelectionState,
  draftApiWorkbenchEnvironmentRows,
  extractApiWorkbenchVariables,
  findDuplicateApiWorkbenchEnvironmentVariableNames,
  formatApiWorkbenchResponseBody,
  normalizeApiWorkbenchDraft,
  resolveApiWorkbenchEnvironmentSelect,
  serializeApiWorkbenchEnvironmentRows,
  splitApiWorkbenchUrlQuery,
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

  it("filters fully empty key-value rows while keeping partial rows", () => {
    const draft = normalizeApiWorkbenchDraft({
      query: [
        { enabled: true, key: "", value: "" },
        { enabled: true, key: "  ", value: "  " },
        { enabled: true, key: "", value: "v" },
        { enabled: true, key: "k", value: "" },
        { enabled: false, key: "off", value: "1" },
      ],
      headers: [{ enabled: true, key: "", value: "" }],
      form: [{ enabled: true, key: " ", value: "" }],
    });
    expect(draft.query).toEqual([
      { enabled: true, key: "", value: "v" },
      { enabled: true, key: "k", value: "" },
      { enabled: false, key: "off", value: "1" },
    ]);
    expect(draft.headers).toEqual([]);
    expect(draft.form).toEqual([]);
  });

  it("formats json response bodies", () => {
    expect(formatApiWorkbenchResponseBody("{\"ok\":true}", "application/json")).toBe(
      "{\n  \"ok\": true\n}",
    );
    expect(formatApiWorkbenchResponseBody("plain", "text/plain")).toBe("plain");
  });

  it("splits url query string into rows without url decoding", () => {
    expect(splitApiWorkbenchUrlQuery("/api?a=1&b=%20x")).toEqual({
      url: "/api",
      rows: [
        { enabled: true, key: "a", value: "1" },
        { enabled: true, key: "b", value: "%20x" },
      ],
    });
    expect(splitApiWorkbenchUrlQuery("https://x.com/p?a={{ID}}")).toEqual({
      url: "https://x.com/p",
      rows: [{ enabled: true, key: "a", value: "{{ID}}" }],
    });
    expect(splitApiWorkbenchUrlQuery("/p?flag")).toEqual({
      url: "/p",
      rows: [{ enabled: true, key: "flag", value: "" }],
    });
  });

  it("returns null when url has no splittable query", () => {
    expect(splitApiWorkbenchUrlQuery("/p?")).toBeNull();
    expect(splitApiWorkbenchUrlQuery("/p")).toBeNull();
    expect(splitApiWorkbenchUrlQuery("")).toBeNull();
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

  it("finds duplicate enabled environment variable names after trimming", () => {
    expect(
      findDuplicateApiWorkbenchEnvironmentVariableNames([
        { enabled: true, key: "TOKEN", value: "1" },
        { enabled: true, key: " TOKEN ", value: "2" },
        { enabled: false, key: "TOKEN", value: "ignored" },
        { enabled: true, key: "", value: "ignored" },
      ]),
    ).toEqual(["TOKEN"]);
  });

  it("summarizes environment draft status for management UI", () => {
    expect(
      buildApiWorkbenchEnvironmentDraftSummary(
        [
          { enabled: true, key: "BASE_URL", value: "http://127.0.0.1:8080" },
          { enabled: true, key: "TOKEN", value: "abc" },
          { enabled: false, key: "IGNORED", value: "x" },
          { enabled: true, key: "", value: "empty" },
        ],
        [
          { name: "BASE_URL", value: "http://127.0.0.1:8080", isSecret: false },
          { name: "TOKEN", value: "abc", isSecret: true },
        ],
      ),
    ).toEqual({
      variableCount: 2,
      hasBaseUrl: true,
      duplicateNames: [],
      changed: false,
    });

    expect(
      buildApiWorkbenchEnvironmentDraftSummary(
        [
          { enabled: true, key: "BASE_URL", value: "" },
          { enabled: true, key: "TOKEN", value: "next" },
          { enabled: true, key: " TOKEN ", value: "duplicate" },
        ],
        [{ name: "TOKEN", value: "abc", isSecret: false }],
      ),
    ).toEqual({
      variableCount: 3,
      hasBaseUrl: false,
      duplicateNames: ["TOKEN"],
      changed: true,
    });
  });

  it("resolves environment selector values without storing the manage option", () => {
    expect(resolveApiWorkbenchEnvironmentSelect(12, 5)).toEqual({
      kind: "environment",
      environmentId: 12,
    });
    expect(
      resolveApiWorkbenchEnvironmentSelect(API_WORKBENCH_ENVIRONMENT_MANAGER_VALUE, 5),
    ).toEqual({
      kind: "manage",
      environmentId: 5,
    });
  });
});
