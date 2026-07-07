use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

pub static MANUAL_SERVERS: OnceLock<HashMap<String, u16>> = OnceLock::new();

const ACTIONS: &[&str] = &[
    "list",
];

pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, _payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported manuals action: {action}"));
    }
    match action {
        "list" => {
            let servers = MANUAL_SERVERS.get();
            let mut list = Vec::new();
            let known = [
                ("vue3", "Vue 3 开发手册", "/guide/introduction.html"),
                (
                    "element-plus",
                    "Element Plus 组件库",
                    "/zh-CN/component/overview",
                ),
                (
                    "mdn-js",
                    "MDN JavaScript 手册",
                    "/zh-CN/docs/Web/JavaScript/Guide/",
                ),
            ];
            for (id, name, home) in known {
                if let Some(port) = servers.and_then(|m| m.get(id)) {
                    list.push(json!({"id": id, "name": name, "url": format!("http://127.0.0.1:{port}{home}")}));
                }
            }
            Ok(json!(list))
        }
        _ => Err(format!("unsupported manuals action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn list_should_return_known_registered_manuals() {
        let mut map = HashMap::new();
        map.insert("vue3".to_string(), 12345);
        map.insert("mdn-js".to_string(), 12346);
        let _ = MANUAL_SERVERS.set(map);

        let out = execute("list", &json!({})).expect("list");
        let arr = out.as_array().cloned().unwrap_or_default();
        assert!(arr.iter().any(|v| v["id"] == "vue3"));
        assert!(arr.iter().any(|v| v["id"] == "mdn-js"));
    }
}
