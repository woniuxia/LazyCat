# Data Dictionary Sort Key Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make data dictionary search results follow left-side dictionary order, then each dictionary's configured record sort, using a precomputed non-empty `sort_key`.

**Architecture:** Add `data_dictionary_records.sort_key` as a derived record-level ordering key. Generate the key from `sort_field_path`, `sort_direction`, and `row_index`; search queries order directly by `sort_key COLLATE BINARY ASC` before applying the existing result limit. Keep `raw_json` as the source of truth and rebuild `sort_key` through import, replace, field config save, index rebuild, and migration backfill paths.

**Tech Stack:** Tauri 2, Rust, rusqlite, SQLite, Vue 3, TypeScript, Vitest.

---

## Scope And Current State

Design spec: `docs/superpowers/specs/2026-06-27-data-dictionary-sort-key-design.md`

Relevant existing files:

- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs`
  - Owns SQLite schema creation and pre-schema `ALTER TABLE` compatibility fixes.
- Modify: `apps/desktop/src-tauri/src/tools/data_dictionary.rs`
  - Owns data dictionary import, replace, field config, search, relation detail, record indexing, and Rust tests.
- Modify only if current source tests require it: `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`
  - Existing front-end source guard tests. No UI behavior change is expected.
- Modify after implementation if the task touches 3+ files or yields reusable lessons: `process.md`
  - Project process log.

The worktree already has unrelated uncommitted edits in `data_dictionary.rs`, `DataDictionaryPanel.vue`, `DataDictionaryPanel.context-menu.test.ts`, and `process.md`. Before implementation, inspect those files and preserve the user's changes. Do not revert or overwrite unrelated work.

---

## File Structure

### `apps/desktop/src-tauri/src/tools/helpers.rs`

Responsibility: schema shape only.

Changes:

- Add compatibility `ALTER TABLE data_dictionary_records ADD COLUMN sort_key TEXT NOT NULL DEFAULT '';`.
- Add `sort_key TEXT NOT NULL DEFAULT ''` to the `CREATE TABLE IF NOT EXISTS data_dictionary_records` statement.
- Add index `idx_data_dictionary_records_dictionary_sort ON data_dictionary_records(dictionary_id, sort_key, id)`.

The schema layer does not parse JSON or generate final sort keys. Empty default values are an SQLite migration bridge only.

### `apps/desktop/src-tauri/src/tools/data_dictionary.rs`

Responsibility: all sort-key semantics and data maintenance.

Changes:

- Add pure sort-key encoding helpers and tests.
- Update `insert_records` to compute and write `sort_key`.
- Add dictionary sort-key refresh helpers for existing records and config changes.
- Update `replace_records`, `update_fields`, and `rebuild_dictionary_indexes` to refresh `sort_key`.
- Update search query ordering to use `d.nav_order ASC, r.sort_key COLLATE BINARY ASC, r.id ASC`.
- Add a query-time guard that refreshes empty `sort_key` rows before search, so migrated databases do not return stale ordering.

---

## Task 1: Add Sort-Key Encoding Helpers With Pure Unit Tests

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/data_dictionary.rs`

- [ ] **Step 1: Add failing unit tests for numeric, row-index fallback, missing-field fallback, and descending order**

Add these tests inside the existing `#[cfg(test)] mod tests` in `apps/desktop/src-tauri/src/tools/data_dictionary.rs`:

```rust
#[test]
fn build_record_sort_key_orders_numbers_numerically() {
    let small = build_record_sort_key(&json!({ "score": 2 }), 0, Some(("score", SortDirection::Asc)))
        .unwrap();
    let large = build_record_sort_key(&json!({ "score": 10 }), 1, Some(("score", SortDirection::Asc)))
        .unwrap();

    assert!(small < large);
}

#[test]
fn build_record_sort_key_supports_desc_without_moving_missing_values_first() {
    let high = build_record_sort_key(&json!({ "score": 100 }), 0, Some(("score", SortDirection::Desc)))
        .unwrap();
    let low = build_record_sort_key(&json!({ "score": 90 }), 1, Some(("score", SortDirection::Desc)))
        .unwrap();
    let missing = build_record_sort_key(&json!({ "name": "missing" }), 2, Some(("score", SortDirection::Desc)))
        .unwrap();

    assert!(high < low);
    assert!(low < missing);
}

#[test]
fn build_record_sort_key_uses_row_index_when_sort_field_is_not_configured() {
    let first = build_record_sort_key(&json!({ "score": 100 }), 0, None).unwrap();
    let second = build_record_sort_key(&json!({ "score": 1 }), 1, None).unwrap();

    assert!(first < second);
}

#[test]
fn build_record_sort_key_uses_missing_bucket_and_row_index_when_sort_field_is_missing() {
    let present = build_record_sort_key(&json!({ "score": 1 }), 9, Some(("score", SortDirection::Asc)))
        .unwrap();
    let missing_first =
        build_record_sort_key(&json!({ "name": "a" }), 0, Some(("score", SortDirection::Asc)))
            .unwrap();
    let missing_second =
        build_record_sort_key(&json!({ "name": "b" }), 1, Some(("score", SortDirection::Asc)))
            .unwrap();

    assert!(present < missing_first);
    assert!(missing_first < missing_second);
}

#[test]
fn build_record_sort_key_orders_strings_by_prefix_before_longer_value() {
    let short = build_record_sort_key(&json!({ "name": "a" }), 0, Some(("name", SortDirection::Asc)))
        .unwrap();
    let long = build_record_sort_key(&json!({ "name": "aa" }), 1, Some(("name", SortDirection::Asc)))
        .unwrap();

    assert!(short < long);
}
```

- [ ] **Step 2: Run the focused Rust test and verify it fails**

Run:

```powershell
cargo test data_dictionary::tests::build_record_sort_key -- --nocapture
```

Expected: FAIL with errors that `build_record_sort_key` is not found.

- [ ] **Step 3: Add the minimal sort-key implementation**

Add these helpers near the existing sort helpers, before `sort_record_rows`:

```rust
fn build_record_sort_key(
    record: &Value,
    row_index: i64,
    sort_config: Option<(&str, SortDirection)>,
) -> Result<String, String> {
    let row_key = encode_row_index_sort_part(row_index)?;
    let Some((field_path, direction)) = sort_config else {
        return Ok(format!("1!{row_key}"));
    };
    let Some(value) = get_value_by_field_path(record, field_path) else {
        return Ok(format!("2!{row_key}"));
    };
    let mut value_bytes = encode_present_sort_value(value)?;
    if direction == SortDirection::Desc {
        invert_bytes_in_place(&mut value_bytes);
    }
    Ok(format!("0!{}!{row_key}", hex_encode_upper(&value_bytes)))
}

fn encode_present_sort_value(value: &Value) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    match value {
        Value::Number(number) => {
            let number = number
                .as_f64()
                .ok_or_else(|| "sort number is not representable as f64".to_string())?;
            out.push(1);
            out.extend_from_slice(&ordered_f64_bytes(number));
        }
        Value::String(text) => {
            out.push(2);
            out.extend_from_slice(text.as_bytes());
            out.push(0);
        }
        Value::Bool(flag) => {
            out.push(3);
            out.push(if *flag { 1 } else { 0 });
        }
        Value::Null => {
            out.push(4);
        }
        Value::Array(_) | Value::Object(_) => {
            out.push(5);
            out.extend_from_slice(value_to_search_text(value).as_bytes());
            out.push(0);
        }
    }
    Ok(out)
}

fn ordered_f64_bytes(value: f64) -> [u8; 8] {
    let bits = value.to_bits();
    let ordered = if bits & (1_u64 << 63) == 0 {
        bits ^ (1_u64 << 63)
    } else {
        !bits
    };
    ordered.to_be_bytes()
}

fn encode_row_index_sort_part(row_index: i64) -> Result<String, String> {
    if row_index < 0 {
        return Err(format!("row_index must not be negative: {row_index}"));
    }
    Ok(format!("{:016X}", row_index as u64))
}

fn invert_bytes_in_place(bytes: &mut [u8]) {
    for byte in bytes {
        *byte = 255_u8 - *byte;
    }
}

fn hex_encode_upper(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0F) as usize] as char);
    }
    out
}
```

- [ ] **Step 4: Run the focused Rust test and verify it passes**

Run:

```powershell
cargo test data_dictionary::tests::build_record_sort_key -- --nocapture
```

Expected: PASS for all `build_record_sort_key_*` tests.

- [ ] **Step 5: Commit the pure helper change**

Run:

```powershell
git add apps/desktop/src-tauri/src/tools/data_dictionary.rs
git commit -m "test(data-dictionary): 覆盖排序键编码"
```

Expected: commit succeeds. If unrelated user edits are present in the same file, use `git diff` carefully and stage only the helper/test hunks.

---

## Task 2: Add Schema Column And Backfill Guard

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs`
- Modify: `apps/desktop/src-tauri/src/tools/data_dictionary.rs`

- [ ] **Step 1: Add a failing schema source test**

Add this test inside `apps/desktop/src-tauri/src/tools/data_dictionary.rs` under the existing test module:

```rust
#[test]
fn query_records_sql_orders_by_sort_key_not_updated_at() {
    let sql = build_record_query_sql(&SearchScope::All, &[], Some(100));

    assert!(sql.contains("ORDER BY d.nav_order ASC, r.sort_key COLLATE BINARY ASC, r.id ASC"));
    assert!(!sql.contains("ORDER BY d.updated_at DESC"));
    assert!(!sql.contains("ORDER BY r.row_index ASC"));
}
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run:

```powershell
cargo test data_dictionary::tests::query_records_sql_orders_by_sort_key_not_updated_at -- --nocapture
```

Expected: FAIL because `build_record_query_sql` does not exist.

- [ ] **Step 3: Add `sort_key` to the schema**

In `apps/desktop/src-tauri/src/tools/helpers.rs`, add this compatibility alter near the existing data dictionary `ALTER TABLE` statements:

```rust
let _ = conn.execute_batch(
    "ALTER TABLE data_dictionary_records ADD COLUMN sort_key TEXT NOT NULL DEFAULT '';",
);
```

Update the `CREATE TABLE IF NOT EXISTS data_dictionary_records` statement to include:

```sql
sort_key TEXT NOT NULL DEFAULT '',
```

between `normalized_search_text TEXT NOT NULL,` and `created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP`.

Add this index after `idx_data_dictionary_records_dictionary`:

```sql
CREATE INDEX IF NOT EXISTS idx_data_dictionary_records_dictionary_sort
    ON data_dictionary_records(dictionary_id, sort_key, id);
```

- [ ] **Step 4: Extract query SQL construction and switch ordering to `sort_key`**

In `apps/desktop/src-tauri/src/tools/data_dictionary.rs`, add this helper near `query_records`:

```rust
fn build_record_query_sql(
    scope: &SearchScope,
    conditions: &[String],
    limit: Option<i64>,
) -> String {
    let mut sql = String::from(
        "SELECT r.id, r.dictionary_id, d.name, r.row_index, r.raw_json
         , d.title_field_path
         FROM data_dictionary_records r
         JOIN data_dictionaries d ON d.id = r.dictionary_id",
    );
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    match scope {
        SearchScope::Current(_) => {
            sql.push_str(" ORDER BY r.sort_key COLLATE BINARY ASC, r.id ASC");
        }
        SearchScope::All => {
            sql.push_str(
                " ORDER BY d.nav_order ASC, r.sort_key COLLATE BINARY ASC, r.id ASC",
            );
        }
    }
    if limit.is_some() {
        sql.push_str(" LIMIT ?");
    }
    sql
}
```

Then update `query_records` to use the helper:

```rust
let sql = build_record_query_sql(scope, &conditions, limit);
if let Some(limit) = limit {
    params_list.push(Box::new(limit));
}
```

Remove the old `empty_keyword`-dependent `updated_at` and `row_index` order block. Keep the `empty_keyword` parameter for this task if removing it would make the diff larger; remove it in Task 4.

- [ ] **Step 5: Run the focused SQL test and schema compile check**

Run:

```powershell
cargo test data_dictionary::tests::query_records_sql_orders_by_sort_key_not_updated_at -- --nocapture
cargo check
```

Expected: focused test PASS and `cargo check` PASS.

- [ ] **Step 6: Commit the schema and query ordering change**

Run:

```powershell
git add apps/desktop/src-tauri/src/tools/helpers.rs apps/desktop/src-tauri/src/tools/data_dictionary.rs
git commit -m "feat(data-dictionary): 添加记录排序键列"
```

Expected: commit succeeds. Stage only the schema/query hunks if unrelated edits are present.

---

## Task 3: Generate Sort Keys On Insert, Replace, Field Save, And Rebuild

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/data_dictionary.rs`

- [ ] **Step 1: Add failing unit tests for insert and refresh behavior**

Add these tests inside the existing test module:

```rust
#[test]
fn build_record_sort_key_desc_keeps_equal_values_in_row_order() {
    let first = build_record_sort_key(&json!({ "score": 100 }), 0, Some(("score", SortDirection::Desc)))
        .unwrap();
    let second = build_record_sort_key(&json!({ "score": 100 }), 1, Some(("score", SortDirection::Desc)))
        .unwrap();

    assert!(first < second);
}

#[test]
fn build_record_sort_key_handles_nested_field_paths() {
    let first =
        build_record_sort_key(&json!({ "user": { "name": "Ada" } }), 0, Some(("user.name", SortDirection::Asc)))
            .unwrap();
    let second =
        build_record_sort_key(&json!({ "user": { "name": "Bob" } }), 1, Some(("user.name", SortDirection::Asc)))
            .unwrap();

    assert!(first < second);
}
```

- [ ] **Step 2: Run the focused tests**

Run:

```powershell
cargo test data_dictionary::tests::build_record_sort_key -- --nocapture
```

Expected: PASS if Task 1 helper already handles the behavior. If it fails, fix `build_record_sort_key` before continuing.

- [ ] **Step 3: Update `insert_records` to write `sort_key`**

Change the signature:

```rust
fn insert_records(
    conn: &Connection,
    dictionary_id: i64,
    records: &[IndexedRecord],
    searchable_paths: &[String],
    sort_config: Option<(&str, SortDirection)>,
) -> Result<(), String> {
```

Inside the loop, after `raw_json` is created, compute:

```rust
let sort_key = build_record_sort_key(&record.value, record.source_row_index, sort_config)?;
```

Change the insert SQL to:

```rust
conn.execute(
    "INSERT INTO data_dictionary_records
     (dictionary_id, row_index, raw_json, search_text, normalized_search_text, sort_key)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    params![
        dictionary_id,
        record.source_row_index,
        raw_json,
        search_text,
        normalized,
        sort_key,
    ],
)
.map_err(|e| format!("insert data dictionary record failed: {e}"))?;
```

- [ ] **Step 4: Update all `insert_records` call sites**

Use these call-site patterns:

```rust
insert_records(&tx, dictionary_id, &records, &searchable_paths, None)?;
```

for `action_create`.

For `action_replace_records`, load current config before the transaction and pass it:

```rust
let sort_config = load_dictionary_sort_config(&conn, dictionary_id)?;
let sort_config_ref = sort_config
    .as_ref()
    .map(|(path, direction)| (path.as_str(), *direction));
insert_records(&tx, dictionary_id, &records, &searchable_paths, sort_config_ref)?;
```

For `action_update_fields`, after parsing `sort_field_path` and `sort_direction`, build:

```rust
let new_sort_config = sort_field_path
    .as_deref()
    .map(|path| (path, sort_direction));
```

Pass `new_sort_config` to `insert_records` when primary-field filtering rewrites records.

- [ ] **Step 5: Add sort-key refresh helpers**

Add these helpers near `rebuild_dictionary_indexes`:

```rust
fn refresh_dictionary_sort_keys(
    conn: &Connection,
    dictionary_id: i64,
    sort_config: Option<(&str, SortDirection)>,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, row_index, raw_json
             FROM data_dictionary_records
             WHERE dictionary_id = ?1
             ORDER BY row_index ASC, id ASC",
        )
        .map_err(|e| format!("prepare dictionary sort key refresh failed: {e}"))?;
    let rows = stmt
        .query_map(params![dictionary_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("query dictionary sort key refresh failed: {e}"))?;

    let mut updates = Vec::new();
    for row in rows {
        let (record_id, row_index, raw_json) = row.map_err(|e| e.to_string())?;
        let record = serde_json::from_str::<Value>(&raw_json)
            .map_err(|e| format!("parse data dictionary record {record_id} failed: {e}"))?;
        let sort_key = build_record_sort_key(&record, row_index, sort_config)?;
        updates.push((record_id, sort_key));
    }

    for (record_id, sort_key) in updates {
        conn.execute(
            "UPDATE data_dictionary_records SET sort_key = ?1 WHERE id = ?2",
            params![sort_key, record_id],
        )
        .map_err(|e| format!("update data dictionary sort key failed: {e}"))?;
    }
    Ok(())
}

fn ensure_scope_sort_keys_ready(conn: &Connection, scope: &SearchScope) -> Result<(), String> {
    let dictionary_ids = load_dictionaries_with_empty_sort_keys(conn, scope)?;
    for dictionary_id in dictionary_ids {
        let sort_config = load_dictionary_sort_config(conn, dictionary_id)?;
        let sort_config_ref = sort_config
            .as_ref()
            .map(|(path, direction)| (path.as_str(), *direction));
        refresh_dictionary_sort_keys(conn, dictionary_id, sort_config_ref)?;
    }
    Ok(())
}

fn load_dictionaries_with_empty_sort_keys(
    conn: &Connection,
    scope: &SearchScope,
) -> Result<Vec<i64>, String> {
    let (sql, params_list): (&str, Vec<SqlParam>) = match scope {
        SearchScope::Current(dictionary_id) => (
            "SELECT DISTINCT dictionary_id
             FROM data_dictionary_records
             WHERE dictionary_id = ?1 AND sort_key = ''",
            vec![Box::new(*dictionary_id)],
        ),
        SearchScope::All => (
            "SELECT DISTINCT dictionary_id
             FROM data_dictionary_records
             WHERE sort_key = ''",
            Vec::new(),
        ),
    };
    let refs: Vec<&dyn ToSql> = params_list.iter().map(|param| param.as_ref()).collect();
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("prepare empty sort key scan failed: {e}"))?;
    let rows = stmt
        .query_map(refs.as_slice(), |row| row.get::<_, i64>(0))
        .map_err(|e| format!("query empty sort key scan failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
```

- [ ] **Step 6: Wire refresh helpers into mutation paths**

In `action_update_fields`, after record rewrite logic:

- If records were reinserted because `primary_field_path` is set, no extra sort refresh is needed because `insert_records` already wrote `sort_key`.
- If records were not reinserted, call:

```rust
refresh_dictionary_sort_keys(&tx, dictionary_id, new_sort_config)?;
```

In `rebuild_dictionary_indexes`, load sort config before iterating records:

```rust
let sort_config = load_dictionary_sort_config(conn, dictionary_id)?;
let sort_config_ref = sort_config
    .as_ref()
    .map(|(path, direction)| (path.as_str(), *direction));
```

When updating `search_text`, also update `sort_key`:

```rust
let sort_key = build_record_sort_key(value, *row_index, sort_config_ref)?;
conn.execute(
    "UPDATE data_dictionary_records
     SET search_text = ?1, normalized_search_text = ?2, sort_key = ?3
     WHERE id = ?4",
    params![search_text, normalized, sort_key, record_id],
)
.map_err(|e| format!("update rebuilt search text failed: {e}"))?;
```

- [ ] **Step 7: Run Rust tests and cargo check**

Run:

```powershell
cargo test data_dictionary -- --nocapture
cargo check
```

Expected: all data dictionary tests PASS and `cargo check` PASS.

- [ ] **Step 8: Commit generation and refresh logic**

Run:

```powershell
git add apps/desktop/src-tauri/src/tools/data_dictionary.rs
git commit -m "feat(data-dictionary): 生成记录排序键"
```

Expected: commit succeeds. Stage only sort-key hunks if unrelated edits exist.

---

## Task 4: Simplify Search Path And Lock Global Ordering

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/data_dictionary.rs`

- [ ] **Step 1: Add tests for search query limit and scope refresh**

Add these tests inside the test module:

```rust
#[test]
fn parse_limit_keeps_existing_search_limit_bounds() {
    assert_eq!(parse_limit(&json!({ "limit": 0 })), 1);
    assert_eq!(parse_limit(&json!({ "limit": 100 })), 100);
    assert_eq!(parse_limit(&json!({ "limit": 1000 })), 500);
}

#[test]
fn query_records_sql_current_uses_sort_key_without_nav_order() {
    let conditions = vec!["r.dictionary_id = ?".to_string()];
    let sql = build_record_query_sql(&SearchScope::Current(7), &conditions, Some(101));

    assert!(sql.contains("ORDER BY r.sort_key COLLATE BINARY ASC, r.id ASC"));
    assert!(!sql.contains("d.nav_order ASC"));
    assert!(sql.ends_with(" LIMIT ?"));
}
```

- [ ] **Step 2: Run the focused tests**

Run:

```powershell
cargo test data_dictionary::tests::query_records_sql_current_uses_sort_key_without_nav_order -- --nocapture
cargo test data_dictionary::tests::parse_limit_keeps_existing_search_limit_bounds -- --nocapture
```

Expected: PASS after Task 2's SQL helper exists.

- [ ] **Step 3: Remove in-memory search sorting from `action_search`**

Change `action_search` to:

```rust
fn action_search(payload: &Value) -> Result<Value, String> {
    let scope = parse_search_scope(payload)?;
    let keyword = payload["keyword"].as_str().unwrap_or("").trim();
    let limit = parse_limit(payload);
    let fetch_limit = limit + 1;
    let conn = db_conn()?;
    if let SearchScope::Current(id) = scope {
        ensure_dictionary_exists(&conn, id)?;
    }
    ensure_scope_sort_keys_ready(&conn, &scope)?;

    let mut rows = if keyword.is_empty() {
        query_empty_records(&conn, &scope, Some(fetch_limit))?
    } else {
        query_like_records(&conn, &scope, keyword, Some(fetch_limit))?
    };

    if !keyword.is_empty() && rows.len() < fetch_limit as usize && data_dictionary_has_fts(&conn) {
        if let Some(fts_keyword) = build_fts_keyword(keyword) {
            if let Ok(fts_rows) = query_fts_records(&conn, &scope, &fts_keyword, Some(fetch_limit)) {
                let mut seen: HashSet<i64> = rows.iter().map(|row| row.id).collect();
                for row in fts_rows {
                    if seen.insert(row.id) {
                        rows.push(row);
                    }
                }
                rows = order_rows_after_fts_merge(&conn, &scope, rows)?;
            }
        }
    }

    let (rows, has_more) = split_limited_rows(rows, limit as usize);
    let items = rows_to_search_items(&conn, rows, keyword)?;
    Ok(json!({ "items": items, "hasMore": has_more }))
}
```

Add this helper:

```rust
fn order_rows_after_fts_merge(
    conn: &Connection,
    scope: &SearchScope,
    rows: Vec<RecordRow>,
) -> Result<Vec<RecordRow>, String> {
    if rows.len() <= 1 {
        return Ok(rows);
    }
    let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    query_records_by_ids_in_sort_order(conn, scope, &ids)
}

fn query_records_by_ids_in_sort_order(
    conn: &Connection,
    scope: &SearchScope,
    ids: &[i64],
) -> Result<Vec<RecordRow>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat("?")
        .take(ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let conditions = vec![format!("r.id IN ({placeholders})")];
    let params_list = ids
        .iter()
        .map(|id| Box::new(*id) as SqlParam)
        .collect::<Vec<_>>();
    query_records(conn, scope, conditions, params_list, None, false)
}
```

- [ ] **Step 4: Remove now-unused in-memory sort config from `action_search`**

Delete this pattern from `action_search`:

```rust
let sort_config = match scope {
    SearchScope::Current(id) => load_dictionary_sort_config(&conn, id)?,
    SearchScope::All => None,
};
let query_limit = if sort_config.is_some() {
    None
} else {
    Some(fetch_limit)
};
...
if let Some((path, direction)) = sort_config.as_ref() {
    sort_record_rows(&mut rows, Some((path.as_str(), *direction)));
}
```

`sort_record_rows` may still be used by relation brief sorting. Do not remove it unless `rg -n "sort_record_rows" apps/desktop/src-tauri/src/tools/data_dictionary.rs` shows no remaining call sites.

- [ ] **Step 5: Run search-related Rust tests**

Run:

```powershell
cargo test data_dictionary -- --nocapture
cargo check
```

Expected: all data dictionary tests PASS and `cargo check` PASS.

- [ ] **Step 6: Commit search ordering change**

Run:

```powershell
git add apps/desktop/src-tauri/src/tools/data_dictionary.rs
git commit -m "feat(data-dictionary): 按排序键查询结果"
```

Expected: commit succeeds.

---

## Task 5: Front-End Guard Check, Full Validation, And Process Log

**Files:**

- Modify if needed: `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`
- Modify if needed: `process.md`

- [ ] **Step 1: Confirm front-end search still delegates ordering to backend**

Run:

```powershell
rg -n "tool:data-dictionary:search|sort_key|sortFieldPath|runSearch" apps/desktop/src/components/DataDictionaryPanel.vue apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts
```

Expected:

- `DataDictionaryPanel.vue` still calls `tool:data-dictionary:search`.
- The component does not sort `searchItems` locally.
- No front-end `sort_key` type or prop is required.

- [ ] **Step 2: Update the front-end source guard only if it breaks**

If `DataDictionaryPanel.context-menu.test.ts` fails because of changed source text, update only the brittle source assertion. Keep the semantic assertion that search requests do not carry front-end sorting parameters:

```ts
it("keeps data dictionary search ordering on the backend", () => {
  expect(source).toContain('"tool:data-dictionary:search"');
  expect(source).toContain("limit: 100");
  expect(source).not.toContain("searchItems.value.sort(");
  expect(source).not.toContain("sortKey");
});
```

- [ ] **Step 3: Run targeted front-end tests**

Run:

```powershell
pnpm test src/components/DataDictionaryPanel.context-menu.test.ts src/utils/dataDictionary.test.ts
```

Expected: PASS.

- [ ] **Step 4: Run full relevant validation**

Run:

```powershell
cargo test data_dictionary -- --nocapture
cargo check
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected:

- `cargo test data_dictionary -- --nocapture`: PASS.
- `cargo check`: PASS.
- `pnpm typecheck`: PASS.
- `pnpm --filter @lazycat/desktop build:web`: PASS.

- [ ] **Step 5: Add process log entry because this touches 3+ files or creates reusable sort-key guidance**

Add a new top entry to `process.md`:

```markdown
## 2026-06-27: 数据字典查询排序使用记录级派生 sort_key

**场景**: 数据字典“全部”查询需要先按左侧字典顺序，再按每个字典自己的记录排序配置返回结果。
**使用次数**: 0
**问题**:
1. 只在查询阶段解析 `raw_json` 排序会让全局搜索排序链路复杂，也不利于截断前排序。
2. 直接用 `normalized_value` 排序会混淆等值匹配和排序语义，数字排序也容易出错。
3. 降序如果反转整个排序键，会把缺失值或同值记录的兜底顺序也反转。
**解决**:
1. 在 `data_dictionary_records` 增加非空派生 `sort_key`，由当前 `sort_field_path`、`sort_direction` 和 `row_index` 编码生成。
2. 未配置排序字段或记录缺失排序字段时，把 `row_index` 编入 `sort_key` 作为兜底排序，不在查询 SQL 里额外补 CASE。
3. 降序只反转业务值编码段，不反转 bucket 和 row_index 兜底段，查询始终 `ORDER BY sort_key COLLATE BINARY ASC`。
**关键点**:
1. 派生排序键必须在导入、替换、字段配置保存、重建索引和历史数据回填路径同步维护。
2. 排序键是可重建索引，不是业务事实源；`raw_json` 仍是唯一事实源。
3. 排序必须发生在结果截断前，不能先取 100 条再在前端排序。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/data_dictionary.rs`
- `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`
**验证**:
- `cargo test data_dictionary -- --nocapture`
- `cargo check`
- `pnpm test src/components/DataDictionaryPanel.context-menu.test.ts src/utils/dataDictionary.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
```

- [ ] **Step 6: Commit validation and process log changes**

Run:

```powershell
git add process.md apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts
git commit -m "docs(data-dictionary): 记录排序键实现经验"
```

Expected: commit succeeds. If `DataDictionaryPanel.context-menu.test.ts` did not change, commit only `process.md`.

---

## Self-Review Checklist

Spec coverage:

- Left-side dictionary order: Task 2 and Task 4 change global SQL to `d.nav_order ASC`.
- Per-dictionary record order: Task 1 and Task 3 generate `sort_key` from each dictionary's config.
- `row_index` fallback encoded into `sort_key`: Task 1 tests and implementation use bucket `1` and bucket `2`.
- Query directly orders by `sort_key`: Task 2 and Task 4 remove `updated_at` and `row_index` query ordering.
- No front-end sort: Task 5 verifies the Vue component still delegates ordering to the backend.
- Validation: Task 5 runs Rust, typecheck, and web build commands.

Placeholder scan:

- The plan contains no intentionally unfinished implementation steps.
- All commands include expected outcomes.
- Code steps include concrete snippets and exact paths.

Type consistency:

- `SortDirection` is the existing Rust enum.
- `SearchScope` is the existing Rust enum.
- `SqlParam` is the existing alias `Box<dyn ToSql>`.
- `build_record_sort_key`, `refresh_dictionary_sort_keys`, and `ensure_scope_sort_keys_ready` are introduced before use.
