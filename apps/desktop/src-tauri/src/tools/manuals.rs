use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

pub static MANUAL_SERVERS: OnceLock<HashMap<String, u16>> = OnceLock::new();

pub fn execute(action: &str, _payload: &Value) -> Result<Value, String> {
    match action {
        "list" => {
            let servers = MANUAL_SERVERS.get();
            let mut list = Vec::new();
            let known = [
                ("vue3",         "Vue 3 开发手册",       "/guide/introduction.html"),
                ("element-plus", "Element Plus 组件库",  "/zh-CN/component/overview"),
                ("mdn-js",       "MDN JavaScript 手册",  "/zh-CN/docs/Web/JavaScript/"),
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
