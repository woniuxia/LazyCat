# Data Dictionary Popular Records Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add primary-key-required data dictionaries and popular-record ranking based on successful record detail views.

**Architecture:** Keep the existing data dictionary module boundaries. Rust owns schema, primary-key validation, usage persistence, popular-record resolution, and stale usage cleanup. Vue owns primary-key selection, legacy no-primary-key gating, popular-record display, deduplication, and explicit usage marking after a current detail response is shown.

**Tech Stack:** Tauri 2, Rust, rusqlite, Vue 3, TypeScript, Vitest, Element Plus.

---

## File Structure

- Modify `apps/desktop/src-tauri/src/tools/helpers.rs`: create `data_dictionary_record_usage` and indexes.
- Modify `apps/desktop/src-tauri/src/tools/data_dictionary.rs`: add `popular_records` and `mark_record_used`, require `primaryFieldPath` on create/update, add confirmed primary pruning flow, add Rust tests.
- Modify `apps/desktop/src/bridge/tauri.ts`: add IPC channels for `popular-records` and `mark-record-used`.
- Modify `apps/desktop/src/types/data-dictionary.ts`: add popular-record request/result types and update write request helper types as needed.
- Modify `apps/desktop/src/utils/dataDictionary.ts`: add pure helpers for popular/default record deduplication and empty-keyword selection.
- Modify `apps/desktop/src/utils/dataDictionary.test.ts`: cover popular/default deduplication and selection priority.
- Modify `apps/desktop/src/components/DataDictionaryPanel.vue`: add import primary selection, no-primary-key restricted state, popular-record section, explicit usage marking, and primary-prune confirmation.
- Modify `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`: keep existing structural regressions green and add source-level checks for new guarded flows if no DOM test harness exists.

Do not modify existing `data_dictionary_records.id` semantics. The new usage table uses `record_id TEXT` for the business primary-key value and `normalized_value TEXT` for lookup.

The worktree is expected to be dirty. Before each commit, stage only files touched by that task with exact paths.

---

### Task 1: Backend Schema And Dispatch Surface

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs`
- Modify: `apps/desktop/src-tauri/src/tools/data_dictionary.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`

- [ ] **Step 1: Add the usage table and indexes**

In `apps/desktop/src-tauri/src/tools/helpers.rs`, add this SQL block near the other data dictionary tables, after `data_dictionary_record_values` is created:

```rust
        CREATE TABLE IF NOT EXISTS data_dictionary_record_usage (
            dictionary_id INTEGER NOT NULL,
            record_id TEXT NOT NULL,
            normalized_value TEXT NOT NULL,
            used_count INTEGER NOT NULL DEFAULT 1,
            last_used_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(dictionary_id, normalized_value),
            FOREIGN KEY(dictionary_id) REFERENCES data_dictionaries(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_data_dictionary_record_usage_order
            ON data_dictionary_record_usage(dictionary_id, used_count DESC, last_used_at DESC);
        CREATE INDEX IF NOT EXISTS idx_data_dictionary_record_usage_global_order
            ON data_dictionary_record_usage(used_count DESC, last_used_at DESC);
```

- [ ] **Step 2: Add backend action dispatch entries**

In `apps/desktop/src-tauri/src/tools/data_dictionary.rs`, extend the `execute` action match:

```rust
        "popular_records" => action_popular_records(payload),
        "mark_record_used" => action_mark_record_used(payload),
```

- [ ] **Step 3: Add frontend bridge channels**

In `apps/desktop/src/bridge/tauri.ts`, add:

```ts
  "tool:data-dictionary:popular-records": { domain: "data_dictionary", action: "popular_records" },
  "tool:data-dictionary:mark-record-used": { domain: "data_dictionary", action: "mark_record_used" },
```

- [ ] **Step 4: Run schema/dispatch smoke checks**

Run:

```powershell
cargo check
```

Expected: build reaches existing project state without new unknown function errors after Task 3 is complete. If this fails because `action_popular_records` and `action_mark_record_used` are not implemented yet, continue to Task 3 before re-running.

- [ ] **Step 5: Commit Task 1**

```powershell
git add apps/desktop/src-tauri/src/tools/helpers.rs apps/desktop/src-tauri/src/tools/data_dictionary.rs apps/desktop/src/bridge/tauri.ts
git commit -m "feat(data-dictionary): add record usage schema"
```

---

### Task 2: Primary Key Enforcement And Confirmed Pruning

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/data_dictionary.rs`

- [ ] **Step 1: Write Rust tests for primary-key-required create and update**

Add tests inside the existing `#[cfg(test)]` module in `apps/desktop/src-tauri/src/tools/data_dictionary.rs`:

```rust
#[test]
fn create_requires_primary_field_path() {
    let _guard = test_db_guard();
    let err = action_create(&json!({
        "name": "Users",
        "input": r#"[{"id":"u1","name":"Alice"}]"#
    }))
    .expect_err("create without primaryFieldPath should fail");

    assert!(err.contains("primaryFieldPath is required"));
}

#[test]
fn update_fields_rejects_empty_primary_field_path() {
    let _guard = test_db_guard();
    let created = action_create(&json!({
        "name": "Users",
        "input": r#"[{"id":"u1","name":"Alice"}]"#,
        "primaryFieldPath": "id"
    }))
    .expect("create dictionary");
    let dictionary_id = created["id"].as_i64().expect("dictionary id");

    let err = action_update_fields(&json!({
        "dictionaryId": dictionary_id,
        "primaryFieldPath": null,
        "titleFieldPath": "name",
        "sortFieldPath": null,
        "sortDirection": "asc",
        "fields": [
            {
                "fieldPath": "id",
                "displayName": "id",
                "meaning": "",
                "searchable": true,
                "visible": true,
                "sortOrder": 0
            },
            {
                "fieldPath": "name",
                "displayName": "name",
                "meaning": "",
                "searchable": true,
                "visible": true,
                "sortOrder": 1
            }
        ],
        "relations": []
    }))
    .expect_err("clearing primary key should fail");

    assert!(err.contains("primaryFieldPath is required"));
}

#[test]
fn update_fields_requires_confirmation_before_pruning_primary_invalid_records() {
    let _guard = test_db_guard();
    let created = action_create(&json!({
        "name": "Users",
        "input": r#"[{"id":"u1","code":"a"},{"id":"u2","code":null}]"#,
        "primaryFieldPath": "id"
    }))
    .expect("create dictionary");
    let dictionary_id = created["id"].as_i64().expect("dictionary id");

    let err = action_update_fields(&json!({
        "dictionaryId": dictionary_id,
        "primaryFieldPath": "code",
        "titleFieldPath": "id",
        "sortFieldPath": null,
        "sortDirection": "asc",
        "fields": [
            {
                "fieldPath": "id",
                "displayName": "id",
                "meaning": "",
                "searchable": true,
                "visible": true,
                "sortOrder": 0
            },
            {
                "fieldPath": "code",
                "displayName": "code",
                "meaning": "",
                "searchable": true,
                "visible": true,
                "sortOrder": 1
            }
        ],
        "relations": []
    }))
    .expect_err("primary change that prunes records should require confirmation");

    assert!(err.contains("primary key change requires confirmation"));
    assert!(err.contains("skippedPrimaryRecordCount"));
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```powershell
cargo test data_dictionary::tests::create_requires_primary_field_path data_dictionary::tests::update_fields_rejects_empty_primary_field_path data_dictionary::tests::update_fields_requires_confirmation_before_pruning_primary_invalid_records -- --nocapture
```

Expected: the first two tests fail because the backend still accepts missing primary keys; the third fails because there is no confirmation error.

- [ ] **Step 3: Require primary field parsing**

In `action_create`, replace optional primary parsing with a required parser:

```rust
fn parse_required_field_path(payload: &Value, key: &str) -> Result<String, String> {
    payload[key]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{key} is required"))
}
```

Use it in `action_create`:

```rust
let primary_field_path = parse_required_field_path(payload, "primaryFieldPath")?;
```

Validate the field exists after `stats` is computed:

```rust
let field_paths: HashSet<String> = stats.iter().map(|stat| stat.path.clone()).collect();
if !field_paths.contains(&primary_field_path) {
    return Err(format!("primaryFieldPath does not exist: {primary_field_path}"));
}
```

Then pass `Some(primary_field_path.as_str())` into `partition_records_by_primary` and write `primary_field_path` into `data_dictionaries.primary_field_path`.

- [ ] **Step 4: Require primary field in update_fields**

After the existing `seen` field-path set is built in `action_update_fields`, require the configured primary path:

```rust
let primary_field_path = parse_configured_field_path(payload, "primaryFieldPath", &seen)?
    .filter(|value| !value.trim().is_empty())
    .ok_or_else(|| "primaryFieldPath is required".to_string())?;
```

Then pass `Some(primary_field_path.as_str())` to relation parsing and dictionary update. Keep `titleFieldPath` and `sortFieldPath` optional.

- [ ] **Step 5: Add confirmed pruning guard**

Before deleting and reinserting records in `action_update_fields`, compare the existing primary field with the requested one:

```rust
let old_primary_field_path = load_dictionary_primary_field_path(&tx, dictionary_id)?;
let primary_changed = old_primary_field_path.as_deref() != Some(primary_field_path.as_str());
let confirm_primary_prune = payload["confirmPrimaryPrune"].as_bool().unwrap_or(false);
```

After `partition_records_by_primary`, before deleting records, add:

```rust
let skipped_record_count = skipped_invalid_count + skipped_duplicate_count;
if primary_changed && skipped_record_count > 0 && !confirm_primary_prune {
    return Err(json!({
        "code": "PRIMARY_PRUNE_CONFIRMATION_REQUIRED",
        "message": "primary key change requires confirmation",
        "skippedPrimaryRecordCount": skipped_record_count,
        "skippedPrimaryInvalidCount": skipped_invalid_count,
        "skippedPrimaryDuplicateCount": skipped_duplicate_count
    })
    .to_string());
}
```

- [ ] **Step 6: Run primary enforcement tests**

Run:

```powershell
cargo test data_dictionary::tests::create_requires_primary_field_path data_dictionary::tests::update_fields_rejects_empty_primary_field_path data_dictionary::tests::update_fields_requires_confirmation_before_pruning_primary_invalid_records -- --nocapture
```

Expected: all three tests pass.

- [ ] **Step 7: Commit Task 2**

```powershell
git add apps/desktop/src-tauri/src/tools/data_dictionary.rs
git commit -m "feat(data-dictionary): require primary key configuration"
```

---

### Task 3: Record Usage Backend APIs

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/data_dictionary.rs`

- [ ] **Step 1: Write Rust tests for mark_record_used and popular_records**

Add tests:

```rust
#[test]
fn mark_record_used_upserts_usage_count() {
    let _guard = test_db_guard();
    let created = action_create(&json!({
        "name": "Users",
        "input": r#"[{"id":"u1","name":"Alice"}]"#,
        "primaryFieldPath": "id"
    }))
    .expect("create dictionary");
    let dictionary_id = created["id"].as_i64().expect("dictionary id");
    let search = action_search(&json!({
        "scope": "current",
        "dictionaryId": dictionary_id,
        "keyword": "Alice",
        "limit": 10
    }))
    .expect("search");
    let record_id = search["items"][0]["id"].as_i64().expect("record id");

    action_mark_record_used(&json!({ "id": record_id })).expect("first mark");
    action_mark_record_used(&json!({ "id": record_id })).expect("second mark");

    let popular = action_popular_records(&json!({
        "dictionaryId": dictionary_id,
        "limit": 10
    }))
    .expect("popular records");
    assert_eq!(popular["items"][0]["recordId"], "u1");
    assert_eq!(popular["items"][0]["usedCount"], 2);
}

#[test]
fn popular_records_resolves_current_record_after_replace() {
    let _guard = test_db_guard();
    let created = action_create(&json!({
        "name": "Users",
        "input": r#"[{"id":"u1","name":"Alice"}]"#,
        "primaryFieldPath": "id"
    }))
    .expect("create dictionary");
    let dictionary_id = created["id"].as_i64().expect("dictionary id");
    let search = action_search(&json!({
        "scope": "current",
        "dictionaryId": dictionary_id,
        "keyword": "Alice",
        "limit": 10
    }))
    .expect("search");
    let old_row_id = search["items"][0]["id"].as_i64().expect("record id");
    action_mark_record_used(&json!({ "id": old_row_id })).expect("mark used");

    action_replace_records(&json!({
        "dictionaryId": dictionary_id,
        "input": r#"[{"id":"u1","name":"Alice Updated"}]"#
    }))
    .expect("replace records");

    let popular = action_popular_records(&json!({
        "dictionaryId": dictionary_id,
        "limit": 10
    }))
    .expect("popular records");
    assert_eq!(popular["items"][0]["recordId"], "u1");
    assert_eq!(popular["items"][0]["title"], "Alice Updated");
    assert_ne!(popular["items"][0]["id"].as_i64().unwrap(), old_row_id);
}

#[test]
fn popular_records_deletes_stale_usage_without_backfilling() {
    let _guard = test_db_guard();
    let created = action_create(&json!({
        "name": "Users",
        "input": r#"[{"id":"u1","name":"Alice"},{"id":"u2","name":"Bob"}]"#,
        "primaryFieldPath": "id"
    }))
    .expect("create dictionary");
    let dictionary_id = created["id"].as_i64().expect("dictionary id");
    let search = action_search(&json!({
        "scope": "current",
        "dictionaryId": dictionary_id,
        "keyword": "",
        "limit": 10
    }))
    .expect("search");
    let first = search["items"][0]["id"].as_i64().expect("first id");
    let second = search["items"][1]["id"].as_i64().expect("second id");
    action_mark_record_used(&json!({ "id": first })).expect("mark first");
    action_mark_record_used(&json!({ "id": second })).expect("mark second");

    action_replace_records(&json!({
        "dictionaryId": dictionary_id,
        "input": r#"[{"id":"u2","name":"Bob"}]"#
    }))
    .expect("replace records");

    let popular = action_popular_records(&json!({
        "dictionaryId": dictionary_id,
        "limit": 2
    }))
    .expect("popular records");
    assert_eq!(popular["items"].as_array().unwrap().len(), 1);
    assert_eq!(popular["items"][0]["recordId"], "u2");
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```powershell
cargo test data_dictionary::tests::mark_record_used_upserts_usage_count data_dictionary::tests::popular_records_resolves_current_record_after_replace data_dictionary::tests::popular_records_deletes_stale_usage_without_backfilling -- --nocapture
```

Expected: tests fail because the new actions are not implemented.

- [ ] **Step 3: Add helper for usable primary value loading**

Add a helper near relation seed helpers:

```rust
fn load_record_primary_usage_value(
    conn: &Connection,
    record_id: i64,
    dictionary_id: i64,
    primary_field_path: &str,
) -> Result<(String, String), String> {
    let row = conn
        .query_row(
            "SELECT value_type, value_text, normalized_value
             FROM data_dictionary_record_values
             WHERE record_id = ?1 AND dictionary_id = ?2 AND field_path = ?3",
            params![record_id, dictionary_id, primary_field_path],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("load record primary usage value failed: {e}"))?;

    let Some((value_type, value_text, normalized_value)) = row else {
        return Err("record primary value not found".to_string());
    };
    if !is_relation_value_usable(&value_type, &normalized_value) {
        return Err("record primary value is not usable".to_string());
    }
    Ok((value_text, normalized_value))
}
```

- [ ] **Step 4: Implement mark_record_used**

Add:

```rust
fn action_mark_record_used(payload: &Value) -> Result<Value, String> {
    let row_id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    let record = load_record_row_by_id(&conn, row_id)?;
    let primary_field_path = load_dictionary_primary_field_path(&conn, record.dictionary_id)?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "dictionary primaryFieldPath is required".to_string())?;
    let (record_business_id, normalized_value) = load_record_primary_usage_value(
        &conn,
        record.id,
        record.dictionary_id,
        &primary_field_path,
    )?;

    conn.execute(
        "INSERT INTO data_dictionary_record_usage
         (dictionary_id, record_id, normalized_value, used_count, last_used_at)
         VALUES (?1, ?2, ?3, 1, CURRENT_TIMESTAMP)
         ON CONFLICT(dictionary_id, normalized_value) DO UPDATE SET
           record_id = excluded.record_id,
           used_count = data_dictionary_record_usage.used_count + 1,
           last_used_at = CURRENT_TIMESTAMP",
        params![record.dictionary_id, record_business_id, normalized_value],
    )
    .map_err(|e| format!("mark data dictionary record used failed: {e}"))?;

    Ok(json!({ "ok": true }))
}
```

- [ ] **Step 5: Implement popular_records**

Add:

```rust
fn action_popular_records(payload: &Value) -> Result<Value, String> {
    let dictionary_id = payload["dictionaryId"].as_i64();
    let limit = payload["limit"].as_i64().unwrap_or(10).clamp(1, 50);
    let conn = db_conn()?;

    let mut params_list: Vec<SqlParam> = Vec::new();
    let mut where_clause = String::from("WHERE d.primary_field_path IS NOT NULL AND trim(d.primary_field_path) <> ''");
    if let Some(dictionary_id) = dictionary_id {
        where_clause.push_str(" AND u.dictionary_id = ?");
        params_list.push(Box::new(dictionary_id));
    }
    params_list.push(Box::new(limit));

    let sql = format!(
        "SELECT u.dictionary_id, u.record_id, u.normalized_value, u.used_count, u.last_used_at,
                d.name, d.primary_field_path
         FROM data_dictionary_record_usage u
         JOIN data_dictionaries d ON d.id = u.dictionary_id
         {where_clause}
         ORDER BY u.used_count DESC, u.last_used_at DESC
         LIMIT ?"
    );
    let refs: Vec<&dyn ToSql> = params_list.iter().map(|param| param.as_ref()).collect();
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("prepare popular data dictionary records failed: {e}"))?;
    let rows = stmt
        .query_map(refs.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| format!("query popular data dictionary records failed: {e}"))?;

    let mut items = Vec::new();
    let mut stale = Vec::new();
    for row in rows {
        let (dictionary_id, business_record_id, normalized_value, used_count, last_used_at, _, primary_field_path) =
            row.map_err(|e| e.to_string())?;
        match load_record_row_by_primary_value(&conn, dictionary_id, &primary_field_path, &normalized_value)? {
            Some(record) => {
                let mut value = record_to_search_item(record)?;
                value["recordId"] = json!(business_record_id);
                value["normalizedValue"] = json!(normalized_value);
                value["usedCount"] = json!(used_count);
                value["lastUsedAt"] = json!(last_used_at);
                items.push(value);
            }
            None => stale.push((dictionary_id, normalized_value)),
        }
    }

    for (dictionary_id, normalized_value) in stale {
        conn.execute(
            "DELETE FROM data_dictionary_record_usage
             WHERE dictionary_id = ?1 AND normalized_value = ?2",
            params![dictionary_id, normalized_value],
        )
        .map_err(|e| format!("delete stale data dictionary usage failed: {e}"))?;
    }

    Ok(json!({ "items": items }))
}
```

Add the lookup helper used above:

```rust
fn load_record_row_by_primary_value(
    conn: &Connection,
    dictionary_id: i64,
    primary_field_path: &str,
    normalized_value: &str,
) -> Result<Option<RecordRow>, String> {
    conn.query_row(
        "SELECT r.id, r.dictionary_id, d.name, r.row_index, r.raw_json,
                d.title_field_path
         FROM data_dictionary_record_values v
         JOIN data_dictionary_records r ON r.id = v.record_id
         JOIN data_dictionaries d ON d.id = r.dictionary_id
         WHERE v.dictionary_id = ?1
           AND v.field_path = ?2
           AND v.normalized_value = ?3
           AND v.value_type IN ('string', 'number', 'boolean')
         ORDER BY r.sort_key COLLATE BINARY ASC, r.id ASC
         LIMIT 1",
        params![dictionary_id, primary_field_path, normalized_value],
        record_row,
    )
    .optional()
    .map_err(|e| format!("load data dictionary record by primary value failed: {e}"))
}
```

If `record_to_search_item` does not exist, extract the existing search-result JSON construction into:

```rust
fn record_to_search_item(row: RecordRow) -> Result<Value, String> {
    Ok(json!({
        "id": row.id,
        "dictionaryId": row.dictionary_id,
        "dictionaryName": row.dictionary_name,
        "titleFieldPath": row.title_field_path,
        "rowIndex": row.row_index,
        "matches": Vec::<Value>::new(),
        "title": build_record_title(&row.raw_json, row.title_field_path.as_deref())?,
        "summary": build_record_summary(&row.raw_json, row.title_field_path.as_deref())?,
    }))
}
```

- [ ] **Step 6: Run record usage tests**

Run:

```powershell
cargo test data_dictionary::tests::mark_record_used_upserts_usage_count data_dictionary::tests::popular_records_resolves_current_record_after_replace data_dictionary::tests::popular_records_deletes_stale_usage_without_backfilling -- --nocapture
```

Expected: all three tests pass.

- [ ] **Step 7: Commit Task 3**

```powershell
git add apps/desktop/src-tauri/src/tools/data_dictionary.rs
git commit -m "feat(data-dictionary): track popular records"
```

---

### Task 4: TypeScript Types And Pure List Helpers

**Files:**

- Modify: `apps/desktop/src/types/data-dictionary.ts`
- Modify: `apps/desktop/src/utils/dataDictionary.ts`
- Modify: `apps/desktop/src/utils/dataDictionary.test.ts`

- [ ] **Step 1: Add TypeScript types**

In `apps/desktop/src/types/data-dictionary.ts`, add:

```ts
export interface DataDictionaryPopularRecord extends DataDictionarySearchItem {
  recordId: string;
  normalizedValue: string;
  usedCount: number;
  lastUsedAt: string;
}

export interface DataDictionaryPopularRecordsRequest {
  dictionaryId?: number;
  limit?: number;
}

export interface DataDictionaryPopularRecordsResult {
  items: DataDictionaryPopularRecord[];
}

export interface MarkDataDictionaryRecordUsedRequest {
  id: number;
}

export interface MarkDataDictionaryRecordUsedResult {
  ok: true;
}
```

- [ ] **Step 2: Write pure helper tests**

In `apps/desktop/src/utils/dataDictionary.test.ts`, add:

```ts
import { mergePopularAndSearchItems, pickInitialRecordItem } from "./dataDictionary";
import type {
  DataDictionaryPopularRecord,
  DataDictionarySearchItem,
} from "../types/data-dictionary";

function searchItem(id: number, title: string): DataDictionarySearchItem {
  return {
    id,
    dictionaryId: 1,
    dictionaryName: "Users",
    titleFieldPath: "name",
    rowIndex: id,
    matches: [],
    title,
    summary: [],
  };
}

function popularItem(id: number, title: string): DataDictionaryPopularRecord {
  return {
    ...searchItem(id, title),
    recordId: `u${id}`,
    normalizedValue: `u${id}`,
    usedCount: 3,
    lastUsedAt: "2026-06-28 10:00:00",
  };
}

it("keeps popular records first and removes duplicate search items", () => {
  const result = mergePopularAndSearchItems(
    [popularItem(1, "Alice")],
    [searchItem(1, "Alice"), searchItem(2, "Bob")],
  );

  expect(result.map((item) => item.id)).toEqual([1, 2]);
});

it("picks first popular record before default search result", () => {
  const picked = pickInitialRecordItem([popularItem(1, "Alice")], [searchItem(2, "Bob")]);

  expect(picked?.id).toBe(1);
});

it("picks first search result when popular records are empty", () => {
  const picked = pickInitialRecordItem([], [searchItem(2, "Bob")]);

  expect(picked?.id).toBe(2);
});
```

- [ ] **Step 3: Run tests and verify they fail**

Run:

```powershell
pnpm test src/utils/dataDictionary.test.ts
```

Expected: fail because `mergePopularAndSearchItems` and `pickInitialRecordItem` are not exported yet.

- [ ] **Step 4: Implement pure helpers**

In `apps/desktop/src/utils/dataDictionary.ts`, add:

```ts
import type {
  DataDictionaryPopularRecord,
  DataDictionarySearchItem,
} from "../types/data-dictionary";

export function mergePopularAndSearchItems(
  popularItems: DataDictionaryPopularRecord[],
  searchItems: DataDictionarySearchItem[],
): Array<DataDictionaryPopularRecord | DataDictionarySearchItem> {
  const popularIds = new Set(popularItems.map((item) => item.id));
  return [...popularItems, ...searchItems.filter((item) => !popularIds.has(item.id))];
}

export function pickInitialRecordItem(
  popularItems: DataDictionaryPopularRecord[],
  searchItems: DataDictionarySearchItem[],
): DataDictionaryPopularRecord | DataDictionarySearchItem | null {
  return popularItems[0] ?? searchItems[0] ?? null;
}
```

If the file already has imports, merge the type import into the existing import block instead of creating duplicate import statements.

- [ ] **Step 5: Run pure helper tests**

Run:

```powershell
pnpm test src/utils/dataDictionary.test.ts
```

Expected: pass.

- [ ] **Step 6: Commit Task 4**

```powershell
git add apps/desktop/src/types/data-dictionary.ts apps/desktop/src/utils/dataDictionary.ts apps/desktop/src/utils/dataDictionary.test.ts
git commit -m "test(data-dictionary): cover popular record list helpers"
```

---

### Task 5: DataDictionaryPanel UI Integration

**Files:**

- Modify: `apps/desktop/src/components/DataDictionaryPanel.vue`
- Modify: `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`

- [ ] **Step 1: Add source-level component tests**

In `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`, add tests that inspect the component source:

```ts
it("requires primary field selection when creating a dictionary", () => {
  expect(source).toContain("importPrimaryPath");
  expect(source).toContain("primaryFieldPath: importPrimaryPath.value");
  expect(source).toContain(':disabled="!canSubmitImport"');
});

it("loads popular records separately from keyword search", () => {
  expect(source).toContain("tool:data-dictionary:popular-records");
  expect(source).toContain("tool:data-dictionary:mark-record-used");
  expect(source).toContain("mergePopularAndSearchItems");
  expect(source).toContain("pickInitialRecordItem");
});

it("guards primary key pruning with explicit confirmation", () => {
  expect(source).toContain("confirmPrimaryPrune");
  expect(source).toContain("PRIMARY_PRUNE_CONFIRMATION_REQUIRED");
});
```

- [ ] **Step 2: Run component tests and verify they fail**

Run:

```powershell
pnpm test src/components/DataDictionaryPanel.context-menu.test.ts
```

Expected: fail because the component has not been updated.

- [ ] **Step 3: Add import primary selection state and template control**

In `DataDictionaryPanel.vue`, import the new types and helpers:

```ts
import type {
  DataDictionaryPopularRecord,
  DataDictionaryPopularRecordsResult,
  MarkDataDictionaryRecordUsedResult,
} from "../types/data-dictionary";
import { mergePopularAndSearchItems, pickInitialRecordItem } from "../utils/dataDictionary";
```

Add state:

```ts
const importPrimaryPath = ref("");
const popularItems = ref<DataDictionaryPopularRecord[]>([]);
const loadingPopular = ref(false);
const popularError = ref("");
```

Add computed:

```ts
const canSubmitImport = computed(() => {
  if (!importPreview.value) return false;
  if (importMode.value === "create") return importPrimaryPath.value.trim().length > 0;
  return true;
});

const displayedSearchItems = computed(() =>
  keyword.value.trim()
    ? searchItems.value
    : mergePopularAndSearchItems(popularItems.value, searchItems.value),
);
```

In the import dialog, add an `el-select` bound to `importPrimaryPath` when `importMode === "create"`:

```vue
<el-form-item v-if="importMode === 'create'" label="主键字段" required>
  <el-select v-model="importPrimaryPath" placeholder="选择用于唯一定位记录的字段" filterable>
    <el-option
      v-for="field in importPreview?.fields ?? []"
      :key="field.fieldPath"
      :label="field.displayName || field.fieldPath"
      :value="field.fieldPath"
    />
  </el-select>
</el-form-item>
```

Update the save button disabled binding to:

```vue
:disabled="!canSubmitImport"
```

When opening the create import dialog and when clearing preview, reset:

```ts
importPrimaryPath.value = "";
```

When calling create, send:

```ts
primaryFieldPath: importPrimaryPath.value,
```

- [ ] **Step 4: Add popular records loading and empty-keyword rendering**

Replace the result list `v-for` source from `searchItems` to `displayedSearchItems`.

Add a small status row in the item template for popular records:

```vue
<span v-if="'usedCount' in item" class="dd-result-meta">使用 {{ item.usedCount }} 次</span>
```

Add a function:

```ts
async function loadPopularRecords() {
  if (keyword.value.trim()) {
    popularItems.value = [];
    return;
  }
  if (searchScope.value === "current" && !selectedId.value) {
    popularItems.value = [];
    return;
  }
  if (
    searchScope.value === "current" &&
    currentDictionary.value &&
    !currentDictionary.value.primaryFieldPath
  ) {
    popularItems.value = [];
    return;
  }

  loadingPopular.value = true;
  popularError.value = "";
  try {
    const result = await ipc<DataDictionaryPopularRecordsResult>(
      "tool:data-dictionary:popular-records",
      {
        dictionaryId: searchScope.value === "current" ? (selectedId.value ?? undefined) : undefined,
        limit: 10,
      },
    );
    popularItems.value = result.items;
  } catch (error) {
    popularError.value = (error as Error).message || "加载常用记录失败";
    popularItems.value = [];
  } finally {
    loadingPopular.value = false;
  }
}
```

In `runSearch`, after `searchItems.value = result.items`, call `await loadPopularRecords()` when `searchKeyword.trim()` is empty. Select the first item with:

```ts
const initialItem = pickInitialRecordItem(popularItems.value, result.items);
if (initialItem) {
  await selectSearchItem(initialItem);
} else {
  selectedItem.value = null;
  resetRecordDetail();
}
```

- [ ] **Step 5: Add legacy no-primary-key restricted state**

Add computed:

```ts
const currentDictionaryRequiresPrimary = computed(
  () =>
    searchScope.value === "current" &&
    currentDictionary.value &&
    !currentDictionary.value.primaryFieldPath,
);
```

In the result panel, before the list, show:

```vue
<el-empty
  v-if="currentDictionaryRequiresPrimary"
  class="dd-empty dd-empty-panel"
  description="请先配置主键字段"
>
  <el-button type="primary" @click="openFieldConfig">配置主键</el-button>
</el-empty>
```

Guard `runSearch` for current no-primary-key dictionaries:

```ts
if (currentDictionaryRequiresPrimary.value) {
  searchItems.value = [];
  popularItems.value = [];
  selectedItem.value = null;
  resetRecordDetail();
  searchHasMore.value = false;
  searchError.value = "";
  return;
}
```

- [ ] **Step 6: Mark record used only after current detail is shown**

In `loadRecordDetailById`, after `recordDetail.value = result`, call:

```ts
void markRecordUsed(result.record.id);
```

Add:

```ts
async function markRecordUsed(id: number) {
  if (!recordDetail.value || recordDetail.value.record.id !== id) return;
  try {
    await ipc<MarkDataDictionaryRecordUsedResult>("tool:data-dictionary:mark-record-used", { id });
    if (!keyword.value.trim()) {
      await loadPopularRecords();
    }
  } catch {
    // Usage tracking must not block record detail display.
  }
}
```

Do not call `markRecordUsed` when the current dictionary has no primary key. The backend also rejects it, but the frontend should skip known invalid cases:

```ts
if (
  searchScope.value === "current" &&
  currentDictionary.value &&
  !currentDictionary.value.primaryFieldPath
)
  return;
```

- [ ] **Step 7: Confirm primary pruning from field config**

Wrap the existing update-fields save call in a helper that can retry:

```ts
async function saveFieldConfigWithConfirmation(confirmPrimaryPrune = false) {
  return ipc<DataDictionaryImportWriteResult>("tool:data-dictionary:update-fields", {
    dictionaryId: fieldConfigDictionaryId.value,
    fields: fieldRows.value,
    primaryFieldPath: fieldPrimaryPath.value,
    titleFieldPath: fieldTitlePath.value || null,
    sortFieldPath: fieldSortPath.value || null,
    sortDirection: fieldSortDirection.value,
    relations: relationRows.value,
    confirmPrimaryPrune,
  });
}
```

In the catch branch, parse JSON error text:

```ts
function parsePrimaryPruneError(error: unknown) {
  const message = (error as Error).message || String(error);
  try {
    const parsed = JSON.parse(message);
    return parsed?.code === "PRIMARY_PRUNE_CONFIRMATION_REQUIRED" ? parsed : null;
  } catch {
    return null;
  }
}
```

If the parsed error exists, show:

```ts
await ElMessageBox.confirm(
  `更换主键会剔除 ${parsed.skippedPrimaryRecordCount} 条主键异常记录，确认继续？`,
  "确认更换主键",
  { type: "warning", confirmButtonText: "继续保存", cancelButtonText: "取消" },
);
const result = await saveFieldConfigWithConfirmation(true);
```

- [ ] **Step 8: Run component tests**

Run:

```powershell
pnpm test src/components/DataDictionaryPanel.context-menu.test.ts
```

Expected: pass.

- [ ] **Step 9: Commit Task 5**

```powershell
git add apps/desktop/src/components/DataDictionaryPanel.vue apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts
git commit -m "feat(data-dictionary): show popular records in panel"
```

---

### Task 6: Full Verification And Cleanup

**Files:**

- Modify only if verification reveals a defect in files touched by Tasks 1-5.

- [ ] **Step 1: Run backend data dictionary tests**

Run:

```powershell
cargo test data_dictionary -- --nocapture
```

Expected: pass.

- [ ] **Step 2: Run targeted frontend tests**

Run:

```powershell
pnpm test src/components/DataDictionaryPanel.context-menu.test.ts src/utils/dataDictionary.test.ts src/utils/dataDictionaryRelations.test.ts src/utils/dataDictionaryMenu.test.ts
```

Expected: pass.

- [ ] **Step 3: Run typecheck**

Run:

```powershell
pnpm typecheck
```

Expected: pass.

- [ ] **Step 4: Run desktop web build**

Run:

```powershell
pnpm --filter @lazycat/desktop build:web
```

Expected: pass.

- [ ] **Step 5: Inspect final diff for unrelated files**

Run:

```powershell
git status --short
git diff --stat
```

Expected: only files listed in this plan are changed by this implementation, plus pre-existing dirty files that were not staged by these tasks.

- [ ] **Step 6: Final commit if verification fixes were needed**

If Step 1-4 required fixes, commit only the touched files:

```powershell
git add apps/desktop/src-tauri/src/tools/helpers.rs apps/desktop/src-tauri/src/tools/data_dictionary.rs apps/desktop/src/bridge/tauri.ts apps/desktop/src/types/data-dictionary.ts apps/desktop/src/utils/dataDictionary.ts apps/desktop/src/utils/dataDictionary.test.ts apps/desktop/src/components/DataDictionaryPanel.vue apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts
git commit -m "fix(data-dictionary): stabilize popular records integration"
```

If no fixes were needed after Task 5, skip this commit.

---

## Self-Review

- Spec coverage: the plan covers required primary key creation, historical no-primary gating, usage table schema, explicit `mark_record_used`, `popular_records`, stale usage cleanup without backfill, empty-keyword popular section, default selection, deduplication, and confirmation before primary-key pruning.
- Placeholder scan: the plan contains concrete files, commands, expected results, and code snippets for each changed area.
- Type consistency: database usage fields are `record_id` and `normalized_value`; API returns current row `id` plus business `recordId`; mark API accepts current row `id`.
