use serde_json::{json, Value};
use std::collections::HashSet;

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "unique_lines" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let case_sensitive = payload["caseSensitive"].as_bool().unwrap_or(false);
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for line in input.lines() {
                let key = if case_sensitive {
                    line.to_string()
                } else {
                    line.to_lowercase()
                };
                if !seen.contains(&key) {
                    seen.insert(key);
                    out.push(line.to_string());
                }
            }
            Ok(json!(out.join("\n")))
        }
        "sort_lines" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let case_sensitive = payload["caseSensitive"].as_bool().unwrap_or(false);
            let mut lines = input.lines().map(|s| s.to_string()).collect::<Vec<_>>();
            if case_sensitive {
                lines.sort();
            } else {
                lines.sort_by_key(|x| x.to_lowercase());
            }
            Ok(json!(lines.join("\n")))
        }
        _ => Err(format!("unsupported text action: {action}")),
    }
}
