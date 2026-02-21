use rand::Rng;
use serde_json::{json, Value};
use uuid::Uuid;

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "uuid" => Ok(json!(Uuid::new_v4().to_string())),
        "guid" => Ok(json!(format!("{{{}}}", Uuid::new_v4().to_string().to_uppercase()))),
        "password" => {
            let length = payload["length"].as_u64().unwrap_or(16) as usize;
            let uppercase = payload["uppercase"].as_bool().unwrap_or(true);
            let lowercase = payload["lowercase"].as_bool().unwrap_or(true);
            let numbers = payload["numbers"].as_bool().unwrap_or(true);
            let symbols = payload["symbols"].as_bool().unwrap_or(false);
            let mut chars = String::new();
            if uppercase {
                chars.push_str("ABCDEFGHIJKLMNOPQRSTUVWXYZ");
            }
            if lowercase {
                chars.push_str("abcdefghijklmnopqrstuvwxyz");
            }
            if numbers {
                chars.push_str("0123456789");
            }
            if symbols {
                chars.push_str("!@#$%^&*()-_=+[]{};:,.<>?");
            }
            if chars.is_empty() {
                return Err("password charset is empty".into());
            }
            let mut rng = rand::thread_rng();
            let bytes = chars.as_bytes();
            let out = (0..length)
                .map(|_| bytes[rng.gen_range(0..bytes.len())] as char)
                .collect::<String>();
            Ok(json!(out))
        }
        _ => Err(format!("unsupported gen action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn uuid_and_guid_should_match_expected_shapes() {
        let uuid = execute("uuid", &json!({})).expect("uuid");
        let s = uuid.as_str().unwrap_or_default();
        assert_eq!(s.len(), 36);
        assert!(s.chars().filter(|c| *c == '-').count() == 4);

        let guid = execute("guid", &json!({})).expect("guid");
        let g = guid.as_str().unwrap_or_default();
        assert!(g.starts_with('{') && g.ends_with('}'));
    }

    #[test]
    fn password_should_obey_length_and_charset() {
        let out = execute(
            "password",
            &json!({
                "length": 24,
                "uppercase": false,
                "lowercase": true,
                "numbers": true,
                "symbols": false
            }),
        )
        .expect("password");
        let s = out.as_str().unwrap_or_default();
        assert_eq!(s.len(), 24);
        assert!(s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    }

    #[test]
    fn password_empty_charset_should_fail() {
        let err = execute(
            "password",
            &json!({
                "uppercase": false,
                "lowercase": false,
                "numbers": false,
                "symbols": false
            }),
        )
        .expect_err("must fail");
        assert!(err.contains("password charset is empty"));
    }
}
