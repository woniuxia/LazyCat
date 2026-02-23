use rand::Rng;
use serde_json::{json, Value};
use uuid::Uuid;

fn password_strength(payload: &Value) -> Result<Value, String> {
    let password = payload["password"]
        .as_str()
        .ok_or_else(|| "missing password".to_string())?;

    let len = password.len();
    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_ascii_alphanumeric());

    // Check for consecutive repeated characters (3+)
    let has_consecutive_repeat = {
        let chars: Vec<char> = password.chars().collect();
        let mut found = false;
        for i in 0..chars.len().saturating_sub(2) {
            if chars[i] == chars[i + 1] && chars[i + 1] == chars[i + 2] {
                found = true;
                break;
            }
        }
        found
    };

    // Check for keyboard sequential patterns (3+ consecutive characters)
    let keyboard_rows = [
        "qwertyuiop",
        "asdfghjkl",
        "zxcvbnm",
        "1234567890",
        "!@#$%^&*()",
    ];
    let has_keyboard_seq = {
        let lower = password.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();
        let mut found = false;
        // Check 3+ consecutive keyboard characters
        if chars.len() >= 3 {
            for i in 0..chars.len().saturating_sub(2) {
                let slice: String = chars[i..i + 3].iter().collect();
                let rev: String = slice.chars().rev().collect();
                for row in &keyboard_rows {
                    if row.contains(&slice) || row.contains(&rev) {
                        found = true;
                        break;
                    }
                }
                if found { break; }
            }
        }
        // Also check common sequential like abc, 123
        if !found && chars.len() >= 3 {
            for i in 0..chars.len().saturating_sub(2) {
                let c0 = chars[i] as i32;
                let c1 = chars[i + 1] as i32;
                let c2 = chars[i + 2] as i32;
                if (c1 - c0 == 1 && c2 - c1 == 1)
                    || (c0 - c1 == 1 && c1 - c2 == 1)
                {
                    found = true;
                    break;
                }
            }
        }
        found
    };

    // Score calculation (each 0-20, total 0-100)
    let length_score: u64 = if len < 6 {
        0
    } else if len < 10 {
        5
    } else if len <= 12 {
        10
    } else if len <= 16 {
        15
    } else {
        20
    };

    let case_score: u64 = if has_upper && has_lower { 20 } else { 0 };
    let digit_score: u64 = if has_digit { 20 } else { 0 };
    let special_score: u64 = if has_special { 20 } else { 0 };
    let repeat_score: u64 = if len >= 10 && !has_consecutive_repeat { 20 } else if len >= 10 { 0 } else { 0 };

    let mut score = length_score + case_score + digit_score + special_score + repeat_score;
    // Penalize keyboard sequences
    if has_keyboard_seq && score > 10 {
        score -= 10;
    }

    let level = if score < 30 {
        "weak"
    } else if score < 60 {
        "medium"
    } else if score < 80 {
        "strong"
    } else {
        "very_strong"
    };

    let details = json!([
        {
            "rule": "length",
            "passed": len >= 10,
            "message": if len >= 10 { format!("长度 {} 位，达标", len) } else { format!("长度仅 {} 位，建议至少 10 位", len) }
        },
        {
            "rule": "case_mix",
            "passed": has_upper && has_lower,
            "message": if has_upper && has_lower { "包含大小写字母" } else { "建议混合大小写字母" }
        },
        {
            "rule": "digit",
            "passed": has_digit,
            "message": if has_digit { "包含数字" } else { "建议包含数字" }
        },
        {
            "rule": "special",
            "passed": has_special,
            "message": if has_special { "包含特殊字符" } else { "建议包含特殊字符" }
        },
        {
            "rule": "no_repeat",
            "passed": len >= 10 && !has_consecutive_repeat,
            "message": if len < 10 { "密码过短，无法评估重复" } else if !has_consecutive_repeat { "无连续重复字符" } else { "存在连续重复字符(3+)" }
        },
        {
            "rule": "no_keyboard_seq",
            "passed": !has_keyboard_seq,
            "message": if !has_keyboard_seq { "无键盘连续字符" } else { "包含键盘连续字符(如 qwe/asd/123)，易被字典攻击" }
        }
    ]);

    Ok(json!({
        "score": score,
        "level": level,
        "details": details,
    }))
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "uuid" => Ok(json!(Uuid::new_v4().to_string())),
        "uuid_simple" => Ok(json!(Uuid::new_v4().to_string().replace('-', ""))),
        "guid" => Ok(json!(format!("{{{}}}", Uuid::new_v4().to_string().to_uppercase()))),
        "snowflake" => {
            // Simplified snowflake: 41-bit timestamp (ms since custom epoch) + 10-bit machine + 12-bit sequence
            use std::time::{SystemTime, UNIX_EPOCH};
            let epoch = 1_700_000_000_000u64; // custom epoch: 2023-11-14
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("系统时钟异常: {e}"))?
                .as_millis() as u64;
            let ts = (now - epoch) & 0x1FFFFFFFFFF; // 41 bits
            let mut rng = rand::thread_rng();
            let machine: u64 = rng.gen_range(0..1024); // 10 bits
            let seq: u64 = rng.gen_range(0..4096); // 12 bits
            let id = (ts << 22) | (machine << 12) | seq;
            Ok(json!(id.to_string()))
        }
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
        "password_strength" => password_strength(payload),
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

    #[test]
    fn password_strength_weak() {
        let r = execute("password_strength", &json!({"password": "123456"})).unwrap();
        assert_eq!(r["level"], "weak");
        assert!(r["score"].as_u64().unwrap() < 30);
    }

    #[test]
    fn password_strength_strong() {
        let r = execute("password_strength", &json!({"password": "Tr0ub4dor&3xY!"})).unwrap();
        let score = r["score"].as_u64().unwrap();
        assert!(score >= 70);
    }

    #[test]
    fn password_strength_details() {
        let r = execute("password_strength", &json!({"password": "abc"})).unwrap();
        assert!(r["details"].as_array().unwrap().len() > 0);
    }
}
