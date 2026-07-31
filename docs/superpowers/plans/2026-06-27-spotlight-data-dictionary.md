# Spotlight Data Dictionary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let Spotlight search data dictionary records on demand, open a selected record in the data dictionary tool, and copy visible fields or full JSON from the action menu.

**Architecture:** Extend the existing data dictionary search response with backend-built `title` and `summary`, while keeping `rawJson` optional through `includeRawJson`. Add an optional query-time `search` hook to Spotlight providers so data dictionary records are never prefetched on empty input; Spotlight merges query-time results with existing prefetched items under request-sequence protection.

**Tech Stack:** Tauri 2, Vue 3, TypeScript, Vitest, Rust, rusqlite, SQLite.

---

## Scope And Current State

Design spec: `docs/superpowers/specs/2026-06-27-spotlight-data-dictionary-design.md`

Current worktree note:

- `docs/superpowers/specs/2026-06-27-spotlight-data-dictionary-design.md` has uncommitted edits that update the design from prefetch to query-time provider behavior. Preserve that change.
- The most recent commit is `8156163 docs(spotlight): 设计数据字典接入`.
- Do not revert unrelated user edits. Before staging, inspect `git diff` and stage only hunks belonging to this implementation.

Relevant existing behavior:

- `apps/desktop/src/components/SpotlightPanel.vue` currently calls every enabled provider's `prefetch()` and uses `searchItems()` for local fuzzy matching.
- `ProviderDescriptor` in `apps/desktop/src/spotlight/types.ts` has no query-time `search` hook yet.
- `DataDictionarySearchItem` in `apps/desktop/src/types/data-dictionary.ts` currently requires `rawJson` and does not include `title` or `summary`.
- Rust `rows_to_search_items()` currently parses invalid JSON as `Value::Null`; this must become an explicit error.
- `DataDictionaryPanel.vue` already has search request sequencing and `record-detail` lazy loading. Add focus navigation by reusing those patterns, not by rewriting search.

---

## File Structure

### `apps/desktop/src-tauri/src/tools/data_dictionary.rs`

Responsibility: data dictionary search result construction.

Changes:

- Parse `includeRawJson` from search payload, defaulting to `true`.
- Return `title` and `summary` in each search item.
- Omit `rawJson` when `includeRawJson: false`.
- Cache both searchable paths and field configs by dictionary in `rows_to_search_items`.
- Return a parse error if a record's `raw_json` is invalid.
- Add Rust tests for title, summary, label ordering, raw JSON omission, and parse failure.

### `apps/desktop/src/types/data-dictionary.ts`

Responsibility: frontend data dictionary API types.

Changes:

- Make `DataDictionarySearchItem.rawJson` optional.
- Add `title: string`.
- Add `summary: DataDictionaryRecordSummaryPart[]`.
- Add `DataDictionarySearchRequest` with `includeRawJson?: boolean`.

### `apps/desktop/src/spotlight/types.ts`

Responsibility: Spotlight provider contracts.

Changes:

- Add `"data-dictionary"` to `SpotlightProviderId`.
- Add `SpotlightSearchContext`.
- Add optional `ProviderDescriptor.search(query, ctx)`.

### `apps/desktop/src/spotlight/search.ts`

Responsibility: pure query-time merge and threshold logic for Spotlight.

Create this file to keep the component small and testable.

Exports:

- `shouldRunQueryProvider(query, scope, providerId)`.
- `mergeSpotlightProviderItems(prefetched, queryTime)`.

### `apps/desktop/src/spotlight/search.test.ts`

Responsibility: pure tests for query-time thresholds and merge dedupe.

### `apps/desktop/src/spotlight/providers/data-dictionary.ts`

Responsibility: map backend data dictionary records to `SpotlightItem` and execute actions.

Create this provider. It owns only Spotlight-specific item construction and actions. It must not reimplement field-path parsing or summary building.

### `apps/desktop/src/spotlight/providers/data-dictionary.test.ts`

Responsibility: provider tests for item construction, search thresholds, actions, lazy full JSON copy, and no `rawJson` payload.

### `apps/desktop/src/components/SpotlightPanel.vue`

Responsibility: UI state and request orchestration.

Changes:

- Import/register data dictionary provider.
- Maintain `queryItemsByProvider` separately from `itemsByProvider`.
- Trigger enabled query-time providers when parsed query changes.
- Use request sequence to drop stale query-time responses.
- Merge prefetched and query-time maps before computing results.

### `apps/desktop/src/components/settings/SpotlightSettings.vue`

Responsibility: provider registration for settings view.

Changes:

- Import/register data dictionary provider so it appears in settings.

### `apps/desktop/src/composables/useDataDictionaryNavigation.ts`

Responsibility: cross-window focus request storage for data dictionary record navigation.

Create a small composable matching `useTodoNavigation`.

### `apps/desktop/src/App.vue`

Responsibility: handle `hotkey-navigate` and open the target tool.

Changes:

- For `target === "data-dictionary"` with numeric `itemId`, call `useDataDictionaryNavigation().requestFocus(recordId)`.

### `apps/desktop/src/components/DataDictionaryPanel.vue`

Responsibility: consume data dictionary focus requests.

Changes:

- On mount, consume `useDataDictionaryNavigation().consumeFocus()`.
- Switch to global search scope.
- Select target from current results if present.
- Otherwise call `record-detail`, set `recordDetail`, and show an explicit warning if not found.
- Do not mutate the user's current keyword for focus navigation.

### Tests To Update

- `apps/desktop/src/spotlight/config-store.test.ts`
  - Import data dictionary provider.
  - Keep descriptor helper compatible with optional `search`.
- `apps/desktop/src/utils/spotlight-query.test.ts`
  - Add `dd` / `dict` alias coverage when a custom alias map contains the provider.
- `apps/desktop/src/utils/hotkeyNavigate.test.ts`
  - Add pure helper only if App navigation is extracted; otherwise leave unchanged and use a source guard.
- `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`
  - Add a narrow source guard for focus navigation if no component test harness exists.

---

## Task 1: Extend Rust Data Dictionary Search Result

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/data_dictionary.rs`
- Modify: `apps/desktop/src/types/data-dictionary.ts`

- [ ] **Step 1: Add failing Rust tests for search item title, summary, and `includeRawJson`**

Add these tests inside the existing `#[cfg(test)] mod tests` in `apps/desktop/src-tauri/src/tools/data_dictionary.rs`:

```rust
#[test]
fn search_item_includes_title_summary_and_can_omit_raw_json() {
    let row = test_record_row(
        10,
        2,
        json!({ "id": 1001, "name": "张三", "dept": "研发", "secret": "hidden" }),
    );
    let fields = vec![
        test_field_config("id", "编号", true, 0),
        test_field_config("name", "姓名", true, 1),
        test_field_config("dept", "部门", true, 2),
        test_field_config("secret", "密钥", false, 3),
    ];

    let item = record_row_to_search_item_json(
        row,
        &["name".to_string(), "dept".to_string()],
        &fields,
        "张",
        false,
    )
    .unwrap();

    assert_eq!(item["title"], "测试字典 #3");
    assert_eq!(item["summary"], json!([
        { "fieldPath": "id", "label": "编号", "value": "1001" },
        { "fieldPath": "name", "label": "姓名", "value": "张三" },
        { "fieldPath": "dept", "label": "部门", "value": "研发" }
    ]));
    assert_eq!(item["matches"], json!([
        { "fieldPath": "name", "value": "张三" }
    ]));
    assert!(item.get("rawJson").is_none());
}

#[test]
fn search_item_keeps_raw_json_by_default_shape() {
    let mut row = test_record_row(5, 0, json!({ "name": "张三" }));
    row.title_field_path = Some("name".to_string());
    let fields = vec![test_field_config("name", "姓名", true, 0)];

    let item = record_row_to_search_item_json(
        row,
        &["name".to_string()],
        &fields,
        "",
        true,
    )
    .unwrap();

    assert_eq!(item["title"], "张三");
    assert_eq!(item["rawJson"], json!({ "name": "张三" }));
    assert_eq!(item["summary"], json!([]));
}

#[test]
fn search_item_returns_parse_error_for_invalid_raw_json() {
    let mut row = test_record_row(8, 0, json!({ "name": "valid" }));
    row.raw_json = "{ invalid json".to_string();
    let err = record_row_to_search_item_json(row, &[], &[], "", false).unwrap_err();

    assert!(err.contains("parse data dictionary record 8 failed"));
}
```

- [ ] **Step 2: Run focused Rust tests and verify failure**

Run:

```powershell
cargo test data_dictionary::tests::search_item_ -- --nocapture
```

Expected: FAIL because `record_row_to_search_item_json` does not exist yet.

- [ ] **Step 3: Implement `includeRawJson` parsing and search item construction**

In `action_search`, parse the flag:

```rust
let include_raw_json = payload["includeRawJson"].as_bool().unwrap_or(true);
```

Pass it into `rows_to_search_items`:

```rust
let items = rows_to_search_items(&conn, rows, keyword, include_raw_json)?;
```

Replace the existing `rows_to_search_items` function signature and body with:

```rust
fn rows_to_search_items(
    conn: &Connection,
    rows: Vec<RecordRow>,
    keyword: &str,
    include_raw_json: bool,
) -> Result<Vec<Value>, String> {
    let mut searchable_cache: HashMap<i64, Vec<String>> = HashMap::new();
    let mut field_cache: HashMap<i64, Vec<FieldConfig>> = HashMap::new();
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let paths = if let Some(paths) = searchable_cache.get(&row.dictionary_id) {
            paths.clone()
        } else {
            let paths = load_searchable_paths(conn, row.dictionary_id)?;
            searchable_cache.insert(row.dictionary_id, paths.clone());
            paths
        };
        let fields = if let Some(fields) = field_cache.get(&row.dictionary_id) {
            fields.clone()
        } else {
            let fields = load_field_configs(conn, row.dictionary_id)?;
            field_cache.insert(row.dictionary_id, fields.clone());
            fields
        };
        out.push(record_row_to_search_item_json(
            row,
            &paths,
            &fields,
            keyword,
            include_raw_json,
        )?);
    }
    Ok(out)
}
```

Add this helper next to `record_row_to_brief_json`:

```rust
fn record_row_to_search_item_json(
    row: RecordRow,
    searchable_paths: &[String],
    fields: &[FieldConfig],
    keyword: &str,
    include_raw_json: bool,
) -> Result<Value, String> {
    let record = serde_json::from_str::<Value>(&row.raw_json)
        .map_err(|e| format!("parse data dictionary record {} failed: {e}", row.id))?;
    let title = build_record_title(
        &record,
        row.title_field_path.as_deref(),
        &row.dictionary_name,
        row.row_index,
    );
    let summary = build_record_summary(&record, fields, row.title_field_path.as_deref());
    let mut item = json!({
        "id": row.id,
        "dictionaryId": row.dictionary_id,
        "dictionaryName": row.dictionary_name,
        "titleFieldPath": row.title_field_path,
        "rowIndex": row.row_index,
        "matches": compute_matches(&record, searchable_paths, keyword),
        "title": title,
        "summary": summary,
    });
    if include_raw_json {
        if let Some(object) = item.as_object_mut() {
            object.insert("rawJson".to_string(), record);
        }
    }
    Ok(item)
}
```

If `FieldConfig` is not cloneable yet, add `Clone` to its derive:

```rust
#[derive(Clone)]
struct FieldConfig {
```

- [ ] **Step 4: Update frontend data dictionary types**

In `apps/desktop/src/types/data-dictionary.ts`, move `DataDictionaryRecordSummaryPart` above `DataDictionarySearchItem` if needed, then update the search item:

```ts
export interface DataDictionarySearchItem {
  id: number;
  dictionaryId: number;
  dictionaryName: string;
  titleFieldPath: string | null;
  rowIndex: number;
  rawJson?: unknown;
  matches: DataDictionaryMatch[];
  title: string;
  summary: DataDictionaryRecordSummaryPart[];
}

export interface DataDictionarySearchRequest {
  scope: DataDictionarySearchScope;
  dictionaryId?: number;
  keyword?: string;
  limit?: number;
  includeRawJson?: boolean;
}
```

- [ ] **Step 5: Run focused validation**

Run:

```powershell
cargo test data_dictionary::tests::search_item_ -- --nocapture
pnpm typecheck
```

Expected: Rust focused tests PASS and TypeScript types compile.

- [ ] **Step 6: Commit backend API extension**

Run:

```powershell
git diff -- apps/desktop/src-tauri/src/tools/data_dictionary.rs apps/desktop/src/types/data-dictionary.ts
git add apps/desktop/src-tauri/src/tools/data_dictionary.rs apps/desktop/src/types/data-dictionary.ts
git commit -m "feat(data-dictionary): 扩展搜索结果摘要"
```

Expected: commit succeeds. If unrelated user edits exist in these files, stage only relevant hunks.

---

## Task 2: Add Query-Time Provider Contract And Pure Merge Tests

**Files:**

- Modify: `apps/desktop/src/spotlight/types.ts`
- Create: `apps/desktop/src/spotlight/search.ts`
- Create: `apps/desktop/src/spotlight/search.test.ts`

- [ ] **Step 1: Add failing pure tests for query thresholds and dedupe merge**

Create `apps/desktop/src/spotlight/search.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { SpotlightItem } from "./types";
import { mergeSpotlightProviderItems, shouldRunQueryProvider } from "./search";

function item(providerId: SpotlightItem["providerId"], itemId: string): SpotlightItem {
  return {
    providerId,
    itemId,
    title: `${providerId}:${itemId}`,
    searchFields: [],
  };
}

describe("shouldRunQueryProvider", () => {
  it("does not run query provider for empty input", () => {
    expect(shouldRunQueryProvider("", null, "data-dictionary")).toBe(false);
    expect(shouldRunQueryProvider("   ", "data-dictionary", "data-dictionary")).toBe(false);
  });

  it("requires two characters in global search", () => {
    expect(shouldRunQueryProvider("a", null, "data-dictionary")).toBe(false);
    expect(shouldRunQueryProvider("ab", null, "data-dictionary")).toBe(true);
  });

  it("allows one character when scoped to the same provider", () => {
    expect(shouldRunQueryProvider("a", "data-dictionary", "data-dictionary")).toBe(true);
    expect(shouldRunQueryProvider("a", "todo", "data-dictionary")).toBe(false);
  });
});

describe("mergeSpotlightProviderItems", () => {
  it("keeps query-time items and dedupes by provider item key", () => {
    const prefetched = new Map([
      ["tool", [item("tool", "json")]],
      ["todo", [item("todo", "1")]],
    ] as const);
    const queryTime = new Map([
      ["todo", [item("todo", "1"), item("todo", "2")]],
      ["data-dictionary", [item("data-dictionary", "9")]],
    ] as const);

    const merged = mergeSpotlightProviderItems(prefetched, queryTime);

    expect(merged.get("tool")?.map((entry) => entry.itemId)).toEqual(["json"]);
    expect(merged.get("todo")?.map((entry) => entry.itemId)).toEqual(["1", "2"]);
    expect(merged.get("data-dictionary")?.map((entry) => entry.itemId)).toEqual(["9"]);
  });
});
```

- [ ] **Step 2: Run the failing test**

Run:

```powershell
pnpm test src/spotlight/search.test.ts
```

Expected: FAIL because `src/spotlight/search.ts` does not exist.

- [ ] **Step 3: Extend Spotlight types**

In `apps/desktop/src/spotlight/types.ts`, add the provider id:

```ts
  | "data-dictionary"
```

Add search context near `SpotlightExecuteContext`:

```ts
export interface SpotlightSearchContext {
  scope: SpotlightProviderId | null;
}
```

Add the optional hook to `ProviderDescriptor`:

```ts
  search?: (
    query: string,
    ctx: SpotlightSearchContext,
  ) => Promise<SpotlightItem[]>;
```

- [ ] **Step 4: Implement pure search helpers**

Create `apps/desktop/src/spotlight/search.ts`:

```ts
import type { SpotlightItem, SpotlightProviderId } from "./types";

export function shouldRunQueryProvider(
  query: string,
  scope: SpotlightProviderId | null,
  providerId: SpotlightProviderId,
): boolean {
  const text = query.trim();
  if (!text) return false;
  if (scope) return scope === providerId && text.length >= 1;
  return text.length >= 2;
}

export function mergeSpotlightProviderItems(
  prefetched: Map<SpotlightProviderId, SpotlightItem[]>,
  queryTime: Map<SpotlightProviderId, SpotlightItem[]>,
): Map<SpotlightProviderId, SpotlightItem[]> {
  const merged = new Map<SpotlightProviderId, SpotlightItem[]>();
  const providerIds = new Set<SpotlightProviderId>([...prefetched.keys(), ...queryTime.keys()]);

  for (const providerId of providerIds) {
    const seen = new Set<string>();
    const items: SpotlightItem[] = [];
    for (const source of [prefetched.get(providerId) ?? [], queryTime.get(providerId) ?? []]) {
      for (const item of source) {
        const key = `${item.providerId}:${item.itemId}`;
        if (seen.has(key)) continue;
        seen.add(key);
        items.push(item);
      }
    }
    merged.set(providerId, items);
  }

  return merged;
}
```

- [ ] **Step 5: Keep config-store tests compiling with optional `search`**

In `apps/desktop/src/spotlight/config-store.test.ts`, update `makeDescriptor` to pass through `search`:

```ts
    search: over.search,
```

This is a compile-safety update; no behavior change is expected.

- [ ] **Step 6: Run pure tests and typecheck**

Run:

```powershell
pnpm test src/spotlight/search.test.ts src/spotlight/config-store.test.ts
pnpm typecheck
```

Expected: tests PASS and TypeScript compiles.

- [ ] **Step 7: Commit provider contract**

Run:

```powershell
git diff -- apps/desktop/src/spotlight/types.ts apps/desktop/src/spotlight/search.ts apps/desktop/src/spotlight/search.test.ts apps/desktop/src/spotlight/config-store.test.ts
git add apps/desktop/src/spotlight/types.ts apps/desktop/src/spotlight/search.ts apps/desktop/src/spotlight/search.test.ts apps/desktop/src/spotlight/config-store.test.ts
git commit -m "feat(spotlight): 支持按查询加载数据源"
```

Expected: commit succeeds.

---

## Task 3: Create Data Dictionary Spotlight Provider

**Files:**

- Create: `apps/desktop/src/spotlight/providers/data-dictionary.ts`
- Create: `apps/desktop/src/spotlight/providers/data-dictionary.test.ts`
- Modify: `apps/desktop/src/spotlight/config-store.test.ts`
- Modify: `apps/desktop/src/utils/spotlight-query.test.ts`

- [ ] **Step 1: Add failing provider tests**

Create `apps/desktop/src/spotlight/providers/data-dictionary.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeToolByChannel = vi.fn();
const invoke = vi.fn();

vi.mock("../../bridge/tauri", () => ({
  invokeToolByChannel: (...args: unknown[]) => invokeToolByChannel(...args),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import {
  buildDataDictionaryActions,
  buildDataDictionaryItem,
  dataDictionaryProvider,
} from "./data-dictionary";
import type { DataDictionarySearchItem } from "../../types/data-dictionary";

const searchItem: DataDictionarySearchItem = {
  id: 12,
  dictionaryId: 3,
  dictionaryName: "用户字典",
  titleFieldPath: "name",
  rowIndex: 0,
  matches: [{ fieldPath: "name", value: "张三" }],
  title: "张三",
  summary: [
    { fieldPath: "id", label: "编号", value: "1001" },
    { fieldPath: "dept", label: "部门", value: "研发" },
  ],
};

beforeEach(() => {
  invokeToolByChannel.mockReset();
  invoke.mockReset();
  Object.assign(navigator, {
    clipboard: {
      writeText: vi.fn(async () => undefined),
    },
  });
});

describe("buildDataDictionaryItem", () => {
  it("maps backend search item to Spotlight item without rawJson payload", () => {
    const item = buildDataDictionaryItem({
      ...searchItem,
      rawJson: { id: 1001, name: "张三" },
    });

    expect(item.providerId).toBe("data-dictionary");
    expect(item.itemId).toBe("12");
    expect(item.title).toBe("张三");
    expect(item.subtitle).toBe("用户字典 · 编号：1001 · 部门：研发");
    expect(item.status).toEqual({ text: "2 字段", tone: "muted" });
    expect(item.payload?.recordId).toBe(12);
    expect(item.payload?.dictionaryId).toBe(3);
    expect(item.payload?.rawJson).toBeUndefined();
    expect(item.searchFields.map((field) => field.text)).toContain("张三");
    expect(item.searchFields.map((field) => field.text)).toContain("用户字典");
    expect(item.searchFields.map((field) => field.text)).toContain("编号 1001");
  });

  it("omits status when no summary fields exist", () => {
    const item = buildDataDictionaryItem({ ...searchItem, summary: [] });
    expect(item.status).toBeUndefined();
  });
});

describe("dataDictionaryProvider search", () => {
  it("requests all dictionaries with includeRawJson disabled", async () => {
    invokeToolByChannel.mockResolvedValue({ items: [searchItem] });

    const results = await dataDictionaryProvider.search?.("张", {
      scope: "data-dictionary",
    });

    expect(invokeToolByChannel).toHaveBeenCalledWith("tool:data-dictionary:search", {
      scope: "all",
      keyword: "张",
      limit: 50,
      includeRawJson: false,
    });
    expect(results?.[0].title).toBe("张三");
  });

  it("returns empty array for empty query and provider failures", async () => {
    expect(await dataDictionaryProvider.search?.("", { scope: null })).toEqual([]);
    invokeToolByChannel.mockRejectedValue(new Error("boom"));
    expect(await dataDictionaryProvider.search?.("张三", { scope: null })).toEqual([]);
  });
});

describe("data dictionary actions", () => {
  it("builds copy actions for visible fields plus full JSON", () => {
    const actions = buildDataDictionaryActions(buildDataDictionaryItem(searchItem));
    expect(actions.map((action) => action.id)).toEqual([
      "copy_field:0",
      "copy_field:1",
      "copy_raw_json",
    ]);
  });

  it("copies a summary field", async () => {
    const result = await dataDictionaryProvider.executeAction?.(
      buildDataDictionaryItem(searchItem),
      "copy_field:1",
      {} as never,
    );

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("研发");
    expect(result).toEqual({
      closeSpotlight: true,
      toast: { message: "字段值已复制", type: "success" },
    });
  });

  it("loads record detail lazily before copying full JSON", async () => {
    invokeToolByChannel.mockResolvedValue({
      record: {
        id: 12,
        dictionaryId: 3,
        dictionaryName: "用户字典",
        title: "张三",
        rowIndex: 0,
        summary: [],
        rawJson: { id: 1001, name: "张三" },
      },
      fields: [],
      forwardRelations: [],
      reverseRelations: [],
    });

    const result = await dataDictionaryProvider.executeAction?.(
      buildDataDictionaryItem(searchItem),
      "copy_raw_json",
      {} as never,
    );

    expect(invokeToolByChannel).toHaveBeenCalledWith("tool:data-dictionary:record-detail", {
      recordId: 12,
    });
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
      JSON.stringify({ id: 1001, name: "张三" }, null, 2),
    );
    expect(result?.closeSpotlight).toBe(true);
  });
});
```

- [ ] **Step 2: Run the failing provider tests**

Run:

```powershell
pnpm test src/spotlight/providers/data-dictionary.test.ts
```

Expected: FAIL because the provider file does not exist.

- [ ] **Step 3: Implement data dictionary provider**

Create `apps/desktop/src/spotlight/providers/data-dictionary.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import { invokeToolByChannel } from "../../bridge/tauri";
import { toPinyinInitials } from "../../utils/fuzzy-match";
import { registerProvider } from "../registry";
import type {
  DataDictionaryRecordDetail,
  DataDictionaryRecordSummaryPart,
  DataDictionarySearchItem,
  DataDictionarySearchResult,
} from "../../types/data-dictionary";
import type {
  ProviderDescriptor,
  SpotlightAction,
  SpotlightExecuteResult,
  SpotlightItem,
} from "../types";

interface DataDictionaryPayload {
  recordId: number;
  dictionaryId: number;
  summary: DataDictionaryRecordSummaryPart[];
}

function makeField(text: string, weight: number) {
  const cleaned = text.trim();
  return {
    text: cleaned,
    initials: toPinyinInitials(cleaned),
    weight,
  };
}

function truncate(text: string, max: number): string {
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

function payloadOf(item: SpotlightItem): DataDictionaryPayload | null {
  const recordId = item.payload?.recordId;
  const dictionaryId = item.payload?.dictionaryId;
  const summary = item.payload?.summary;
  if (typeof recordId !== "number" || typeof dictionaryId !== "number") return null;
  if (!Array.isArray(summary)) return null;
  return { recordId, dictionaryId, summary: summary as DataDictionaryRecordSummaryPart[] };
}

export function buildDataDictionaryItem(row: DataDictionarySearchItem): SpotlightItem {
  const summary = Array.isArray(row.summary) ? row.summary : [];
  const subtitleParts = [
    row.dictionaryName,
    ...summary.slice(0, 3).map((part) => `${part.label}：${part.value}`),
  ].filter(Boolean);
  const searchFields = [
    makeField(row.title, 1.2),
    makeField(row.dictionaryName, 0.8),
    ...summary.flatMap((part) => [
      makeField(part.label, 0.5),
      makeField(`${part.label} ${part.value}`, 0.9),
      makeField(part.value, 0.9),
    ]),
    ...row.matches.flatMap((match) => [
      makeField(match.fieldPath, 0.5),
      makeField(match.value, 0.9),
    ]),
  ].filter((field) => field.text);

  return {
    providerId: "data-dictionary",
    itemId: String(row.id),
    title: row.title || `${row.dictionaryName} #${row.rowIndex + 1}`,
    subtitle: truncate(subtitleParts.join(" · "), 96),
    badge: { short: "典", tone: "info" },
    status: summary.length > 0 ? { text: `${summary.length} 字段`, tone: "muted" } : undefined,
    searchFields,
    payload: {
      recordId: row.id,
      dictionaryId: row.dictionaryId,
      summary,
    },
  };
}

async function searchDataDictionary(query: string): Promise<SpotlightItem[]> {
  const keyword = query.trim();
  if (!keyword) return [];
  try {
    const result = (await invokeToolByChannel("tool:data-dictionary:search", {
      scope: "all",
      keyword,
      limit: 50,
      includeRawJson: false,
    })) as DataDictionarySearchResult;
    return Array.isArray(result?.items)
      ? result.items.map((row) => buildDataDictionaryItem(row))
      : [];
  } catch (err) {
    console.warn("[Spotlight] data dictionary search failed:", err);
    return [];
  }
}

async function openRecord(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  const payload = payloadOf(item);
  if (!payload) return { errorMessage: "无效数据字典记录" };
  await invoke("spotlight_pick", {
    target: "data-dictionary",
    itemId: String(payload.recordId),
  });
  return { closeSpotlight: true };
}

export function buildDataDictionaryActions(item: SpotlightItem): SpotlightAction[] {
  const payload = payloadOf(item);
  const summary = payload?.summary ?? [];
  return [
    ...summary.map((part, index) => ({
      id: `copy_field:${index}`,
      label: `复制${part.label}`,
      icon: "copy",
    })),
    {
      id: "copy_raw_json",
      label: "复制完整 JSON",
      icon: "copy",
    },
  ];
}

async function copySummaryField(
  item: SpotlightItem,
  actionId: string,
): Promise<SpotlightExecuteResult> {
  const payload = payloadOf(item);
  if (!payload) return { errorMessage: "无效数据字典记录" };
  const indexText = actionId.slice("copy_field:".length);
  const index = Number(indexText);
  if (!Number.isInteger(index) || index < 0 || index >= payload.summary.length) {
    return { errorMessage: "字段不存在" };
  }
  try {
    await navigator.clipboard.writeText(payload.summary[index].value);
    return {
      closeSpotlight: true,
      toast: { message: "字段值已复制", type: "success" },
    };
  } catch {
    return { errorMessage: "复制字段值失败" };
  }
}

async function copyRawJson(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  const payload = payloadOf(item);
  if (!payload) return { errorMessage: "无效数据字典记录" };
  try {
    const detail = (await invokeToolByChannel("tool:data-dictionary:record-detail", {
      recordId: payload.recordId,
    })) as DataDictionaryRecordDetail;
    const text = JSON.stringify(detail.record.rawJson, null, 2);
    await navigator.clipboard.writeText(text);
    return {
      closeSpotlight: true,
      toast: { message: "完整 JSON 已复制", type: "success" },
    };
  } catch {
    return { errorMessage: "复制 JSON 失败" };
  }
}

async function executeAction(
  item: SpotlightItem,
  actionId: string,
): Promise<SpotlightExecuteResult> {
  if (actionId.startsWith("copy_field:")) return copySummaryField(item, actionId);
  if (actionId === "copy_raw_json") return copyRawJson(item);
  return { errorMessage: `未知动作 ${actionId}` };
}

export const dataDictionaryProvider: ProviderDescriptor = {
  id: "data-dictionary",
  name: "数据字典",
  description: "搜索数据字典记录",
  badgeShort: "典",
  badgeTone: "info",
  weight: 0.72,
  defaultAliases: ["dd", "dict"],
  defaultEnabled: true,
  prefetch: async () => [],
  search: searchDataDictionary,
  defaultAction: openRecord,
  buildActions: buildDataDictionaryActions,
  executeAction,
};

registerProvider(dataDictionaryProvider);
```

- [ ] **Step 4: Register provider in config-store tests and alias parser tests**

In `apps/desktop/src/spotlight/config-store.test.ts`, add:

```ts
import "./providers/data-dictionary";
```

In `apps/desktop/src/utils/spotlight-query.test.ts`, add:

```ts
it("recognizes data dictionary aliases when provided by config", () => {
  const map = new Map<string, SpotlightProviderId>([
    ["dd", "data-dictionary"],
    ["dict", "data-dictionary"],
  ]);
  expect(parseSpotlightQuery("dd 张三", map)).toEqual({
    scope: "data-dictionary",
    query: "张三",
  });
  expect(parseSpotlightQuery("dict 1001", map)).toEqual({
    scope: "data-dictionary",
    query: "1001",
  });
});
```

- [ ] **Step 5: Run provider and related tests**

Run:

```powershell
pnpm test src/spotlight/providers/data-dictionary.test.ts src/spotlight/config-store.test.ts src/utils/spotlight-query.test.ts
pnpm typecheck
```

Expected: tests PASS and TypeScript compiles.

- [ ] **Step 6: Commit provider**

Run:

```powershell
git diff -- apps/desktop/src/spotlight/providers/data-dictionary.ts apps/desktop/src/spotlight/providers/data-dictionary.test.ts apps/desktop/src/spotlight/config-store.test.ts apps/desktop/src/utils/spotlight-query.test.ts
git add apps/desktop/src/spotlight/providers/data-dictionary.ts apps/desktop/src/spotlight/providers/data-dictionary.test.ts apps/desktop/src/spotlight/config-store.test.ts apps/desktop/src/utils/spotlight-query.test.ts
git commit -m "feat(spotlight): 接入数据字典记录源"
```

Expected: commit succeeds.

---

## Task 4: Wire Query-Time Search Into SpotlightPanel

**Files:**

- Modify: `apps/desktop/src/components/SpotlightPanel.vue`
- Modify: `apps/desktop/src/components/settings/SpotlightSettings.vue`
- Modify: `apps/desktop/src/spotlight/search.test.ts`

- [ ] **Step 1: Add a pure stale-response test**

Update the existing import from `./search` in `apps/desktop/src/spotlight/search.test.ts` to include the new helper:

```ts
import {
  createQueryTimeResultGuard,
  mergeSpotlightProviderItems,
  shouldRunQueryProvider,
} from "./search";
```

Append this test after the existing tests:

```ts
describe("createQueryTimeResultGuard", () => {
  it("accepts only the latest query signature", () => {
    const guard = createQueryTimeResultGuard();
    const first = guard.next("a", null);
    const second = guard.next("ab", null);

    expect(guard.isCurrent(first, "a", null)).toBe(false);
    expect(guard.isCurrent(second, "ab", null)).toBe(true);
    expect(guard.isCurrent(second, "ab", "data-dictionary")).toBe(false);
  });
});
```

- [ ] **Step 2: Run the failing pure test**

Run:

```powershell
pnpm test src/spotlight/search.test.ts
```

Expected: FAIL because `createQueryTimeResultGuard` does not exist.

- [ ] **Step 3: Add the request guard helper**

In `apps/desktop/src/spotlight/search.ts`, add:

```ts
export function createQueryTimeResultGuard() {
  let seq = 0;
  let latestQuery = "";
  let latestScope: SpotlightProviderId | null = null;
  return {
    next(query: string, scope: SpotlightProviderId | null): number {
      seq += 1;
      latestQuery = query;
      latestScope = scope;
      return seq;
    },
    isCurrent(requestSeq: number, query: string, scope: SpotlightProviderId | null): boolean {
      if (requestSeq !== seq) return false;
      return query === latestQuery && scope === latestScope;
    },
  };
}
```

- [ ] **Step 4: Import and register data dictionary provider in both UI entry points**

In `apps/desktop/src/components/SpotlightPanel.vue`, add:

```ts
import "../spotlight/providers/data-dictionary";
```

after the existing provider imports.

In `apps/desktop/src/components/settings/SpotlightSettings.vue`, add:

```ts
import "../../spotlight/providers/data-dictionary";
```

after the existing provider imports.

- [ ] **Step 5: Add query-time state and merged item map to SpotlightPanel**

In `SpotlightPanel.vue`, update imports:

```ts
import {
  createQueryTimeResultGuard,
  mergeSpotlightProviderItems,
  shouldRunQueryProvider,
} from "../spotlight/search";
```

Add state near `itemsByProvider`:

```ts
const queryItemsByProvider = ref<ScopedItemsMap>(new Map());
const queryLoading = ref(false);
const queryGuard = createQueryTimeResultGuard();
```

Add computed merged map:

```ts
const searchableItemsByProvider = computed(() =>
  mergeSpotlightProviderItems(itemsByProvider.value, queryItemsByProvider.value),
);
```

Update loading computed:

```ts
const isLoadingView = computed(() => loading.value || keywordLoading.value || queryLoading.value);
```

Replace the non-empty query `searchItems` call to use the merged map:

```ts
return searchItems(text, searchableItemsByProvider.value, {
  scope: scope.value,
  limit: RESULT_LIMIT,
  enabledIds: enabledProviderIds.value ?? undefined,
});
```

Replace the empty-query iteration:

```ts
for (const [pid, items] of searchableItemsByProvider.value) {
```

- [ ] **Step 6: Add query-time refresh function and watcher**

In `SpotlightPanel.vue`, add:

```ts
async function refreshQueryProviders() {
  if (keywordInvocation.value || quickCommand.value) {
    queryItemsByProvider.value = new Map();
    queryLoading.value = false;
    return;
  }

  const text = parsed.value.query;
  const currentScope = scope.value;
  const requestSeq = queryGuard.next(text, currentScope);
  const v = view.value;
  const providers = (v ? v.providers : listProviders()).filter((provider) => {
    if (!provider.enabled && v) return false;
    if (!provider.search) return false;
    if (currentScope && provider.id !== currentScope) return false;
    return shouldRunQueryProvider(text, currentScope, provider.id);
  });

  if (providers.length === 0) {
    queryItemsByProvider.value = new Map();
    queryLoading.value = false;
    return;
  }

  queryLoading.value = true;
  const next = new Map<SpotlightProviderId, SpotlightItem[]>();
  await Promise.allSettled(
    providers.map(async (provider) => {
      try {
        const items = await provider.search!(text, { scope: currentScope });
        next.set(provider.id, items);
      } catch (err) {
        console.warn(`[Spotlight] provider ${provider.id} query search failed:`, err);
        next.set(provider.id, []);
      }
    }),
  );

  if (!queryGuard.isCurrent(requestSeq, text, currentScope)) {
    return;
  }
  queryItemsByProvider.value = next;
  queryLoading.value = false;
}
```

Add a watcher after `watch(results, ...)`:

```ts
watch(
  [() => parsed.value.query, scope, view, keywordInvocation, quickCommand],
  () => {
    void refreshQueryProviders();
  },
  { immediate: false },
);
```

In the `spotlight-reset` handler, after clearing query and action state, clear query items:

```ts
queryItemsByProvider.value = new Map();
```

In the config subscription branch after `view.value = nextView`, call:

```ts
void refreshQueryProviders();
```

After initial `await prefetchAll();`, call:

```ts
void refreshQueryProviders();
```

- [ ] **Step 7: Run Spotlight search tests and typecheck**

Run:

```powershell
pnpm test src/spotlight/search.test.ts src/spotlight/providers/data-dictionary.test.ts
pnpm typecheck
```

Expected: tests PASS and TypeScript compiles.

- [ ] **Step 8: Commit query-time UI wiring**

Run:

```powershell
git diff -- apps/desktop/src/components/SpotlightPanel.vue apps/desktop/src/components/settings/SpotlightSettings.vue apps/desktop/src/spotlight/search.ts apps/desktop/src/spotlight/search.test.ts
git add apps/desktop/src/components/SpotlightPanel.vue apps/desktop/src/components/settings/SpotlightSettings.vue apps/desktop/src/spotlight/search.ts apps/desktop/src/spotlight/search.test.ts
git commit -m "feat(spotlight): 按输入实时查询数据源"
```

Expected: commit succeeds.

---

## Task 5: Add Data Dictionary Navigation Focus

**Files:**

- Create: `apps/desktop/src/composables/useDataDictionaryNavigation.ts`
- Modify: `apps/desktop/src/App.vue`
- Modify: `apps/desktop/src/components/DataDictionaryPanel.vue`
- Modify: `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`

- [ ] **Step 1: Add composable**

Create `apps/desktop/src/composables/useDataDictionaryNavigation.ts`:

```ts
import { ref } from "vue";

export interface DataDictionaryFocusRequest {
  recordId: number;
}

const pendingFocus = ref<DataDictionaryFocusRequest | null>(null);

export function useDataDictionaryNavigation() {
  function requestFocus(recordId: number) {
    pendingFocus.value = { recordId };
  }

  function consumeFocus(): DataDictionaryFocusRequest | null {
    const req = pendingFocus.value;
    pendingFocus.value = null;
    return req;
  }

  return { pendingFocus, requestFocus, consumeFocus };
}
```

- [ ] **Step 2: Add source guard tests for App and panel wiring**

In `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`, append:

```ts
it("consumes Spotlight focus requests without mutating the search keyword", () => {
  expect(source).toContain("useDataDictionaryNavigation");
  expect(source).toContain("consumeDataDictionaryFocus");
  expect(source).toContain("focusDataDictionaryRecord");
  expect(source).not.toContain("keyword.value = String(focus.recordId)");
});
```

If this file only reads `DataDictionaryPanel.vue`, create a second source constant for `App.vue`:

```ts
const appSource = readFileSync(resolve(__dirname, "../App.vue"), "utf8");
```

Then add:

```ts
it("routes hotkey navigation to data dictionary focus requests", () => {
  expect(appSource).toContain('target === "data-dictionary"');
  expect(appSource).toContain("useDataDictionaryNavigation");
  expect(appSource).toContain("requestFocus(parsedItem)");
});
```

- [ ] **Step 3: Run source guard and verify failure**

Run:

```powershell
pnpm test src/components/DataDictionaryPanel.context-menu.test.ts
```

Expected: FAIL until App and panel wiring are added.

- [ ] **Step 4: Wire App hotkey navigation**

In `apps/desktop/src/App.vue`, add a branch in the existing `hotkey-navigate` listener after the todo branch:

```ts
          } else if (target === "data-dictionary") {
            const { useDataDictionaryNavigation } = await import("./composables/useDataDictionaryNavigation");
            useDataDictionaryNavigation().requestFocus(parsedItem);
```

Keep the existing `onSelect(target)` call unchanged.

- [ ] **Step 5: Consume focus in DataDictionaryPanel**

In `apps/desktop/src/components/DataDictionaryPanel.vue`, add import:

```ts
import { useDataDictionaryNavigation } from "../composables/useDataDictionaryNavigation";
```

Add near other composable constants:

```ts
const { consumeFocus: consumeDataDictionaryFocus } = useDataDictionaryNavigation();
```

Add this helper near `loadRecordDetailById`:

```ts
async function focusDataDictionaryRecord(recordId: number) {
  searchScope.value = "all";
  selectedId.value = null;
  currentDictionary.value = null;
  fields.value = [];
  const existing = searchItems.value.find((item) => item.id === recordId);
  if (existing) {
    await selectSearchItem(existing);
    return;
  }
  selectedItem.value = null;
  try {
    await loadRecordDetailById(recordId);
  } catch {
    detailError.value = "定位记录失败";
  }
  if (!recordDetail.value) {
    ElMessage.warning("未找到该数据字典记录，可能已被删除");
  }
}
```

Update `onMounted`:

```ts
onMounted(() => {
  void loadDictionaries().then(async () => {
    const focus = consumeDataDictionaryFocus();
    if (focus) {
      await focusDataDictionaryRecord(focus.recordId);
    }
  });
});
```

If the existing `onMounted` must remain simple, replace only:

```ts
onMounted(() => {
  void loadDictionaries();
});
```

with the snippet above.

- [ ] **Step 6: Run navigation tests and typecheck**

Run:

```powershell
pnpm test src/components/DataDictionaryPanel.context-menu.test.ts
pnpm typecheck
```

Expected: tests PASS and TypeScript compiles.

- [ ] **Step 7: Commit navigation**

Run:

```powershell
git diff -- apps/desktop/src/composables/useDataDictionaryNavigation.ts apps/desktop/src/App.vue apps/desktop/src/components/DataDictionaryPanel.vue apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts
git add apps/desktop/src/composables/useDataDictionaryNavigation.ts apps/desktop/src/App.vue apps/desktop/src/components/DataDictionaryPanel.vue apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts
git commit -m "feat(data-dictionary): 支持 Spotlight 定位记录"
```

Expected: commit succeeds.

---

## Task 6: Final Validation And Process Log

**Files:**

- Modify: `process.md`

- [ ] **Step 1: Run targeted backend and frontend tests**

Run:

```powershell
cargo test data_dictionary -- --nocapture
pnpm test src/spotlight/providers/data-dictionary.test.ts src/spotlight/search.test.ts src/spotlight/config-store.test.ts src/utils/spotlight-query.test.ts src/components/DataDictionaryPanel.context-menu.test.ts
```

Expected:

- `cargo test data_dictionary -- --nocapture`: PASS.
- All listed Vitest suites: PASS.

- [ ] **Step 2: Run broad validation**

Run:

```powershell
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected:

- `pnpm typecheck`: PASS.
- `pnpm --filter @lazycat/desktop build:web`: PASS.

- [ ] **Step 3: Add process log because implementation touches 3+ files**

Add a new top entry to `process.md`:

```markdown
## 2026-06-27: Spotlight 数据字典接入使用 query-time provider

**场景**: Spotlight 需要搜索数据字典记录，并支持打开定位、复制显示字段和懒加载复制完整 JSON。
**使用次数**: 0
**问题**:

1. 数据字典记录数量和单条 JSON 体积不可控，不适合在 Spotlight 空输入时预取。
2. Spotlight 前端不应重复实现数据字典字段路径解析、标题字段和显示字段摘要规则。
3. 异步 query-time provider 可能出现旧响应覆盖新查询结果。
   **解决**:
4. 扩展数据字典 `search` 返回 `title` 和 `summary`，并用 `includeRawJson: false` 支持轻量候选。
5. Spotlight provider 增加可选 `search(query, ctx)`，数据字典只在有效关键词下按需请求。
6. Spotlight 查询结果用请求序号绑定当前 query 和 scope，旧响应直接丢弃。
7. 完整 JSON 复制通过 `record-detail` 懒加载，候选 payload 不保存 `rawJson`。
   **关键点**:
8. 大数据源优先 query-time 搜索，不要塞进通用预取集合。
9. 动态 JSON 展示规则由数据字典后端单一维护，Spotlight 只做展示映射和动作编排。
10. `providerId:itemId` 去重时要合并预取和 query-time 结果，避免重复行。
    **涉及文件**:

- `apps/desktop/src-tauri/src/tools/data_dictionary.rs`
- `apps/desktop/src/types/data-dictionary.ts`
- `apps/desktop/src/spotlight/types.ts`
- `apps/desktop/src/spotlight/search.ts`
- `apps/desktop/src/spotlight/providers/data-dictionary.ts`
- `apps/desktop/src/components/SpotlightPanel.vue`
- `apps/desktop/src/components/DataDictionaryPanel.vue`
- `apps/desktop/src/App.vue`
  **验证**:
- `cargo test data_dictionary -- --nocapture`
- `pnpm test src/spotlight/providers/data-dictionary.test.ts src/spotlight/search.test.ts src/spotlight/config-store.test.ts src/utils/spotlight-query.test.ts src/components/DataDictionaryPanel.context-menu.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
```

- [ ] **Step 4: Inspect final diff**

Run:

```powershell
git status --short
git diff --stat
```

Expected:

- Only intended implementation files and `process.md` are modified.
- The uncommitted spec edit is either still present as user work or separately committed only if the user requested it.

- [ ] **Step 5: Commit process log**

Run:

```powershell
git add process.md
git commit -m "docs(spotlight): 记录数据字典接入经验"
```

Expected: commit succeeds if `process.md` changed.

---

## Self-Review Checklist

Spec coverage:

- Default enabled data dictionary provider: Task 3 defines `defaultEnabled: true`; Task 4 registers it in panel and settings.
- Aliases `dd` and `dict`: Task 3 defines defaults and adds parser coverage through config alias map.
- Result row title/source/summary: Task 1 returns backend `title` and `summary`; Task 3 maps them to `SpotlightItem`.
- Enter opens and locates record: Task 3 calls `spotlight_pick`; Task 5 routes `hotkey-navigate` and consumes focus in `DataDictionaryPanel`.
- Tab actions copy fields and full JSON: Task 3 covers `copy_field:*` and lazy `copy_raw_json`.
- No empty-input prefetch: Task 3 provider `prefetch` returns `[]`; Task 2 and Task 4 threshold logic avoids query-time calls for empty input.
- `includeRawJson: false`: Task 1 adds backend support; Task 3 provider sends it and tests that payload omits `rawJson`.
- Stale query response protection: Task 4 adds request guard and test.
- Error isolation: Task 3 provider returns empty results on search failure; copy failures return `errorMessage`.

Placeholder scan:

- The plan contains no unfinished marker text or intentionally unspecified code.
- Each code-changing step includes exact target files and concrete snippets.
- Each validation step includes commands and expected outcomes.

Type consistency:

- `DataDictionaryRecordSummaryPart` is the existing frontend summary type.
- `SpotlightSearchContext` is introduced before `ProviderDescriptor.search` uses it.
- `DataDictionarySearchRequest.includeRawJson` maps to Rust payload key `includeRawJson`.
- `data-dictionary` is added to `SpotlightProviderId` before provider registration and config alias tests use it.
