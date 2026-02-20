use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "split" => file_split(payload),
        "merge" => file_merge(payload),
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
    let parts = payload["parts"]
        .as_array()
        .ok_or("parts should be array".to_string())?;
    let output_path = PathBuf::from(payload["outputPath"].as_str().unwrap_or_default());
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create output parent failed: {e}"))?;
    }
    let mut writer = File::create(&output_path).map_err(|e| format!("create output failed: {e}"))?;
    let mut total_bytes = 0usize;
    for p in parts {
        let part_path = PathBuf::from(p.as_str().unwrap_or_default());
        let bytes = fs::read(&part_path).map_err(|e| format!("read part failed: {e}"))?;
        total_bytes += bytes.len();
        writer
            .write_all(&bytes)
            .map_err(|e| format!("write output failed: {e}"))?;
    }
    Ok(json!({
      "outputPath": output_path.to_string_lossy().to_string(),
      "totalBytes": total_bytes
    }))
}
