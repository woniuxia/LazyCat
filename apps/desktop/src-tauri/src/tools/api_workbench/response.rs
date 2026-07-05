use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[cfg(not(test))]
use crate::tools::helpers::get_data_dir;

use super::types::ResponseBodyPayload;

#[cfg(test)]
pub(crate) fn get_api_workbench_response_cache_dir() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir()
        .join("lazycat-api-workbench-tests")
        .join("api-workbench")
        .join("response-cache");
    fs::create_dir_all(&dir).map_err(|e| format!("create response cache dir failed: {e}"))?;
    Ok(dir)
}

#[cfg(not(test))]
pub(crate) fn get_api_workbench_response_cache_dir() -> Result<PathBuf, String> {
    let dir = get_data_dir()?.join("api-workbench").join("response-cache");
    fs::create_dir_all(&dir).map_err(|e| format!("create response cache dir failed: {e}"))?;
    Ok(dir)
}

pub(crate) fn normalized_mime(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

pub(crate) fn extension_from_mime(mime: &str) -> Option<&'static str> {
    match mime {
        "application/json" => Some("json"),
        "text/html" | "application/xhtml+xml" => Some("html"),
        "text/plain" => Some("txt"),
        "text/css" => Some("css"),
        "text/csv" => Some("csv"),
        "application/xml" | "text/xml" => Some("xml"),
        "application/javascript" | "text/javascript" => Some("js"),
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        "application/pdf" => Some("pdf"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Some("docx"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => Some("xlsx"),
        "application/vnd.ms-excel" => Some("xls"),
        "application/vnd.oasis.opendocument.spreadsheet" => Some("ods"),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => Some("pptx"),
        "application/vnd.ms-powerpoint" => Some("ppt"),
        _ => None,
    }
}

pub(crate) fn extension_from_url(final_url: &str) -> Option<String> {
    let parsed = url::Url::parse(final_url).ok()?;
    let segment = parsed.path_segments()?.next_back()?;
    let (_, ext) = segment.rsplit_once('.')?;
    let ext = ext
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    if ext.is_empty() || ext.len() > 12 {
        None
    } else {
        Some(ext)
    }
}

pub(crate) fn filename_from_content_disposition(content_disposition: &str) -> Option<String> {
    for part in content_disposition.split(';') {
        let trimmed = part.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("filename*=") {
            let value = trimmed.split_once('=')?.1.trim().trim_matches('"');
            if let Some((_, encoded)) = value.rsplit_once("''") {
                return Some(sanitize_file_name(&urlencoding::decode(encoded).ok()?));
            }
        }
        if lower.starts_with("filename=") {
            let value = trimmed.split_once('=')?.1.trim().trim_matches('"');
            return Some(sanitize_file_name(value));
        }
    }
    None
}

pub(crate) fn sanitize_file_name(input: &str) -> String {
    let name = input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let name = name.trim_matches('.').trim_matches('_').to_string();
    if name.is_empty() {
        "response".to_string()
    } else {
        name.chars().take(96).collect()
    }
}

pub(crate) fn extension_from_file_name(file_name: &str) -> Option<String> {
    let (_, ext) = file_name.rsplit_once('.')?;
    let ext = ext
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    if ext.is_empty() || ext.len() > 12 {
        None
    } else {
        Some(ext)
    }
}

pub(crate) fn extension_from_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("jpg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("gif");
    }
    if bytes.starts_with(b"%PDF-") {
        return Some("pdf");
    }
    if bytes.starts_with(b"PK\x03\x04") {
        return Some("zip");
    }
    None
}

pub(crate) fn looks_textual_mime(mime: &str) -> bool {
    mime.starts_with("text/")
        || matches!(
            mime,
            "application/json"
                | "application/xml"
                | "application/xhtml+xml"
                | "application/javascript"
                | "application/x-www-form-urlencoded"
                | "image/svg+xml"
        )
        || mime.ends_with("+json")
        || mime.ends_with("+xml")
}

pub(crate) fn looks_binary_mime(mime: &str) -> bool {
    mime.starts_with("image/")
        || matches!(
            mime,
            "application/pdf"
                | "application/octet-stream"
                | "application/zip"
                | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
                | "application/vnd.ms-excel"
                | "application/vnd.ms-powerpoint"
                | "application/vnd.oasis.opendocument.spreadsheet"
        )
}

pub(crate) fn classify_response_storage(mime: &str, bytes: &[u8]) -> &'static str {
    if bytes.is_empty() {
        return "empty";
    }
    if looks_textual_mime(mime) {
        return "text";
    }
    if looks_binary_mime(mime) || extension_from_bytes(bytes).is_some() {
        return "file";
    }
    if std::str::from_utf8(bytes).is_ok() {
        "text"
    } else {
        "file"
    }
}

pub(crate) fn persist_response_cache_file(
    cache_dir: &Path,
    bytes: &[u8],
    file_name_hint: Option<String>,
    extension_hint: Option<String>,
) -> Result<(String, String, String, String), String> {
    fs::create_dir_all(cache_dir).map_err(|e| format!("create response cache dir failed: {e}"))?;
    let cache_dir = cache_dir
        .canonicalize()
        .map_err(|e| format!("resolve response cache dir failed: {e}"))?;
    let hash = blake3::hash(bytes).to_hex().to_string();
    let hash_prefix = &hash[..16];
    let extension = extension_hint.unwrap_or_else(|| "bin".to_string());
    let display_name = file_name_hint.unwrap_or_else(|| format!("response.{extension}"));
    let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S");
    let file_name = format!("{timestamp}-{hash_prefix}.{extension}");
    let target = cache_dir.join(&file_name);
    fs::write(&target, bytes).map_err(|e| format!("write response cache failed: {e}"))?;
    let canonical_target = target
        .canonicalize()
        .map_err(|e| format!("resolve response cache file failed: {e}"))?;
    if !canonical_target.starts_with(&cache_dir) {
        let _ = fs::remove_file(&canonical_target);
        return Err("response cache path escaped cache dir".to_string());
    }
    Ok((
        canonical_target.to_string_lossy().to_string(),
        display_name,
        extension,
        hash,
    ))
}

pub(crate) fn build_response_body_payload(
    final_url: &str,
    content_type: &str,
    content_disposition: &str,
    bytes: Vec<u8>,
    body_truncated: bool,
) -> ResponseBodyPayload {
    let body_size = bytes.len();
    let mime = normalized_mime(content_type);
    let storage = classify_response_storage(&mime, &bytes);
    if storage == "empty" {
        return ResponseBodyPayload {
            body_text: String::new(),
            body_size,
            body_truncated,
            body_storage: "empty".to_string(),
            body_file_path: String::new(),
            body_file_name: String::new(),
            body_extension: String::new(),
            body_hash: String::new(),
            body_preview_error: None,
        };
    }
    if storage == "text" {
        return ResponseBodyPayload {
            body_text: String::from_utf8_lossy(&bytes).to_string(),
            body_size,
            body_truncated,
            body_storage: "text".to_string(),
            body_file_path: String::new(),
            body_file_name: String::new(),
            body_extension: String::new(),
            body_hash: String::new(),
            body_preview_error: None,
        };
    }
    if body_truncated {
        return ResponseBodyPayload {
            body_text: String::new(),
            body_size,
            body_truncated,
            body_storage: "truncated-binary".to_string(),
            body_file_path: String::new(),
            body_file_name: String::new(),
            body_extension: String::new(),
            body_hash: String::new(),
            body_preview_error: Some("二进制响应已截断，未生成预览缓存".to_string()),
        };
    }

    let file_name_hint = filename_from_content_disposition(content_disposition);
    let extension_hint = file_name_hint
        .as_deref()
        .and_then(extension_from_file_name)
        .or_else(|| extension_from_url(final_url))
        .or_else(|| extension_from_mime(&mime).map(str::to_string))
        .or_else(|| extension_from_bytes(&bytes).map(str::to_string))
        .unwrap_or_else(|| "bin".to_string());
    match get_api_workbench_response_cache_dir().and_then(|dir| {
        persist_response_cache_file(&dir, &bytes, file_name_hint, Some(extension_hint))
    }) {
        Ok((path, display_name, extension, hash)) => ResponseBodyPayload {
            body_text: String::new(),
            body_size,
            body_truncated,
            body_storage: "file".to_string(),
            body_file_path: path,
            body_file_name: display_name,
            body_extension: extension,
            body_hash: hash,
            body_preview_error: None,
        },
        Err(error) => ResponseBodyPayload {
            body_text: String::new(),
            body_size,
            body_truncated,
            body_storage: "file".to_string(),
            body_file_path: String::new(),
            body_file_name: String::new(),
            body_extension: String::new(),
            body_hash: String::new(),
            body_preview_error: Some(error),
        },
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HistoryCacheRef {
    pub(crate) file_path: String,
}

pub(crate) fn validate_response_cache_file_path(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("cache path is empty".to_string());
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err("cache path must be absolute".to_string());
    }
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err("cache path contains `..`".to_string());
        }
    }
    #[cfg(windows)]
    {
        if trimmed.starts_with(r"\\.\") {
            return Err("device namespace path not allowed".to_string());
        }
    }
    let cache_dir = get_api_workbench_response_cache_dir()?
        .canonicalize()
        .map_err(|e| format!("resolve response cache dir failed: {e}"))?;
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("resolve response cache file failed: {e}"))?;
    if !canonical.starts_with(cache_dir) {
        return Err("cache path is outside response cache dir".to_string());
    }
    Ok(canonical)
}

pub(crate) fn remove_response_cache_file_if_safe(file_path: &str) -> Result<(), String> {
    if file_path.trim().is_empty() {
        return Ok(());
    }
    if !PathBuf::from(file_path).exists() {
        return Ok(());
    }
    let canonical = validate_response_cache_file_path(file_path)?;
    fs::remove_file(&canonical).map_err(|e| format!("remove response cache failed: {e}"))
}

pub(crate) fn collect_history_cache_refs(
    conn: &Connection,
    where_sql: &str,
) -> Result<Vec<HistoryCacheRef>, String> {
    let sql = format!("SELECT response_body_file_path FROM api_workbench_history {where_sql}");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("prepare history cache refs failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(HistoryCacheRef {
                file_path: row.get::<_, String>(0)?,
            })
        })
        .map_err(|e| format!("query history cache refs failed: {e}"))?;
    let mut refs = Vec::new();
    for row in rows {
        let item = row.map_err(|e| e.to_string())?;
        if !item.file_path.trim().is_empty() {
            refs.push(item);
        }
    }
    Ok(refs)
}

pub(crate) fn cleanup_unreferenced_history_cache_files(conn: &Connection, refs: &[HistoryCacheRef]) {
    let mut seen = HashSet::new();
    for item in refs {
        if item.file_path.trim().is_empty() || !seen.insert(item.file_path.clone()) {
            continue;
        }
        let remaining: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM api_workbench_history WHERE response_body_file_path=?1",
                params![item.file_path],
                |row| row.get(0),
            )
            .unwrap_or(1);
        if remaining == 0 {
            let _ = remove_response_cache_file_if_safe(&item.file_path);
        }
    }
}

pub(crate) fn response_preview_office_with_conn(_conn: &Connection, payload: &Value) -> Result<Value, String> {
    let file_path = payload["filePath"]
        .as_str()
        .ok_or_else(|| "filePath is required".to_string())?;
    let kind = payload["kind"]
        .as_str()
        .ok_or_else(|| "kind is required".to_string())?;
    let path = validate_response_cache_file_path(file_path)?;
    match kind {
        "word" => preview_word_file(&path),
        "sheet" => preview_sheet_file(&path, payload),
        "slides" => preview_slides_file(&path),
        other => Err(format!("unsupported office preview kind: {other}")),
    }
}

pub(crate) fn response_cache_open_with_conn(_conn: &Connection, payload: &Value) -> Result<Value, String> {
    let file_path = payload["filePath"]
        .as_str()
        .ok_or_else(|| "filePath is required".to_string())?;
    let path = validate_response_cache_file_path(file_path)?;
    open::that(path).map_err(|e| format!("open response cache failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

pub(crate) fn response_cache_reveal_with_conn(_conn: &Connection, payload: &Value) -> Result<Value, String> {
    let file_path = payload["filePath"]
        .as_str()
        .ok_or_else(|| "filePath is required".to_string())?;
    let path = validate_response_cache_file_path(file_path)?;
    reveal_response_cache_file(&path)?;
    Ok(json!({ "ok": true }))
}

#[cfg(windows)]
pub(crate) fn reveal_response_cache_file(path: &Path) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map_err(|e| format!("explorer launch failed: {e}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn reveal_response_cache_file(path: &Path) -> Result<(), String> {
    std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn()
        .map_err(|e| format!("open -R failed: {e}"))?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn reveal_response_cache_file(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "cache file has no parent".to_string())?;
    std::process::Command::new("xdg-open")
        .arg(parent)
        .spawn()
        .map_err(|e| format!("xdg-open failed: {e}"))?;
    Ok(())
}

pub(crate) fn path_extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

pub(crate) fn preview_sheet_file(path: &Path, payload: &Value) -> Result<Value, String> {
    let ext = path_extension(path);
    if ext == "csv" {
        return preview_csv_file(path, payload);
    }
    preview_workbook_file(path, payload)
}

pub(crate) fn preview_csv_file(path: &Path, payload: &Value) -> Result<Value, String> {
    let offset = payload["offset"].as_u64().unwrap_or(0) as usize;
    let limit = payload["limit"].as_u64().unwrap_or(200).clamp(1, 200) as usize;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(path)
        .map_err(|e| format!("无法打开 CSV 文件: {e}"))?;
    let mut rows = Vec::new();
    let mut total_rows = 0usize;
    let mut column_count = 0usize;
    for record in reader.records() {
        let record = record.map_err(|e| format!("无法读取 CSV 文件: {e}"))?;
        if total_rows >= offset && rows.len() < limit {
            let row: Vec<String> = record.iter().map(|cell| cell.to_string()).collect();
            column_count = column_count.max(row.len());
            rows.push(row);
        }
        total_rows += 1;
    }
    Ok(json!({
        "kind": "sheet",
        "sheetNames": ["CSV"],
        "activeSheet": "CSV",
        "offset": offset,
        "limit": limit,
        "totalRows": total_rows,
        "totalColumns": column_count,
        "truncated": total_rows > offset + rows.len(),
        "rows": rows
    }))
}

pub(crate) fn api_workbench_cell_to_string(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::Empty => String::new(),
        calamine::Data::String(value) => value.trim().to_string(),
        calamine::Data::Float(value) => format!("{value}"),
        calamine::Data::Int(value) => format!("{value}"),
        calamine::Data::Bool(value) => value.to_string(),
        calamine::Data::DateTime(value) => value.to_string(),
        _ => String::new(),
    }
}

pub(crate) fn preview_workbook_file(path: &Path, payload: &Value) -> Result<Value, String> {
    use calamine::Reader;

    let offset = payload["offset"].as_u64().unwrap_or(0) as usize;
    let limit = payload["limit"].as_u64().unwrap_or(200).clamp(1, 200) as usize;
    let mut workbook =
        calamine::open_workbook_auto(path).map_err(|e| format!("无法打开表格文件: {e}"))?;
    let sheet_names = workbook.sheet_names().to_vec();
    let active_sheet = payload["sheetName"]
        .as_str()
        .filter(|name| sheet_names.iter().any(|item| item == *name))
        .map(str::to_string)
        .or_else(|| sheet_names.first().cloned())
        .ok_or_else(|| "表格文件没有工作表".to_string())?;
    let range = workbook
        .worksheet_range(&active_sheet)
        .map_err(|e| format!("无法读取工作表: {e}"))?;
    let total_rows = range.height();
    let total_columns = range.width();
    let rows: Vec<Vec<String>> = range
        .rows()
        .skip(offset)
        .take(limit)
        .map(|row| row.iter().map(api_workbench_cell_to_string).collect())
        .collect();
    Ok(json!({
        "kind": "sheet",
        "sheetNames": sheet_names,
        "activeSheet": active_sheet,
        "offset": offset,
        "limit": limit,
        "totalRows": total_rows,
        "totalColumns": total_columns,
        "truncated": total_rows > offset + rows.len(),
        "rows": rows
    }))
}

pub(crate) fn extract_xml_texts(xml: &str, max_chars: usize) -> Vec<String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut texts = Vec::new();
    let mut used_chars = 0usize;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Text(event)) => {
                let text = String::from_utf8_lossy(event.as_ref()).trim().to_string();
                if text.is_empty() {
                    continue;
                }
                used_chars += text.chars().count();
                if used_chars > max_chars {
                    break;
                }
                texts.push(text);
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    texts
}

pub(crate) fn read_zip_text_entry(
    archive: &mut zip::ZipArchive<fs::File>,
    name: &str,
) -> Result<String, String> {
    let mut entry = archive
        .by_name(name)
        .map_err(|e| format!("无法读取 OpenXML 内容: {e}"))?;
    let mut xml = String::new();
    entry
        .read_to_string(&mut xml)
        .map_err(|e| format!("无法读取 OpenXML 文本: {e}"))?;
    Ok(xml)
}

pub(crate) fn preview_word_file(path: &Path) -> Result<Value, String> {
    let ext = path_extension(path);
    if ext != "docx" {
        return Ok(json!({
            "kind": "word",
            "paragraphs": [],
            "tables": [],
            "imageCount": 0,
            "truncated": false,
            "unsupported": true,
            "message": "该格式暂不支持基础预览"
        }));
    }
    let file = fs::File::open(path).map_err(|e| format!("无法打开 Word 文件: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("无法读取 docx 文件: {e}"))?;
    let xml = read_zip_text_entry(&mut archive, "word/document.xml")?;
    let paragraphs = extract_xml_texts(&xml, 200_000);
    let image_count = (0..archive.len())
        .filter_map(|idx| {
            archive
                .by_index(idx)
                .ok()
                .map(|entry| entry.name().to_string())
        })
        .filter(|name| name.starts_with("word/media/"))
        .count();
    Ok(json!({
        "kind": "word",
        "paragraphs": paragraphs,
        "tables": [],
        "imageCount": image_count,
        "truncated": false,
        "unsupported": false
    }))
}

pub(crate) fn slide_sort_key(name: &str) -> i64 {
    name.rsplit_once("slide")
        .and_then(|(_, tail)| tail.split('.').next())
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(i64::MAX)
}

pub(crate) fn preview_slides_file(path: &Path) -> Result<Value, String> {
    let ext = path_extension(path);
    if ext != "pptx" {
        return Ok(json!({
            "kind": "slides",
            "slides": [],
            "truncated": false,
            "unsupported": true,
            "message": "该格式暂不支持基础预览"
        }));
    }
    let file = fs::File::open(path).map_err(|e| format!("无法打开 PowerPoint 文件: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("无法读取 pptx 文件: {e}"))?;
    let mut slide_names = Vec::new();
    let mut image_count = 0usize;
    for idx in 0..archive.len() {
        let entry = archive
            .by_index(idx)
            .map_err(|e| format!("无法读取 pptx 条目: {e}"))?;
        let name = entry.name().to_string();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slide_names.push(name);
        } else if name.starts_with("ppt/media/") {
            image_count += 1;
        }
    }
    slide_names.sort_by_key(|name| slide_sort_key(name));
    let mut slides = Vec::new();
    let mut truncated = false;
    for (index, name) in slide_names.iter().enumerate() {
        if index >= 100 {
            truncated = true;
            break;
        }
        let xml = read_zip_text_entry(&mut archive, name)?;
        let texts = extract_xml_texts(&xml, 100_000);
        let title = texts
            .first()
            .cloned()
            .unwrap_or_else(|| format!("幻灯片 {}", index + 1));
        slides.push(json!({
            "index": index + 1,
            "title": title,
            "texts": texts,
            "notes": [],
            "imageCount": image_count
        }));
    }
    Ok(json!({
        "kind": "slides",
        "slides": slides,
        "truncated": truncated,
        "unsupported": false
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::api_workbench::test_conn;

    #[test]
    fn response_cache_path_validation_rejects_outside_file() {
        let outside = std::env::temp_dir().join("lazycat-api-workbench-outside.bin");
        fs::write(&outside, b"outside").expect("outside file");

        let err = validate_response_cache_file_path(&outside.to_string_lossy())
            .expect_err("outside path");
        assert!(err.contains("outside"));
    }

    #[test]
    fn response_preview_office_rejects_outside_cache_path() {
        let conn = test_conn();
        let outside = std::env::temp_dir().join("lazycat-api-workbench-outside.docx");
        fs::write(&outside, b"outside").expect("outside file");

        let err = response_preview_office_with_conn(
            &conn,
            &json!({ "filePath": outside.to_string_lossy(), "kind": "word" }),
        )
        .expect_err("outside path");
        assert!(err.contains("outside"));
    }

    #[test]
    fn response_preview_office_reads_csv_sheet() {
        let conn = test_conn();
        let cache_dir = get_api_workbench_response_cache_dir().expect("cache dir");
        let (file_path, _, _, _) = persist_response_cache_file(
            &cache_dir,
            b"name,age\nAlice,30\nBob,31\n",
            Some("users.csv".into()),
            Some("csv".into()),
        )
        .expect("cache file");

        let preview = response_preview_office_with_conn(
            &conn,
            &json!({ "filePath": file_path, "kind": "sheet" }),
        )
        .expect("preview");

        assert_eq!(preview["kind"], "sheet");
        assert_eq!(preview["sheetNames"], json!(["CSV"]));
        assert_eq!(preview["rows"][0], json!(["name", "age"]));
        assert_eq!(preview["rows"][1], json!(["Alice", "30"]));
    }

    fn write_openxml_zip(path: &Path, entries: &[(&str, &str)]) {
        let file = fs::File::create(path).expect("zip file");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default();
        for (name, content) in entries {
            zip.start_file(*name, options).expect("zip entry");
            std::io::Write::write_all(&mut zip, content.as_bytes()).expect("zip content");
        }
        zip.finish().expect("finish zip");
    }

    #[test]
    fn response_preview_office_extracts_docx_text() {
        let conn = test_conn();
        let cache_dir = get_api_workbench_response_cache_dir().expect("cache dir");
        let source = cache_dir.join("sample.docx");
        write_openxml_zip(
            &source,
            &[(
                "word/document.xml",
                r#"<w:document xmlns:w="w"><w:body><w:p><w:r><w:t>API 文档</w:t></w:r></w:p><w:p><w:r><w:t>响应预览</w:t></w:r></w:p></w:body></w:document>"#,
            )],
        );
        let source_bytes = fs::read(&source).expect("docx bytes");
        let (file_path, _, _, _) = persist_response_cache_file(
            &cache_dir,
            &source_bytes,
            Some("sample.docx".into()),
            Some("docx".into()),
        )
        .expect("cache file");

        let preview = response_preview_office_with_conn(
            &conn,
            &json!({ "filePath": file_path, "kind": "word" }),
        )
        .expect("preview");

        assert_eq!(preview["kind"], "word");
        assert_eq!(preview["paragraphs"][0], "API 文档");
        assert_eq!(preview["paragraphs"][1], "响应预览");
    }

    #[test]
    fn response_preview_office_extracts_pptx_slides() {
        let conn = test_conn();
        let cache_dir = get_api_workbench_response_cache_dir().expect("cache dir");
        let source = cache_dir.join("deck.pptx");
        write_openxml_zip(
            &source,
            &[(
                "ppt/slides/slide1.xml",
                r#"<p:sld xmlns:a="a" xmlns:p="p"><p:cSld><p:spTree><a:t>标题</a:t><a:t>要点一</a:t></p:spTree></p:cSld></p:sld>"#,
            )],
        );
        let source_bytes = fs::read(&source).expect("pptx bytes");
        let (file_path, _, _, _) = persist_response_cache_file(
            &cache_dir,
            &source_bytes,
            Some("deck.pptx".into()),
            Some("pptx".into()),
        )
        .expect("cache file");

        let preview = response_preview_office_with_conn(
            &conn,
            &json!({ "filePath": file_path, "kind": "slides" }),
        )
        .expect("preview");

        assert_eq!(preview["kind"], "slides");
        assert_eq!(preview["slides"][0]["title"], "标题");
        assert_eq!(preview["slides"][0]["texts"][1], "要点一");
    }
}
