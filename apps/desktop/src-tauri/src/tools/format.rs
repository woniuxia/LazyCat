use serde_json::{json, Value};

const ACTIONS: &[&str] = &["json", "xml", "html", "java", "sql"];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported format action: {action}"));
    }
    match action {
        "json" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let v: Value = serde_json::from_str(input).map_err(|e| format!("invalid json: {e}"))?;
            Ok(json!(
                serde_json::to_string_pretty(&v).unwrap_or_else(|_| input.to_string())
            ))
        }
        "xml" => Ok(json!(payload["input"]
            .as_str()
            .unwrap_or_default()
            .to_string())),
        "html" => Ok(json!(payload["input"]
            .as_str()
            .unwrap_or_default()
            .to_string())),
        "java" => Ok(json!(payload["input"]
            .as_str()
            .unwrap_or_default()
            .lines()
            .map(|l| l.trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n"))),
        "sql" => Ok(json!(payload["input"]
            .as_str()
            .unwrap_or_default()
            .to_string())),
        _ => Err(format!("unsupported format action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_json_should_pretty_print() {
        let out = execute("json", &json!({ "input": r#"{"b":1,"a":2}"# })).expect("json");
        let s = out.as_str().unwrap_or_default();
        assert!(s.contains('\n'));
        assert!(s.contains("\"a\""));
    }

    #[test]
    fn format_java_should_trim_line_endings() {
        let out = execute("java", &json!({ "input": "a  \n b\t  " })).expect("java");
        assert_eq!(out, json!("a\n b"));
    }

    #[test]
    fn invalid_json_should_fail() {
        let err = execute("json", &json!({ "input": "{bad}" })).expect_err("must fail");
        assert!(err.contains("invalid json"));
    }
}
