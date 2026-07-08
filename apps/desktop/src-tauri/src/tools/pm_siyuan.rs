use chrono::{Local, Utc};
use serde::{Deserialize, Serialize};
use rusqlite::{params, Connection, Error as RusqliteError};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use url::Url;

use super::helpers::db_conn;

const SIYUAN_BASE_URL_SETTING_KEY: &str = "pm_siyuan_base_url";
const SIYUAN_TOKEN_SETTING_KEY: &str = "pm_siyuan_token";
const SIYUAN_TEST_TIMEOUT_MS: u64 = 5_000;
const SIYUAN_DIRECTORY_TIMEOUT_MS: u64 = 10_000;
const SIYUAN_SEARCH_PAGE_SIZE: usize = 32;
const SIYUAN_DOC_SQL: &str =
    "SELECT id, box, path, hpath, content FROM blocks WHERE type = 'd' ORDER BY box ASC, hpath ASC, id ASC";
const SIYUAN_DIRECTORY_PAGE_SIZE: usize = 200;

// ── Types ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct SiyuanConfig {
    pub(super) base_url: String,
    pub(super) token: String,
}

#[derive(Debug, Clone)]
pub(super) struct SiyuanNotebook {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) icon: Option<String>,
    pub(super) closed: bool,
}

#[derive(Debug, Clone)]
pub(super) struct SiyuanDocRow {
    pub(super) id: String,
    pub(super) box_id: String,
    pub(super) path: Option<String>,
    pub(super) hpath: String,
    pub(super) name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SiyuanTreeNode {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) hpath: String,
    pub(super) path: Option<String>,
    pub(super) leaf: bool,
    pub(super) children: Vec<SiyuanTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SiyuanNotebookDirectory {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) icon: Option<String>,
    pub(super) closed: bool,
    pub(super) doc_count: usize,
    pub(super) children: Vec<SiyuanTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SiyuanDirectoryResult {
    pub(super) notebooks: Vec<SiyuanNotebookDirectory>,
    pub(super) fetched_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct SiyuanLocation {
    pub(super) notebook_id: String,
    pub(super) notebook_name: String,
    pub(super) parent_doc_id: Option<String>,
    pub(super) parent_doc_title: Option<String>,
    pub(super) parent_hpath: Option<String>,
    pub(super) parent_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct SiyuanPageRef {
    pub(super) doc_id: String,
    pub(super) doc_title: String,
    pub(super) doc_hpath: String,
    pub(super) doc_path: Option<String>,
    pub(super) notebook_id: String,
    pub(super) notebook_name: String,
}

// ── Config & HTTP ──────────────────────────────────────

pub(super) fn normalize_siyuan_base_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("请输入思源服务地址".into());
    }

    let normalized = trimmed.trim_end_matches('/').to_string();
    let parsed = Url::parse(&normalized).map_err(|_| "思源服务地址格式不正确".to_string())?;
    match parsed.scheme() {
        "http" | "https" => Ok(normalized),
        _ => Err("思源服务地址必须以 http:// 或 https:// 开头".into()),
    }
}

fn load_setting_value(key: &str) -> Result<Option<String>, String> {
    let conn = db_conn()?;
    match conn.query_row(
        "SELECT value FROM user_settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    ) {
        Ok(value) => Ok(Some(value)),
        Err(RusqliteError::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(format!("读取设置失败: {err}")),
    }
}

pub(super) fn read_siyuan_config(payload: &Value) -> Result<SiyuanConfig, String> {
    let raw_base_url = if let Some(base_url) = parse_string(payload, "baseUrl") {
        base_url
    } else {
        load_setting_value(SIYUAN_BASE_URL_SETTING_KEY)?
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or("请输入思源服务地址")?
    };
    let base_url = normalize_siyuan_base_url(&raw_base_url)?;

    let token = if let Some(token) = parse_string(payload, "token") {
        token
    } else {
        load_setting_value(SIYUAN_TOKEN_SETTING_KEY)?
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or("请输入思源 API Token")?
    };

    Ok(SiyuanConfig { base_url, token })
}

fn is_siyuan_auth_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("token")
        || lower.contains("auth")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
        || msg.contains("鉴权")
        || msg.contains("认证")
        || msg.contains("未授权")
}

fn normalize_siyuan_error_message(msg: &str) -> String {
    let trimmed = msg.trim();
    if trimmed.is_empty() {
        "思源返回了未知错误".into()
    } else if let Some(mapped) = normalize_siyuan_query_error_message(trimmed) {
        mapped
    } else if is_siyuan_auth_error(trimmed) {
        "思源鉴权失败，请检查 API Token".into()
    } else {
        trimmed.to_string()
    }
}

pub(super) fn parse_siyuan_envelope(text: &str) -> Result<Value, String> {
    let envelope: Value =
        serde_json::from_str(text).map_err(|_| "思源响应格式异常".to_string())?;
    let code = envelope
        .get("code")
        .and_then(Value::as_i64)
        .ok_or("思源响应格式异常")?;
    if code != 0 {
        let msg = envelope
            .get("msg")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(normalize_siyuan_error_message(msg));
    }

    Ok(envelope.get("data").cloned().unwrap_or(Value::Null))
}

fn map_siyuan_http_error(status: u16, body: Option<String>) -> String {
    if status == 401 || status == 403 {
        return "思源鉴权失败，请检查 API Token".into();
    }

    if let Some(text) = body {
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            if let Some(msg) = value.get("msg").and_then(Value::as_str) {
                return normalize_siyuan_error_message(msg);
            }
        }
    }

    format!("思源服务返回 HTTP {status}")
}

pub(super) fn post_siyuan_json(
    config: &SiyuanConfig,
    endpoint: &str,
    body: &Value,
    timeout_ms: u64,
) -> Result<Value, String> {
    let url = format!("{}{}", config.base_url, endpoint);
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(timeout_ms))
        .build();
    let body_text = body.to_string();

    match agent
        .post(&url)
        .set("Authorization", &format!("Token {}", config.token))
        .set("Content-Type", "application/json")
        .send_string(&body_text)
    {
        Ok(response) => {
            let text = response
                .into_string()
                .map_err(|_| "读取思源响应失败".to_string())?;
            parse_siyuan_envelope(&text)
        }
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().ok();
            Err(map_siyuan_http_error(status, body))
        }
        Err(ureq::Error::Transport(_)) => Err("无法连接到思源服务，请检查地址和本地服务状态".into()),
    }
}

// ── Parse helpers ──────────────────────────────────────

pub(super) fn parse_siyuan_notebooks(data: &Value) -> Result<Vec<SiyuanNotebook>, String> {
    let items = data
        .get("notebooks")
        .and_then(Value::as_array)
        .ok_or("思源返回的笔记本数据格式异常")?;

    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("思源返回的第 {} 个笔记本缺少 id", index + 1))?
                .to_string();
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("思源返回的第 {} 个笔记本缺少名称", index + 1))?
                .to_string();
            let icon = item
                .get("icon")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);
            let closed = item.get("closed").and_then(Value::as_bool).unwrap_or(false);

            Ok(SiyuanNotebook {
                id,
                name,
                icon,
                closed,
            })
        })
        .collect()
}

pub(super) fn parse_siyuan_doc_rows(data: &Value) -> Result<Vec<SiyuanDocRow>, String> {
    if data.is_null() {
        return Ok(Vec::new());
    }
    let rows = data.as_array().ok_or("思源返回的目录数据格式异常")?;

    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let id = row
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("思源目录数据第 {} 项缺少 id", index + 1))?
                .to_string();
            let box_id = row
                .get("box")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("思源目录数据第 {} 项缺少 notebook", index + 1))?
                .to_string();
            let hpath = row
                .get("hpath")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("思源目录数据第 {} 项缺少 hpath", index + 1))?
                .to_string();
            let path = row
                .get("path")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);
            let name = row
                .get("content")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .or_else(|| {
                    hpath
                        .rsplit('/')
                        .find(|part| !part.is_empty())
                        .map(ToString::to_string)
                })
                .unwrap_or_else(|| "未命名文档".to_string());

            Ok(SiyuanDocRow {
                id,
                box_id,
                path,
                hpath,
                name,
            })
        })
        .collect()
}

pub(super) fn parse_siyuan_location_value(value: &Value) -> Result<Option<SiyuanLocation>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let Some(object) = value.as_object() else {
        return Err("思源位置格式不正确".into());
    };
    if object.is_empty() {
        return Ok(None);
    }

    let notebook_id = parse_string(value, "notebookId").ok_or("思源位置缺少 notebookId")?;
    let notebook_name =
        parse_string(value, "notebookName").ok_or("思源位置缺少 notebookName")?;

    Ok(Some(SiyuanLocation {
        notebook_id,
        notebook_name,
        parent_doc_id: parse_string(value, "parentDocId"),
        parent_doc_title: parse_string(value, "parentDocTitle"),
        parent_hpath: parse_string(value, "parentHpath"),
        parent_path: parse_string(value, "parentPath"),
    }))
}

pub(super) fn parse_siyuan_page_ref_value(value: &Value) -> Result<Option<SiyuanPageRef>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let Some(object) = value.as_object() else {
        return Err("思源页面引用格式不正确".into());
    };
    if object.is_empty() {
        return Ok(None);
    }

    let doc_id = parse_string(value, "docId").ok_or("思源页面引用缺少 docId")?;
    let doc_title = parse_string(value, "docTitle").ok_or("思源页面引用缺少 docTitle")?;
    let doc_hpath = parse_string(value, "docHpath").ok_or("思源页面引用缺少 docHpath")?;
    let notebook_id = parse_string(value, "notebookId").ok_or("思源页面引用缺少 notebookId")?;
    let notebook_name =
        parse_string(value, "notebookName").ok_or("思源页面引用缺少 notebookName")?;

    Ok(Some(SiyuanPageRef {
        doc_id,
        doc_title,
        doc_hpath,
        doc_path: parse_string(value, "docPath"),
        notebook_id,
        notebook_name,
    }))
}

pub(super) fn parse_siyuan_page_ref_array(value: Option<&Value>) -> Result<Option<Vec<SiyuanPageRef>>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(Some(Vec::new()));
    }

    let arr = value.as_array().ok_or("思源页面列表格式不正确")?;
    let mut seen_doc_ids = HashSet::new();
    let mut pages = Vec::new();
    for item in arr {
        if let Some(page) = parse_siyuan_page_ref_value(item)? {
            if seen_doc_ids.insert(page.doc_id.clone()) {
                pages.push(page);
            }
        }
    }
    Ok(Some(pages))
}

// ── Build helpers (for parent module reuse) ───────────

pub(super) fn build_siyuan_location_from_parts(
    notebook_id: Option<String>,
    notebook_name: Option<String>,
    parent_doc_id: Option<String>,
    parent_doc_title: Option<String>,
    parent_hpath: Option<String>,
    parent_path: Option<String>,
) -> Option<SiyuanLocation> {
    let notebook_id = notebook_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let notebook_name = notebook_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| notebook_id.clone());

    Some(SiyuanLocation {
        notebook_id,
        notebook_name,
        parent_doc_id: parent_doc_id.filter(|value| !value.trim().is_empty()),
        parent_doc_title: parent_doc_title.filter(|value| !value.trim().is_empty()),
        parent_hpath: parent_hpath.filter(|value| !value.trim().is_empty()),
        parent_path: parent_path.filter(|value| !value.trim().is_empty()),
    })
}

pub(super) fn build_siyuan_page_ref_from_parts(
    doc_id: Option<String>,
    doc_title: Option<String>,
    doc_hpath: Option<String>,
    doc_path: Option<String>,
    notebook_id: Option<String>,
    notebook_name: Option<String>,
) -> Option<SiyuanPageRef> {
    let doc_id = doc_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let doc_title = doc_title
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let doc_hpath = doc_hpath
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let notebook_id = notebook_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let notebook_name = notebook_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| notebook_id.clone());

    Some(SiyuanPageRef {
        doc_id,
        doc_title,
        doc_hpath,
        doc_path: doc_path.filter(|value| !value.trim().is_empty()),
        notebook_id,
        notebook_name,
    })
}

// ── Internal helpers ──────────────────────────────────

fn parse_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
}

fn escape_sql_string(value: &str) -> String {
    value.replace('\'', "''").replace('%', "\\%").replace('_', "\\_")
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn normalize_siyuan_page_title(title: &str) -> Result<String, String> {
    let normalized = title
        .trim()
        .replace('/', "／")
        .replace('\\', "＼");
    if normalized.is_empty() {
        Err("页面标题不能为空".into())
    } else {
        Ok(normalized)
    }
}

pub(super) fn build_siyuan_target_hpath(location: &SiyuanLocation, title: &str) -> Result<String, String> {
    let title = normalize_siyuan_page_title(title)?;
    if let Some(parent_hpath) = location
        .parent_hpath
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Ok(format!("{}/{}", parent_hpath.trim_end_matches('/'), title))
    } else {
        Ok(format!("/{}", title))
    }
}

fn build_siyuan_markdown(payload: &Value) -> String {
    let project_name = parse_string(payload, "projectName").unwrap_or_else(|| "未归项目".into());
    let status = parse_string(payload, "status")
        .map(|value| match value.as_str() {
            "todo" => "待办".to_string(),
            "in_progress" => "进行中".to_string(),
            "testing" => "测试中".to_string(),
            "done" => "已完成".to_string(),
            _ => value,
        })
        .unwrap_or_else(|| "待办".into());
    let priority = parse_string(payload, "priority").unwrap_or_else(|| "P2".into());
    let start_at = parse_string(payload, "startAt").unwrap_or_else(|| "-".into());
    let end_at = parse_string(payload, "endAt").unwrap_or_else(|| "-".into());
    let description =
        parse_string(payload, "description").unwrap_or_else(|| "（暂无描述）".into());

    format!(
        "- 项目：{project_name}\n- 状态：{status}\n- 优先级：{priority}\n- 开始日期：{start_at}\n- 截止日期：{end_at}\n- 创建时间：{}\n- 来源：Lazycat 项目管理\n\n## 描述\n\n{description}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    )
}

pub(super) fn build_siyuan_deep_link(doc_id: &str) -> Result<String, String> {
    let doc_id = doc_id.trim();
    if doc_id.is_empty() {
        return Err("页面 ID 不能为空".into());
    }
    Ok(format!("siyuan://blocks/{doc_id}"))
}

// ── Item Siyuan links (for parent module reuse) ────────

pub(super) fn load_item_siyuan_links(conn: &Connection, item_id: i64) -> Vec<SiyuanPageRef> {
    conn.prepare(
        "SELECT doc_id, doc_title, doc_hpath, doc_path, notebook_id, notebook_name
         FROM pm_item_siyuan_links
         WHERE item_id = ?1
         ORDER BY sort_order ASC, id ASC",
    )
    .and_then(|mut stmt| {
        stmt.query_map(params![item_id], |row| {
            Ok(build_siyuan_page_ref_from_parts(
                Some(row.get::<_, String>(0)?),
                Some(row.get::<_, String>(1)?),
                Some(row.get::<_, String>(2)?),
                row.get::<_, Option<String>>(3)?,
                Some(row.get::<_, String>(4)?),
                Some(row.get::<_, String>(5)?),
            ))
        })
        .map(|rows| rows.filter_map(|row| row.ok().flatten()).collect())
    })
    .unwrap_or_default()
}

pub(super) fn save_item_siyuan_links(
    conn: &Connection,
    item_id: i64,
    primary_page: Option<&SiyuanPageRef>,
    extra_pages: &[SiyuanPageRef],
    now: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM pm_item_siyuan_links WHERE item_id = ?1",
        params![item_id],
    )
    .map_err(|err| format!("delete pm_item_siyuan_links failed: {err}"))?;

    let extra_pages = dedupe_siyuan_extra_pages(primary_page, extra_pages);
    for (index, page) in extra_pages.iter().enumerate() {
        conn.execute(
            "INSERT INTO pm_item_siyuan_links (
                item_id, doc_id, doc_title, doc_hpath, doc_path, notebook_id, notebook_name,
                sort_order, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                item_id,
                page.doc_id,
                page.doc_title,
                page.doc_hpath,
                page.doc_path,
                page.notebook_id,
                page.notebook_name,
                index as i64,
                now,
                now
            ],
        )
        .map_err(|err| format!("insert pm_item_siyuan_links failed: {err}"))?;
    }

    Ok(())
}

fn dedupe_siyuan_extra_pages(
    primary_page: Option<&SiyuanPageRef>,
    extra_pages: &[SiyuanPageRef],
) -> Vec<SiyuanPageRef> {
    let mut seen_doc_ids = HashSet::new();
    if let Some(primary_page) = primary_page {
        seen_doc_ids.insert(primary_page.doc_id.clone());
    }

    let mut deduped = Vec::new();
    for page in extra_pages {
        if seen_doc_ids.insert(page.doc_id.clone()) {
            deduped.push(page.clone());
        }
    }
    deduped
}

// ── Directory tree ────────────────────────────────────

fn extract_siyuan_parent_doc_id(path: Option<&str>) -> Option<String> {
    let path = path
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let segments: Vec<&str> = path
        .split('/')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    if segments.len() < 2 {
        return None;
    }
    Some(segments[segments.len() - 2].to_string())
}

fn materialize_siyuan_tree_node(
    node_id: &str,
    nodes_by_id: &mut HashMap<String, SiyuanTreeNode>,
    child_ids_by_parent: &mut HashMap<String, Vec<String>>,
) -> Option<SiyuanTreeNode> {
    let mut node = nodes_by_id.remove(node_id)?;
    let child_ids = child_ids_by_parent.remove(node_id).unwrap_or_default();
    node.children = child_ids
        .into_iter()
        .filter_map(|child_id| {
            materialize_siyuan_tree_node(&child_id, nodes_by_id, child_ids_by_parent)
        })
        .collect();
    node.leaf = node.children.is_empty();
    Some(node)
}

fn build_siyuan_tree(rows: &[SiyuanDocRow]) -> Vec<SiyuanTreeNode> {
    let mut nodes_by_id: HashMap<String, SiyuanTreeNode> = HashMap::new();
    for row in rows {
        nodes_by_id.insert(
            row.id.clone(),
            SiyuanTreeNode {
                id: row.id.clone(),
                name: row.name.clone(),
                hpath: row.hpath.clone(),
                path: row.path.clone(),
                leaf: true,
                children: Vec::new(),
            },
        );
    }

    let mut child_ids_by_parent: HashMap<String, Vec<String>> = HashMap::new();
    let mut root_ids = Vec::new();
    for row in rows {
        let parent_doc_id = extract_siyuan_parent_doc_id(row.path.as_deref());
        if let Some(parent_doc_id) = parent_doc_id {
            if nodes_by_id.contains_key(&parent_doc_id) {
                child_ids_by_parent
                    .entry(parent_doc_id)
                    .or_default()
                    .push(row.id.clone());
                continue;
            }
        }
        root_ids.push(row.id.clone());
    }

    let mut tree = Vec::new();
    for root_id in root_ids {
        if let Some(node) =
            materialize_siyuan_tree_node(&root_id, &mut nodes_by_id, &mut child_ids_by_parent)
        {
            tree.push(node);
        }
    }

    for row in rows {
        if let Some(node) =
            materialize_siyuan_tree_node(&row.id, &mut nodes_by_id, &mut child_ids_by_parent)
        {
            tree.push(node);
        }
    }

    tree
}

pub(super) fn build_siyuan_directory(
    notebooks: Vec<SiyuanNotebook>,
    doc_rows: Vec<SiyuanDocRow>,
) -> SiyuanDirectoryResult {
    let mut grouped_rows: HashMap<String, Vec<SiyuanDocRow>> = HashMap::new();
    for row in doc_rows {
        grouped_rows.entry(row.box_id.clone()).or_default().push(row);
    }

    let notebooks = notebooks
        .into_iter()
        .map(|notebook| {
            let rows = grouped_rows.remove(&notebook.id).unwrap_or_default();
            let children = build_siyuan_tree(&rows);

            SiyuanNotebookDirectory {
                id: notebook.id,
                name: notebook.name,
                icon: notebook.icon,
                closed: notebook.closed,
                doc_count: rows.len(),
                children,
            }
        })
        .collect();

    SiyuanDirectoryResult {
        notebooks,
        fetched_at: now_rfc3339(),
    }
}

fn normalize_siyuan_query_error_message(msg: &str) -> Option<String> {
    let lower = msg.to_lowercase();
    if lower.contains("sql")
        && (lower.contains("publish")
            || lower.contains("readonly")
            || msg.contains("公开")
            || msg.contains("只读"))
    {
        Some("当前思源实例未开放 SQL 查询能力".into())
    } else {
        None
    }
}

// ── Search ────────────────────────────────────────────

pub(super) fn notebook_map(notebooks: &[SiyuanNotebook]) -> HashMap<String, SiyuanNotebook> {
    notebooks
        .iter()
        .map(|notebook| (notebook.id.clone(), notebook.clone()))
        .collect()
}

pub(super) fn load_siyuan_notebooks(config: &SiyuanConfig, timeout_ms: u64) -> Result<Vec<SiyuanNotebook>, String> {
    let notebooks_data = post_siyuan_json(
        config,
        "/api/notebook/lsNotebooks",
        &json!({}),
        timeout_ms,
    )?;
    parse_siyuan_notebooks(&notebooks_data)
}

pub(super) fn load_open_siyuan_notebooks(
    config: &SiyuanConfig,
    timeout_ms: u64,
) -> Result<Vec<SiyuanNotebook>, String> {
    Ok(load_siyuan_notebooks(config, timeout_ms)?
        .into_iter()
        .filter(|notebook| !notebook.closed)
        .collect())
}

pub(super) fn query_siyuan_doc_rows(
    config: &SiyuanConfig,
    stmt: &str,
    timeout_ms: u64,
) -> Result<Vec<SiyuanDocRow>, String> {
    let rows_data = post_siyuan_json(
        config,
        "/api/query/sql",
        &json!({ "stmt": stmt }),
        timeout_ms,
    )?;
    parse_siyuan_doc_rows(&rows_data)
}

pub(super) fn query_siyuan_doc_rows_all(
    config: &SiyuanConfig,
    base_sql: &str,
    timeout_ms: u64,
) -> Result<Vec<SiyuanDocRow>, String> {
    let mut all_rows: Vec<SiyuanDocRow> = Vec::new();
    let mut offset = 0usize;
    loop {
        let paged_sql = format!("{base_sql} LIMIT {SIYUAN_DIRECTORY_PAGE_SIZE} OFFSET {offset}");
        let page = query_siyuan_doc_rows(config, &paged_sql, timeout_ms)?;
        let page_len = page.len();
        all_rows.extend(page);
        if page_len < SIYUAN_DIRECTORY_PAGE_SIZE {
            break;
        }
        offset += SIYUAN_DIRECTORY_PAGE_SIZE;
    }
    Ok(all_rows)
}

pub(super) fn build_siyuan_page_ref_from_row(
    row: &SiyuanDocRow,
    notebooks: &HashMap<String, SiyuanNotebook>,
) -> Option<SiyuanPageRef> {
    let notebook = notebooks.get(&row.box_id)?;
    Some(SiyuanPageRef {
        doc_id: row.id.clone(),
        doc_title: row.name.clone(),
        doc_hpath: row.hpath.clone(),
        doc_path: row.path.clone(),
        notebook_id: row.box_id.clone(),
        notebook_name: notebook.name.clone(),
    })
}

pub(super) fn fetch_siyuan_page_ref(
    config: &SiyuanConfig,
    notebooks: &HashMap<String, SiyuanNotebook>,
    doc_id: &str,
) -> Result<SiyuanPageRef, String> {
    let escaped_doc_id = escape_sql_string(doc_id);
    let stmt = format!(
        "SELECT id, box, path, hpath, content FROM blocks WHERE type = 'd' AND id = '{escaped_doc_id}' LIMIT 1"
    );
    let rows = query_siyuan_doc_rows(config, &stmt, SIYUAN_DIRECTORY_TIMEOUT_MS)?;
    let Some(row) = rows.first() else {
        return Err("未找到思源页面".into());
    };
    build_siyuan_page_ref_from_row(row, notebooks).ok_or("思源页面所在笔记本不存在".into())
}

pub(super) fn normalize_siyuan_search_scope_prefix(path: &str) -> Option<String> {
    let trimmed = path.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.strip_suffix(".sy").unwrap_or(trimmed).trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

pub(super) fn build_siyuan_search_scope_path(location: &SiyuanLocation) -> Result<String, String> {
    let notebook_id = location.notebook_id.trim();
    if notebook_id.is_empty() {
        return Err("思源位置缺少 notebookId".into());
    }

    let scope_prefix = location
        .parent_path
        .as_deref()
        .and_then(normalize_siyuan_search_scope_prefix)
        .or_else(|| {
            location
                .parent_doc_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| format!("/{value}"))
        });

    if let Some(scope_prefix) = scope_prefix {
        let normalized = scope_prefix.trim_start_matches('/');
        if normalized.is_empty() {
            Ok(notebook_id.to_string())
        } else {
            Ok(format!("{notebook_id}/{normalized}"))
        }
    } else {
        Ok(notebook_id.to_string())
    }
}

fn read_siyuan_search_field_string(value: &Value, keys: &[&str]) -> Option<String> {
    let object = value.as_object()?;
    for key in keys {
        if let Some(field) = object.get(*key) {
            if let Some(text) = field.as_str() {
                let text = text.trim();
                if !text.is_empty() {
                    return Some(text.to_string());
                }
            }
        }
    }
    None
}

fn read_siyuan_ial_title(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    let ial = object.get("ial").or_else(|| object.get("IAL"))?;

    if let Some(ial_object) = ial.as_object() {
        let title = ial_object.get("title")?.as_str()?.trim();
        return if title.is_empty() {
            None
        } else {
            Some(title.to_string())
        };
    }

    let ial_text = ial.as_str()?.trim();
    if ial_text.is_empty() {
        return None;
    }
    let parsed = serde_json::from_str::<Value>(ial_text).ok()?;
    let title = parsed.get("title")?.as_str()?.trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn extract_siyuan_title_from_hpath(hpath: &str) -> Option<String> {
    hpath
        .rsplit('/')
        .find(|segment| !segment.trim().is_empty())
        .map(|segment| segment.trim().to_string())
}

pub(super) fn normalize_siyuan_search_doc_hpath(hpath: Option<String>, title: &str) -> String {
    let trimmed_title = title.trim();
    let raw_hpath = hpath
        .unwrap_or_default()
        .trim()
        .trim_end_matches('/')
        .to_string();

    if raw_hpath.is_empty() || raw_hpath == "/" {
        return if trimmed_title.is_empty() {
            "/".into()
        } else {
            format!("/{trimmed_title}")
        };
    }

    if trimmed_title.is_empty() {
        return raw_hpath;
    }

    if extract_siyuan_title_from_hpath(&raw_hpath)
        .as_deref()
        .map(str::trim)
        == Some(trimmed_title)
    {
        raw_hpath
    } else {
        format!("{}/{}", raw_hpath, trimmed_title)
    }
}

pub(super) fn build_siyuan_page_ref_from_search_block(
    value: &Value,
    notebooks: &HashMap<String, SiyuanNotebook>,
) -> Option<SiyuanPageRef> {
    if let Some(block_type) = read_siyuan_search_field_string(value, &["type", "Type"]) {
        if !matches!(block_type.as_str(), "d" | "doc" | "NodeDocument") {
            return None;
        }
    }

    let notebook_id = read_siyuan_search_field_string(value, &["box", "Box"])?;
    let notebook = notebooks.get(&notebook_id)?;
    let doc_id = read_siyuan_search_field_string(value, &["rootID", "rootId", "RootID"])
        .or_else(|| read_siyuan_search_field_string(value, &["id", "ID"]))?;
    let doc_title = read_siyuan_ial_title(value)
        .or_else(|| read_siyuan_search_field_string(value, &["name", "Name"]))
        .or_else(|| read_siyuan_search_field_string(value, &["content", "Content"]))
        .or_else(|| {
            read_siyuan_search_field_string(value, &["hPath", "HPath", "hpath"])
                .and_then(|hpath| extract_siyuan_title_from_hpath(&hpath))
        })
        .unwrap_or_else(|| doc_id.clone());
    let raw_hpath =
        read_siyuan_search_field_string(value, &["hPath", "HPath", "hpath"]).or_else(|| {
            read_siyuan_search_field_string(value, &["path", "Path"])
        });
    let doc_hpath = normalize_siyuan_search_doc_hpath(raw_hpath, &doc_title);

    Some(SiyuanPageRef {
        doc_id,
        doc_title,
        doc_hpath,
        doc_path: read_siyuan_search_field_string(value, &["path", "Path"]),
        notebook_id,
        notebook_name: notebook.name.clone(),
    })
}

pub(super) fn parse_siyuan_search_page_refs(
    data: &Value,
    notebooks: &HashMap<String, SiyuanNotebook>,
) -> Result<Vec<SiyuanPageRef>, String> {
    let blocks = match data {
        Value::Array(items) => items,
        Value::Object(object) => object
            .get("blocks")
            .and_then(Value::as_array)
            .ok_or("思源搜索结果格式异常：缺少 blocks")?,
        _ => return Err("思源搜索结果格式异常".into()),
    };

    Ok(blocks
        .iter()
        .filter_map(|item| build_siyuan_page_ref_from_search_block(item, notebooks))
        .collect())
}

pub(super) fn search_siyuan_pages_by_api(
    config: &SiyuanConfig,
    open_notebooks: &[SiyuanNotebook],
    notebooks: &HashMap<String, SiyuanNotebook>,
    keyword: &str,
    location: Option<&SiyuanLocation>,
) -> Result<Vec<SiyuanPageRef>, String> {
    let paths = if let Some(location) = location {
        vec![build_siyuan_search_scope_path(location)?]
    } else {
        open_notebooks
            .iter()
            .map(|notebook| notebook.id.clone())
            .collect::<Vec<_>>()
    };

    let data = post_siyuan_json(
        config,
        "/api/search/fullTextSearchBlock",
        &json!({
            "query": keyword,
            "paths": paths,
            "types": { "document": true },
            "method": 0,
            "orderBy": 7,
            "groupBy": 0,
            "page": 1,
            "pageSize": SIYUAN_SEARCH_PAGE_SIZE,
        }),
        SIYUAN_DIRECTORY_TIMEOUT_MS,
    )?;

    parse_siyuan_search_page_refs(&data, notebooks)
}

pub(super) fn score_siyuan_page_match(page: &SiyuanPageRef, keyword: &str) -> i32 {
    let keyword = keyword.to_lowercase();
    let title = page.doc_title.to_lowercase();
    let hpath = page.doc_hpath.to_lowercase();
    if title == keyword {
        0
    } else if title.contains(&keyword) {
        1
    } else if hpath.contains(&keyword) {
        2
    } else {
        3
    }
}

pub(super) fn sort_and_limit_siyuan_pages(
    pages: Vec<SiyuanPageRef>,
    keyword: &str,
) -> Vec<SiyuanPageRef> {
    let mut seen_doc_ids = HashSet::new();
    let mut pages = pages
        .into_iter()
        .filter(|page| seen_doc_ids.insert(page.doc_id.clone()))
        .collect::<Vec<_>>();

    pages.sort_by(|left, right| {
        score_siyuan_page_match(left, keyword)
            .cmp(&score_siyuan_page_match(right, keyword))
            .then(left.doc_title.len().cmp(&right.doc_title.len()))
            .then(right.doc_id.cmp(&left.doc_id))
    });
    pages.truncate(20);
    pages
}

// ── Action handlers ───────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SiyuanSearchPagesResult {
    pub(super) items: Vec<SiyuanPageRef>,
    pub(super) scope: String,
}

pub fn siyuan_test(payload: &Value) -> Result<Value, String> {
    let config = read_siyuan_config(payload)?;
    let data = post_siyuan_json(
        &config,
        "/api/system/version",
        &json!({}),
        SIYUAN_TEST_TIMEOUT_MS,
    )?;
    let version = data
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("思源返回的版本信息格式异常")?;

    Ok(json!({
        "ok": true,
        "version": version,
    }))
}

pub fn siyuan_directory(payload: &Value) -> Result<Value, String> {
    let config = read_siyuan_config(payload)?;
    let notebooks = load_siyuan_notebooks(&config, SIYUAN_DIRECTORY_TIMEOUT_MS)?;
    let doc_rows = query_siyuan_doc_rows_all(&config, SIYUAN_DOC_SQL, SIYUAN_DIRECTORY_TIMEOUT_MS)?;
    let directory = build_siyuan_directory(notebooks, doc_rows);

    serde_json::to_value(directory)
        .map_err(|err| format!("serialize siyuan directory failed: {err}"))
}

pub fn siyuan_search_pages(payload: &Value) -> Result<Value, String> {
    let keyword = parse_string(payload, "keyword").ok_or("请输入搜索关键词")?;
    let keyword_len = keyword.chars().count();
    if !(2..=50).contains(&keyword_len) {
        return Err("搜索关键词长度需在 2 到 50 个字符之间".into());
    }

    let config = read_siyuan_config(payload)?;
    let search_all = payload
        .get("searchAll")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let location = match payload.get("location") {
        Some(value) => parse_siyuan_location_value(value)?,
        None => None,
    };

    let open_notebooks = load_open_siyuan_notebooks(&config, SIYUAN_DIRECTORY_TIMEOUT_MS)?;
    if open_notebooks.is_empty() {
        return Err("当前没有可用的打开笔记本".into());
    }
    let notebook_map = notebook_map(&open_notebooks);

    let search_location = if search_all {
        None
    } else {
        let location = location.ok_or("当前没有可用的默认位置，请先配置存储位置")?;
        if !notebook_map.contains_key(&location.notebook_id) {
            return Err("当前默认位置所在笔记本已关闭或不存在，请重新选择位置".into());
        }
        Some(location)
    };
    let items = search_siyuan_pages_by_api(
        &config,
        &open_notebooks,
        &notebook_map,
        &keyword,
        search_location.as_ref(),
    )?;
    let items = sort_and_limit_siyuan_pages(items, &keyword);

    serde_json::to_value(SiyuanSearchPagesResult {
        items,
        scope: if search_all {
            "all".into()
        } else {
            "location".into()
        },
    })
    .map_err(|err| format!("serialize siyuan_search_pages failed: {err}"))
}

pub fn siyuan_create_page(payload: &Value) -> Result<Value, String> {
    let config = read_siyuan_config(payload)?;
    let location = match payload.get("location") {
        Some(value) => parse_siyuan_location_value(value)?,
        None => None,
    }
    .ok_or("请选择存储位置")?;
    let title = parse_string(payload, "title").ok_or("请输入页面标题")?;
    let target_hpath = build_siyuan_target_hpath(&location, &title)?;

    let open_notebooks = load_open_siyuan_notebooks(&config, SIYUAN_DIRECTORY_TIMEOUT_MS)?;
    let notebook_map = notebook_map(&open_notebooks);
    if !notebook_map.contains_key(&location.notebook_id) {
        return Err("当前存储位置所在笔记本已关闭或不存在，请重新选择位置".into());
    }

    let existing_ids = post_siyuan_json(
        &config,
        "/api/filetree/getIDsByHPath",
        &json!({
            "path": target_hpath.as_str(),
            "notebook": location.notebook_id.as_str(),
        }),
        SIYUAN_DIRECTORY_TIMEOUT_MS,
    )?;
    let existing_doc_id = existing_ids
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    let (created, doc_id) = if let Some(doc_id) = existing_doc_id {
        (false, doc_id)
    } else {
        let markdown = build_siyuan_markdown(payload);
        let doc_id = post_siyuan_json(
            &config,
            "/api/filetree/createDocWithMd",
            &json!({
                "notebook": location.notebook_id.as_str(),
                "path": target_hpath.as_str(),
                "markdown": markdown,
            }),
            SIYUAN_DIRECTORY_TIMEOUT_MS,
        )?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("思源返回的文档 ID 格式异常")?
        .to_string();
        (true, doc_id)
    };

    let page = fetch_siyuan_page_ref(&config, &notebook_map, &doc_id)?;
    Ok(json!({
        "created": created,
        "page": page,
    }))
}

pub fn siyuan_open_page(payload: &Value) -> Result<Value, String> {
    let doc_id = parse_string(payload, "docId").ok_or("docId 不能为空")?;
    let deep_link = build_siyuan_deep_link(&doc_id)?;
    open::that(&deep_link)
        .map_err(|_| "无法拉起思源协议，请确认桌面版已安装且对应笔记本处于打开状态".to_string())?;
    Ok(json!({ "ok": true, "url": deep_link }))
}

pub fn open_link(payload: &Value) -> Result<Value, String> {
    let raw_url = payload
        .get("url")
        .and_then(Value::as_str)
        .ok_or("url 不能为空")?;
    let url = crate::tools::pm::normalize_item_link_url(raw_url)?;
    open::that(&url).map_err(|e| format!("打开链接失败: {e}"))?;
    Ok(json!({ "ok": true, "url": url }))
}

// ── Launch helpers ─────────────────────────────────────

pub fn siyuan_check_running(payload: &Value) -> Result<Value, String> {
    let config = read_siyuan_config(payload)?;
    match post_siyuan_json(&config, "/api/system/version", &json!({}), 3_000) {
        Ok(_) => Ok(json!({ "running": true })),
        Err(_) => Ok(json!({ "running": false })),
    }
}

pub fn siyuan_launch(_payload: &Value) -> Result<Value, String> {
    let exe_path = find_siyuan_executable().ok_or(
        "未找到思源可执行文件，请确认思源已安装。\n\n常见安装位置：\n• Program Files\\SiYuan\n• 用户程序目录\\SiYuan\n• Scoop 安装目录",
    )?;
    std::process::Command::new(&exe_path)
        .spawn()
        .map_err(|e| format!("启动思源失败: {e}"))?;
    Ok(json!({ "launched": true }))
}

fn find_siyuan_executable() -> Option<std::path::PathBuf> {
    // 1. Try where command (searches PATH)
    if let Ok(output) = std::process::Command::new("where")
        .arg("SiYuan.exe")
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(first_line) = stdout.lines().next() {
                let path = std::path::PathBuf::from(first_line.trim());
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }
    // 2. Try common install paths
    siyuan_install_candidates()
        .into_iter()
        .find(|p| p.exists())
}

fn siyuan_install_candidates() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        paths.push(
            std::path::PathBuf::from(&local)
                .join("Programs")
                .join("SiYuan")
                .join("SiYuan.exe"),
        );
        paths.push(std::path::PathBuf::from(&local).join("SiYuan").join("SiYuan.exe"));
    }
    paths.push(std::path::PathBuf::from(
        "C:\\Program Files\\SiYuan\\SiYuan.exe",
    ));
    paths.push(std::path::PathBuf::from(
        "C:\\Program Files (x86)\\SiYuan\\SiYuan.exe",
    ));
    if let Ok(home) = std::env::var("USERPROFILE") {
        paths.push(
            std::path::PathBuf::from(&home)
                .join("scoop")
                .join("apps")
                .join("siyuan")
                .join("current")
                .join("SiYuan.exe"),
        );
        paths.push(
            std::path::PathBuf::from(&home)
                .join("AppData")
                .join("Local")
                .join("Programs")
                .join("siyuan")
                .join("SiYuan.exe"),
        );
    }
    paths
}
