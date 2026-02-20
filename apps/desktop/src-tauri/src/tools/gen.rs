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
