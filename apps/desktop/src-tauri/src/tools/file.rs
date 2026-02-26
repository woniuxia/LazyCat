use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "split" => file_split(payload),
        "merge" => file_merge(payload),
        "write_text" => write_text(payload),
        _ => Err(format!("unsupported file action: {action}")),
    }
}

fn file_split(payload: &Value) -> Result<Value, String> {
    let source_path = PathBuf::from(payload["sourcePath"].as_str().unwrap_or_default());
    let output_dir = PathBuf::from(payload["outputDir"].as_str().unwrap_or_default());
    let chunk_mb = payload["chunkSizeMb"].as_u64().unwrap_or(100) as usize;
    if !source_path.exists() {
        return Err("source file not found".into());
    }
    fs::create_dir_all(&output_dir).map_err(|e| format!("create output dir failed: {e}"))?;
    let metadata = fs::metadata(&source_path).map_err(|e| format!("stat source failed: {e}"))?;
    let chunk_size = chunk_mb * 1024 * 1024;
    let total = metadata.len() as usize;
    let mut reader = File::open(&source_path).map_err(|e| format!("open source failed: {e}"))?;
    let mut idx = 0usize;
    let filename = source_path
        .file_name()
        .and_then(|x| x.to_str())
        .ok_or("invalid source filename".to_string())?;
    loop {
        let mut buf = vec![0u8; chunk_size];
        let n = reader.read(&mut buf).map_err(|e| format!("read source failed: {e}"))?;
        if n == 0 {
            break;
        }
        buf.truncate(n);
        let part_name = format!("{filename}.part{:04}", idx + 1);
        let part_path = output_dir.join(&part_name);
        fs::write(&part_path, &buf).map_err(|e| format!("write part failed: {e}"))?;
        idx += 1;
    }
    Ok(json!({
      "chunkCount": idx,
      "outputDir": output_dir.to_string_lossy().to_string(),
      "totalBytes": total
    }))
}

fn file_merge(payload: &Value) -> Result<Value, String> {
    let parts = collect_merge_parts(payload)?;
    let output_path = PathBuf::from(payload["outputPath"].as_str().unwrap_or_default());
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create output parent failed: {e}"))?;
    }
    let mut writer = File::create(&output_path).map_err(|e| format!("create output failed: {e}"))?;
    let mut total_bytes = 0usize;
    for part_path in &parts {
        let bytes = fs::read(&part_path).map_err(|e| format!("read part failed: {e}"))?;
        total_bytes += bytes.len();
        writer
            .write_all(&bytes)
            .map_err(|e| format!("write output failed: {e}"))?;
    }
    Ok(json!({
      "outputPath": output_path.to_string_lossy().to_string(),
      "totalBytes": total_bytes,
      "partCount": parts.len()
    }))
}

fn collect_merge_parts(payload: &Value) -> Result<Vec<PathBuf>, String> {
    let mut parts = payload["parts"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if !parts.is_empty() {
        return Ok(parts);
    }

    let parts_dir_str = payload["partsDir"].as_str().unwrap_or_default().trim().to_string();
    if parts_dir_str.is_empty() {
        return Err("parts is empty and partsDir is missing".into());
    }

    let parts_dir = PathBuf::from(parts_dir_str);
    if !parts_dir.exists() {
        return Err("partsDir not found".into());
    }
    if !parts_dir.is_dir() {
        return Err("partsDir should be a directory".into());
    }

    let mut candidates = fs::read_dir(&parts_dir)
        .map_err(|e| format!("read partsDir failed: {e}"))?
        .filter_map(|entry| entry.ok().map(|v| v.path()))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    candidates.sort();
    parts.append(&mut candidates);

    if parts.is_empty() {
        return Err("no files found in partsDir".into());
    }
    Ok(parts)
}

fn write_text(payload: &Value) -> Result<Value, String> {
    let path = payload["path"]
        .as_str()
        .ok_or("缺少 path 参数")?;
    let content = payload["content"]
        .as_str()
        .ok_or("缺少 content 参数")?;
    fs::write(path, content.as_bytes())
        .map_err(|e| format!("写入文件失败: {e}"))?;
    Ok(json!({ "path": path }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    #[test]
    fn split_and_merge_should_keep_content() {
        let dir = std::env::temp_dir().join(format!("lazycat-file-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("mkdir");
        let source = dir.join("source.bin");
        let output_dir = dir.join("parts");
        let merged = dir.join("merged.bin");
        let content = vec![1u8; 1024 * 1024 + 123];
        fs::write(&source, &content).expect("write source");

        let split_out = execute(
            "split",
            &json!({
                "sourcePath": source.to_string_lossy().to_string(),
                "outputDir": output_dir.to_string_lossy().to_string(),
                "chunkSizeMb": 1
            }),
        )
        .expect("split");
        assert!(split_out["chunkCount"].as_u64().unwrap_or(0) >= 2);

        let mut parts = fs::read_dir(&output_dir)
            .expect("list parts")
            .filter_map(|e| e.ok().map(|v| v.path()))
            .collect::<Vec<_>>();
        parts.sort();
        let parts_json = parts
            .iter()
            .map(|p| json!(p.to_string_lossy().to_string()))
            .collect::<Vec<_>>();

        execute(
            "merge",
            &json!({
                "parts": parts_json,
                "outputPath": merged.to_string_lossy().to_string()
            }),
        )
        .expect("merge");

        let merged_content = fs::read(&merged).expect("read merged");
        assert_eq!(merged_content, content);
    }

    #[test]
    fn merge_should_support_parts_dir() {
        let dir = std::env::temp_dir().join(format!("lazycat-file-dir-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("mkdir");
        let source = dir.join("source.bin");
        let output_dir = dir.join("parts");
        let merged = dir.join("merged.bin");
        let content = vec![7u8; 1024 * 1024 + 223];
        fs::write(&source, &content).expect("write source");

        execute(
            "split",
            &json!({
                "sourcePath": source.to_string_lossy().to_string(),
                "outputDir": output_dir.to_string_lossy().to_string(),
                "chunkSizeMb": 1
            }),
        )
        .expect("split");

        execute(
            "merge",
            &json!({
                "partsDir": output_dir.to_string_lossy().to_string(),
                "outputPath": merged.to_string_lossy().to_string()
            }),
        )
        .expect("merge");

        let merged_content = fs::read(&merged).expect("read merged");
        assert_eq!(merged_content, content);
    }

    #[test]
    fn write_text_should_write_file() {
        let path = std::env::temp_dir().join(format!("lazycat-write-{}.txt", std::process::id()));
        execute(
            "write_text",
            &json!({ "path": path.to_string_lossy().to_string(), "content": "abc" }),
        )
        .expect("write_text");
        assert_eq!(fs::read_to_string(&path).expect("read"), "abc");
        let _ = fs::remove_file(path);
    }
}
