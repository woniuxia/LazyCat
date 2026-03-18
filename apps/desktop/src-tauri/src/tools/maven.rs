use serde_json::{json, Value};
use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "locate" => locate(payload),
        "open_path" => open_path(payload),
        _ => Err(format!("unsupported maven action: {action}")),
    }
}

fn locate(payload: &Value) -> Result<Value, String> {
    let group_id = payload["groupId"].as_str().unwrap_or_default().trim();
    let artifact_id = payload["artifactId"].as_str().unwrap_or_default().trim();
    let version = payload["version"].as_str().unwrap_or_default().trim();
    let repo_path = payload["repoPath"].as_str().unwrap_or_default().trim();

    if group_id.is_empty() {
        return Err("groupId 不能为空".into());
    }
    if artifact_id.is_empty() {
        return Err("artifactId 不能为空".into());
    }
    // version 为空时，不定位具体版本，而是查询 artifact 的所有版本信息
    if repo_path.is_empty() {
        return Err("repoPath 不能为空".into());
    }

    let repo_dir = PathBuf::from(repo_path);
    if !repo_dir.is_dir() {
        return Err(format!("仓库路径不存在或不是目录: {repo_path}"));
    }

    // Build artifact base dir: {repoPath}/{groupId with . -> /}/{artifactId}/
    let group_path = group_id.replace('.', std::path::MAIN_SEPARATOR_STR);
    let artifact_dir = repo_dir.join(&group_path).join(artifact_id);

    // Build target jar path (only when version is provided)
    let (target_jar_path, found, jar_size) = if version.is_empty() {
        let empty_path = PathBuf::new();
        (empty_path, false, 0)
    } else {
        let jar_name = format!("{artifact_id}-{version}.jar");
        let jar_path = artifact_dir.join(version).join(&jar_name);
        let jar_exists = jar_path.is_file();
        let size = if jar_exists {
            fs::metadata(&jar_path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };
        (jar_path, jar_exists, size)
    };

    // Scan all version subdirectories
    let mut versions: Vec<Value> = Vec::new();
    if artifact_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&artifact_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let ver_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                let ver_jar = path.join(format!("{artifact_id}-{ver_name}.jar"));
                let has_jar = ver_jar.is_file();
                let size = if has_jar {
                    fs::metadata(&ver_jar).map(|m| m.len()).unwrap_or(0)
                } else {
                    0
                };
                versions.push(json!({
                    "version": ver_name,
                    "hasJar": has_jar,
                    "jarSize": size,
                }));
            }
        }
    }

    // Sort versions semantically (descending - newest first)
    versions.sort_by(|a, b| {
        let va = a["version"].as_str().unwrap_or_default();
        let vb = b["version"].as_str().unwrap_or_default();
        compare_versions(vb, va) // reverse order for descending
    });

    let total_versions = versions.len();

    Ok(json!({
        "groupId": group_id,
        "artifactId": artifact_id,
        "version": version,
        "artifactDir": artifact_dir.to_string_lossy(),
        "targetJarPath": target_jar_path.to_string_lossy(),
        "found": found,
        "jarSize": jar_size,
        "versions": versions,
        "totalVersions": total_versions,
    }))
}

fn open_path(payload: &Value) -> Result<Value, String> {
    let path = payload["path"].as_str().unwrap_or_default().trim();
    if path.is_empty() {
        return Err("path 不能为空".into());
    }

    let p = PathBuf::from(path);
    let dir = if p.is_file() {
        p.parent()
            .ok_or_else(|| "无法获取父目录".to_string())?
            .to_path_buf()
    } else {
        p
    };

    if !dir.is_dir() {
        return Err(format!("目录不存在: {}", dir.display()));
    }

    open::that(&dir).map_err(|e| format!("打开目录失败: {e}"))?;
    Ok(json!({ "ok": true }))
}

/// Compare two version strings semantically.
/// Splits by `.` and `-`, compares numeric segments numerically, others lexicographically.
fn compare_versions(a: &str, b: &str) -> Ordering {
    let sa = split_version(a);
    let sb = split_version(b);
    let len = sa.len().max(sb.len());
    for i in 0..len {
        let pa = sa.get(i).map(|s| s.as_str()).unwrap_or("");
        let pb = sb.get(i).map(|s| s.as_str()).unwrap_or("");
        let ord = match (pa.parse::<u64>(), pb.parse::<u64>()) {
            (Ok(na), Ok(nb)) => na.cmp(&nb),
            _ => pa.cmp(pb),
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    sa.len().cmp(&sb.len())
}

fn split_version(v: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    for ch in v.chars() {
        if ch == '.' || ch == '-' {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_sorting() {
        assert_eq!(compare_versions("1.2.3", "1.2.4"), Ordering::Less);
        assert_eq!(compare_versions("1.10.0", "1.9.0"), Ordering::Greater);
        assert_eq!(compare_versions("2.0.0", "2.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0-alpha", "1.0-beta"), Ordering::Less);
    }

    #[test]
    fn missing_fields_should_fail() {
        let err = execute("locate", &json!({})).expect_err("must fail");
        assert!(err.contains("groupId"));
    }

    #[test]
    fn unsupported_action_should_fail() {
        let err = execute("nope", &json!({})).expect_err("must fail");
        assert!(err.contains("unsupported maven action"));
    }
}
