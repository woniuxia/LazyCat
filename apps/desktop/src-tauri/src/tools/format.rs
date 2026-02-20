use serde_json::{json, Value};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "json" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let v: Value = serde_json::from_str(input).map_err(|e| format!("invalid json: {e}"))?;
            Ok(json!(serde_json::to_string_pretty(&v).unwrap_or_else(|_| input.to_string())))
        }
        "xml" => Ok(json!(payload["input"].as_str().unwrap_or_default().to_string())),
        "html" => Ok(json!(payload["input"].as_str().unwrap_or_default().to_string())),
        "java" => Ok(json!(
            payload["input"]
                .as_str()
                .unwrap_or_default()
                .lines()
                .map(|l| l.trim_end().to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )),
        "sql" => Ok(json!(payload["input"].as_str().unwrap_or_default().to_string())),
        _ => Err(format!("unsupported format action: {action}")),
    }
}
