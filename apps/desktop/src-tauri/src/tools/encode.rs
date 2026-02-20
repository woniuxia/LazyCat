use base64::{engine::general_purpose::{STANDARD as BASE64, URL_SAFE_NO_PAD as BASE64URL}, Engine};
use image::ImageFormat;
use qrcode::QrCode;
use serde_json::{json, Value};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "base64_encode" => {
            let input = payload["input"].as_str().unwrap_or_default();
            Ok(json!(BASE64.encode(input.as_bytes())))
        }
        "base64_decode" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let decoded = BASE64
                .decode(input)
                .map_err(|e| format!("base64 decode failed: {e}"))?;
            Ok(json!(String::from_utf8_lossy(&decoded).to_string()))
        }
        "base64_url_encode" => {
            let input = payload["input"].as_str().unwrap_or_default();
            Ok(json!(BASE64URL.encode(input.as_bytes())))
        }
        "base64_url_decode" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let decoded = BASE64URL
                .decode(input)
                .map_err(|e| format!("base64url decode failed: {e}"))?;
            Ok(json!(String::from_utf8_lossy(&decoded).to_string()))
        }
        "url_encode" => {
            let input = payload["input"].as_str().unwrap_or_default();
            Ok(json!(urlencoding::encode(input).to_string()))
        }
        "url_decode" => {
            let input = payload["input"].as_str().unwrap_or_default();
            Ok(json!(
                urlencoding::decode(input)
                    .map_err(|e| format!("url decode failed: {e}"))?
                    .to_string()
            ))
        }
        "md5" => {
            let input = payload["input"].as_str().unwrap_or_default();
            Ok(json!(format!("{:x}", md5::compute(input.as_bytes()))))
        }
        "qr_generate" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let code = QrCode::new(input.as_bytes()).map_err(|e| format!("qr generation failed: {e}"))?;
            let image = code.render::<image::Luma<u8>>().build();
            let mut cursor = std::io::Cursor::new(Vec::new());
            image
                .write_to(&mut cursor, ImageFormat::Png)
                .map_err(|e| format!("png encode failed: {e}"))?;
            Ok(json!(format!(
                "data:image/png;base64,{}",
                BASE64.encode(cursor.into_inner())
            )))
        }
        "sha1" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let digest = openssl::hash::hash(openssl::hash::MessageDigest::sha1(), input.as_bytes())
                .map_err(|e| format!("sha1 failed: {e}"))?;
            Ok(json!(hex::encode(digest)))
        }
        "sha256" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let digest = openssl::hash::hash(openssl::hash::MessageDigest::sha256(), input.as_bytes())
                .map_err(|e| format!("sha256 failed: {e}"))?;
            Ok(json!(hex::encode(digest)))
        }
        "sha512" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let digest = openssl::hash::hash(openssl::hash::MessageDigest::sha512(), input.as_bytes())
                .map_err(|e| format!("sha512 failed: {e}"))?;
            Ok(json!(hex::encode(digest)))
        }
        "hmac_sha256" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let key = payload["key"].as_str().unwrap_or_default();
            let pkey = openssl::pkey::PKey::hmac(key.as_bytes())
                .map_err(|e| format!("hmac key failed: {e}"))?;
            let mut signer = openssl::sign::Signer::new(openssl::hash::MessageDigest::sha256(), &pkey)
                .map_err(|e| format!("hmac init failed: {e}"))?;
            signer.update(input.as_bytes())
                .map_err(|e| format!("hmac update failed: {e}"))?;
            let result = signer.sign_to_vec()
                .map_err(|e| format!("hmac sign failed: {e}"))?;
            Ok(json!(hex::encode(result)))
        }
        _ => Err(format!("unsupported encode action: {action}")),
    }
}
