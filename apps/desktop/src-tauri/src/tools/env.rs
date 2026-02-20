use serde_json::{json, Value};
use std::process::Command;

pub fn execute(action: &str, _payload: &Value) -> Result<Value, String> {
    match action {
        "detect" => {
            let node = Command::new("node")
                .arg("-v")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "NOT_FOUND".into());
            let java = Command::new("java")
                .arg("-version")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(if o.stderr.is_empty() { &o.stdout } else { &o.stderr }).lines().next().unwrap_or("UNKNOWN").to_string())
                .unwrap_or_else(|| "NOT_FOUND".into());
            Ok(json!({"node": node, "java": java}))
        }
        _ => Err(format!("unsupported env action: {action}")),
    }
}
