use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FlattenedField {
    pub(super) path: String,
    pub(super) value_text: String,
    pub(super) type_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FieldStat {
    pub(super) path: String,
    pub(super) type_hint: String,
    pub(super) sample_value: String,
    pub(super) present_count: i64,
    pub(super) sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RecordValue {
    pub(super) record_id: i64,
    pub(super) dictionary_id: i64,
    pub(super) field_path: String,
    pub(super) value_type: String,
    pub(super) value_text: String,
    pub(super) normalized_value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct IndexedRecord {
    pub(super) source_row_index: i64,
    pub(super) value: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PrimaryPartition {
    pub(super) accepted_records: Vec<IndexedRecord>,
    pub(super) skipped_invalid_count: usize,
    pub(super) skipped_duplicate_count: usize,
}

impl PrimaryPartition {
    pub(super) fn skipped_record_count(&self) -> usize {
        self.skipped_invalid_count + self.skipped_duplicate_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RebuildStats {
    pub(super) record_count: usize,
    pub(super) value_count: usize,
    pub(super) skipped_invalid_count: usize,
    pub(super) skipped_duplicate_count: usize,
}

impl RebuildStats {
    pub(super) fn skipped_record_count(&self) -> usize {
        self.skipped_invalid_count + self.skipped_duplicate_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RelationDraft {
    pub(super) source_field_path: String,
    pub(super) target_dictionary_id: i64,
    pub(super) relation_name: String,
    pub(super) reverse_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FieldConfig {
    pub(super) field_path: String,
    pub(super) display_name: String,
    pub(super) meaning: String,
    pub(super) searchable: bool,
    pub(super) visible: bool,
    pub(super) sort_order: i64,
    pub(super) type_hint: String,
    pub(super) sample_value: String,
    pub(super) present_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SearchScope {
    Current(i64),
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub(super) struct RecordRow {
    pub(super) id: i64,
    pub(super) dictionary_id: i64,
    pub(super) dictionary_name: String,
    pub(super) title_field_path: Option<String>,
    pub(super) row_index: i64,
    pub(super) raw_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RelationConfig {
    pub(super) id: i64,
    pub(super) source_dictionary_id: i64,
    pub(super) source_field_path: String,
    pub(super) target_dictionary_id: i64,
    pub(super) target_primary_field_path: Option<String>,
    pub(super) relation_name: String,
    pub(super) reverse_name: String,
}
