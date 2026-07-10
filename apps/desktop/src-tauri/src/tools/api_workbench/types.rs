use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub(crate) struct ResponseBodyPayload {
    pub(crate) body_text: String,
    pub(crate) body_size: usize,
    pub(crate) body_truncated: bool,
    pub(crate) body_storage: String,
    pub(crate) body_file_path: String,
    pub(crate) body_file_name: String,
    pub(crate) body_extension: String,
    pub(crate) body_hash: String,
    pub(crate) body_preview_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KeyValueRow {
    pub(crate) enabled: bool,
    pub(crate) key: String,
    pub(crate) value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestDraft {
    pub(crate) method: String,
    pub(crate) url: String,
    pub(crate) query: Vec<KeyValueRow>,
    pub(crate) headers: Vec<KeyValueRow>,
    pub(crate) body_type: String,
    pub(crate) body: String,
    pub(crate) form: Vec<KeyValueRow>,
    pub(crate) timeout_ms: u64,
    #[serde(default)]
    pub(crate) follow_redirects: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecutedRequestSnapshot {
    pub(crate) method: String,
    pub(crate) final_url: String,
    pub(crate) headers: Vec<KeyValueRow>,
    pub(crate) body_type: String,
    pub(crate) body: String,
    pub(crate) form: Vec<KeyValueRow>,
    pub(crate) timeout_ms: u64,
    #[serde(default)]
    pub(crate) follow_redirects: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedBody {
    pub(crate) body: Option<Vec<u8>>,
    pub(crate) content_type: Option<String>,
}
