import { describe, expect, it } from "vitest";
import type { ApiWorkbenchCollection, ApiWorkbenchTab } from "../types/api-workbench";
import { createApiWorkbenchBlankDraft, normalizeApiWorkbenchDraft } from "./apiWorkbench";
import {
  API_WORKBENCH_MAX_TABS,
  backfillApiWorkbenchTabFolderIds,
  createApiWorkbenchTab,
  isApiWorkbenchTabDirty,
  normalizeApiWorkbenchRestoredTabs,
  pickApiWorkbenchNeighborTabId,
} from "./apiWorkbenchTabs";

function requestTab(overrides: Partial<ApiWorkbenchTab> = {}): ApiWorkbenchTab {
  const draft = normalizeApiWorkbenchDraft({ url: "/api/users", method: "GET" });
  return createApiWorkbenchTab({
    id: 1,
    kind: "request",
    requestId: 11,
    collectionId: 5,
    folderId: null,
    name: "List users",
    draft,
    savedSnapshot: { name: "List users", draft },
    ...overrides,
  });
}

function tempTab(overrides: Partial<ApiWorkbenchTab> = {}): ApiWorkbenchTab {
  return createApiWorkbenchTab({
    id: 2,
    kind: "temp",
    requestId: null,
    collectionId: 5,
    folderId: null,
    name: "",
    draft: createApiWorkbenchBlankDraft(),
    savedSnapshot: null,
    ...overrides,
  });
}

describe("isApiWorkbenchTabDirty", () => {
  it("request tab compares normalized draft and name against snapshot", () => {
    const clean = requestTab();
    expect(isApiWorkbenchTabDirty(clean)).toBe(false);

    const editedDraft = requestTab({
      draft: normalizeApiWorkbenchDraft({ url: "/api/users", method: "POST" }),
    });
    expect(isApiWorkbenchTabDirty(editedDraft)).toBe(true);

    const renamed = requestTab({ name: "Renamed" });
    expect(isApiWorkbenchTabDirty(renamed)).toBe(true);
  });

  it("request tab without snapshot counts as dirty", () => {
    expect(isApiWorkbenchTabDirty(requestTab({ savedSnapshot: null }))).toBe(true);
  });

  it("temp tab is dirty when draft has content or name set", () => {
    expect(isApiWorkbenchTabDirty(tempTab())).toBe(false);
    expect(
      isApiWorkbenchTabDirty(tempTab({ draft: normalizeApiWorkbenchDraft({ url: "/x" }) })),
    ).toBe(true);
    expect(isApiWorkbenchTabDirty(tempTab({ name: "草稿" }))).toBe(true);
  });
});

describe("pickApiWorkbenchNeighborTabId", () => {
  const tabs = [requestTab({ id: 1 }), tempTab({ id: 2 }), requestTab({ id: 3, requestId: 12 })];

  it("prefers right neighbor, then left, then null", () => {
    expect(pickApiWorkbenchNeighborTabId(tabs, 2)).toBe(3);
    expect(pickApiWorkbenchNeighborTabId(tabs, 3)).toBe(2);
    expect(pickApiWorkbenchNeighborTabId([tabs[0]], 1)).toBeNull();
  });

  it("returns null for unknown id", () => {
    expect(pickApiWorkbenchNeighborTabId(tabs, 99)).toBeNull();
  });
});

describe("normalizeApiWorkbenchRestoredTabs", () => {
  const ctx = {
    collectionIds: new Set([5]),
    requestIds: new Set([11]),
    fallbackCollectionId: 5 as number | null,
  };

  function persisted(tab: ApiWorkbenchTab) {
    const { response: _r, editorTab: _e, responseTab: _t, ...rest } = tab;
    return rest;
  }

  it("returns empty result for wrong version or malformed input", () => {
    expect(normalizeApiWorkbenchRestoredTabs(null, ctx).tabs).toEqual([]);
    expect(normalizeApiWorkbenchRestoredTabs({ version: 2, tabs: [] }, ctx).tabs).toEqual([]);
    expect(normalizeApiWorkbenchRestoredTabs({ version: 1, tabs: "x" }, ctx).tabs).toEqual([]);
  });

  it("restores valid tabs and keeps active id", () => {
    const raw = {
      version: 1,
      activeTabId: 1,
      tabs: [persisted(requestTab()), persisted(tempTab())],
    };
    const result = normalizeApiWorkbenchRestoredTabs(raw, ctx);
    expect(result.tabs).toHaveLength(2);
    expect(result.tabs[0].kind).toBe("request");
    expect(result.tabs[0].response).toBeNull();
    expect(result.activeTabId).toBe(1);
  });

  it("falls back active id to first tab when missing", () => {
    const raw = { version: 1, activeTabId: 99, tabs: [persisted(tempTab())] };
    expect(normalizeApiWorkbenchRestoredTabs(raw, ctx).activeTabId).toBe(2);
  });

  it("converts stale request tab to temp and reassigns stale collection", () => {
    const staleRequest = persisted(requestTab({ requestId: 999 }));
    const staleCollection = persisted(tempTab({ collectionId: 42, folderId: 7 }));
    const result = normalizeApiWorkbenchRestoredTabs(
      { version: 1, activeTabId: null, tabs: [staleRequest, staleCollection] },
      ctx,
    );
    expect(result.tabs[0].kind).toBe("temp");
    expect(result.tabs[0].requestId).toBeNull();
    expect(result.tabs[1].collectionId).toBe(5);
    expect(result.tabs[1].folderId).toBeNull();
  });

  it("drops invalid tab entries and truncates to the limit", () => {
    const many = Array.from({ length: API_WORKBENCH_MAX_TABS + 5 }, (_, i) =>
      persisted(tempTab({ id: i + 1 })),
    );
    const result = normalizeApiWorkbenchRestoredTabs(
      { version: 1, activeTabId: null, tabs: [{ bogus: true }, ...many] },
      ctx,
    );
    expect(result.tabs).toHaveLength(API_WORKBENCH_MAX_TABS);
  });
});

describe("backfillApiWorkbenchTabFolderIds", () => {
  const collections = [
    {
      id: 5,
      name: "Demo",
      description: "",
      activeEnvironmentId: null,
      sortOrder: 0,
      createdAt: "",
      updatedAt: "",
      folders: [],
      requests: [
        {
          id: 11,
          collectionId: 5,
          folderId: 3,
          name: "List users",
          method: "GET",
          url: "/api/users",
          sortOrder: 0,
        },
      ],
    },
  ] as unknown as ApiWorkbenchCollection[];

  it("syncs folder id for request tabs and leaves temp tabs untouched", () => {
    const tabs = [requestTab({ folderId: null }), tempTab({ folderId: 9 })];
    const next = backfillApiWorkbenchTabFolderIds(tabs, collections);
    expect(next[0].folderId).toBe(3);
    expect(next[1].folderId).toBe(9);
  });

  it("clears folder id when request no longer exists", () => {
    const tabs = [requestTab({ requestId: 999, folderId: 4 })];
    const next = backfillApiWorkbenchTabFolderIds(tabs, collections);
    expect(next[0].folderId).toBeNull();
  });
});
