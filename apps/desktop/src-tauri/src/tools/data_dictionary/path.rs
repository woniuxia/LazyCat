use serde_json::Value;

pub(super) fn escape_path_segment(segment: &str) -> String {
    let mut out = String::new();
    for ch in segment.chars() {
        match ch {
            '\\' => out.push_str(r#"\\ "#.trim()),
            '.' => out.push_str(r#"\."#),
            _ => out.push(ch),
        }
    }
    out
}

pub(super) fn unescape_path_segment(segment: &str) -> String {
    let mut out = String::new();
    let mut escaped = false;
    for ch in segment.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            out.push(ch);
        }
    }
    if escaped {
        out.push('\\');
    }
    out
}

pub(super) fn default_display_name(field_path: &str) -> String {
    let mut current = String::new();
    let mut last = String::new();
    let mut escaped = false;
    for ch in field_path.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '.' => {
                last = current;
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    if escaped {
        current.push('\\');
    }
    if current.is_empty() {
        unescape_path_segment(&last)
    } else {
        current
    }
}

pub(super) fn get_value_by_field_path<'a>(
    source: &'a Value,
    field_path: &str,
) -> Option<&'a Value> {
    let mut current = source;
    for part in split_escaped_path(field_path) {
        current = current.as_object()?.get(&part)?;
    }
    Some(current)
}

fn split_escaped_path(field_path: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for ch in field_path.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '.' {
            parts.push(current);
            current = String::new();
            continue;
        }
        current.push(ch);
    }
    if escaped {
        current.push('\\');
    }
    parts.push(current);
    parts
}
