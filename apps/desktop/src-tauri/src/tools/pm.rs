use chrono::{Local, Utc};
use rusqlite::{params, Connection, Error as RusqliteError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use url::Url;

use super::helpers::db_conn;

const STATUSES: [&str; 4] = ["todo", "in_progress", "testing", "done"];
const ITEM_TYPES: [&str; 4] = ["task", "bug", "feature", "improvement"];
const PRIORITIES: [&str; 4] = ["P0", "P1", "P2", "P3"];
const SIYUAN_BASE_URL_SETTING_KEY: &str = "pm_siyuan_base_url";
const SIYUAN_TOKEN_SETTING_KEY: &str = "pm_siyuan_token";
const SIYUAN_TEST_TIMEOUT_MS: u64 = 5_000;
const SIYUAN_DIRECTORY_TIMEOUT_MS: u64 = 10_000;
const SIYUAN_SEARCH_PAGE_SIZE: usize = 32;
const SIYUAN_DOC_SQL: &str =
    "SELECT id, box, path, hpath, content FROM blocks WHERE type = 'd' ORDER BY box ASC, hpath ASC, id ASC";
const SIYUAN_DIRECTORY_PAGE_SIZE: usize = 200;

// ── Entry point ──────────────────────────────────────────

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "project_list" => project_list(),
        "project_create" => project_create(payload),
        "project_update" => project_update(payload),
        "project_archive" => project_archive(payload),
        "project_restore" => project_restore(payload),
        "project_delete" => project_delete(payload),
        "item_list" => item_list(payload),
        "item_create" => item_create(payload),
        "item_update" => item_update(payload),
        "item_change_status" => item_change_status(payload),
        "item_reorder" => item_reorder(payload),
        "item_toggle_pin" => item_toggle_pin(payload),
        "item_delete" => item_delete(payload),
        "item_move_project" => item_move_project(payload),
        "tag_list" => tag_list(payload),
        "weekly_work" => weekly_work(payload),
        "siyuan_test" => siyuan_test(payload),
        "siyuan_directory" => siyuan_directory(payload),
        "siyuan_search_pages" => siyuan_search_pages(payload),
        "siyuan_create_page" => siyuan_create_page(payload),
        "siyuan_open_page" => siyuan_open_page(payload),
        _ => Err(format!("unsupported pm action: {action}")),
    }
}

// ── Helpers ──────────────────────────────────────────────

fn parse_i64(payload: &Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(Value::as_i64)
}

fn parse_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
}

fn parse_string_array(payload: &Value, key: &str) -> Vec<String> {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

#[derive(Debug, Clone)]
struct SiyuanConfig {
    base_url: String,
    token: String,
}

#[derive(Debug, Clone)]
struct SiyuanNotebook {
    id: String,
    name: String,
    icon: Option<String>,
    closed: bool,
}

#[derive(Debug, Clone)]
struct SiyuanDocRow {
    id: String,
    box_id: String,
    path: Option<String>,
    hpath: String,
    name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SiyuanTreeNode {
    id: String,
    name: String,
    hpath: String,
    path: Option<String>,
    leaf: bool,
    children: Vec<SiyuanTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SiyuanNotebookDirectory {
    id: String,
    name: String,
    icon: Option<String>,
    closed: bool,
    doc_count: usize,
    children: Vec<SiyuanTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SiyuanDirectoryResult {
    notebooks: Vec<SiyuanNotebookDirectory>,
    fetched_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SiyuanLocation {
    notebook_id: String,
    notebook_name: String,
    parent_doc_id: Option<String>,
    parent_doc_title: Option<String>,
    parent_hpath: Option<String>,
    parent_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SiyuanPageRef {
    doc_id: String,
    doc_title: String,
    doc_hpath: String,
    doc_path: Option<String>,
    notebook_id: String,
    notebook_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SiyuanSearchPagesResult {
    items: Vec<SiyuanPageRef>,
    scope: String,
}

fn normalize_siyuan_base_url(input: &str) -> Result<String, String> {
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

fn read_siyuan_config(payload: &Value) -> Result<SiyuanConfig, String> {
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

fn parse_siyuan_envelope(text: &str) -> Result<Value, String> {
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

fn post_siyuan_json(
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

fn parse_siyuan_notebooks(data: &Value) -> Result<Vec<SiyuanNotebook>, String> {
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

fn parse_siyuan_doc_rows(data: &Value) -> Result<Vec<SiyuanDocRow>, String> {
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

    // 路径异常或父节点缺失时，兜底保留剩余节点，避免整条分支丢失。
    for row in rows {
        if let Some(node) =
            materialize_siyuan_tree_node(&row.id, &mut nodes_by_id, &mut child_ids_by_parent)
        {
            tree.push(node);
        }
    }

    tree
}

fn build_siyuan_directory(
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

fn parse_siyuan_location_value(value: &Value) -> Result<Option<SiyuanLocation>, String> {
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

fn parse_siyuan_page_ref_value(value: &Value) -> Result<Option<SiyuanPageRef>, String> {
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

fn parse_siyuan_page_ref_array(value: Option<&Value>) -> Result<Option<Vec<SiyuanPageRef>>, String> {
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

fn build_siyuan_location_from_parts(
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

fn build_siyuan_page_ref_from_parts(
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

fn escape_sql_string(value: &str) -> String {
    value.replace('\'', "''")
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

fn build_siyuan_target_hpath(location: &SiyuanLocation, title: &str) -> Result<String, String> {
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

fn build_siyuan_deep_link(doc_id: &str) -> Result<String, String> {
    let doc_id = doc_id.trim();
    if doc_id.is_empty() {
        return Err("页面 ID 不能为空".into());
    }
    Ok(format!("siyuan://blocks/{doc_id}"))
}

fn load_item_siyuan_links(conn: &Connection, item_id: i64) -> Vec<SiyuanPageRef> {
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

fn save_item_siyuan_links(
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

fn load_siyuan_notebooks(config: &SiyuanConfig, timeout_ms: u64) -> Result<Vec<SiyuanNotebook>, String> {
    let notebooks_data = post_siyuan_json(
        config,
        "/api/notebook/lsNotebooks",
        &json!({}),
        timeout_ms,
    )?;
    parse_siyuan_notebooks(&notebooks_data)
}

fn load_open_siyuan_notebooks(
    config: &SiyuanConfig,
    timeout_ms: u64,
) -> Result<Vec<SiyuanNotebook>, String> {
    Ok(load_siyuan_notebooks(config, timeout_ms)?
        .into_iter()
        .filter(|notebook| !notebook.closed)
        .collect())
}

fn query_siyuan_doc_rows(
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

fn query_siyuan_doc_rows_all(
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

fn notebook_map(notebooks: &[SiyuanNotebook]) -> HashMap<String, SiyuanNotebook> {
    notebooks
        .iter()
        .map(|notebook| (notebook.id.clone(), notebook.clone()))
        .collect()
}

fn build_siyuan_page_ref_from_row(
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

fn fetch_siyuan_page_ref(
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

fn normalize_siyuan_search_scope_prefix(path: &str) -> Option<String> {
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

fn build_siyuan_search_scope_path(location: &SiyuanLocation) -> Result<String, String> {
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

fn normalize_siyuan_search_doc_hpath(hpath: Option<String>, title: &str) -> String {
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

fn build_siyuan_page_ref_from_search_block(
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

fn parse_siyuan_search_page_refs(
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

fn search_siyuan_pages_by_api(
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

fn score_siyuan_page_match(page: &SiyuanPageRef, keyword: &str) -> i32 {
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

fn sort_and_limit_siyuan_pages(
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

// ── Project CRUD ─────────────────────────────────────────

fn project_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, description, color, status,
                    siyuan_notebook_id, siyuan_notebook_name,
                    siyuan_parent_doc_id, siyuan_parent_doc_title,
                    siyuan_parent_hpath, siyuan_parent_path,
                    sort_order, created_at, updated_at
             FROM pm_projects ORDER BY status ASC, sort_order ASC, id DESC",
        )
        .map_err(|e| format!("prepare project_list: {e}"))?;

    let rows: Vec<Value> = stmt
        .query_map([], |r| {
            let location = build_siyuan_location_from_parts(
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<String>>(10)?,
            );
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "name": r.get::<_, String>(1)?,
                "description": r.get::<_, String>(2)?,
                "color": r.get::<_, String>(3)?,
                "status": r.get::<_, String>(4)?,
                "siyuanLocationOverride": location,
                "sortOrder": r.get::<_, i64>(11)?,
                "createdAt": r.get::<_, String>(12)?,
                "updatedAt": r.get::<_, String>(13)?,
            }))
        })
        .map_err(|e| format!("query project_list: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(json!(rows))
}

fn project_create(payload: &Value) -> Result<Value, String> {
    let name = parse_string(payload, "name").ok_or("name is required")?;
    let desc = parse_string(payload, "description").unwrap_or_default();
    let color = parse_string(payload, "color").unwrap_or_else(|| "#409eff".to_string());
    let location = match payload.get("siyuanLocationOverride") {
        Some(value) => parse_siyuan_location_value(value)?,
        None => None,
    };
    let now = now_rfc3339();

    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO pm_projects (
            name, description, color,
            siyuan_notebook_id, siyuan_notebook_name,
            siyuan_parent_doc_id, siyuan_parent_doc_title, siyuan_parent_hpath, siyuan_parent_path,
            created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            name,
            desc,
            color,
            location.as_ref().map(|item| item.notebook_id.as_str()),
            location.as_ref().map(|item| item.notebook_name.as_str()),
            location.as_ref().and_then(|item| item.parent_doc_id.as_deref()),
            location.as_ref().and_then(|item| item.parent_doc_title.as_deref()),
            location.as_ref().and_then(|item| item.parent_hpath.as_deref()),
            location.as_ref().and_then(|item| item.parent_path.as_deref()),
            now,
            now
        ],
    )
    .map_err(|e| format!("project_create: {e}"))?;

    let id = conn.last_insert_rowid();
    Ok(json!({ "id": id }))
}

fn project_update(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;
    let current_location = conn
        .query_row(
            "SELECT siyuan_notebook_id, siyuan_notebook_name,
                    siyuan_parent_doc_id, siyuan_parent_doc_title,
                    siyuan_parent_hpath, siyuan_parent_path
             FROM pm_projects WHERE id = ?1",
            params![id],
            |row| {
                Ok(build_siyuan_location_from_parts(
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .map_err(|e| format!("project not found: {e}"))?;
    let name = parse_string(payload, "name").ok_or("name is required")?;
    let desc = parse_string(payload, "description").unwrap_or_default();
    let color = parse_string(payload, "color").unwrap_or_else(|| "#409eff".to_string());
    let sort_order = parse_i64(payload, "sortOrder").unwrap_or(0);
    let location = match payload.get("siyuanLocationOverride") {
        Some(value) => parse_siyuan_location_value(value)?,
        None => current_location,
    };
    let now = now_rfc3339();
    conn.execute(
        "UPDATE pm_projects
         SET name = ?1, description = ?2, color = ?3,
             siyuan_notebook_id = ?4, siyuan_notebook_name = ?5,
             siyuan_parent_doc_id = ?6, siyuan_parent_doc_title = ?7,
             siyuan_parent_hpath = ?8, siyuan_parent_path = ?9,
             sort_order = ?10, updated_at = ?11
         WHERE id = ?12",
        params![
            name,
            desc,
            color,
            location.as_ref().map(|item| item.notebook_id.as_str()),
            location.as_ref().map(|item| item.notebook_name.as_str()),
            location.as_ref().and_then(|item| item.parent_doc_id.as_deref()),
            location.as_ref().and_then(|item| item.parent_doc_title.as_deref()),
            location.as_ref().and_then(|item| item.parent_hpath.as_deref()),
            location.as_ref().and_then(|item| item.parent_path.as_deref()),
            sort_order,
            now,
            id
        ],
    )
    .map_err(|e| format!("project_update: {e}"))?;

    Ok(json!({ "updated": true }))
}

fn project_archive(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute(
        "UPDATE pm_projects SET status = 'archived', updated_at = ?1 WHERE id = ?2",
        params![now_rfc3339(), id],
    )
    .map_err(|e| format!("project_archive: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn project_restore(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute(
        "UPDATE pm_projects SET status = 'active', updated_at = ?1 WHERE id = ?2",
        params![now_rfc3339(), id],
    )
    .map_err(|e| format!("project_restore: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn project_delete(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;

    // Check if any todo_items reference this project
    let todo_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM todo_items WHERE project_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if todo_count > 0 {
        return Err(format!(
            "该项目下有 {todo_count} 条待办事项，请先移除待办的项目归属后再删除"
        ));
    }

    // Cascade deletes pm_items + pm_item_tags via FK
    conn.execute("DELETE FROM pm_projects WHERE id = ?1", params![id])
        .map_err(|e| format!("project_delete: {e}"))?;
    Ok(json!({ "ok": true }))
}

// ── Item CRUD ────────────────────────────────────────────

fn item_list(payload: &Value) -> Result<Value, String> {
    let project_id = parse_i64(payload, "projectId");
    let conn = db_conn()?;

    let items: Vec<Value> = if let Some(pid) = project_id {
        let mut stmt = conn
            .prepare(
                "SELECT i.id, i.project_id, i.title, i.description, i.item_type, i.priority,
                        i.status, i.start_at, i.end_at, i.pinned, i.sort_order,
                        i.siyuan_doc_id, i.siyuan_doc_title, i.siyuan_doc_hpath,
                        i.siyuan_doc_path, i.siyuan_notebook_id, i.siyuan_notebook_name,
                        i.completed_at, i.created_at, i.updated_at,
                        p.name, p.color
                 FROM pm_items i
                 LEFT JOIN pm_projects p ON i.project_id = p.id
                 WHERE i.project_id = ?1
                 ORDER BY i.pinned DESC, i.sort_order ASC,
                          CASE i.priority WHEN 'P0' THEN 0 WHEN 'P1' THEN 1 WHEN 'P2' THEN 2 ELSE 3 END ASC,
                          i.id DESC",
            )
            .map_err(|e| format!("prepare item_list: {e}"))?;
        let result: Vec<Value> = stmt.query_map(params![pid], |r| {
            let primary_page = build_siyuan_page_ref_from_parts(
                r.get::<_, Option<String>>(11)?,
                r.get::<_, Option<String>>(12)?,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, Option<String>>(14)?,
                r.get::<_, Option<String>>(15)?,
                r.get::<_, Option<String>>(16)?,
            );
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "projectId": r.get::<_, i64>(1)?,
                "title": r.get::<_, String>(2)?,
                "description": r.get::<_, String>(3)?,
                "itemType": r.get::<_, String>(4)?,
                "priority": r.get::<_, String>(5)?,
                "status": r.get::<_, String>(6)?,
                "startAt": r.get::<_, Option<String>>(7)?,
                "endAt": r.get::<_, Option<String>>(8)?,
                "pinned": r.get::<_, bool>(9)?,
                "sortOrder": r.get::<_, i64>(10)?,
                "siyuanPrimaryPage": primary_page,
                "completedAt": r.get::<_, Option<String>>(17)?,
                "createdAt": r.get::<_, String>(18)?,
                "updatedAt": r.get::<_, String>(19)?,
                "projectName": r.get::<_, Option<String>>(20)?,
                "projectColor": r.get::<_, Option<String>>(21)?,
            }))
        })
        .map_err(|e| format!("query item_list: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
        result
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT i.id, i.project_id, i.title, i.description, i.item_type, i.priority,
                        i.status, i.start_at, i.end_at, i.pinned, i.sort_order,
                        i.siyuan_doc_id, i.siyuan_doc_title, i.siyuan_doc_hpath,
                        i.siyuan_doc_path, i.siyuan_notebook_id, i.siyuan_notebook_name,
                        i.completed_at, i.created_at, i.updated_at,
                        p.name, p.color
                 FROM pm_items i
                 LEFT JOIN pm_projects p ON i.project_id = p.id
                 WHERE p.status = 'active'
                 ORDER BY i.pinned DESC, i.sort_order ASC,
                          CASE i.priority WHEN 'P0' THEN 0 WHEN 'P1' THEN 1 WHEN 'P2' THEN 2 ELSE 3 END ASC,
                          i.id DESC",
            )
            .map_err(|e| format!("prepare item_list: {e}"))?;
        let result: Vec<Value> = stmt.query_map([], |r| {
            let primary_page = build_siyuan_page_ref_from_parts(
                r.get::<_, Option<String>>(11)?,
                r.get::<_, Option<String>>(12)?,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, Option<String>>(14)?,
                r.get::<_, Option<String>>(15)?,
                r.get::<_, Option<String>>(16)?,
            );
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "projectId": r.get::<_, i64>(1)?,
                "title": r.get::<_, String>(2)?,
                "description": r.get::<_, String>(3)?,
                "itemType": r.get::<_, String>(4)?,
                "priority": r.get::<_, String>(5)?,
                "status": r.get::<_, String>(6)?,
                "startAt": r.get::<_, Option<String>>(7)?,
                "endAt": r.get::<_, Option<String>>(8)?,
                "pinned": r.get::<_, bool>(9)?,
                "sortOrder": r.get::<_, i64>(10)?,
                "siyuanPrimaryPage": primary_page,
                "completedAt": r.get::<_, Option<String>>(17)?,
                "createdAt": r.get::<_, String>(18)?,
                "updatedAt": r.get::<_, String>(19)?,
                "projectName": r.get::<_, Option<String>>(20)?,
                "projectColor": r.get::<_, Option<String>>(21)?,
            }))
        })
        .map_err(|e| format!("query item_list: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
        result
    };

    // Attach tags
    let result: Vec<Value> = items
        .into_iter()
        .map(|mut item| {
            let item_id = item["id"].as_i64().unwrap_or(0);
            let tags = load_tags(&conn, item_id);
            let extra_pages = load_item_siyuan_links(&conn, item_id);
            item.as_object_mut().unwrap().insert("tags".to_string(), json!(tags));
            item.as_object_mut()
                .unwrap()
                .insert("siyuanExtraPages".to_string(), json!(extra_pages));
            item
        })
        .collect();

    Ok(json!(result))
}

fn load_tags(conn: &Connection, item_id: i64) -> Vec<String> {
    conn.prepare("SELECT tag FROM pm_item_tags WHERE item_id = ?1 ORDER BY tag")
        .and_then(|mut stmt| {
            stmt.query_map(params![item_id], |r| r.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

fn save_tags(conn: &Connection, item_id: i64, tags: &[String]) -> Result<(), String> {
    conn.execute("DELETE FROM pm_item_tags WHERE item_id = ?1", params![item_id])
        .map_err(|e| format!("delete tags: {e}"))?;
    for tag in tags {
        let t = tag.trim();
        if t.is_empty() {
            continue;
        }
        conn.execute(
            "INSERT OR IGNORE INTO pm_item_tags (item_id, tag) VALUES (?1, ?2)",
            params![item_id, t],
        )
        .map_err(|e| format!("insert tag: {e}"))?;
    }
    Ok(())
}

fn item_create(payload: &Value) -> Result<Value, String> {
    let project_id = parse_i64(payload, "projectId").ok_or("projectId is required")?;
    let title = parse_string(payload, "title").ok_or("title is required")?;
    let desc = parse_string(payload, "description").unwrap_or_default();
    let item_type = parse_string(payload, "itemType")
        .filter(|v| ITEM_TYPES.contains(&v.as_str()))
        .unwrap_or_else(|| "task".to_string());
    let priority = parse_string(payload, "priority")
        .filter(|v| PRIORITIES.contains(&v.as_str()))
        .unwrap_or_else(|| "P2".to_string());
    let status = parse_string(payload, "status")
        .filter(|v| STATUSES.contains(&v.as_str()))
        .unwrap_or_else(|| "todo".to_string());
    let start_at = parse_string(payload, "startAt");
    let end_at = parse_string(payload, "endAt");
    let tags = parse_string_array(payload, "tags");
    let primary_page = match payload.get("siyuanPrimaryPage") {
        Some(value) => parse_siyuan_page_ref_value(value)?,
        None => None,
    };
    let extra_pages = parse_siyuan_page_ref_array(payload.get("siyuanExtraPages"))?
        .unwrap_or_default();
    let now = now_rfc3339();

    let completed_at = if status == "done" { Some(now.clone()) } else { None };

    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO pm_items (
            project_id, title, description, item_type, priority, status,
            start_at, end_at,
            siyuan_doc_id, siyuan_doc_title, siyuan_doc_hpath, siyuan_doc_path,
            siyuan_notebook_id, siyuan_notebook_name,
            completed_at, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            project_id,
            title,
            desc,
            item_type,
            priority,
            status,
            start_at,
            end_at,
            primary_page.as_ref().map(|item| item.doc_id.as_str()),
            primary_page.as_ref().map(|item| item.doc_title.as_str()),
            primary_page.as_ref().map(|item| item.doc_hpath.as_str()),
            primary_page.as_ref().and_then(|item| item.doc_path.as_deref()),
            primary_page.as_ref().map(|item| item.notebook_id.as_str()),
            primary_page.as_ref().map(|item| item.notebook_name.as_str()),
            completed_at,
            now,
            now
        ],
    )
    .map_err(|e| format!("item_create: {e}"))?;

    let id = conn.last_insert_rowid();
    if !tags.is_empty() {
        save_tags(&conn, id, &tags)?;
    }
    save_item_siyuan_links(&conn, id, primary_page.as_ref(), &extra_pages, &now)?;

    Ok(json!({ "id": id }))
}

fn item_update(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;
    let now = now_rfc3339();

    // Read current row
    let (
        cur_title,
        cur_desc,
        cur_type,
        cur_prio,
        cur_status,
        cur_start,
        cur_end,
        cur_primary_page,
    ): (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<SiyuanPageRef>,
    ) = conn
        .query_row(
            "SELECT title, description, item_type, priority, status, start_at, end_at,
                    siyuan_doc_id, siyuan_doc_title, siyuan_doc_hpath,
                    siyuan_doc_path, siyuan_notebook_id, siyuan_notebook_name
             FROM pm_items WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    build_siyuan_page_ref_from_parts(
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, Option<String>>(9)?,
                        r.get::<_, Option<String>>(10)?,
                        r.get::<_, Option<String>>(11)?,
                        r.get::<_, Option<String>>(12)?,
                    ),
                ))
            },
        )
        .map_err(|e| format!("item not found: {e}"))?;
    let cur_extra_pages = load_item_siyuan_links(&conn, id);

    let title = parse_string(payload, "title").unwrap_or(cur_title);
    let desc = if payload.get("description").is_some() {
        parse_string(payload, "description").unwrap_or_default()
    } else {
        cur_desc
    };
    let item_type = parse_string(payload, "itemType")
        .filter(|v| ITEM_TYPES.contains(&v.as_str()))
        .unwrap_or(cur_type);
    let priority = parse_string(payload, "priority")
        .filter(|v| PRIORITIES.contains(&v.as_str()))
        .unwrap_or(cur_prio);
    let new_status = parse_string(payload, "status")
        .filter(|v| STATUSES.contains(&v.as_str()))
        .unwrap_or(cur_status.clone());
    let start_at = if payload.get("startAt").is_some() {
        parse_string(payload, "startAt")
    } else {
        cur_start
    };
    let end_at = if payload.get("endAt").is_some() {
        parse_string(payload, "endAt")
    } else {
        cur_end
    };
    let primary_page = match payload.get("siyuanPrimaryPage") {
        Some(value) => parse_siyuan_page_ref_value(value)?,
        None => cur_primary_page,
    };
    let extra_pages = match parse_siyuan_page_ref_array(payload.get("siyuanExtraPages"))? {
        Some(pages) => pages,
        None => cur_extra_pages,
    };

    let completed_at: Option<String> = if new_status == "done" && cur_status != "done" {
        Some(now.clone())
    } else if new_status == "done" {
        // Keep existing completed_at
        conn.query_row("SELECT completed_at FROM pm_items WHERE id = ?1", params![id], |r| r.get(0))
            .unwrap_or(Some(now.clone()))
    } else {
        None
    };

    conn.execute(
        "UPDATE pm_items
         SET title=?1, description=?2, item_type=?3, priority=?4, status=?5,
             start_at=?6, end_at=?7,
             siyuan_doc_id=?8, siyuan_doc_title=?9, siyuan_doc_hpath=?10,
             siyuan_doc_path=?11, siyuan_notebook_id=?12, siyuan_notebook_name=?13,
             completed_at=?14, updated_at=?15
         WHERE id=?16",
        params![
            title,
            desc,
            item_type,
            priority,
            new_status,
            start_at,
            end_at,
            primary_page.as_ref().map(|item| item.doc_id.as_str()),
            primary_page.as_ref().map(|item| item.doc_title.as_str()),
            primary_page.as_ref().map(|item| item.doc_hpath.as_str()),
            primary_page.as_ref().and_then(|item| item.doc_path.as_deref()),
            primary_page.as_ref().map(|item| item.notebook_id.as_str()),
            primary_page.as_ref().map(|item| item.notebook_name.as_str()),
            completed_at,
            now,
            id
        ],
    )
    .map_err(|e| format!("item_update: {e}"))?;

    if payload.get("tags").is_some() {
        let tags = parse_string_array(payload, "tags");
        save_tags(&conn, id, &tags)?;
    }
    save_item_siyuan_links(&conn, id, primary_page.as_ref(), &extra_pages, &now)?;

    Ok(json!({ "updated": true }))
}

fn item_change_status(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let new_status = parse_string(payload, "status").ok_or("status is required")?;
    if !STATUSES.contains(&new_status.as_str()) {
        return Err(format!("invalid status: {new_status}"));
    }
    let now = now_rfc3339();
    let conn = db_conn()?;

    if new_status == "done" {
        conn.execute(
            "UPDATE pm_items SET status = ?1, completed_at = ?2, updated_at = ?2 WHERE id = ?3",
            params![new_status, now, id],
        )
        .map_err(|e| format!("item_change_status: {e}"))?;
    } else {
        conn.execute(
            "UPDATE pm_items SET status = ?1, completed_at = NULL, updated_at = ?2 WHERE id = ?3",
            params![new_status, now, id],
        )
        .map_err(|e| format!("item_change_status: {e}"))?;
    }

    Ok(json!({ "ok": true }))
}

fn item_reorder(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let now = now_rfc3339();

    // Supports two modes:
    // 1) Array of { id, sortOrder } for within-column reorder
    // 2) { id, status, sortOrder } for cross-column move
    if let Some(items) = payload.get("items").and_then(Value::as_array) {
        for item in items {
            let id = item.get("id").and_then(Value::as_i64).unwrap_or(0);
            let sort = item.get("sortOrder").and_then(Value::as_i64).unwrap_or(0);
            let status = item.get("status").and_then(Value::as_str);

            if let Some(st) = status {
                if st == "done" {
                    conn.execute(
                        "UPDATE pm_items SET sort_order = ?1, status = ?2, completed_at = COALESCE(completed_at, ?4), updated_at = ?4 WHERE id = ?3",
                        params![sort, st, id, now],
                    )
                    .map_err(|e| format!("item_reorder: {e}"))?;
                } else {
                    conn.execute(
                        "UPDATE pm_items SET sort_order = ?1, status = ?2, completed_at = NULL, updated_at = ?4 WHERE id = ?3",
                        params![sort, st, id, now],
                    )
                    .map_err(|e| format!("item_reorder: {e}"))?;
                }
            } else {
                conn.execute(
                    "UPDATE pm_items SET sort_order = ?1, updated_at = ?3 WHERE id = ?2",
                    params![sort, id, now],
                )
                .map_err(|e| format!("item_reorder: {e}"))?;
            }
        }
        return Ok(json!({ "ok": true }));
    }

    // Single item cross-column move
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let new_status = parse_string(payload, "status");
    let sort_order = parse_i64(payload, "sortOrder").unwrap_or(0);

    if let Some(st) = new_status {
        if st == "done" {
            conn.execute(
                "UPDATE pm_items SET sort_order = ?1, status = ?2, completed_at = COALESCE(completed_at, ?4), updated_at = ?4 WHERE id = ?3",
                params![sort_order, st, id, now],
            )
            .map_err(|e| format!("item_reorder: {e}"))?;
        } else {
            conn.execute(
                "UPDATE pm_items SET sort_order = ?1, status = ?2, completed_at = NULL, updated_at = ?4 WHERE id = ?3",
                params![sort_order, st, id, now],
            )
            .map_err(|e| format!("item_reorder: {e}"))?;
        }
    } else {
        conn.execute(
            "UPDATE pm_items SET sort_order = ?1, updated_at = ?3 WHERE id = ?2",
            params![sort_order, id, now],
        )
        .map_err(|e| format!("item_reorder: {e}"))?;
    }

    Ok(json!({ "ok": true }))
}

fn item_toggle_pin(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute(
        "UPDATE pm_items SET pinned = 1 - pinned, updated_at = ?1 WHERE id = ?2",
        params![now_rfc3339(), id],
    )
    .map_err(|e| format!("item_toggle_pin: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn item_delete(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute("DELETE FROM pm_items WHERE id = ?1", params![id])
        .map_err(|e| format!("item_delete: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn item_move_project(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let project_id = parse_i64(payload, "projectId").ok_or("projectId is required")?;
    let conn = db_conn()?;
    conn.execute(
        "UPDATE pm_items SET project_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![project_id, now_rfc3339(), id],
    )
    .map_err(|e| format!("item_move_project: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn tag_list(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let project_id = parse_i64(payload, "projectId");

    let tags: Vec<Value> = if let Some(pid) = project_id {
        let mut stmt = conn
            .prepare(
                "SELECT t.tag, COUNT(*) as cnt
                 FROM pm_item_tags t JOIN pm_items i ON t.item_id = i.id
                 WHERE i.project_id = ?1
                 GROUP BY t.tag ORDER BY cnt DESC, t.tag",
            )
            .map_err(|e| format!("tag_list: {e}"))?;
        let result: Vec<Value> = stmt
            .query_map(params![pid], |r| {
                Ok(json!({ "tag": r.get::<_, String>(0)?, "count": r.get::<_, i64>(1)? }))
            })
            .map_err(|e| format!("tag_list query: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        result
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT tag, COUNT(*) as cnt FROM pm_item_tags GROUP BY tag ORDER BY cnt DESC, tag",
            )
            .map_err(|e| format!("tag_list: {e}"))?;
        let result: Vec<Value> = stmt
            .query_map([], |r| {
                Ok(json!({ "tag": r.get::<_, String>(0)?, "count": r.get::<_, i64>(1)? }))
            })
            .map_err(|e| format!("tag_list query: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    Ok(json!(tags))
}

fn siyuan_test(payload: &Value) -> Result<Value, String> {
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

fn siyuan_directory(payload: &Value) -> Result<Value, String> {
    let config = read_siyuan_config(payload)?;
    let notebooks = load_siyuan_notebooks(&config, SIYUAN_DIRECTORY_TIMEOUT_MS)?;
    let doc_rows = query_siyuan_doc_rows_all(&config, SIYUAN_DOC_SQL, SIYUAN_DIRECTORY_TIMEOUT_MS)?;
    let directory = build_siyuan_directory(notebooks, doc_rows);

    serde_json::to_value(directory)
        .map_err(|err| format!("serialize siyuan directory failed: {err}"))
}

fn siyuan_search_pages(payload: &Value) -> Result<Value, String> {
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

fn siyuan_create_page(payload: &Value) -> Result<Value, String> {
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

fn siyuan_open_page(payload: &Value) -> Result<Value, String> {
    let doc_id = parse_string(payload, "docId").ok_or("docId 不能为空")?;
    let deep_link = build_siyuan_deep_link(&doc_id)?;
    open::that(&deep_link)
        .map_err(|_| "无法拉起思源协议，请确认桌面版已安装且对应笔记本处于打开状态".to_string())?;
    Ok(json!({ "ok": true, "url": deep_link }))
}

// ── Weekly work ──────────────────────────────────────────

fn weekly_work(_payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;

    // Calculate 7-day window in local timezone, convert to UTC for comparison
    let now_local = Local::now();
    let start_local = (now_local - chrono::Duration::days(6))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let start_utc = start_local
        .and_local_timezone(Local)
        .single()
        .map(|dt| dt.with_timezone(&Utc))
        .ok_or("timezone conversion failed")?;
    let start_str = start_utc.to_rfc3339();

    // PM items completed in window
    let mut pm_stmt = conn
        .prepare(
            "SELECT i.id, i.project_id, i.title, i.item_type, i.priority, i.status,
                    i.completed_at, i.created_at,
                    p.name as project_name, p.color as project_color, p.status as project_status
             FROM pm_items i
             JOIN pm_projects p ON i.project_id = p.id
             WHERE i.status = 'done' AND i.completed_at >= ?1
             ORDER BY i.completed_at DESC",
        )
        .map_err(|e| format!("weekly_work pm: {e}"))?;

    let pm_items: Vec<Value> = pm_stmt
        .query_map(params![start_str], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "projectId": r.get::<_, i64>(1)?,
                "title": r.get::<_, String>(2)?,
                "itemType": r.get::<_, String>(3)?,
                "priority": r.get::<_, String>(4)?,
                "status": r.get::<_, String>(5)?,
                "completedAt": r.get::<_, Option<String>>(6)?,
                "createdAt": r.get::<_, String>(7)?,
                "projectName": r.get::<_, String>(8)?,
                "projectColor": r.get::<_, String>(9)?,
                "projectStatus": r.get::<_, String>(10)?,
                "source": "pm",
            }))
        })
        .map_err(|e| format!("weekly_work pm query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    // Todo items completed in window (project_id column may not exist yet)
    let has_project_col = conn
        .prepare("SELECT project_id FROM todo_items LIMIT 0")
        .is_ok();

    let todo_items: Vec<Value> = if has_project_col {
        let mut todo_stmt = conn
            .prepare(
                "SELECT t.id, t.title, t.priority, t.status, t.completed_at, t.created_at,
                        t.project_id,
                        p.name as project_name, p.color as project_color, p.status as project_status
                 FROM todo_items t
                 LEFT JOIN pm_projects p ON t.project_id = p.id
                 WHERE t.status = 'completed' AND t.completed_at >= ?1
                 ORDER BY t.completed_at DESC",
            )
            .map_err(|e| format!("weekly_work todo: {e}"))?;

        let result: Vec<Value> = todo_stmt
            .query_map(params![start_str], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "priority": r.get::<_, String>(2)?,
                    "status": r.get::<_, String>(3)?,
                    "completedAt": r.get::<_, Option<String>>(4)?,
                    "createdAt": r.get::<_, String>(5)?,
                    "projectId": r.get::<_, Option<i64>>(6)?,
                    "projectName": r.get::<_, Option<String>>(7)?,
                    "projectColor": r.get::<_, Option<String>>(8)?,
                    "projectStatus": r.get::<_, Option<String>>(9)?,
                    "source": "todo",
                }))
            })
            .map_err(|e| format!("weekly_work todo query: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        result
    } else {
        let mut todo_stmt = conn
            .prepare(
                "SELECT t.id, t.title, t.priority, t.status, t.completed_at, t.created_at
                 FROM todo_items t
                 WHERE t.status = 'completed' AND t.completed_at >= ?1
                 ORDER BY t.completed_at DESC",
            )
            .map_err(|e| format!("weekly_work todo: {e}"))?;

        let result: Vec<Value> = todo_stmt
            .query_map(params![start_str], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "priority": r.get::<_, String>(2)?,
                    "status": r.get::<_, String>(3)?,
                    "completedAt": r.get::<_, Option<String>>(4)?,
                    "createdAt": r.get::<_, String>(5)?,
                    "projectId": null,
                    "projectName": null,
                    "projectColor": null,
                    "projectStatus": null,
                    "source": "todo",
                }))
            })
            .map_err(|e| format!("weekly_work todo query: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    Ok(json!({
        "pmItems": pm_items,
        "todoItems": todo_items,
        "windowStart": start_str,
        "windowEnd": now_local.to_rfc3339(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_siyuan_base_url_should_trim_and_strip_trailing_slash() {
        let normalized =
            normalize_siyuan_base_url("  http://127.0.0.1:6806/  ").expect("normalize");
        assert_eq!(normalized, "http://127.0.0.1:6806");
    }

    #[test]
    fn normalize_siyuan_base_url_should_reject_invalid_scheme() {
        let err = normalize_siyuan_base_url("ftp://127.0.0.1:6806").expect_err("invalid scheme");
        assert!(err.contains("http://") || err.contains("https://"));
    }

    #[test]
    fn build_siyuan_directory_should_nest_docs_by_hpath() {
        let notebooks = vec![SiyuanNotebook {
            id: "nb1".into(),
            name: "工作台".into(),
            icon: None,
            closed: false,
        }];
        let rows = vec![
            SiyuanDocRow {
                id: "doc-root".into(),
                box_id: "nb1".into(),
                path: Some("/doc-root.sy".into()),
                hpath: "/根文档".into(),
                name: "根文档".into(),
            },
            SiyuanDocRow {
                id: "doc-child".into(),
                box_id: "nb1".into(),
                path: Some("/doc-root/doc-child.sy".into()),
                hpath: "/根文档/子文档".into(),
                name: "子文档".into(),
            },
        ];

        let directory = build_siyuan_directory(notebooks, rows);
        assert_eq!(directory.notebooks.len(), 1);
        assert_eq!(directory.notebooks[0].doc_count, 2);
        assert_eq!(directory.notebooks[0].children.len(), 1);
        assert_eq!(directory.notebooks[0].children[0].id, "doc-root");
        assert!(!directory.notebooks[0].children[0].leaf);
        assert_eq!(directory.notebooks[0].children[0].children.len(), 1);
        assert_eq!(
            directory.notebooks[0].children[0].children[0].id,
            "doc-child"
        );
        assert!(directory.notebooks[0].children[0].children[0].leaf);
    }

    #[test]
    fn build_siyuan_directory_should_keep_duplicate_title_siblings_separate() {
        let notebooks = vec![SiyuanNotebook {
            id: "nb1".into(),
            name: "测试".into(),
            icon: None,
            closed: false,
        }];
        let rows = vec![
            SiyuanDocRow {
                id: "doc-a".into(),
                box_id: "nb1".into(),
                path: Some("/doc-a.sy".into()),
                hpath: "/测试".into(),
                name: "测试".into(),
            },
            SiyuanDocRow {
                id: "doc-a-child".into(),
                box_id: "nb1".into(),
                path: Some("/doc-a/doc-a-child.sy".into()),
                hpath: "/测试/子文档A".into(),
                name: "子文档A".into(),
            },
            SiyuanDocRow {
                id: "doc-b".into(),
                box_id: "nb1".into(),
                path: Some("/doc-b.sy".into()),
                hpath: "/测试".into(),
                name: "测试".into(),
            },
            SiyuanDocRow {
                id: "doc-b-child".into(),
                box_id: "nb1".into(),
                path: Some("/doc-b/doc-b-child.sy".into()),
                hpath: "/测试/子文档B".into(),
                name: "子文档B".into(),
            },
        ];

        let directory = build_siyuan_directory(notebooks, rows);
        assert_eq!(directory.notebooks.len(), 1);
        assert_eq!(directory.notebooks[0].doc_count, 4);
        assert_eq!(directory.notebooks[0].children.len(), 2);
        assert_eq!(directory.notebooks[0].children[0].id, "doc-a");
        assert_eq!(directory.notebooks[0].children[1].id, "doc-b");
        assert_eq!(
            directory.notebooks[0].children[0].children[0].id,
            "doc-a-child"
        );
        assert_eq!(
            directory.notebooks[0].children[1].children[0].id,
            "doc-b-child"
        );
    }

    #[test]
    fn normalize_siyuan_error_message_should_map_sql_disabled() {
        let message = normalize_siyuan_error_message("SQL is not available in publish mode");
        assert_eq!(message, "当前思源实例未开放 SQL 查询能力");
    }

    #[test]
    fn build_siyuan_target_hpath_should_escape_path_separator() {
        let location = SiyuanLocation {
            notebook_id: "nb1".into(),
            notebook_name: "工作台".into(),
            parent_doc_id: Some("doc-root".into()),
            parent_doc_title: Some("根文档".into()),
            parent_hpath: Some("/根文档".into()),
            parent_path: Some("/root.sy".into()),
        };
        let hpath = build_siyuan_target_hpath(&location, "迭代/计划").expect("build hpath");
        assert_eq!(hpath, "/根文档/迭代／计划");
    }

    #[test]
    fn build_siyuan_search_scope_path_should_use_notebook_root() {
        let location = SiyuanLocation {
            notebook_id: "nb1".into(),
            notebook_name: "工作台".into(),
            parent_doc_id: None,
            parent_doc_title: None,
            parent_hpath: None,
            parent_path: None,
        };
        let path = build_siyuan_search_scope_path(&location).expect("build search scope");
        assert_eq!(path, "nb1");
    }

    #[test]
    fn build_siyuan_search_scope_path_should_scope_to_parent_subtree() {
        let location = SiyuanLocation {
            notebook_id: "nb1".into(),
            notebook_name: "工作台".into(),
            parent_doc_id: Some("doc-root".into()),
            parent_doc_title: Some("根文档".into()),
            parent_hpath: Some("/根文档".into()),
            parent_path: Some("/root.sy".into()),
        };
        let path = build_siyuan_search_scope_path(&location).expect("build search scope");
        assert_eq!(path, "nb1/root");
    }

    #[test]
    fn build_siyuan_search_scope_path_should_fallback_to_parent_doc_id() {
        let location = SiyuanLocation {
            notebook_id: "nb1".into(),
            notebook_name: "工作台".into(),
            parent_doc_id: Some("20260329232612-w2j085v".into()),
            parent_doc_title: Some("b".into()),
            parent_hpath: Some("/b".into()),
            parent_path: None,
        };
        let path = build_siyuan_search_scope_path(&location).expect("build search scope");
        assert_eq!(path, "nb1/20260329232612-w2j085v");
    }

    #[test]
    fn parse_siyuan_search_page_refs_should_map_document_blocks() {
        let notebooks = notebook_map(&[SiyuanNotebook {
            id: "nb1".into(),
            name: "测试".into(),
            icon: None,
            closed: false,
        }]);
        let data = json!({
            "blocks": [
                {
                    "id": "doc-bb",
                    "rootID": "doc-bb",
                    "box": "nb1",
                    "path": "/20260329232612-w2j085v/202604010001-bb.sy",
                    "hPath": "/b/",
                    "name": "",
                    "content": "<mark>bb</mark>a",
                    "type": "NodeDocument",
                    "ial": {
                        "title": "bba"
                    }
                }
            ]
        });

        let pages = parse_siyuan_search_page_refs(&data, &notebooks).expect("parse search pages");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].doc_id, "doc-bb");
        assert_eq!(pages[0].doc_title, "bba");
        assert_eq!(pages[0].doc_hpath, "/b/bba");
        assert_eq!(pages[0].doc_path.as_deref(), Some("/20260329232612-w2j085v/202604010001-bb.sy"));
        assert_eq!(pages[0].notebook_name, "测试");
    }

    #[test]
    fn normalize_siyuan_search_doc_hpath_should_append_title_for_parent_path() {
        assert_eq!(
            normalize_siyuan_search_doc_hpath(Some("/b/".into()), "bba"),
            "/b/bba"
        );
        assert_eq!(
            normalize_siyuan_search_doc_hpath(Some("/".into()), "首页"),
            "/首页"
        );
        assert_eq!(
            normalize_siyuan_search_doc_hpath(Some("/b/bba".into()), "bba"),
            "/b/bba"
        );
    }

    #[test]
    fn sort_and_limit_siyuan_pages_should_dedupe_and_prefer_title_match() {
        let pages = vec![
            SiyuanPageRef {
                doc_id: "doc-1".into(),
                doc_title: "bb".into(),
                doc_hpath: "/b/bb".into(),
                doc_path: Some("/doc-1.sy".into()),
                notebook_id: "nb1".into(),
                notebook_name: "测试".into(),
            },
            SiyuanPageRef {
                doc_id: "doc-1".into(),
                doc_title: "bb".into(),
                doc_hpath: "/b/bb".into(),
                doc_path: Some("/doc-1.sy".into()),
                notebook_id: "nb1".into(),
                notebook_name: "测试".into(),
            },
            SiyuanPageRef {
                doc_id: "doc-2".into(),
                doc_title: "需求记录".into(),
                doc_hpath: "/记录/bb-关联".into(),
                doc_path: Some("/doc-2.sy".into()),
                notebook_id: "nb1".into(),
                notebook_name: "测试".into(),
            },
        ];

        let sorted = sort_and_limit_siyuan_pages(pages, "bb");
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].doc_id, "doc-1");
    }

    #[test]
    fn build_siyuan_deep_link_should_follow_blocks_protocol() {
        let link = build_siyuan_deep_link("20260329120000-abc123").expect("deep link");
        assert_eq!(link, "siyuan://blocks/20260329120000-abc123");
    }
}
