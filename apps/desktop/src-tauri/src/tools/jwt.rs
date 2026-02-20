use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "decode" => {
            let token = payload["token"]
                .as_str()
                .unwrap_or_default()
                .trim();

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
