use serde_json::Value;

mod actions;
mod import;
mod model;
mod path;
mod records;
mod repository;

use actions::*;

pub(crate) fn ensure_search_index_schema(conn: &rusqlite::Connection) -> Result<(), String> {
    repository::ensure_search_index_schema(conn)
}

#[cfg(test)]
use import::*;
#[cfg(test)]
use model::*;
#[cfg(test)]
use records::*;
#[cfg(test)]
use repository::*;
#[cfg(test)]
use rusqlite::Connection;
#[cfg(test)]
use std::collections::HashSet;

const ACTIONS: &[&str] = &[
    "list",
    "get",
    "import_preview",
    "create",
    "rename",
    "replace_records",
    "update_fields",
    "reorder",
    "search",
    "popular_records",
    "mark_record_used",
    "record_detail",
    "rebuild_indexes",
    "delete",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported data dictionary action: {action}"));
    }
    match action {
        "list" => action_list(),
        "get" => action_get(payload),
        "import_preview" => action_import_preview(payload),
        "create" => action_create(payload),
        "rename" => action_rename(payload),
        "replace_records" => action_replace_records(payload),
        "update_fields" => action_update_fields(payload),
        "reorder" => action_reorder(payload),
        "search" => action_search(payload),
        "popular_records" => action_popular_records(payload),
        "mark_record_used" => action_mark_record_used(payload),
        "record_detail" => action_record_detail(payload),
        "rebuild_indexes" => action_rebuild_indexes(payload),
        "delete" => action_delete(payload),
        _ => Err(format!("unsupported data dictionary action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_import_payload_rejects_invalid_json() {
        let err = parse_import_array("{ nope").expect_err("invalid json must fail");
        assert!(err.contains("JSON 解析失败"));
    }

    #[test]
    fn parse_import_payload_requires_array() {
        let err = parse_import_array(r#"{"id":1}"#).expect_err("object root must fail");
        assert!(err.contains("JSON array"));
    }

    #[test]
    fn parse_import_payload_requires_object_items() {
        let err = parse_import_array(r#"[{"id":1}, 2]"#).expect_err("scalar item must fail");
        assert!(err.contains("第 2 行"));
    }

    #[test]
    fn import_preview_reads_large_json_from_file_path() {
        let large_value = "x".repeat(1024);
        let items = (0..10_000)
            .map(|idx| format!(r#"{{"id":{idx},"name":"item-{idx}","payload":"{large_value}"}}"#))
            .collect::<Vec<_>>();
        let input = format!("[{}]", items.join(","));
        assert!(input.len() > 10 * 1024 * 1024);

        let path = std::env::temp_dir().join(format!(
            "lazycat-data-dictionary-large-import-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, &input).expect("write large import payload");

        let out = action_import_preview(&json!({ "inputPath": path.to_string_lossy() }))
            .expect("preview large file import");
        let _ = std::fs::remove_file(path);

        assert_eq!(out["recordCount"], json!(10_000));
    }

    #[test]
    fn flatten_object_supports_nested_dot_path() {
        let fields = flatten_record(&json!({
            "id": 1,
            "user": { "name": "张三", "role": "admin" },
            "active": true
        }));

        let pairs: Vec<(String, String, String)> = fields
            .into_iter()
            .map(|field| (field.path, field.value_text, field.type_hint))
            .collect();
        assert_eq!(
            pairs,
            vec![
                (
                    "active".to_string(),
                    "true".to_string(),
                    "boolean".to_string()
                ),
                ("id".to_string(), "1".to_string(), "number".to_string()),
                (
                    "user.name".to_string(),
                    "张三".to_string(),
                    "string".to_string()
                ),
                (
                    "user.role".to_string(),
                    "admin".to_string(),
                    "string".to_string()
                ),
            ]
        );
    }

    #[test]
    fn flatten_object_escapes_dot_and_backslash_in_key() {
        let fields = flatten_record(&json!({
            "user.name": { "a\\b": "value" }
        }));
        assert_eq!(fields[0].path, r#"user\.name.a\\b"#);
    }

    #[test]
    fn flatten_object_treats_array_as_leaf() {
        let fields = flatten_record(&json!({ "tags": ["A", "B"] }));
        assert_eq!(fields[0].path, "tags");
        assert_eq!(fields[0].type_hint, "array");
        assert_eq!(fields[0].value_text, r#"["A","B"]"#);
    }

    #[test]
    fn record_values_persists_value_type_for_null_vs_string_null() {
        let values =
            build_record_values(11, 7, r#"{"empty":null,"text":"null","count":3}"#).unwrap();

        let pairs = values
            .into_iter()
            .map(|value| (value.field_path, value.value_type, value.normalized_value))
            .collect::<Vec<_>>();

        assert_eq!(
            pairs,
            vec![
                ("count".to_string(), "number".to_string(), "3".to_string()),
                ("empty".to_string(), "null".to_string(), "null".to_string()),
                ("text".to_string(), "string".to_string(), "null".to_string()),
            ],
        );
    }

    #[test]
    fn partition_records_by_primary_skips_invalid_and_duplicate_values() {
        let partition = partition_records_by_primary(
            vec![
                IndexedRecord {
                    source_row_index: 0,
                    value: json!({ "employeeNo": "" }),
                },
                IndexedRecord {
                    source_row_index: 1,
                    value: json!({ "name": "missing" }),
                },
                IndexedRecord {
                    source_row_index: 2,
                    value: json!({ "employeeNo": "A001" }),
                },
                IndexedRecord {
                    source_row_index: 3,
                    value: json!({ "employeeNo": " A001 " }),
                },
                IndexedRecord {
                    source_row_index: 4,
                    value: json!({ "employeeNo": "A002" }),
                },
            ],
            Some("employeeNo"),
        )
        .unwrap();

        assert_eq!(partition.skipped_invalid_count, 2);
        assert_eq!(partition.skipped_duplicate_count, 1);
        assert_eq!(
            partition
                .accepted_records
                .iter()
                .map(|record| record.source_row_index)
                .collect::<Vec<_>>(),
            vec![2, 4],
        );
    }

    #[test]
    fn build_search_text_uses_only_searchable_fields() {
        let fields = vec![
            FlattenedField {
                path: "id".to_string(),
                value_text: "1001".to_string(),
                type_hint: "number".to_string(),
            },
            FlattenedField {
                path: "secret".to_string(),
                value_text: "hidden".to_string(),
                type_hint: "string".to_string(),
            },
        ];

        assert_eq!(build_search_text(&fields, &["id".to_string()]), "1001");
    }

    #[test]
    fn normalize_search_text_lowercases_and_collapses_spaces() {
        assert_eq!(normalize_search_text("  Foo\tBAR \n Baz  "), "foo bar baz");
    }

    #[test]
    fn record_search_normalization_equates_common_separators() {
        assert_eq!(
            normalize_record_search_text(" Request_Forward.Rule / API "),
            "request forward rule api"
        );
    }

    #[test]
    fn pinyin_search_text_contains_full_compact_and_initial_forms() {
        let index = build_pinyin_search_text("数据字典");

        assert!(index.contains("shu ju zi dian"));
        assert!(index.contains("shujuzidian"));
        assert!(index.contains("sjzd"));
    }

    #[test]
    fn search_like_pattern_escapes_percent_underscore_and_backslash() {
        assert_eq!(escape_like_pattern(r#"a%b_c\d"#), r#"a\%b\_c\\d"#);
    }

    #[test]
    fn parse_search_scope_supports_current_and_all() {
        assert_eq!(
            parse_search_scope(&json!({ "scope": "current", "dictionaryId": 7 })).unwrap(),
            SearchScope::Current(7)
        );
        assert_eq!(
            parse_search_scope(&json!({ "scope": "all", "dictionaryId": 7 })).unwrap(),
            SearchScope::All
        );
        assert!(parse_search_scope(&json!({ "scope": "current" })).is_err());
    }

    #[test]
    fn compute_matches_returns_full_field_paths() {
        let matches = compute_matches(
            &json!({ "user": { "name": "张三" }, "code": "A001" }),
            &["user.name".to_string(), "code".to_string()],
            "张",
        );

        assert_eq!(
            matches,
            vec![json!({ "fieldPath": "user.name", "value": "张三" })]
        );
    }

    #[test]
    fn record_brief_uses_title_field_and_visible_summary() {
        let mut row = test_record_row(
            9,
            4,
            json!({ "name": "张三", "dept": "研发", "secret": "hidden" }),
        );
        row.title_field_path = Some("name".to_string());
        let fields = vec![
            test_field_config("name", "姓名", true, 0),
            test_field_config("dept", "部门", true, 1),
            test_field_config("secret", "密钥", false, 2),
        ];

        let brief = record_row_to_brief_json(row, &fields).unwrap();

        assert_eq!(brief["title"], "张三");
        assert_eq!(
            brief["summary"],
            json!([
                { "fieldPath": "dept", "label": "部门", "value": "研发" }
            ])
        );
    }

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
        assert_eq!(
            item["summary"],
            json!([
                { "fieldPath": "id", "label": "编号", "value": "1001" },
                { "fieldPath": "name", "label": "姓名", "value": "张三" },
                { "fieldPath": "dept", "label": "部门", "value": "研发" }
            ])
        );
        assert_eq!(
            item["matches"],
            json!([
                { "fieldPath": "name", "value": "张三" }
            ])
        );
        assert!(item.get("rawJson").is_none());
    }

    #[test]
    fn search_item_keeps_raw_json_by_default_shape() {
        let mut row = test_record_row(5, 0, json!({ "name": "张三" }));
        row.title_field_path = Some("name".to_string());
        let fields = vec![test_field_config("name", "姓名", true, 0)];

        let item =
            record_row_to_search_item_json(row, &["name".to_string()], &fields, "", true).unwrap();

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

    #[test]
    fn relation_value_filter_ignores_null_and_blank_values() {
        assert!(is_relation_value_usable("string", "a001"));
        assert!(is_relation_value_usable("number", "100"));
        assert!(!is_relation_value_usable("string", ""));
        assert!(!is_relation_value_usable("null", "null"));
        assert!(!is_relation_value_usable("array", "[\"A\"]"));
    }

    #[test]
    fn split_limited_rows_marks_truncated_results() {
        let rows = (0..3)
            .map(|idx| RecordRow {
                id: idx + 1,
                dictionary_id: 1,
                dictionary_name: "测试字典".to_string(),
                title_field_path: None,
                row_index: idx,
                raw_json: "{}".to_string(),
            })
            .collect::<Vec<_>>();

        let (limited, has_more) = split_limited_rows(rows, 2);

        assert!(has_more);
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[1].row_index, 1);
    }

    #[test]
    fn build_record_sort_key_orders_numbers_numerically() {
        let small = build_record_sort_key(
            &json!({ "score": 2 }),
            0,
            Some(("score", SortDirection::Asc)),
        )
        .unwrap();
        let large = build_record_sort_key(
            &json!({ "score": 10 }),
            1,
            Some(("score", SortDirection::Asc)),
        )
        .unwrap();

        assert!(small < large);
    }

    #[test]
    fn build_record_sort_key_supports_desc_without_moving_missing_values_first() {
        let high = build_record_sort_key(
            &json!({ "score": 100 }),
            0,
            Some(("score", SortDirection::Desc)),
        )
        .unwrap();
        let low = build_record_sort_key(
            &json!({ "score": 90 }),
            1,
            Some(("score", SortDirection::Desc)),
        )
        .unwrap();
        let missing = build_record_sort_key(
            &json!({ "name": "missing" }),
            2,
            Some(("score", SortDirection::Desc)),
        )
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
        let present = build_record_sort_key(
            &json!({ "score": 1 }),
            9,
            Some(("score", SortDirection::Asc)),
        )
        .unwrap();
        let missing_first = build_record_sort_key(
            &json!({ "name": "a" }),
            0,
            Some(("score", SortDirection::Asc)),
        )
        .unwrap();
        let missing_second = build_record_sort_key(
            &json!({ "name": "b" }),
            1,
            Some(("score", SortDirection::Asc)),
        )
        .unwrap();

        assert!(present < missing_first);
        assert!(missing_first < missing_second);
    }

    #[test]
    fn build_record_sort_key_orders_strings_by_prefix_before_longer_value() {
        let short = build_record_sort_key(
            &json!({ "name": "a" }),
            0,
            Some(("name", SortDirection::Asc)),
        )
        .unwrap();
        let long = build_record_sort_key(
            &json!({ "name": "aa" }),
            1,
            Some(("name", SortDirection::Asc)),
        )
        .unwrap();

        assert!(short < long);
    }

    #[test]
    fn query_records_sql_orders_by_sort_key_not_updated_at() {
        let sql = build_record_query_sql(&SearchScope::All, &[], Some(100));

        assert!(sql.contains("ORDER BY d.nav_order ASC, r.sort_key COLLATE BINARY ASC, r.id ASC"));
        assert!(!sql.contains("ORDER BY d.updated_at DESC"));
        assert!(!sql.contains("ORDER BY r.row_index ASC"));
    }

    #[test]
    fn query_records_sql_current_uses_sort_key_without_nav_order() {
        let conditions = vec!["r.dictionary_id = ?".to_string()];
        let sql = build_record_query_sql(&SearchScope::Current(7), &conditions, Some(101));

        assert!(sql.contains("ORDER BY r.sort_key COLLATE BINARY ASC, r.id ASC"));
        assert!(!sql.contains("d.nav_order ASC"));
        assert!(sql.ends_with(" LIMIT ?"));
    }

    #[test]
    fn ranked_search_query_uses_token_and_quality_order_without_fts() {
        let sql = build_ranked_record_query_sql(&SearchScope::All, 2);

        assert_eq!(sql.matches("r.pinyin_search_text LIKE").count(), 6);
        assert!(sql.contains(" AND (r.normalized_search_text LIKE"));
        assert!(sql.contains("ORDER BY recall_score DESC, d.nav_order ASC"));
        assert!(!sql.contains("data_dictionary_fts"));
        assert!(!sql.contains("MATCH"));
    }

    #[test]
    fn popular_records_query_uses_unified_usage_identity() {
        let sql = build_popular_records_query("WHERE 1 = 1");

        assert!(sql.contains("u.resource_id AS normalized_value"));
        assert!(sql.contains("u.resource_type = 'data-dictionary-record'"));
        assert!(sql.contains("SUM(u.use_count) AS used_count"));
        assert!(!sql.contains("data_dictionary_record_usage"));
    }

    #[test]
    fn parse_limit_keeps_existing_search_limit_bounds() {
        assert_eq!(parse_limit(&json!({ "limit": 0 })), 1);
        assert_eq!(parse_limit(&json!({ "limit": 100 })), 100);
        assert_eq!(parse_limit(&json!({ "limit": 1000 })), 500);
    }

    #[test]
    fn load_dictionaries_with_empty_sort_keys_filters_scope() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE data_dictionary_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                dictionary_id INTEGER NOT NULL,
                row_index INTEGER NOT NULL,
                raw_json TEXT NOT NULL,
                search_text TEXT NOT NULL,
                normalized_search_text TEXT NOT NULL,
                pinyin_search_text TEXT NOT NULL DEFAULT '',
                sort_key TEXT NOT NULL DEFAULT ''
            );
            INSERT INTO data_dictionary_records
                (dictionary_id, row_index, raw_json, search_text, normalized_search_text, sort_key)
            VALUES
                (1, 0, '{}', '', '', ''),
                (2, 0, '{}', '', '', '1!0000000000000000'),
                (3, 0, '{}', '', '', '');
            ",
        )
        .unwrap();

        assert_eq!(
            load_dictionaries_with_empty_sort_keys(&conn, &SearchScope::All).unwrap(),
            vec![1, 3]
        );
        assert_eq!(
            load_dictionaries_with_empty_sort_keys(&conn, &SearchScope::Current(3)).unwrap(),
            vec![3]
        );
    }

    #[test]
    fn insert_records_persists_row_index_sort_key_without_sort_config() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE data_dictionary_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                dictionary_id INTEGER NOT NULL,
                row_index INTEGER NOT NULL,
                raw_json TEXT NOT NULL,
                search_text TEXT NOT NULL,
                normalized_search_text TEXT NOT NULL,
                pinyin_search_text TEXT NOT NULL DEFAULT '',
                sort_key TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE data_dictionary_record_values (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                record_id INTEGER NOT NULL,
                dictionary_id INTEGER NOT NULL,
                field_path TEXT NOT NULL,
                value_type TEXT NOT NULL,
                value_text TEXT NOT NULL,
                normalized_value TEXT NOT NULL,
                UNIQUE(record_id, field_path)
            );
            ",
        )
        .unwrap();
        let records = vec![
            IndexedRecord {
                source_row_index: 0,
                value: json!({ "score": 100 }),
            },
            IndexedRecord {
                source_row_index: 1,
                value: json!({ "score": 1 }),
            },
        ];

        insert_records(&conn, 1, &records, &["score".to_string()], None).unwrap();

        let mut stmt = conn
            .prepare("SELECT sort_key FROM data_dictionary_records ORDER BY row_index ASC")
            .unwrap();
        let keys = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|key| !key.is_empty()));
        assert!(keys[0] < keys[1]);
    }

    #[test]
    fn search_index_migration_backfills_normalized_and_pinyin_text_atomically() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE data_dictionary_records (
                 id INTEGER PRIMARY KEY,
                 search_text TEXT NOT NULL,
                 normalized_search_text TEXT NOT NULL
             );
             INSERT INTO data_dictionary_records(id, search_text, normalized_search_text)
             VALUES(1, '数据-字典', 'stale');",
        )
        .unwrap();

        ensure_search_index_schema(&conn).unwrap();
        ensure_search_index_schema(&conn).unwrap();

        let indexed = conn
            .query_row(
                "SELECT normalized_search_text, pinyin_search_text
                 FROM data_dictionary_records WHERE id = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap();
        assert_eq!(indexed.0, "数据 字典");
        assert!(indexed.1.contains("shujuzidian"));
    }

    #[test]
    fn ranked_search_prefers_exact_title_and_recalls_full_pinyin() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE data_dictionaries (
                 id INTEGER PRIMARY KEY,
                 name TEXT NOT NULL,
                 title_field_path TEXT,
                 nav_order INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE data_dictionary_records (
                 id INTEGER PRIMARY KEY,
                 dictionary_id INTEGER NOT NULL,
                 row_index INTEGER NOT NULL,
                 raw_json TEXT NOT NULL,
                 search_text TEXT NOT NULL,
                 normalized_search_text TEXT NOT NULL,
                 pinyin_search_text TEXT NOT NULL,
                 sort_key TEXT NOT NULL
             );
             CREATE TABLE data_dictionary_record_values (
                 record_id INTEGER NOT NULL,
                 field_path TEXT NOT NULL,
                 normalized_value TEXT NOT NULL
             );
             INSERT INTO data_dictionaries(id, name, title_field_path, nav_order)
             VALUES(1, '测试字典', 'name', 0);
             INSERT INTO data_dictionary_records(
                 id, dictionary_id, row_index, raw_json, search_text,
                 normalized_search_text, pinyin_search_text, sort_key
             ) VALUES
                 (1, 1, 0, '{\"name\":\"Later\",\"note\":\"target user\"}',
                  'Later target user', 'later target user', 'later target user', '1!0'),
                 (2, 1, 1, '{\"name\":\"User\"}',
                  'User', 'user', 'user', '1!1'),
                 (3, 1, 2, '{\"name\":\"数据字典\"}',
                  '数据字典', '数据字典', 'shu ju zi dian shujuzidian sjzd', '1!2');
             INSERT INTO data_dictionary_record_values(record_id, field_path, normalized_value)
             VALUES(1, 'name', 'later'), (2, 'name', 'user'), (3, 'name', '数据字典');",
        )
        .unwrap();

        let title_results = query_ranked_records(&conn, &SearchScope::All, "user", 10).unwrap();
        let pinyin_results =
            query_ranked_records(&conn, &SearchScope::All, "shujuzidian", 10).unwrap();
        let cross_field =
            query_ranked_records(&conn, &SearchScope::All, "target user", 10).unwrap();

        assert_eq!(title_results[0].row.id, 2);
        assert!(title_results[0].recall_score > title_results[1].recall_score);
        assert_eq!(pinyin_results[0].row.id, 3);
        assert_eq!(cross_field[0].row.id, 1);
    }

    #[test]
    fn build_record_sort_key_desc_keeps_equal_values_in_row_order() {
        let first = build_record_sort_key(
            &json!({ "score": 100 }),
            0,
            Some(("score", SortDirection::Desc)),
        )
        .unwrap();
        let second = build_record_sort_key(
            &json!({ "score": 100 }),
            1,
            Some(("score", SortDirection::Desc)),
        )
        .unwrap();

        assert!(first < second);
    }

    #[test]
    fn build_record_sort_key_handles_nested_field_paths() {
        let first = build_record_sort_key(
            &json!({ "user": { "name": "Ada" } }),
            0,
            Some(("user.name", SortDirection::Asc)),
        )
        .unwrap();
        let second = build_record_sort_key(
            &json!({ "user": { "name": "Bob" } }),
            1,
            Some(("user.name", SortDirection::Asc)),
        )
        .unwrap();

        assert!(first < second);
    }

    #[test]
    fn sort_record_rows_orders_by_configured_number_field() {
        let mut rows = vec![
            test_record_row(1, 0, json!({ "code": "B", "rank": 20 })),
            test_record_row(2, 1, json!({ "code": "A", "rank": 3 })),
            test_record_row(3, 2, json!({ "code": "C", "rank": 11 })),
        ];

        sort_record_rows(&mut rows, Some(("rank", SortDirection::Asc)));

        assert_eq!(
            rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![2, 3, 1]
        );
    }

    #[test]
    fn sort_record_rows_places_missing_values_last_in_both_directions() {
        let mut rows = vec![
            test_record_row(1, 0, json!({ "rank": 2 })),
            test_record_row(2, 1, json!({ "name": "missing" })),
            test_record_row(3, 2, json!({ "rank": 5 })),
        ];

        sort_record_rows(&mut rows, Some(("rank", SortDirection::Desc)));

        assert_eq!(
            rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![3, 1, 2]
        );
    }

    #[test]
    fn sort_record_rows_keeps_row_index_order_without_sort_config() {
        let mut rows = vec![
            test_record_row(3, 2, json!({ "rank": 11 })),
            test_record_row(1, 0, json!({ "rank": 20 })),
            test_record_row(2, 1, json!({ "rank": 3 })),
        ];

        sort_record_rows(&mut rows, None);

        assert_eq!(
            rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn parse_reorder_dictionary_ids_deduplicates_and_assigns_gapless_order() {
        let updates = parse_reorder_dictionary_ids(&json!({ "ids": [3, 1, 3, 2] })).unwrap();

        assert_eq!(updates, vec![(3, 0), (1, 1), (2, 2)]);
    }

    #[test]
    fn parse_reorder_dictionary_ids_rejects_empty_and_invalid_ids() {
        assert!(parse_reorder_dictionary_ids(&json!({ "ids": [] })).is_err());
        assert!(parse_reorder_dictionary_ids(&json!({ "ids": [1, 0] })).is_err());
        assert!(parse_reorder_dictionary_ids(&json!({ "ids": [1, "2"] })).is_err());
    }

    #[test]
    fn require_non_empty_field_payload_rejects_empty_fields() {
        let fields: Vec<Value> = Vec::new();

        let err = require_non_empty_field_payload(&fields)
            .expect_err("empty field configuration payload must fail");

        assert_eq!(err, "fields must not be empty");
    }

    #[test]
    fn parse_configured_field_path_accepts_known_field_and_blank_default() {
        let seen = HashSet::from(["name".to_string(), "code".to_string()]);

        assert_eq!(
            parse_configured_field_path(
                &json!({ "titleFieldPath": " name " }),
                "titleFieldPath",
                &seen,
            )
            .unwrap(),
            Some("name".to_string())
        );
        assert_eq!(
            parse_configured_field_path(&json!({ "titleFieldPath": "" }), "titleFieldPath", &seen,)
                .unwrap(),
            None
        );
    }

    #[test]
    fn parse_configured_field_path_rejects_unknown_field() {
        let seen = HashSet::from(["name".to_string()]);

        let err = parse_configured_field_path(
            &json!({ "titleFieldPath": "missing" }),
            "titleFieldPath",
            &seen,
        )
        .expect_err("unknown configured field must fail");

        assert_eq!(err, "titleFieldPath not found: missing");
    }

    #[test]
    fn parse_required_field_path_rejects_blank_values() {
        assert_eq!(
            parse_required_field_path(&json!({ "primaryFieldPath": " id " }), "primaryFieldPath")
                .unwrap(),
            "id"
        );

        let err =
            parse_required_field_path(&json!({ "primaryFieldPath": "   " }), "primaryFieldPath")
                .expect_err("blank primary field path must fail");

        assert_eq!(err, "primaryFieldPath is required");
    }

    #[test]
    fn load_record_primary_usage_value_returns_business_and_normalized_values() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE data_dictionary_record_values (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                record_id INTEGER NOT NULL,
                dictionary_id INTEGER NOT NULL,
                field_path TEXT NOT NULL,
                value_type TEXT NOT NULL,
                value_text TEXT NOT NULL,
                normalized_value TEXT NOT NULL
            );
            INSERT INTO data_dictionary_record_values
                (record_id, dictionary_id, field_path, value_type, value_text, normalized_value)
            VALUES
                (10, 1, 'id', 'string', ' U-001 ', 'u-001'),
                (11, 1, 'id', 'null', 'null', 'null');
            ",
        )
        .unwrap();

        assert_eq!(
            load_record_primary_usage_value(&conn, 10, 1, "id").unwrap(),
            (" U-001 ".to_string(), "u-001".to_string())
        );
        assert!(load_record_primary_usage_value(&conn, 11, 1, "id").is_err());
    }

    #[test]
    fn load_record_row_by_primary_value_uses_normalized_value_not_row_id() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE data_dictionaries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                title_field_path TEXT DEFAULT NULL
            );
            CREATE TABLE data_dictionary_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                dictionary_id INTEGER NOT NULL,
                row_index INTEGER NOT NULL,
                raw_json TEXT NOT NULL,
                search_text TEXT NOT NULL,
                normalized_search_text TEXT NOT NULL,
                sort_key TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE data_dictionary_record_values (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                record_id INTEGER NOT NULL,
                dictionary_id INTEGER NOT NULL,
                field_path TEXT NOT NULL,
                value_type TEXT NOT NULL,
                value_text TEXT NOT NULL,
                normalized_value TEXT NOT NULL
            );
            INSERT INTO data_dictionaries (id, name, title_field_path)
            VALUES (1, 'Users', 'name');
            INSERT INTO data_dictionary_records
                (id, dictionary_id, row_index, raw_json, search_text, normalized_search_text, sort_key)
            VALUES
                (100, 1, 0, '{\"id\":\"u1\",\"name\":\"Old\"}', '', '', '1!0000000000000000'),
                (200, 1, 1, '{\"id\":\"u2\",\"name\":\"Current\"}', '', '', '1!0000000000000001');
            INSERT INTO data_dictionary_record_values
                (record_id, dictionary_id, field_path, value_type, value_text, normalized_value)
            VALUES
                (100, 1, 'id', 'string', 'u1', 'u1'),
                (200, 1, 'id', 'string', 'u2', 'u2');
            ",
        )
        .unwrap();

        let row = load_record_row_by_primary_value(&conn, 1, "id", "u2")
            .unwrap()
            .expect("record by normalized primary value");

        assert_eq!(row.id, 200);
        assert_eq!(row.dictionary_name, "Users");
        assert_eq!(row.title_field_path, Some("name".to_string()));
    }

    fn test_record_row(id: i64, row_index: i64, raw_json: Value) -> RecordRow {
        RecordRow {
            id,
            dictionary_id: 1,
            dictionary_name: "测试字典".to_string(),
            title_field_path: None,
            row_index,
            raw_json: serde_json::to_string(&raw_json).unwrap(),
        }
    }

    fn test_field_config(
        field_path: &str,
        display_name: &str,
        visible: bool,
        sort_order: i64,
    ) -> FieldConfig {
        FieldConfig {
            field_path: field_path.to_string(),
            display_name: display_name.to_string(),
            meaning: String::new(),
            searchable: true,
            visible,
            sort_order,
            type_hint: "string".to_string(),
            sample_value: String::new(),
            present_count: 1,
        }
    }
}
