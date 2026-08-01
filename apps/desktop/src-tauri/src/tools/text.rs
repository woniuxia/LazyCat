use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::Instant;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
enum TextOperationType {
    Trim,
    RemoveEmpty,
    Dedupe,
    Sort,
    IncludeFilter,
    ExcludeFilter,
    Replace,
    AddPrefix,
    AddSuffix,
    ExtractColumn,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
enum MatchMode {
    Contains,
    Equals,
    Regex,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TextOperation {
    #[serde(rename = "type")]
    op_type: TextOperationType,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    case_sensitive: Option<bool>,
    #[serde(default)]
    pattern: Option<String>,
    #[serde(default)]
    replacement: Option<String>,
    #[serde(default)]
    match_mode: Option<MatchMode>,
    #[serde(default)]
    sort_order: Option<SortOrder>,
    #[serde(default)]
    delimiter: Option<String>,
    #[serde(default)]
    column_index: Option<usize>,
    #[serde(default)]
    keep_unmatched: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProcessRequest {
    #[serde(default)]
    input: String,
    #[serde(default = "default_line_ending")]
    line_ending: String,
    #[serde(default)]
    operations: Vec<TextOperation>,
    #[serde(default = "default_preview_limit")]
    preview_limit: usize,
}

fn default_line_ending() -> String {
    "keep".to_string()
}

fn default_preview_limit() -> usize {
    200
}

fn split_identifier(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for ch in s.chars() {
        if ch == '_' || ch == '-' || ch == '.' || ch == ' ' {
            if !current.is_empty() {
                words.push(current.to_lowercase());
                current.clear();
            }
        } else if ch.is_uppercase()
            && !current.is_empty()
            && current.chars().last().map_or(false, |c| c.is_lowercase())
        {
            words.push(current.to_lowercase());
            current.clear();
            current.push(ch);
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        words.push(current.to_lowercase());
    }
    words
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
    }
}

fn naming_convert(payload: &Value) -> Result<Value, String> {
    let input = payload["input"]
        .as_str()
        .ok_or_else(|| "missing input".to_string())?;

    let mut camel_lines = Vec::new();
    let mut pascal_lines = Vec::new();
    let mut snake_lines = Vec::new();
    let mut screaming_lines = Vec::new();
    let mut kebab_lines = Vec::new();
    let mut dot_lines = Vec::new();

    for line in input.split('\n') {
        let words = split_identifier(line);
        if words.is_empty() {
            camel_lines.push(String::new());
            pascal_lines.push(String::new());
            snake_lines.push(String::new());
            screaming_lines.push(String::new());
            kebab_lines.push(String::new());
            dot_lines.push(String::new());
            continue;
        }

        // camelCase
        let camel: String = words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                if i == 0 {
                    w.to_lowercase()
                } else {
                    capitalize(w)
                }
            })
            .collect();
        camel_lines.push(camel);

        // PascalCase
        let pascal: String = words.iter().map(|w| capitalize(w)).collect();
        pascal_lines.push(pascal);

        // snake_case
        snake_lines.push(words.join("_"));

        // SCREAMING_SNAKE_CASE
        screaming_lines.push(words.join("_").to_uppercase());

        // kebab-case
        kebab_lines.push(words.join("-"));

        // dot.case
        dot_lines.push(words.join("."));
    }

    Ok(json!({
        "camelCase": camel_lines.join("\n"),
        "pascalCase": pascal_lines.join("\n"),
        "snakeCase": snake_lines.join("\n"),
        "screamingSnake": screaming_lines.join("\n"),
        "kebabCase": kebab_lines.join("\n"),
        "dotCase": dot_lines.join("\n"),
    }))
}

const ACTIONS: &[&str] = &["process", "presets", "naming_convert"];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported text action: {action}"));
    }
    match action {
        "process" => process_text(payload),
        "presets" => Ok(json!(builtin_presets())),
        "naming_convert" => naming_convert(payload),
        _ => Err(format!("unsupported text action: {action}")),
    }
}

fn process_text(payload: &Value) -> Result<Value, String> {
    let request: ProcessRequest =
        serde_json::from_value(payload.clone()).map_err(|e| format!("无效请求参数: {e}"))?;

    let started_at = Instant::now();
    let mut warnings = Vec::new();
    let input_lines = split_lines(&request.input);
    let input_lines_count = input_lines.len();
    let input_chars = request.input.chars().count();

    let mut processed_lines = input_lines.clone();
    for op in request.operations.iter().filter(|op| op.enabled) {
        processed_lines = apply_operation(processed_lines, op, &mut warnings)?;
    }

    let output = apply_line_ending(
        &processed_lines.join("\n"),
        &request.line_ending,
        detect_input_line_ending(&request.input),
    );
    let output_lines = split_lines(&output);
    let output_lines_count = output_lines.len();
    let output_chars = output.chars().count();
    let changed_lines = diff_line_count(&input_lines, &output_lines);
    let preview_limit = request.preview_limit.max(1);
    let samples = build_preview_samples(&input_lines, &output_lines, preview_limit);
    let duration_ms = started_at.elapsed().as_millis() as u64;

    Ok(json!({
        "output": output,
        "stats": {
            "inputLines": input_lines_count,
            "outputLines": output_lines_count,
            "changedLines": changed_lines,
            "inputChars": input_chars,
            "outputChars": output_chars,
            "durationMs": duration_ms,
            "charsWithSpaces": request.input.chars().count(),
            "charsNoSpaces": request.input.chars().filter(|c| !c.is_whitespace()).count(),
            "chineseChars": request.input.chars().filter(|c| ('一'..='鿿').contains(c)).count(),
            "englishWords": request.input.split_whitespace().filter(|w| w.chars().any(|c| c.is_ascii_alphabetic())).count(),
            "bytesUtf8": request.input.len(),
            "longestLine": request.input.lines().map(|l| l.chars().count()).max().unwrap_or(0),
        },
        "preview": {
            "changed": changed_lines,
            "samples": samples,
        },
        "warnings": warnings,
    }))
}

fn apply_operation(
    lines: Vec<String>,
    op: &TextOperation,
    warnings: &mut Vec<String>,
) -> Result<Vec<String>, String> {
    match op.op_type {
        TextOperationType::Trim => Ok(lines
            .into_iter()
            .map(|line| line.trim().to_string())
            .collect()),
        TextOperationType::RemoveEmpty => Ok(lines
            .into_iter()
            .filter(|line| !line.trim().is_empty())
            .collect()),
        TextOperationType::Dedupe => {
            let case_sensitive = op.case_sensitive.unwrap_or(false);
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for line in lines {
                let key = if case_sensitive {
                    line.clone()
                } else {
                    line.to_lowercase()
                };
                if seen.insert(key) {
                    out.push(line);
                }
            }
            Ok(out)
        }
        TextOperationType::Sort => {
            let case_sensitive = op.case_sensitive.unwrap_or(false);
            let order = op.sort_order.clone().unwrap_or(SortOrder::Asc);
            let mut out = lines;
            if case_sensitive {
                out.sort();
            } else {
                out.sort_by_cached_key(|line| line.to_lowercase());
            }
            if matches!(order, SortOrder::Desc) {
                out.reverse();
            }
            Ok(out)
        }
        TextOperationType::IncludeFilter => filter_lines(lines, op, true),
        TextOperationType::ExcludeFilter => filter_lines(lines, op, false),
        TextOperationType::Replace => replace_lines(lines, op),
        TextOperationType::AddPrefix => {
            let prefix = op.pattern.clone().unwrap_or_default();
            Ok(lines
                .into_iter()
                .map(|line| format!("{prefix}{line}"))
                .collect())
        }
        TextOperationType::AddSuffix => {
            let suffix = op.pattern.clone().unwrap_or_default();
            Ok(lines
                .into_iter()
                .map(|line| format!("{line}{suffix}"))
                .collect())
        }
        TextOperationType::ExtractColumn => extract_column(lines, op, warnings),
    }
}

fn filter_lines(
    lines: Vec<String>,
    op: &TextOperation,
    include_mode: bool,
) -> Result<Vec<String>, String> {
    let pattern = op.pattern.clone().unwrap_or_default();
    if pattern.is_empty() {
        return Err("过滤规则缺少 pattern 参数".to_string());
    }
    let match_mode = op.match_mode.clone().unwrap_or(MatchMode::Contains);
    let case_sensitive = op.case_sensitive.unwrap_or(false);
    let regex = if matches!(match_mode, MatchMode::Regex) {
        Some(Regex::new(&pattern).map_err(|e| format!("过滤正则无效: {e}"))?)
    } else {
        None
    };
    let normalized_pattern = if case_sensitive {
        pattern.clone()
    } else {
        pattern.to_lowercase()
    };

    let out = lines
        .into_iter()
        .filter(|line| {
            let is_match = match match_mode {
                MatchMode::Contains => {
                    if case_sensitive {
                        line.contains(&pattern)
                    } else {
                        line.to_lowercase().contains(&normalized_pattern)
                    }
                }
                MatchMode::Equals => {
                    if case_sensitive {
                        line == &pattern
                    } else {
                        line.to_lowercase() == normalized_pattern
                    }
                }
                MatchMode::Regex => regex.as_ref().is_some_and(|re| re.is_match(line)),
            };

            if include_mode {
                is_match
            } else {
                !is_match
            }
        })
        .collect();
    Ok(out)
}

fn replace_lines(lines: Vec<String>, op: &TextOperation) -> Result<Vec<String>, String> {
    let pattern = op.pattern.clone().unwrap_or_default();
    if pattern.is_empty() {
        return Err("替换规则缺少 pattern 参数".to_string());
    }
    let replacement = op.replacement.clone().unwrap_or_default();
    let match_mode = op.match_mode.clone().unwrap_or(MatchMode::Contains);
    let case_sensitive = op.case_sensitive.unwrap_or(false);

    match match_mode {
        MatchMode::Regex => {
            let re = Regex::new(&pattern).map_err(|e| format!("替换正则无效: {e}"))?;
            Ok(lines
                .into_iter()
                .map(|line| re.replace_all(&line, replacement.as_str()).to_string())
                .collect())
        }
        MatchMode::Contains => {
            if case_sensitive {
                Ok(lines
                    .into_iter()
                    .map(|line| line.replace(&pattern, &replacement))
                    .collect())
            } else {
                let re = Regex::new(&format!("(?i){}", regex::escape(&pattern)))
                    .map_err(|e| format!("替换规则构建失败: {e}"))?;
                Ok(lines
                    .into_iter()
                    .map(|line| re.replace_all(&line, replacement.as_str()).to_string())
                    .collect())
            }
        }
        MatchMode::Equals => {
            let normalized_pattern = pattern.to_lowercase();
            Ok(lines
                .into_iter()
                .map(|line| {
                    let matched = if case_sensitive {
                        line == pattern
                    } else {
                        line.to_lowercase() == normalized_pattern
                    };
                    if matched {
                        replacement.clone()
                    } else {
                        line
                    }
                })
                .collect())
        }
    }
}

fn extract_column(
    lines: Vec<String>,
    op: &TextOperation,
    warnings: &mut Vec<String>,
) -> Result<Vec<String>, String> {
    let delimiter = op.delimiter.clone().unwrap_or_default();
    if delimiter.is_empty() {
        return Err("提取列规则缺少 delimiter 参数".to_string());
    }
    let column_index = op.column_index.unwrap_or(1);
    if column_index == 0 {
        return Err("columnIndex 必须从 1 开始".to_string());
    }
    let keep_unmatched = op.keep_unmatched.unwrap_or(false);
    let target_idx = column_index - 1;
    let mut unmatched = 0usize;
    let mut out = Vec::new();

    for line in lines {
        let fields: Vec<&str> = line.split(&delimiter).collect();
        if let Some(value) = fields.get(target_idx) {
            out.push((*value).to_string());
        } else if keep_unmatched {
            out.push(line);
            unmatched += 1;
        } else {
            unmatched += 1;
        }
    }

    if unmatched > 0 {
        warnings.push(format!(
            "提取列时有 {unmatched} 行未命中第 {column_index} 列。"
        ));
    }
    Ok(out)
}

fn split_lines(input: &str) -> Vec<String> {
    if input.is_empty() {
        return Vec::new();
    }
    input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|line| line.to_string())
        .collect()
}

fn detect_input_line_ending(input: &str) -> &'static str {
    if input.contains("\r\n") {
        "crlf"
    } else if input.contains('\n') {
        "lf"
    } else if input.contains('\r') {
        "crlf"
    } else {
        "lf"
    }
}

fn apply_line_ending(text: &str, line_ending: &str, input_style: &str) -> String {
    let target = match line_ending {
        "lf" => "lf",
        "crlf" => "crlf",
        "keep" => input_style,
        _ => "lf",
    };

    if target == "crlf" {
        text.replace('\n', "\r\n")
    } else {
        text.to_string()
    }
}

fn diff_line_count(before: &[String], after: &[String]) -> usize {
    let max_len = before.len().max(after.len());
    let mut changed = 0usize;
    for index in 0..max_len {
        let b = before.get(index).map(String::as_str).unwrap_or("");
        let a = after.get(index).map(String::as_str).unwrap_or("");
        if b != a {
            changed += 1;
        }
    }
    changed
}

fn build_preview_samples(before: &[String], after: &[String], limit: usize) -> Vec<Value> {
    let max_len = before.len().max(after.len());
    let mut samples = Vec::new();
    for index in 0..max_len {
        if samples.len() >= limit {
            break;
        }
        let b = before.get(index).cloned().unwrap_or_default();
        let a = after.get(index).cloned().unwrap_or_default();
        if b != a {
            samples.push(json!({
                "line": index + 1,
                "before": b,
                "after": a,
            }));
        }
    }
    samples
}

fn builtin_presets() -> Vec<Value> {
    vec![
        json!({
            "id": "log-cleanup",
            "name": "日志清洗",
            "description": "按行去空白、去空行、去重后排序",
            "operations": [
                { "type": "trim", "enabled": true },
                { "type": "remove_empty", "enabled": true },
                { "type": "dedupe", "enabled": true, "caseSensitive": false },
                { "type": "sort", "enabled": true, "caseSensitive": false, "sortOrder": "asc" }
            ]
        }),
        json!({
            "id": "config-kv-key",
            "name": "配置键提取",
            "description": "提取 key=value 的 key，并去重排序",
            "operations": [
                { "type": "trim", "enabled": true },
                { "type": "remove_empty", "enabled": true },
                { "type": "exclude_filter", "enabled": true, "pattern": "#", "matchMode": "contains", "caseSensitive": false },
                { "type": "extract_column", "enabled": true, "delimiter": "=", "columnIndex": 1, "keepUnmatched": false },
                { "type": "dedupe", "enabled": true, "caseSensitive": false },
                { "type": "sort", "enabled": true, "caseSensitive": false, "sortOrder": "asc" }
            ]
        }),
        json!({
            "id": "error-lines",
            "name": "错误日志提取",
            "description": "仅保留包含 error 的行",
            "operations": [
                { "type": "trim", "enabled": true },
                { "type": "remove_empty", "enabled": true },
                { "type": "include_filter", "enabled": true, "pattern": "error", "matchMode": "contains", "caseSensitive": false }
            ]
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process_for_test(input: &str, operations: Vec<Value>) -> Value {
        process_text(&json!({
            "input": input,
            "lineEnding": "lf",
            "operations": operations,
            "previewLimit": 20
        }))
        .expect("process should succeed")
    }

    #[test]
    fn test_dedupe_keeps_first_occurrence_order() {
        let data = process_for_test(
            "b\nA\na\nb\nA",
            vec![json!({ "type": "dedupe", "enabled": true, "caseSensitive": false })],
        );
        assert_eq!(data["output"], "b\nA");
    }

    #[test]
    fn test_sort_desc_case_sensitive() {
        let data = process_for_test(
            "a\nB\nc",
            vec![json!({
                "type": "sort",
                "enabled": true,
                "caseSensitive": true,
                "sortOrder": "desc"
            })],
        );
        assert_eq!(data["output"], "c\na\nB");
    }

    #[test]
    fn test_include_filter_regex() {
        let data = process_for_test(
            "order-1\nfoo\norder-22",
            vec![json!({
                "type": "include_filter",
                "enabled": true,
                "pattern": "^order-\\d+$",
                "matchMode": "regex"
            })],
        );
        assert_eq!(data["output"], "order-1\norder-22");
    }

    #[test]
    fn test_replace_regex() {
        let data = process_for_test(
            "id=12\nid=98",
            vec![json!({
                "type": "replace",
                "enabled": true,
                "pattern": "\\d+",
                "replacement": "X",
                "matchMode": "regex"
            })],
        );
        assert_eq!(data["output"], "id=X\nid=X");
    }

    #[test]
    fn test_extract_column_and_keep_unmatched() {
        let data = process_for_test(
            "k1=v1\ninvalid\nk2=v2",
            vec![json!({
                "type": "extract_column",
                "enabled": true,
                "delimiter": "=",
                "columnIndex": 2,
                "keepUnmatched": true
            })],
        );
        assert_eq!(data["output"], "v1\ninvalid\nv2");
        assert_eq!(data["warnings"].as_array().map_or(0, |a| a.len()), 1);
    }

    #[test]
    fn test_pipeline_chain_trim_remove_empty_dedupe_sort() {
        let data = process_for_test(
            "  b \n\n a\nA \n",
            vec![
                json!({ "type": "trim", "enabled": true }),
                json!({ "type": "remove_empty", "enabled": true }),
                json!({ "type": "dedupe", "enabled": true, "caseSensitive": false }),
                json!({ "type": "sort", "enabled": true, "caseSensitive": false, "sortOrder": "asc" }),
            ],
        );
        assert_eq!(data["output"], "a\nb");
    }

    #[test]
    fn test_unicode_processing() {
        let data = process_for_test(
            "错误\n error \n错误",
            vec![
                json!({ "type": "trim", "enabled": true }),
                json!({ "type": "include_filter", "enabled": true, "pattern": "错误", "matchMode": "contains", "caseSensitive": true }),
                json!({ "type": "dedupe", "enabled": true, "caseSensitive": true }),
            ],
        );
        assert_eq!(data["output"], "错误");
    }

    #[test]
    fn naming_convert_camel() {
        let r = execute("naming_convert", &json!({"input": "hello_world"})).unwrap();
        assert_eq!(r["camelCase"], "helloWorld");
        assert_eq!(r["pascalCase"], "HelloWorld");
        assert_eq!(r["snakeCase"], "hello_world");
        assert_eq!(r["screamingSnake"], "HELLO_WORLD");
        assert_eq!(r["kebabCase"], "hello-world");
        assert_eq!(r["dotCase"], "hello.world");
    }

    #[test]
    fn naming_convert_from_camel() {
        let r = execute("naming_convert", &json!({"input": "helloWorld"})).unwrap();
        assert_eq!(r["snakeCase"], "hello_world");
        assert_eq!(r["kebabCase"], "hello-world");
    }

    #[test]
    fn naming_convert_multiline() {
        let r = execute(
            "naming_convert",
            &json!({"input": "hello_world
foo_bar"}),
        )
        .unwrap();
        assert_eq!(
            r["camelCase"],
            "helloWorld
fooBar"
        );
    }

    #[test]
    fn process_returns_extended_stats() {
        let r = execute(
            "process",
            &json!({
                "input": "Hello 你好
            World",
                "ops": { "trim": false, "removeEmpty": false, "dedupe": false, "sort": false,
                         "includeFilter": false, "excludeFilter": false, "replace": false,
                         "addPrefix": false, "addSuffix": false, "extractColumn": false },
                "lineEnding": "keep"
            }),
        )
        .unwrap();
        let stats = &r["stats"];
        assert!(stats["charsWithSpaces"].as_u64().unwrap() > 0);
        assert!(stats["charsNoSpaces"].as_u64().unwrap() > 0);
        assert!(stats["chineseChars"].as_u64().unwrap() >= 2);
        assert!(stats["englishWords"].as_u64().unwrap() >= 2);
        assert!(stats["bytesUtf8"].as_u64().unwrap() > 0);
        assert!(stats["longestLine"].as_u64().unwrap() > 0);
    }
}
