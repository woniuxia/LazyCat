use regex::Regex;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::OnceLock;

pub static REGEX_TEMPLATES_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Build a regex pattern string with inline flags from the user-provided flags.
/// Supported: i (case-insensitive), m (multi-line), s (dot-matches-newline), x (extended).
/// The 'g' flag is ignored -- `captures_iter` is always used for global matching.
fn build_pattern_with_flags(pattern: &str, flags: &str) -> String {
    let mut flag_str = String::new();
    for ch in flags.chars() {
        match ch {
            'i' | 'm' | 's' | 'x' => flag_str.push(ch),
            _ => {} // ignore 'g' and unknown flags
        }
    }
    if flag_str.is_empty() {
        pattern.to_string()
    } else {
        format!("(?{flag_str}){pattern}")
    }
}

/// Convert a byte offset in `input` to a character offset.
fn byte_offset_to_char_offset(input: &str, byte_offset: usize) -> usize {
    input[..byte_offset].chars().count()
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "test" => {
            let pattern = payload["pattern"].as_str().unwrap_or_default();
            let flags = payload["flags"].as_str().unwrap_or("");
            let input = payload["input"].as_str().unwrap_or_default();

            if pattern.is_empty() {
                return Ok(Value::Array(vec![]));
            }

            let full_pattern = build_pattern_with_flags(pattern, flags);
            let re = Regex::new(&full_pattern).map_err(|e| format!("regex invalid: {e}"))?;

            let mut results = Vec::new();
            for caps in re.captures_iter(input) {
                let whole = caps.get(0).unwrap();
                let start = byte_offset_to_char_offset(input, whole.start());
                let end = byte_offset_to_char_offset(input, whole.end());

                let mut groups = Vec::new();
                for i in 1..caps.len() {
                    let group_name = re
                        .capture_names()
                        .nth(i)
                        .flatten()
                        .map(|n| Value::String(n.to_string()))
                        .unwrap_or(Value::Null);

                    if let Some(m) = caps.get(i) {
                        groups.push(json!({
                            "index": i,
                            "name": group_name,
                            "value": m.as_str(),
                            "start": byte_offset_to_char_offset(input, m.start()),
                            "end": byte_offset_to_char_offset(input, m.end()),
                        }));
                    } else {
                        groups.push(json!({
                            "index": i,
                            "name": group_name,
                            "value": null,
                            "start": null,
                            "end": null,
                        }));
                    }
                }

                results.push(json!({
                    "index": start,
                    "end": end,
                    "match": whole.as_str(),
                    "groups": groups,
                }));
            }
            Ok(Value::Array(results))
        }

        "replace" => {
            let pattern = payload["pattern"].as_str().unwrap_or_default();
            let flags = payload["flags"].as_str().unwrap_or("");
            let input = payload["input"].as_str().unwrap_or_default();
            let replacement = payload["replacement"].as_str().unwrap_or_default();

            if pattern.is_empty() {
                return Ok(json!(input));
            }

            let full_pattern = build_pattern_with_flags(pattern, flags);
            let re = Regex::new(&full_pattern).map_err(|e| format!("regex invalid: {e}"))?;
            let result = re.replace_all(input, replacement);
            Ok(json!(result.as_ref()))
        }

        "generate" => {
            // Look up template by id from the templates file
            let kind = payload["kind"].as_str().unwrap_or("email");
            let templates = load_templates()?;
            if let Some(tpl) = templates.iter().find(|t| {
                t.get("id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|id| id == kind)
            }) {
                if let Some(expr) = tpl.get("expression").and_then(|v| v.as_str()) {
                    return Ok(json!(expr));
                }
            }
            // Fallback to email pattern
            Ok(json!(
                "[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}"
            ))
        }

        "templates" => {
            let templates = load_templates()?;
            Ok(Value::Array(templates))
        }

        _ => Err(format!("unsupported regex action: {action}")),
    }
}

fn load_templates() -> Result<Vec<Value>, String> {
    let dir = REGEX_TEMPLATES_DIR
        .get()
        .ok_or_else(|| "regex templates directory not initialized".to_string())?;
    let path = dir.join("templates.json");
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("failed to read templates: {e}"))?;
    let templates: Vec<Value> =
        serde_json::from_str(&content).map_err(|e| format!("failed to parse templates: {e}"))?;
    Ok(templates)
}
