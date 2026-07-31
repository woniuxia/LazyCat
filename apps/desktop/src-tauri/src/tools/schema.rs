use jsonschema::JSONSchema;
use serde_json::{json, Map, Number, Value};
use std::collections::HashSet;

const ACTIONS: &[&str] = &["validate", "generate_example"];
const MAX_EXAMPLE_DEPTH: usize = 32;

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported schema action: {action}"));
    }
    match action {
        "validate" => validate(payload),
        "generate_example" => generate_example(payload),
        _ => Err(format!("unsupported schema action: {action}")),
    }
}

fn parse_json_input(input: &str, label: &str) -> Result<Value, String> {
    serde_json::from_str(input).map_err(|error| {
        format!(
            "{label} 不是合法 JSON：{}（第 {} 行，第 {} 列）",
            error,
            error.line(),
            error.column()
        )
    })
}

fn validate(payload: &Value) -> Result<Value, String> {
    let schema_str = payload["schema"].as_str().unwrap_or_default();
    let document_str = payload["document"].as_str().unwrap_or_default();
    if schema_str.trim().is_empty() {
        return Err("Schema 不能为空".into());
    }
    if document_str.trim().is_empty() {
        return Err("待校验 JSON 不能为空".into());
    }

    let schema = parse_json_input(schema_str, "Schema")?;
    let document = parse_json_input(document_str, "待校验内容")?;
    let compiled =
        JSONSchema::compile(&schema).map_err(|error| format!("Schema 编译失败：{error}"))?;

    let result = match compiled.validate(&document) {
        Ok(_) => Ok(json!({ "valid": true, "errors": [] })),
        Err(errors) => {
            let list = errors
                .map(|error| {
                    json!({
                        "instancePath": error.instance_path.to_string(),
                        "schemaPath": error.schema_path.to_string(),
                        "message": error.to_string(),
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({ "valid": false, "errors": list }))
        }
    };
    result
}

fn generate_example(payload: &Value) -> Result<Value, String> {
    let schema_str = payload["schema"].as_str().unwrap_or_default();
    if schema_str.trim().is_empty() {
        return Err("Schema 不能为空".into());
    }
    let schema = parse_json_input(schema_str, "Schema")?;
    let branch_index = payload["branchIndex"].as_u64().unwrap_or(0) as usize;
    let mut context = ExampleContext::new(&schema, branch_index);
    let example = context.generate(&schema, "#", 0);

    match JSONSchema::compile(&schema) {
        Ok(compiled) => {
            if let Err(errors) = compiled.validate(&example) {
                let details = errors
                    .take(3)
                    .map(|error| error.to_string())
                    .collect::<Vec<_>>()
                    .join("；");
                context.warn(format!("生成样例未完全满足 Schema：{details}"));
            }
        }
        Err(error) => context.warn(format!("Schema 编译失败，样例仅按可识别字段生成：{error}")),
    }

    Ok(json!({
        "example": example,
        "warnings": context.warnings,
        "branchIndex": branch_index
    }))
}

struct ExampleContext<'a> {
    root: &'a Value,
    branch_index: usize,
    resolving_refs: Vec<String>,
    warnings: Vec<String>,
    warning_set: HashSet<String>,
}

impl<'a> ExampleContext<'a> {
    fn new(root: &'a Value, branch_index: usize) -> Self {
        Self {
            root,
            branch_index,
            resolving_refs: Vec::new(),
            warnings: Vec::new(),
            warning_set: HashSet::new(),
        }
    }

    fn warn(&mut self, warning: String) {
        if self.warning_set.insert(warning.clone()) {
            self.warnings.push(warning);
        }
    }

    fn generate(&mut self, schema: &Value, path: &str, depth: usize) -> Value {
        if depth >= MAX_EXAMPLE_DEPTH {
            self.warn(format!(
                "{path}：Schema 嵌套超过 {MAX_EXAMPLE_DEPTH} 层，已停止展开"
            ));
            return Value::Null;
        }

        if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
            return self.generate_ref(schema, reference, path, depth);
        }

        if let Some(value) = schema.get("const") {
            return value.clone();
        }
        if let Some(value) = schema
            .get("examples")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
        {
            return value.clone();
        }
        if let Some(value) = schema.get("default") {
            return value.clone();
        }
        if let Some(value) = schema
            .get("enum")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
        {
            return value.clone();
        }

        if let Some(branches) = schema.get("allOf").and_then(Value::as_array) {
            let mut base = schema.clone();
            base.as_object_mut().map(|object| object.remove("allOf"));
            let mut example = self.generate(&base, path, depth + 1);
            for (index, branch) in branches.iter().enumerate() {
                let branch_path = format!("{path}/allOf/{index}");
                let branch_example = self.generate(branch, &branch_path, depth + 1);
                merge_examples(&mut example, branch_example, &branch_path, self);
            }
            return example;
        }

        for keyword in ["oneOf", "anyOf"] {
            if let Some(branches) = schema.get(keyword).and_then(Value::as_array) {
                if branches.is_empty() {
                    self.warn(format!("{path}/{keyword}：没有可用分支"));
                    return Value::Null;
                }
                let selected = self.branch_index.min(branches.len() - 1);
                if selected != self.branch_index {
                    self.warn(format!(
                        "{path}/{keyword}：请求的第 {} 个分支不存在，已使用第 {} 个分支",
                        self.branch_index + 1,
                        selected + 1
                    ));
                } else {
                    self.warn(format!(
                        "{path}/{keyword}：按明确分支策略使用第 {} 个分支",
                        selected + 1
                    ));
                }
                let mut base = schema.clone();
                base.as_object_mut().map(|object| object.remove(keyword));
                let mut example = self.generate(&base, path, depth + 1);
                let branch_path = format!("{path}/{keyword}/{selected}");
                let branch_example = self.generate(&branches[selected], &branch_path, depth + 1);
                merge_examples(&mut example, branch_example, &branch_path, self);
                return example;
            }
        }

        match schema_type(schema, path, self) {
            "string" => string_example(schema),
            "integer" => integer_example(schema),
            "number" => number_example(schema),
            "boolean" => json!(true),
            "null" => Value::Null,
            "array" => {
                let item_schema = schema.get("items").unwrap_or(&Value::Null);
                let count = schema
                    .get("minItems")
                    .and_then(Value::as_u64)
                    .unwrap_or(1)
                    .clamp(1, 3) as usize;
                let item = self.generate(item_schema, &format!("{path}/items"), depth + 1);
                Value::Array((0..count).map(|_| item.clone()).collect())
            }
            "object" => self.generate_object(schema, path, depth),
            unknown => {
                self.warn(format!("{path}：暂不支持类型 {unknown}，已生成 null"));
                Value::Null
            }
        }
    }

    fn generate_ref(&mut self, schema: &Value, reference: &str, path: &str, depth: usize) -> Value {
        if !reference.starts_with('#') {
            self.warn(format!("{path}：暂不支持外部引用 {reference}"));
            return Value::Null;
        }
        if self.resolving_refs.iter().any(|item| item == reference) {
            self.warn(format!("{path}：检测到循环引用 {reference}，已停止展开"));
            return Value::Null;
        }
        let Some(target) = resolve_local_ref(self.root, reference) else {
            self.warn(format!("{path}：无法解析本地引用 {reference}"));
            return Value::Null;
        };

        self.resolving_refs.push(reference.to_string());
        let mut example = self.generate(target, reference, depth + 1);
        self.resolving_refs.pop();

        let mut siblings = schema.clone();
        if let Some(object) = siblings.as_object_mut() {
            object.remove("$ref");
            if !object.is_empty() {
                let sibling_example = self.generate(&siblings, path, depth + 1);
                merge_examples(&mut example, sibling_example, path, self);
            }
        }
        example
    }

    fn generate_object(&mut self, schema: &Value, path: &str, depth: usize) -> Value {
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
            return json!({});
        };

        let include_all = required.is_empty();
        let mut map = Map::new();
        for (key, property_schema) in properties {
            if !include_all && !required.contains(key.as_str()) {
                continue;
            }
            if map.len() >= 8 {
                self.warn(format!("{path}：样例对象最多展开前 8 个字段"));
                break;
            }
            map.insert(
                key.clone(),
                self.generate(
                    property_schema,
                    &format!("{path}/properties/{}", escape_pointer_token(key)),
                    depth + 1,
                ),
            );
        }
        Value::Object(map)
    }
}

fn resolve_local_ref<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    match reference {
        "#" => Some(root),
        value if value.starts_with("#/") => root.pointer(&value[1..]),
        _ => None,
    }
}

fn schema_type<'a>(schema: &'a Value, path: &str, context: &mut ExampleContext<'_>) -> &'a str {
    if let Some(value) = schema.get("type").and_then(Value::as_str) {
        return value;
    }
    if let Some(types) = schema.get("type").and_then(Value::as_array) {
        if let Some(selected) = types
            .iter()
            .filter_map(Value::as_str)
            .find(|value| *value != "null")
            .or_else(|| types.iter().filter_map(Value::as_str).next())
        {
            context.warn(format!("{path}/type：联合类型按顺序使用 {selected}"));
            return selected;
        }
    }
    if schema.get("properties").is_some() || schema.get("required").is_some() {
        "object"
    } else if schema.get("items").is_some() {
        "array"
    } else {
        "object"
    }
}

fn string_example(schema: &Value) -> Value {
    let value = match schema
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "date-time" => "1970-01-01T00:00:00Z".to_string(),
        "date" => "1970-01-01".to_string(),
        "time" => "00:00:00Z".to_string(),
        "email" => "user@example.com".to_string(),
        "hostname" => "example.com".to_string(),
        "ipv4" => "127.0.0.1".to_string(),
        "ipv6" => "::1".to_string(),
        "uuid" => "00000000-0000-0000-0000-000000000000".to_string(),
        "uri" | "url" => "https://example.com".to_string(),
        _ => "example".to_string(),
    };
    let min_length = schema
        .get("minLength")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(64) as usize;
    if value.chars().count() >= min_length {
        Value::String(value)
    } else {
        Value::String(format!(
            "{value}{}",
            "x".repeat(min_length - value.chars().count())
        ))
    }
}

fn integer_example(schema: &Value) -> Value {
    if let Some(value) = schema.get("minimum").and_then(Value::as_i64) {
        return json!(value);
    }
    if let Some(value) = schema.get("exclusiveMinimum").and_then(Value::as_i64) {
        return json!(value.saturating_add(1));
    }
    json!(0)
}

fn number_example(schema: &Value) -> Value {
    let value = schema
        .get("minimum")
        .and_then(Value::as_f64)
        .or_else(|| {
            schema
                .get("exclusiveMinimum")
                .and_then(Value::as_f64)
                .map(|value| value + f64::EPSILON)
        })
        .unwrap_or(0.0);
    Number::from_f64(value)
        .map(Value::Number)
        .unwrap_or_else(|| json!(0.0))
}

fn merge_examples(
    target: &mut Value,
    incoming: Value,
    path: &str,
    context: &mut ExampleContext<'_>,
) {
    match (target, incoming) {
        (Value::Object(target), Value::Object(incoming)) => {
            for (key, value) in incoming {
                if let Some(existing) = target.get_mut(&key) {
                    merge_examples(existing, value, &format!("{path}/{key}"), context);
                } else {
                    target.insert(key, value);
                }
            }
        }
        (target, incoming) if target.is_null() => *target = incoming,
        (target, incoming) if *target == incoming => {}
        (target, incoming) => {
            context.warn(format!("{path}：组合分支生成值冲突，使用后声明的值"));
            *target = incoming;
        }
    }
}

fn escape_pointer_token(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_should_return_true_for_valid_document() {
        let schema =
            r#"{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}"#;
        let doc = r#"{"name":"lazycat"}"#;
        let out =
            execute("validate", &json!({ "schema": schema, "document": doc })).expect("validate");
        assert_eq!(out["valid"], true);
        assert_eq!(out["errors"], json!([]));
    }

    #[test]
    fn validate_should_return_errors_for_invalid_document() {
        let schema =
            r#"{"type":"object","properties":{"age":{"type":"integer"}},"required":["age"]}"#;
        let doc = r#"{"age":"x"}"#;
        let out =
            execute("validate", &json!({ "schema": schema, "document": doc })).expect("validate");
        assert_eq!(out["valid"], false);
        assert!(!out["errors"].as_array().expect("errors").is_empty());
    }

    #[test]
    fn validate_should_report_external_ref_without_resolver() {
        let schema = r#"{"$ref":"https://example.invalid/schema.json"}"#;
        let out = execute("validate", &json!({ "schema": schema, "document": "{}" }))
            .expect("external ref should return a validation result");

        assert_eq!(out["valid"], false);
        assert!(out["errors"]
            .as_array()
            .expect("errors")
            .iter()
            .any(|error| error["message"]
                .as_str()
                .unwrap_or_default()
                .contains("`resolve-http` feature or a custom resolver is required")));
    }

    #[test]
    fn generate_example_resolves_local_ref_and_merges_all_of() {
        let schema = r##"{
          "$defs":{"identity":{"type":"object","properties":{"id":{"type":"integer","minimum":1}},"required":["id"]}},
          "allOf":[
            {"$ref":"#/$defs/identity"},
            {"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}
          ]
        }"##;
        let out = execute("generate_example", &json!({ "schema": schema })).expect("example");
        assert_eq!(out["example"], json!({ "id": 1, "name": "example" }));
        assert!(out["warnings"].as_array().expect("warnings").is_empty());
    }

    #[test]
    fn generate_example_uses_requested_combination_branch() {
        let schema = r#"{"oneOf":[{"type":"integer"},{"type":"string","default":"chosen"}]}"#;
        let out = execute(
            "generate_example",
            &json!({ "schema": schema, "branchIndex": 1 }),
        )
        .expect("example");
        assert_eq!(out["example"], "chosen");
        assert!(out["warnings"][0]
            .as_str()
            .unwrap_or_default()
            .contains("第 2 个分支"));
    }

    #[test]
    fn generate_example_stops_cyclic_ref() {
        let schema = r##"{"$defs":{"node":{"type":"object","properties":{"next":{"$ref":"#/$defs/node"}},"required":["next"]}},"$ref":"#/$defs/node"}"##;
        let out = execute("generate_example", &json!({ "schema": schema })).expect("example");
        assert_eq!(out["example"], json!({ "next": null }));
        assert!(out["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning.as_str().unwrap_or_default().contains("循环引用")));
    }

    #[test]
    fn invalid_json_error_contains_location() {
        let error = execute(
            "validate",
            &json!({ "schema": "{\n  bad", "document": "{}" }),
        )
        .expect_err("invalid schema");
        assert!(error.contains("第 2 行"));
        assert!(error.contains("第"));
    }

    #[test]
    fn schema_empty_input_should_fail() {
        let error = execute("validate", &json!({ "schema": "", "document": "{}" }))
            .expect_err("empty schema");
        assert!(error.contains("Schema 不能为空"));
    }
}
