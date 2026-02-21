use jsonschema::JSONSchema;
use serde_json::{json, Map, Value};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "validate" => validate(payload),
        "generate_example" => generate_example(payload),
        _ => Err(format!("unsupported schema action: {action}")),
    }
}

fn validate(payload: &Value) -> Result<Value, String> {
    let schema_str = payload["schema"].as_str().unwrap_or_default();
    let document_str = payload["document"].as_str().unwrap_or_default();
    if schema_str.trim().is_empty() {
        return Err("schema is empty".into());
    }
    if document_str.trim().is_empty() {
        return Err("document is empty".into());
    }

    let schema: Value =
        serde_json::from_str(schema_str).map_err(|e| format!("invalid schema json: {e}"))?;
    let document: Value =
        serde_json::from_str(document_str).map_err(|e| format!("invalid document json: {e}"))?;

    let compiled =
        JSONSchema::compile(&schema).map_err(|e| format!("schema compile failed: {e}"))?;

    let validation = compiled.validate(&document);
    match validation {
        Ok(_) => Ok(json!({
            "valid": true,
            "errors": []
        })),
        Err(errors) => {
            let list = errors
                .map(|e| {
                    json!({
                        "instancePath": e.instance_path.to_string(),
                        "schemaPath": e.schema_path.to_string(),
                        "message": e.to_string(),
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({
                "valid": false,
                "errors": list
            }))
        }
    }
}

fn generate_example(payload: &Value) -> Result<Value, String> {
    let schema_str = payload["schema"].as_str().unwrap_or_default();
    if schema_str.trim().is_empty() {
        return Err("schema is empty".into());
    }
    let schema: Value =
        serde_json::from_str(schema_str).map_err(|e| format!("invalid schema json: {e}"))?;
    let mut warnings = Vec::new();
    let example = example_from_schema(&schema, &mut warnings);
    Ok(json!({
        "example": example,
        "warnings": warnings
    }))
}

fn example_from_schema(schema: &Value, warnings: &mut Vec<String>) -> Value {
    if let Some(enum_values) = schema.get("enum").and_then(Value::as_array) {
        if let Some(first) = enum_values.first() {
            return first.clone();
        }
    }

    for key in ["oneOf", "anyOf", "allOf"] {
        if let Some(arr) = schema.get(key).and_then(Value::as_array) {
            if let Some(first) = arr.first() {
                warnings.push(format!("{key} detected, using first branch as example"));
                return example_from_schema(first, warnings);
            }
        }
    }

    if let Some(default) = schema.get("default") {
        return default.clone();
    }

    let type_name = schema.get("type").and_then(Value::as_str).unwrap_or("object");
    match type_name {
        "string" => {
            let format = schema.get("format").and_then(Value::as_str).unwrap_or_default();
            match format {
                "date-time" => json!("1970-01-01T00:00:00Z"),
                "date" => json!("1970-01-01"),
                "email" => json!("user@example.com"),
                "uuid" => json!("00000000-0000-0000-0000-000000000000"),
                "uri" => json!("https://example.com"),
                _ => json!(""),
            }
        }
        "integer" => json!(0),
        "number" => json!(0.0),
        "boolean" => json!(true),
        "array" => {
            let item_schema = schema.get("items").unwrap_or(&Value::Null);
            json!([example_from_schema(item_schema, warnings)])
        }
        "object" => {
            let mut map = Map::new();
            let required = schema
                .get("required")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            if let Some(props) = schema.get("properties").and_then(Value::as_object) {
                for (idx, (key, prop_schema)) in props.iter().enumerate() {
                    if !required.is_empty() && !required.iter().any(|r| r == key) {
                        continue;
                    }
                    if idx >= 8 {
                        warnings.push("example object truncated to first 8 properties".into());
                        break;
                    }
                    map.insert(key.clone(), example_from_schema(prop_schema, warnings));
                }
            }
            Value::Object(map)
        }
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validate_should_return_true_for_valid_document() {
        let schema = r#"{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}"#;
        let doc = r#"{"name":"lazycat"}"#;
        let out = execute("validate", &json!({ "schema": schema, "document": doc })).expect("validate");
        assert_eq!(out["valid"], true);
        assert_eq!(out["errors"], json!([]));
    }

    #[test]
    fn validate_should_return_errors_for_invalid_document() {
        let schema = r#"{"type":"object","properties":{"age":{"type":"integer"}},"required":["age"]}"#;
        let doc = r#"{"age":"x"}"#;
        let out = execute("validate", &json!({ "schema": schema, "document": doc })).expect("validate");
        assert_eq!(out["valid"], false);
        assert!(out["errors"].as_array().map_or(0, |a| a.len()) >= 1);
    }

    #[test]
    fn generate_example_should_handle_enum_and_one_of() {
        let schema_enum = r#"{"type":"string","enum":["A","B"]}"#;
        let out = execute("generate_example", &json!({ "schema": schema_enum })).expect("example");
        assert_eq!(out["example"], "A");

        let schema_oneof = r#"{"oneOf":[{"type":"integer"},{"type":"string"}]}"#;
        let out = execute("generate_example", &json!({ "schema": schema_oneof })).expect("example");
        assert_eq!(out["example"], 0);
        assert!(out["warnings"].as_array().map_or(0, |a| a.len()) >= 1);
    }

    #[test]
    fn schema_empty_input_should_fail() {
        let err = execute("validate", &json!({ "schema": "", "document": "{}" })).expect_err("empty schema");
        assert!(err.contains("schema is empty"));
    }
}
