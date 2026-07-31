use serde_json::Value;
use std::collections::BTreeMap;

fn parse_properties(input: &str) -> Result<Value, String> {
    let mut root = serde_json::Map::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }
        if let Some(pos) = trimmed.find('=') {
            let key = trimmed[..pos].trim();
            let val = trimmed[pos + 1..].trim();
            // Support nested keys: a.b.c = v
            let parts: Vec<&str> = key.split('.').collect();
            set_nested(&mut root, &parts, Value::String(val.to_string()));
        }
    }
    Ok(Value::Object(root))
}

fn set_nested(map: &mut serde_json::Map<String, Value>, parts: &[&str], value: Value) {
    if parts.len() == 1 {
        map.insert(parts[0].to_string(), value);
        return;
    }
    let entry = map
        .entry(parts[0].to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if let Value::Object(ref mut child) = entry {
        set_nested(child, &parts[1..], value);
    }
}

fn parse_env(input: &str) -> Result<Value, String> {
    let mut map = serde_json::Map::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(pos) = trimmed.find('=') {
            let key = trimmed[..pos].trim();
            let val = trimmed[pos + 1..]
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            map.insert(key.to_string(), Value::String(val.to_string()));
        }
    }
    Ok(Value::Object(map))
}

fn serialize_properties(value: &Value) -> String {
    let mut lines = Vec::new();
    flatten_value(value, "", &mut lines);
    lines.sort();
    lines.join("\n")
}

fn flatten_value(value: &Value, prefix: &str, lines: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_value(v, &key, lines);
            }
        }
        _ => {
            let s = match value {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            lines.push(format!("{prefix}={s}"));
        }
    }
}

fn serialize_env(value: &Value) -> String {
    let mut lines = Vec::new();
    if let Value::Object(map) = value {
        let sorted: BTreeMap<_, _> = map.iter().collect();
        for (k, v) in sorted {
            let s = match v {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            lines.push(format!("{k}={s}"));
        }
    } else {
        flatten_value(value, "", &mut lines);
        lines.sort();
    }
    lines.join("\n")
}

pub(super) fn config_convert(payload: &Value) -> Result<Value, String> {
    let input = payload["input"].as_str().unwrap_or_default();
    let from = payload["from"].as_str().unwrap_or_default();
    let to = payload["to"].as_str().unwrap_or_default();

    let intermediate: Value = match from {
        "properties" => parse_properties(input)?,
        "yaml" => serde_norway::from_str(input).map_err(|e| format!("YAML 解析失败: {e}"))?,
        "toml" => toml::from_str(input).map_err(|e| format!("TOML 解析失败: {e}"))?,
        "env" => parse_env(input)?,
        _ => return Err(format!("不支持的源格式: {from}")),
    };

    let output =
        match to {
            "properties" => serialize_properties(&intermediate),
            "yaml" => serde_norway::to_string(&intermediate)
                .map_err(|e| format!("YAML 序列化失败: {e}"))?,
            "toml" => toml::to_string_pretty(&intermediate)
                .map_err(|e| format!("TOML 序列化失败: {e}"))?,
            "env" => serialize_env(&intermediate),
            _ => return Err(format!("不支持的目标格式: {to}")),
        };

    Ok(serde_json::json!({ "output": output }))
}

pub(super) fn yaml_validate(payload: &Value) -> Result<Value, String> {
    let input = payload["input"].as_str().unwrap_or_default();
    match serde_norway::from_str::<Value>(input) {
        Ok(_) => Ok(serde_json::json!({ "valid": true, "error": null })),
        Err(e) => {
            let loc = e.location();
            Ok(serde_json::json!({
                "valid": false,
                "error": {
                    "line": loc.map(|l| l.line()).unwrap_or(0),
                    "message": e.to_string(),
                }
            }))
        }
    }
}

pub(super) fn yaml_format(payload: &Value) -> Result<Value, String> {
    let input = payload["input"].as_str().unwrap_or_default();
    let value: Value = serde_norway::from_str(input).map_err(|e| format!("YAML 解析失败: {e}"))?;
    let output = serde_norway::to_string(&value).map_err(|e| format!("YAML 序列化失败: {e}"))?;
    Ok(serde_json::json!({ "output": output }))
}
