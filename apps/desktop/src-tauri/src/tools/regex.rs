use regex::Regex;
use serde_json::{json, Value};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "test" => {
            let pattern = payload["pattern"].as_str().unwrap_or_default();
            let flags = payload["flags"].as_str().unwrap_or("");
            let input = payload["input"].as_str().unwrap_or_default();
            let mut p = pattern.to_string();
            if flags.contains('i') {
                p = format!("(?i){pattern}");
            }
            let re = Regex::new(&p).map_err(|e| format!("regex invalid: {e}"))?;
            let mut out = Vec::new();
            for m in re.find_iter(input) {
                out.push(json!({
                    "index": m.start(),
                    "match": m.as_str(),
                    "groups": []
                }));
            }
            Ok(Value::Array(out))
        }
        "generate" => {
            let kind = payload["kind"].as_str().unwrap_or("email");
            let exp = match kind {
                "ipv4" => "^(25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)(\\.(25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)){3}$",
                "url" => "^(https?:\\/\\/)?([\\w-]+\\.)+[\\w-]+([\\w\\-./?%&=]*)?$",
                "phone-cn" => "^1[3-9]\\d{9}$",
                _ => "^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}$",
            };
            Ok(json!(exp))
        }
        "templates" => Ok(json!([
            {"name":"邮箱地址","category":"common","expression":"^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}$"},
            {"name":"IPv4 地址","category":"network","expression":"^(25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)(\\.(25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)){3}$"},
            {"name":"URL 链接","category":"common","expression":"^(https?:\\/\\/)?([\\w-]+\\.)+[\\w-]+([\\w\\-./?%&=]*)?$"},
            {"name":"中国手机号","category":"common","expression":"^1[3-9]\\d{9}$"}
        ])),
        _ => Err(format!("unsupported regex action: {action}")),
    }
}
