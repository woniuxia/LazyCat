use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};

const ACTIONS: &[&str] = &["decode"];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported jwt action: {action}"));
    }
    match action {
        "decode" => {
            let token = payload["token"].as_str().unwrap_or_default().trim();

            let parts: Vec<&str> = token.split('.').collect();
            if parts.len() != 3 {
                return Err("Invalid JWT: expected 3 parts separated by '.'".to_string());
            }

            let header_bytes = URL_SAFE_NO_PAD
                .decode(parts[0])
                .map_err(|e| format!("Failed to decode header: {e}"))?;
            let header: Value = serde_json::from_slice(&header_bytes)
                .map_err(|e| format!("Failed to parse header JSON: {e}"))?;

            let payload_bytes = URL_SAFE_NO_PAD
                .decode(parts[1])
                .map_err(|e| format!("Failed to decode payload: {e}"))?;
            let payload_val: Value = serde_json::from_slice(&payload_bytes)
                .map_err(|e| format!("Failed to parse payload JSON: {e}"))?;

            let signature = hex::encode(
                URL_SAFE_NO_PAD
                    .decode(parts[2])
                    .unwrap_or_else(|_| parts[2].as_bytes().to_vec()),
            );

            let mut result = json!({
                "header": header,
                "payload": payload_val,
                "signature": signature,
            });

            // Check exp claim
            if let Some(exp) = payload_val.get("exp").and_then(|v| v.as_i64()) {
                let now = chrono::Utc::now().timestamp();
                let expired = now > exp;
                let exp_dt = chrono::DateTime::from_timestamp(exp, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                    .unwrap_or_else(|| "invalid timestamp".to_string());

                result["expired"] = json!(expired);
                result["exp_readable"] = json!(exp_dt);
            }

            Ok(result)
        }
        _ => Err(format!("unsupported jwt action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn b64url_json(v: &Value) -> String {
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(v).expect("serialize"))
    }

    #[test]
    fn decode_valid_jwt_should_return_parts_and_exp_status() {
        let header = json!({"alg":"HS256","typ":"JWT"});
        let payload = json!({"sub":"u1","exp": chrono::Utc::now().timestamp() + 3600});
        let token = format!(
            "{}.{}.{}",
            b64url_json(&header),
            b64url_json(&payload),
            URL_SAFE_NO_PAD.encode("sig")
        );

        let out = execute("decode", &json!({ "token": token })).expect("decode");
        assert_eq!(out["header"]["alg"], "HS256");
        assert_eq!(out["payload"]["sub"], "u1");
        assert_eq!(out["signature"], "736967");
        assert_eq!(out["expired"], false);
        assert!(out["exp_readable"]
            .as_str()
            .unwrap_or_default()
            .contains("UTC"));
    }

    #[test]
    fn decode_invalid_token_should_fail() {
        let err = execute("decode", &json!({ "token": "a.b" })).expect_err("invalid parts");
        assert!(err.contains("expected 3 parts"));

        let err = execute("decode", &json!({ "token": "bad.abc.def" })).expect_err("bad header");
        assert!(
            err.contains("Failed to decode header") || err.contains("Failed to parse header JSON")
        );
    }
}
