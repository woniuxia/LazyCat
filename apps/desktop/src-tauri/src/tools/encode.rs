use base64::{
    engine::general_purpose::{STANDARD as BASE64, URL_SAFE_NO_PAD as BASE64URL},
    Engine,
};
use image::ImageFormat;
use qrcode::QrCode;
use serde_json::{json, Value};

const ACTIONS: &[&str] = &[
    "base64_encode",
    "base64_decode",
    "base64_url_encode",
    "base64_url_decode",
    "url_encode",
    "url_decode",
    "md5",
    "qr_generate",
    "sha1",
    "sha256",
    "sha512",
    "hmac_sha256",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported encode action: {action}"));
    }
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
            Ok(json!(urlencoding::decode(input)
                .map_err(|e| format!("url decode failed: {e}"))?
                .to_string()))
        }
        "md5" => {
            let input = payload["input"].as_str().unwrap_or_default();
            Ok(json!(format!("{:x}", md5::compute(input.as_bytes()))))
        }
        "qr_generate" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let code =
                QrCode::new(input.as_bytes()).map_err(|e| format!("qr generation failed: {e}"))?;
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
            let digest =
                openssl::hash::hash(openssl::hash::MessageDigest::sha1(), input.as_bytes())
                    .map_err(|e| format!("sha1 failed: {e}"))?;
            Ok(json!(hex::encode(digest)))
        }
        "sha256" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let digest =
                openssl::hash::hash(openssl::hash::MessageDigest::sha256(), input.as_bytes())
                    .map_err(|e| format!("sha256 failed: {e}"))?;
            Ok(json!(hex::encode(digest)))
        }
        "sha512" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let digest =
                openssl::hash::hash(openssl::hash::MessageDigest::sha512(), input.as_bytes())
                    .map_err(|e| format!("sha512 failed: {e}"))?;
            Ok(json!(hex::encode(digest)))
        }
        "hmac_sha256" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let key = payload["key"].as_str().unwrap_or_default();
            let pkey = openssl::pkey::PKey::hmac(key.as_bytes())
                .map_err(|e| format!("hmac key failed: {e}"))?;
            let mut signer =
                openssl::sign::Signer::new(openssl::hash::MessageDigest::sha256(), &pkey)
                    .map_err(|e| format!("hmac init failed: {e}"))?;
            signer
                .update(input.as_bytes())
                .map_err(|e| format!("hmac update failed: {e}"))?;
            let result = signer
                .sign_to_vec()
                .map_err(|e| format!("hmac sign failed: {e}"))?;
            Ok(json!(hex::encode(result)))
        }
        _ => Err(format!("unsupported encode action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn base64_round_trip_with_unicode() {
        let input = "Hello, 懒猫 😺";
        let encoded = execute("base64_encode", &json!({ "input": input })).expect("encode");
        let decoded = execute("base64_decode", &json!({ "input": encoded })).expect("decode");
        assert_eq!(decoded, json!(input));
    }

    #[test]
    fn base64_url_round_trip() {
        let input = "a+b/c?d=e";
        let encoded = execute("base64_url_encode", &json!({ "input": input })).expect("encode");
        let decoded = execute("base64_url_decode", &json!({ "input": encoded })).expect("decode");
        assert_eq!(decoded, json!(input));
    }

    #[test]
    fn url_encode_decode_round_trip() {
        let input = "a b+中/文?x=1&y=2";
        let encoded = execute("url_encode", &json!({ "input": input })).expect("url encode");
        let decoded = execute("url_decode", &json!({ "input": encoded })).expect("url decode");
        assert_eq!(decoded, json!(input));
    }

    #[test]
    fn digest_vectors_should_match() {
        let input = "hello";
        assert_eq!(
            execute("md5", &json!({ "input": input })).expect("md5"),
            json!("5d41402abc4b2a76b9719d911017c592")
        );
        assert_eq!(
            execute("sha1", &json!({ "input": input })).expect("sha1"),
            json!("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d")
        );
        assert_eq!(
            execute("sha256", &json!({ "input": input })).expect("sha256"),
            json!("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
        );
        assert_eq!(
            execute("sha512", &json!({ "input": input })).expect("sha512"),
            json!("9b71d224bd62f3785d96d46ad3ea3d73319bfb c2890caadae2dff72519673ca72323c3d99ba5c11d7c7acc6e14b8c5da0c4663475c2e5c3adef46f73bcdec043".replace(' ', ""))
        );
    }

    #[test]
    fn hmac_sha256_vector_should_match() {
        let output = execute(
            "hmac_sha256",
            &json!({
                "input": "The quick brown fox jumps over the lazy dog",
                "key": "key"
            }),
        )
        .expect("hmac");
        assert_eq!(
            output,
            json!("f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8")
        );
    }

    #[test]
    fn qr_generate_should_return_png_data_url() {
        let output = execute("qr_generate", &json!({ "input": "lazycat" })).expect("qr");
        let s = output.as_str().expect("string output");
        assert!(s.starts_with("data:image/png;base64,"));
        let b64 = s.trim_start_matches("data:image/png;base64,");
        let bytes = BASE64.decode(b64).expect("valid base64 image");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]));
    }

    #[test]
    fn invalid_decode_inputs_should_fail() {
        let err = execute("base64_decode", &json!({ "input": "%%%not-base64%%%" }))
            .expect_err("must fail");
        assert!(err.contains("base64 decode failed"));

        // urlencoding crate keeps invalid escape fragments as-is instead of erroring.
        let out = execute("url_decode", &json!({ "input": "%" })).expect("url decode passthrough");
        assert_eq!(out, json!("%"));
    }
}
