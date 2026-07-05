use rusqlite::Connection;
use serde_json::{json, Value};

use super::helpers::{build_final_url, parse_i64, prepare_request_body, resolve_template};
use super::executor::{load_variables, resolve_rows};
use super::request_get_with_conn;
use super::types::{KeyValueRow, RequestDraft};

pub(crate) fn parse_export_shell(payload: &Value) -> Result<&'static str, String> {
    match payload["targetShell"].as_str().unwrap_or("powershell") {
        "powershell" => Ok("powershell"),
        "bash" => Ok("bash"),
        other => Err(format!("unsupported shell: {other}")),
    }
}

pub(crate) fn quote_curl_arg(shell: &str, value: &str) -> Result<String, String> {
    if value.contains('\n') || value.contains('\r') {
        return Err("cURL 导出暂不支持包含换行的 Header 或 Body".to_string());
    }
    match shell {
        "powershell" => Ok(format!("'{}'", value.replace('\'', "''"))),
        "bash" => Ok(format!("'{}'", value.replace('\'', "'\\''"))),
        _ => Err(format!("unsupported shell: {shell}")),
    }
}

pub(crate) fn export_curl_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let environment_id = parse_i64(payload, "environmentId")?;
    let shell = parse_export_shell(payload)?;
    let draft: RequestDraft = serde_json::from_value(payload["draft"].clone())
        .map_err(|e| format!("请求草稿格式错误: {e}"))?;

    let env_owner: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_environments WHERE id=?1",
            [environment_id],
            |row| row.get(0),
        )
        .map_err(|_| "环境不存在".to_string())?;
    if env_owner != collection_id {
        return Err("环境不属于当前集合".to_string());
    }

    let (vars, base_url) = load_variables(conn, environment_id)?;
    let resolved_url = resolve_template(&draft.url, &vars)?;
    let resolved_query = resolve_rows(&draft.query, &vars)?;
    let resolved_headers = resolve_rows(&draft.headers, &vars)?;
    let resolved_body = if matches!(draft.body_type.as_str(), "json" | "text") {
        resolve_template(&draft.body, &vars)?
    } else {
        String::new()
    };
    let resolved_form = if draft.body_type == "form-urlencoded" {
        resolve_rows(&draft.form, &vars)?
    } else {
        Vec::new()
    };
    let final_url = build_final_url(&base_url, &resolved_url, &resolved_query)?;
    let prepared = prepare_request_body(
        &draft.body_type,
        &resolved_body,
        &resolved_form,
        &resolved_headers,
    )?;

    let method = draft.method.to_ascii_uppercase();
    let mut parts = vec![
        "curl".to_string(),
        "-X".to_string(),
        method,
        quote_curl_arg(shell, &final_url)?,
    ];
    for header in resolved_headers
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
    {
        parts.push("-H".to_string());
        parts.push(quote_curl_arg(
            shell,
            &format!("{}: {}", header.key.trim(), header.value),
        )?);
    }
    if let Some(content_type) = prepared.content_type.as_deref() {
        parts.push("-H".to_string());
        parts.push(quote_curl_arg(
            shell,
            &format!("Content-Type: {content_type}"),
        )?);
    }
    if let Some(body) = prepared.body {
        if !body.is_empty() {
            let body_text = String::from_utf8_lossy(&body);
            parts.push("--data-raw".to_string());
            parts.push(quote_curl_arg(shell, &body_text)?);
        }
    }

    Ok(json!({ "shell": shell, "command": parts.join(" ") }))
}


pub(crate) fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "cookie" | "x-api-key" | "x-auth-token"
    )
}

pub(crate) fn markdown_escape(text: &str) -> String {
    text.replace('|', "\\|")
}

pub(crate) fn render_header_lines(headers: &[KeyValueRow]) -> String {
    let mut lines = Vec::new();
    for header in headers
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
    {
        let value = if is_sensitive_header(&header.key) {
            "******".to_string()
        } else {
            header.value.clone()
        };
        lines.push(format!(
            "- {}: {}",
            markdown_escape(&header.key),
            markdown_escape(&value)
        ));
    }
    if lines.is_empty() {
        "- 无".to_string()
    } else {
        lines.join("\n")
    }
}

pub(crate) fn render_request_markdown(item: &Value) -> String {
    let name = item["name"].as_str().unwrap_or("未命名接口");
    let description = item["description"].as_str().unwrap_or("");
    let draft = &item["draft"];
    let method = draft["method"].as_str().unwrap_or("GET");
    let url = draft["url"].as_str().unwrap_or("");
    let headers: Vec<KeyValueRow> =
        serde_json::from_value(draft["headers"].clone()).unwrap_or_default();
    let body_type = draft["bodyType"].as_str().unwrap_or("none");
    let body = draft["body"].as_str().unwrap_or("");
    let mut out = String::new();
    out.push_str(&format!("### {name}\n\n"));
    if !description.is_empty() {
        out.push_str(description);
        out.push_str("\n\n");
    }
    out.push_str(&format!("`{method} {url}`\n\n"));
    out.push_str("#### Headers\n\n");
    out.push_str(&render_header_lines(&headers));
    out.push_str("\n\n");
    out.push_str("#### Body\n\n");
    if body_type == "none" || body.trim().is_empty() {
        out.push_str("无\n\n");
    } else {
        out.push_str(&format!("```{body_type}\n{body}\n```\n\n"));
    }
    if let Some(example) = item["exampleResponse"].as_str() {
        if let Ok(example) = serde_json::from_str::<Value>(example) {
            out.push_str("#### 示例响应\n\n");
            let status = example["status"]
                .as_i64()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "ERR".to_string());
            let status_text = example["statusText"].as_str().unwrap_or_default();
            let content_type = example["contentType"].as_str().unwrap_or_default();
            let body_text = example["bodyText"].as_str().unwrap_or_default();
            let truncated = example["bodyTruncated"].as_bool().unwrap_or(false);
            out.push_str(&format!("`{status} {status_text}`\n\n"));
            if !content_type.is_empty() {
                out.push_str(&format!(
                    "- Content-Type: `{}`\n\n",
                    markdown_escape(content_type)
                ));
            }
            if body_text.trim().is_empty() {
                out.push_str("无响应体\n\n");
            } else {
                out.push_str(&format!("```text\n{body_text}\n```\n\n"));
            }
            if truncated {
                out.push_str("> 响应体已截断。\n\n");
            }
        }
    }
    out
}

pub(crate) fn export_markdown_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let collection = conn
        .query_row(
            "SELECT name, description FROM api_workbench_collections WHERE id=?1",
            [collection_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|_| "集合不存在".to_string())?;
    let mut markdown = String::new();
    markdown.push_str(&format!("# {}\n\n", collection.0));
    if !collection.1.is_empty() {
        markdown.push_str(&collection.1);
        markdown.push_str("\n\n");
    }

    markdown.push_str("## 环境变量\n\n");
    let mut var_stmt = conn
        .prepare(
            "SELECT e.name, v.name
             FROM api_workbench_environments e
             LEFT JOIN api_workbench_environment_variables v ON v.environment_id=e.id
             WHERE e.collection_id=?1
             ORDER BY e.sort_order ASC, e.id ASC, v.sort_order ASC",
        )
        .map_err(|e| format!("prepare vars failed: {e}"))?;
    let var_rows = var_stmt
        .query_map([collection_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|e| format!("query vars failed: {e}"))?;
    for row in var_rows {
        let (env_name, var_name) = row.map_err(|e| e.to_string())?;
        if let Some(var_name) = var_name {
            markdown.push_str(&format!("- {}: `{}`\n", env_name, var_name));
        }
    }
    markdown.push_str("\n## 接口\n\n");

    let mut stmt = conn
        .prepare(
            "SELECT id FROM api_workbench_requests
             WHERE collection_id=?1 ORDER BY folder_id IS NOT NULL, folder_id, sort_order, id",
        )
        .map_err(|e| format!("prepare requests failed: {e}"))?;
    let ids = stmt
        .query_map([collection_id], |row| row.get::<_, i64>(0))
        .map_err(|e| format!("query requests failed: {e}"))?;
    for id in ids {
        let request =
            request_get_with_conn(conn, &json!({ "id": id.map_err(|e| e.to_string())? }))?;
        markdown.push_str(&render_request_markdown(&request));
    }

    let file_name = format!("{}-api.md", collection.0.trim().replace(' ', "-"));
    Ok(json!({ "fileName": file_name, "markdown": markdown }))
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::api_workbench::test_conn;
    use crate::tools::api_workbench::*;

    #[test]
    fn export_curl_resolves_variables_and_quotes_for_powershell() {
        let conn = test_conn();
        let collection =
            collection_create_with_conn(&conn, &json!({ "name": "Demo", "description": "" }))
                .expect("collection");
        let collection_id = collection["id"].as_i64().unwrap();
        let environment_id = collection["activeEnvironmentId"].as_i64().unwrap();
        environment_save_with_conn(
            &conn,
            &json!({
                "id": environment_id,
                "collectionId": collection_id,
                "name": "开发",
                "variables": [
                    { "name": "BASE_URL", "value": "http://127.0.0.1:8080", "isSecret": false },
                    { "name": "TOKEN", "value": "abc'123", "isSecret": false }
                ]
            }),
        )
        .expect("environment");

        let exported = export_curl_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "environmentId": environment_id,
                "targetShell": "powershell",
                "draft": {
                    "method": "POST",
                    "url": "/api/users",
                    "query": [{ "enabled": true, "key": "page", "value": "1" }],
                    "headers": [{ "enabled": true, "key": "Authorization", "value": "Bearer {{ TOKEN }}" }],
                    "bodyType": "json",
                    "body": "{\"name\":\"Tom\"}",
                    "form": [],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("export");

        assert_eq!(exported["shell"], "powershell");
        let command = exported["command"].as_str().unwrap();
        assert!(command.contains("curl -X POST 'http://127.0.0.1:8080/api/users?page=1'"));
        assert!(command.contains("-H 'Authorization: Bearer abc''123'"));
        assert!(command.contains("--data-raw '{\"name\":\"Tom\"}'"));
    }

    #[test]
    fn export_curl_rejects_multiline_values() {
        let conn = test_conn();
        let collection =
            collection_create_with_conn(&conn, &json!({ "name": "Demo", "description": "" }))
                .expect("collection");
        let collection_id = collection["id"].as_i64().unwrap();
        let environment_id = collection["activeEnvironmentId"].as_i64().unwrap();

        let err = export_curl_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "environmentId": environment_id,
                "targetShell": "powershell",
                "draft": {
                    "method": "POST",
                    "url": "http://127.0.0.1:8080/api/users",
                    "query": [],
                    "headers": [],
                    "bodyType": "text",
                    "body": "line1\nline2",
                    "form": [],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect_err("newline");

        assert!(err.contains("换行"));
    }

    #[test]
    fn export_markdown_redacts_sensitive_headers_and_hides_variable_values() {
        let conn = test_conn();
        let c = collection_create_with_conn(
            &conn,
            &json!({ "name": "Demo", "description": "API docs" }),
        )
        .expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "Auth",
                "description": "Login",
                "draft": {
                    "method": "POST",
                    "url": "/api/login",
                    "query": [],
                    "headers": [{ "enabled": true, "key": "Authorization", "value": "Bearer secret" }],
                    "bodyType": "json",
                    "body": "{\"name\":\"demo\"}",
                    "form": [],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("request");
        let result = export_markdown_with_conn(&conn, &json!({ "collectionId": collection_id }))
            .expect("export");
        let markdown = result["markdown"].as_str().unwrap();
        assert!(markdown.contains("# Demo"));
        assert!(markdown.contains("POST /api/login"));
        assert!(markdown.contains("Authorization: ******"));
        assert!(!markdown.contains("Bearer secret"));
        assert!(markdown.contains("BASE_URL"));
    }
}
