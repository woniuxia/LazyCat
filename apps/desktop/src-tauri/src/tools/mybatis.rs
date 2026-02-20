use regex::Regex;
use roxmltree::{Document, Node};
use serde_json::{json, Value};
use std::borrow::Cow;

#[derive(Debug)]
struct RenderContext {
    params: Value,
    bindings: Vec<Value>,
    warnings: Vec<String>,
    safe_substitution: bool,
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "render" => render(payload),
        "lint" => lint(payload),
        _ => Err(format!("unsupported mybatis action: {action}")),
    }
}

fn render(payload: &Value) -> Result<Value, String> {
    let sql_template = payload["sqlTemplate"].as_str().unwrap_or_default();
    if sql_template.trim().is_empty() {
        return Err("sqlTemplate is empty".into());
    }
    let params_raw = payload["params"].as_str().unwrap_or("{}");
    let params: Value =
        serde_json::from_str(params_raw).map_err(|e| format!("invalid params json: {e}"))?;
    let safe_substitution = payload["safeSubstitution"].as_bool().unwrap_or(true);

    let mut ctx = RenderContext {
        params,
        bindings: Vec::new(),
        warnings: Vec::new(),
        safe_substitution,
    };

    let sql = if sql_template.contains('<') && sql_template.contains('>') {
        render_xml_template(sql_template, &mut ctx)?
    } else {
        let params_snapshot = ctx.params.clone();
        substitute_placeholders(sql_template, &params_snapshot, None, &mut ctx)?
    };
    let normalized = normalize_whitespace(&sql);

    Ok(json!({
        "sql": normalized,
        "bindings": ctx.bindings,
        "warnings": ctx.warnings,
    }))
}

fn lint(payload: &Value) -> Result<Value, String> {
    let sql_template = payload["sqlTemplate"].as_str().unwrap_or_default();
    if sql_template.trim().is_empty() {
        return Err("sqlTemplate is empty".into());
    }
    let mut issues = Vec::new();
    let mut stack = Vec::new();
    let tag_re = Regex::new(r"</?([a-zA-Z][\w-]*)\b[^>]*>").map_err(|e| e.to_string())?;

    for m in tag_re.captures_iter(sql_template) {
        let full = m.get(0).map(|v| v.as_str()).unwrap_or("");
        let name = m.get(1).map(|v| v.as_str()).unwrap_or("");
        let self_closed = full.ends_with("/>");
        if self_closed {
            continue;
        }
        if full.starts_with("</") {
            if let Some(last) = stack.pop() {
                if last != name {
                    issues.push(json!({
                        "level": "error",
                        "message": format!("tag mismatch: expected </{last}> but got </{name}>")
                    }));
                }
            } else {
                issues.push(json!({
                    "level": "error",
                    "message": format!("unexpected closing tag </{name}>")
                }));
            }
        } else {
            stack.push(name.to_string());
        }
    }

    if !stack.is_empty() {
        for name in stack {
            issues.push(json!({
                "level": "error",
                "message": format!("unclosed tag <{name}>")
            }));
        }
    }

    if sql_template.contains("${") {
        issues.push(json!({
            "level": "warn",
            "message": "`${}` may lead to SQL injection, ensure value is sanitized."
        }));
    }

    Ok(json!({ "issues": issues }))
}

fn render_xml_template(template: &str, ctx: &mut RenderContext) -> Result<String, String> {
    let wrapped = if template.trim_start().starts_with('<') {
        if template.contains("<script") {
            template.to_string()
        } else {
            format!("<script>{template}</script>")
        }
    } else {
        format!("<script>{template}</script>")
    };
    let doc = Document::parse(&wrapped).map_err(|e| format!("invalid mybatis xml: {e}"))?;
    let root = doc.root_element();
    render_children(root, ctx, None)
}

fn render_children(
    node: Node<'_, '_>,
    ctx: &mut RenderContext,
    local_scope: Option<&Value>,
) -> Result<String, String> {
    let mut out = String::new();
    for child in node.children() {
        if child.is_text() {
            let t = child.text().unwrap_or_default();
            let params_snapshot = ctx.params.clone();
            let rendered = substitute_placeholders(t, &params_snapshot, local_scope, ctx)?;
            out.push_str(&rendered);
            continue;
        }
        if !child.is_element() {
            continue;
        }
        let name = child.tag_name().name();
        let part = match name {
            "if" => {
                let test = child.attribute("test").unwrap_or_default();
                if eval_test(test, &ctx.params, local_scope) {
                    render_children(child, ctx, local_scope)?
                } else {
                    String::new()
                }
            }
            "where" => {
                let body = normalize_whitespace(&render_children(child, ctx, local_scope)?);
                let body = trim_leading_logic(&body);
                if body.is_empty() {
                    String::new()
                } else {
                    format!(" WHERE {body}")
                }
            }
            "set" => {
                let body = normalize_whitespace(&render_children(child, ctx, local_scope)?);
                let body = body.trim().trim_end_matches(',').trim().to_string();
                if body.is_empty() {
                    String::new()
                } else {
                    format!(" SET {body}")
                }
            }
            "trim" => {
                let mut body = normalize_whitespace(&render_children(child, ctx, local_scope)?);
                let prefix = child.attribute("prefix").unwrap_or_default();
                let suffix = child.attribute("suffix").unwrap_or_default();
                let prefix_overrides = child.attribute("prefixOverrides").unwrap_or_default();
                let suffix_overrides = child.attribute("suffixOverrides").unwrap_or_default();
                body = apply_overrides(body, prefix_overrides, suffix_overrides);
                if body.is_empty() {
                    String::new()
                } else {
                    format!(" {prefix}{body}{suffix}")
                }
            }
            "foreach" => render_foreach(child, ctx, local_scope)?,
            "choose" => render_choose(child, ctx, local_scope)?,
            "when" | "otherwise" => String::new(),
            "script" | "select" | "update" | "delete" | "insert" => {
                render_children(child, ctx, local_scope)?
            }
            _ => render_children(child, ctx, local_scope)?,
        };
        out.push_str(&part);
    }
    Ok(out)
}

fn render_choose(
    node: Node<'_, '_>,
    ctx: &mut RenderContext,
    local_scope: Option<&Value>,
) -> Result<String, String> {
    let mut otherwise = String::new();
    for child in node.children().filter(|n| n.is_element()) {
        let name = child.tag_name().name();
        if name == "when" {
            let test = child.attribute("test").unwrap_or_default();
            if eval_test(test, &ctx.params, local_scope) {
                return render_children(child, ctx, local_scope);
            }
        } else if name == "otherwise" {
            otherwise = render_children(child, ctx, local_scope)?;
        }
    }
    Ok(otherwise)
}

fn render_foreach(
    node: Node<'_, '_>,
    ctx: &mut RenderContext,
    local_scope: Option<&Value>,
) -> Result<String, String> {
    let collection_expr = node.attribute("collection").unwrap_or_default();
    let open = node.attribute("open").unwrap_or_default();
    let close = node.attribute("close").unwrap_or_default();
    let separator = node.attribute("separator").unwrap_or(",");
    let item_name = node.attribute("item").unwrap_or("item");
    let coll = resolve_path(collection_expr, &ctx.params, local_scope);
    let arr = coll.and_then(Value::as_array).cloned().unwrap_or_default();
    if arr.is_empty() {
        return Ok(String::new());
    }
    let mut parts = Vec::new();
    for item in arr {
        let local = json!({ item_name: item });
        let rendered = normalize_whitespace(&render_children(node, ctx, Some(&local))?);
        if !rendered.is_empty() {
            parts.push(rendered);
        }
    }
    if parts.is_empty() {
        return Ok(String::new());
    }
    Ok(format!("{open}{}{close}", parts.join(separator)))
}

fn substitute_placeholders(
    input: &str,
    params: &Value,
    local_scope: Option<&Value>,
    ctx: &mut RenderContext,
) -> Result<String, String> {
    let hash_re = Regex::new(r"#\{\s*([a-zA-Z0-9_.$]+)\s*\}").map_err(|e| e.to_string())?;
    let mut out = hash_re
        .replace_all(input, |caps: &regex::Captures<'_>| {
            let path = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            let v = resolve_path(path, params, local_scope).cloned().unwrap_or(Value::Null);
            ctx.bindings.push(json!({
                "name": path,
                "value": v,
                "mode": "#{}"
            }));
            sql_literal(&v)
        })
        .to_string();

    let dollar_re = Regex::new(r"\$\{\s*([a-zA-Z0-9_.$]+)\s*\}").map_err(|e| e.to_string())?;
    out = dollar_re
        .replace_all(&out, |caps: &regex::Captures<'_>| {
            let path = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            let v = resolve_path(path, params, local_scope).cloned().unwrap_or(Value::Null);
            let raw = value_to_raw_string(&v);
            if ctx.safe_substitution && looks_unsafe(&raw) {
                ctx.warnings
                    .push(format!("unsafe `${{{path}}}` content blocked"));
                "/*blocked*/".to_string()
            } else {
                raw
            }
        })
        .to_string();
    Ok(out)
}

fn looks_unsafe(raw: &str) -> bool {
    let s = raw.to_ascii_lowercase();
    s.contains(';') || s.contains("--") || s.contains("/*") || s.contains("*/")
}

fn value_to_raw_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => {
            if *b {
                "true".into()
            } else {
                "false".into()
            }
        }
        Value::Number(n) => n.to_string(),
        _ => v.to_string(),
    }
}

fn sql_literal(v: &Value) -> String {
    match v {
        Value::Null => "NULL".into(),
        Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        Value::Bool(b) => {
            if *b {
                "1".into()
            } else {
                "0".into()
            }
        }
        Value::Number(n) => n.to_string(),
        Value::Array(_) | Value::Object(_) => format!("'{}'", v.to_string().replace('\'', "''")),
    }
}

fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn trim_leading_logic(input: &str) -> String {
    let s = input.trim_start();
    for p in ["AND ", "OR ", "and ", "or "] {
        if let Some(rest) = s.strip_prefix(p) {
            return rest.trim_start().to_string();
        }
    }
    s.to_string()
}

fn apply_overrides(mut body: String, prefix_overrides: &str, suffix_overrides: &str) -> String {
    if !prefix_overrides.trim().is_empty() {
        for token in prefix_overrides.split('|') {
            let t = token.trim();
            if t.is_empty() {
                continue;
            }
            if body.to_ascii_uppercase().starts_with(&t.to_ascii_uppercase()) {
                body = body[t.len()..].trim_start().to_string();
                break;
            }
        }
    }
    if !suffix_overrides.trim().is_empty() {
        for token in suffix_overrides.split('|') {
            let t = token.trim();
            if t.is_empty() {
                continue;
            }
            if body.to_ascii_uppercase().ends_with(&t.to_ascii_uppercase()) && body.len() >= t.len()
            {
                body = body[..body.len() - t.len()].trim_end().to_string();
                break;
            }
        }
    }
    body
}

fn eval_test(test: &str, params: &Value, local_scope: Option<&Value>) -> bool {
    let expr = test.trim();
    if expr.is_empty() {
        return false;
    }
    if expr.contains(" or ") {
        return expr
            .split(" or ")
            .any(|part| eval_test(part.trim(), params, local_scope));
    }
    if expr.contains(" and ") {
        return expr
            .split(" and ")
            .all(|part| eval_test(part.trim(), params, local_scope));
    }
    if let Some(rest) = expr.strip_prefix('!') {
        return !eval_test(rest.trim(), params, local_scope);
    }

    for op in ["==", "!=", ">=", "<=", ">", "<"] {
        if let Some((left, right)) = split_once(expr, op) {
            let l = resolve_expr_value(left.trim(), params, local_scope);
            let r = parse_literal_or_path(right.trim(), params, local_scope);
            return compare_values(&l, &r, op);
        }
    }

    let v = resolve_expr_value(expr, params, local_scope);
    truthy(&v)
}

fn split_once<'a>(s: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    let idx = s.find(op)?;
    Some((&s[..idx], &s[idx + op.len()..]))
}

fn parse_literal_or_path(expr: &str, params: &Value, local_scope: Option<&Value>) -> Value {
    let s = expr.trim();
    if s.eq_ignore_ascii_case("null") {
        return Value::Null;
    }
    if s.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if s.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        return Value::String(s[1..s.len() - 1].to_string());
    }
    if let Ok(i) = s.parse::<i64>() {
        return json!(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return json!(f);
    }
    resolve_expr_value(s, params, local_scope)
}

fn resolve_expr_value(expr: &str, params: &Value, local_scope: Option<&Value>) -> Value {
    resolve_path(expr, params, local_scope)
        .cloned()
        .unwrap_or(Value::Null)
}

fn resolve_path<'a>(path: &str, params: &'a Value, local_scope: Option<&'a Value>) -> Option<&'a Value> {
    let p = path.trim();
    if p.is_empty() {
        return None;
    }
    let segs = p.split('.').collect::<Vec<_>>();
    if let Some(local) = local_scope {
        if let Some(v) = resolve_segments(local, &segs) {
            return Some(v);
        }
    }
    resolve_segments(params, &segs)
}

fn resolve_segments<'a>(root: &'a Value, segs: &[&str]) -> Option<&'a Value> {
    let mut current = root;
    for seg in segs {
        current = current.get(*seg)?;
    }
    Some(current)
}

fn compare_values(left: &Value, right: &Value, op: &str) -> bool {
    match op {
        "==" => left == right,
        "!=" => left != right,
        ">" | "<" | ">=" | "<=" => {
            let ln = to_f64(left);
            let rn = to_f64(right);
            if let (Some(a), Some(b)) = (ln, rn) {
                match op {
                    ">" => a > b,
                    "<" => a < b,
                    ">=" => a >= b,
                    "<=" => a <= b,
                    _ => false,
                }
            } else {
                let ls = value_to_cmp_string(left);
                let rs = value_to_cmp_string(right);
                match op {
                    ">" => ls > rs,
                    "<" => ls < rs,
                    ">=" => ls >= rs,
                    "<=" => ls <= rs,
                    _ => false,
                }
            }
        }
        _ => false,
    }
}

fn to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

fn value_to_cmp_string(v: &Value) -> Cow<'_, str> {
    match v {
        Value::String(s) => Cow::Borrowed(s.as_str()),
        _ => Cow::Owned(v.to_string()),
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_i64().unwrap_or(0) != 0 || n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(arr) => !arr.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}
