use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use openssl::pkey::{Private, Public};
use openssl::rsa::{Padding, Rsa};
use openssl::symm::{decrypt, encrypt, Cipher};
use serde_json::{json, Value};

fn bcrypt_hash(payload: &Value) -> Result<Value, String> {
    let password = payload["password"].as_str().unwrap_or_default();
    let cost = payload["cost"].as_u64().unwrap_or(12) as u32;
    let hash = bcrypt::hash(password, cost).map_err(|e| format!("bcrypt hash failed: {e}"))?;
    Ok(json!({ "hash": hash }))
}

fn bcrypt_verify(payload: &Value) -> Result<Value, String> {
    let password = payload["password"].as_str().unwrap_or_default();
    let hash = payload["hash"].as_str().unwrap_or_default();
    let valid = bcrypt::verify(password, hash).map_err(|e| format!("bcrypt verify failed: {e}"))?;
    Ok(json!({ "valid": valid }))
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "rsa_encrypt" => {
            let plaintext = payload["plaintext"]
                .as_str()
                .unwrap_or_default()
                .as_bytes()
                .to_vec();
            let public_pem = payload["publicKeyPem"].as_str().unwrap_or_default();
            let rsa: Rsa<Public> = Rsa::public_key_from_pem(public_pem.as_bytes())
                .map_err(|e| format!("invalid public key: {e}"))?;
            let mut buf = vec![0; rsa.size() as usize];
            let len = rsa
                .public_encrypt(&plaintext, &mut buf, Padding::PKCS1_OAEP)
                .map_err(|e| format!("rsa encrypt failed: {e}"))?;
            buf.truncate(len);
            Ok(json!(BASE64.encode(buf)))
        }
        "rsa_decrypt" => {
            let cipher = payload["cipherTextBase64"].as_str().unwrap_or_default();
            let data = BASE64
                .decode(cipher)
                .map_err(|e| format!("invalid base64: {e}"))?;
            let private_pem = payload["privateKeyPem"].as_str().unwrap_or_default();
            let rsa: Rsa<Private> = Rsa::private_key_from_pem(private_pem.as_bytes())
                .map_err(|e| format!("invalid private key: {e}"))?;
            let mut buf = vec![0; rsa.size() as usize];
            let len = rsa
                .private_decrypt(&data, &mut buf, Padding::PKCS1_OAEP)
                .map_err(|e| format!("rsa decrypt failed: {e}"))?;
            buf.truncate(len);
            Ok(json!(String::from_utf8_lossy(&buf).to_string()))
        }
        "aes_encrypt" => {
            let plaintext = payload["plaintext"].as_str().unwrap_or_default().as_bytes();
            let key = payload["key"].as_str().unwrap_or_default().as_bytes();
            let iv = payload["iv"].as_str().unwrap_or_default().as_bytes();
            let algorithm = payload["algorithm"].as_str().unwrap_or("aes-256-cbc");
            let cipher = match algorithm {
                "aes-128-cbc" => Cipher::aes_128_cbc(),
                "aes-192-cbc" => Cipher::aes_192_cbc(),
                _ => Cipher::aes_256_cbc(),
            };
            let out = encrypt(cipher, key, Some(iv), plaintext)
                .map_err(|e| format!("aes encrypt failed: {e}"))?;
            Ok(json!(BASE64.encode(out)))
        }
        "aes_decrypt" => {
            let cipher_text = payload["cipherTextBase64"].as_str().unwrap_or_default();
            let cipher_data = BASE64
                .decode(cipher_text)
                .map_err(|e| format!("invalid base64: {e}"))?;
            let key = payload["key"].as_str().unwrap_or_default().as_bytes();
            let iv = payload["iv"].as_str().unwrap_or_default().as_bytes();
            let algorithm = payload["algorithm"].as_str().unwrap_or("aes-256-cbc");
            let cipher = match algorithm {
                "aes-128-cbc" => Cipher::aes_128_cbc(),
                "aes-192-cbc" => Cipher::aes_192_cbc(),
                _ => Cipher::aes_256_cbc(),
            };
            let out = decrypt(cipher, key, Some(iv), &cipher_data)
                .map_err(|e| format!("aes decrypt failed: {e}"))?;
            Ok(json!(String::from_utf8_lossy(&out).to_string()))
        }
        "des_encrypt" => {
            let plaintext = payload["plaintext"].as_str().unwrap_or_default().as_bytes();
            let key = payload["key"].as_str().unwrap_or_default().as_bytes();
            let iv = payload["iv"].as_str().unwrap_or_default().as_bytes();
            let algorithm = payload["algorithm"].as_str().unwrap_or("des-ede3-cbc");
            let cipher = if algorithm == "des-cbc" {
                Cipher::des_cbc()
            } else {
                Cipher::des_ede3_cbc()
            };
            let out = encrypt(cipher, key, Some(iv), plaintext)
                .map_err(|e| format!("des encrypt failed: {e}"))?;
            Ok(json!(BASE64.encode(out)))
        }
        "des_decrypt" => {
            let cipher_text = payload["cipherTextBase64"].as_str().unwrap_or_default();
            let cipher_data = BASE64
                .decode(cipher_text)
                .map_err(|e| format!("invalid base64: {e}"))?;
            let key = payload["key"].as_str().unwrap_or_default().as_bytes();
            let iv = payload["iv"].as_str().unwrap_or_default().as_bytes();
            let algorithm = payload["algorithm"].as_str().unwrap_or("des-ede3-cbc");
            let cipher = if algorithm == "des-cbc" {
                Cipher::des_cbc()
            } else {
                Cipher::des_ede3_cbc()
            };
            let out = decrypt(cipher, key, Some(iv), &cipher_data)
                .map_err(|e| format!("des decrypt failed: {e}"))?;
            Ok(json!(String::from_utf8_lossy(&out).to_string()))
        }
        "bcrypt_hash" => bcrypt_hash(payload),
        "bcrypt_verify" => bcrypt_verify(payload),
        _ => Err(format!("unsupported crypto action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rsa_encrypt_decrypt_round_trip() {
        let keypair = Rsa::generate(2048).expect("generate keypair");
        let private_pem = String::from_utf8(keypair.private_key_to_pem().expect("private pem"))
            .expect("utf8 private pem");
        let public_pem = String::from_utf8(keypair.public_key_to_pem().expect("public pem"))
            .expect("utf8 public pem");

        let plaintext = "LazyCat RSA ✅";
        let cipher = execute(
            "rsa_encrypt",
            &json!({
                "plaintext": plaintext,
                "publicKeyPem": public_pem
            }),
        )
        .expect("rsa encrypt");

        let output = execute(
            "rsa_decrypt",
            &json!({
                "cipherTextBase64": cipher,
                "privateKeyPem": private_pem
            }),
        )
        .expect("rsa decrypt");

        assert_eq!(output, json!(plaintext));
    }

    #[test]
    fn aes_round_trip_for_all_supported_algorithms() {
        let plaintext = "hello aes";

        let out128 = execute(
            "aes_encrypt",
            &json!({
                "plaintext": plaintext,
                "key": "1234567890abcdef",
                "iv": "abcdef1234567890",
                "algorithm": "aes-128-cbc"
            }),
        )
        .expect("aes128 enc");
        let dec128 = execute(
            "aes_decrypt",
            &json!({
                "cipherTextBase64": out128,
                "key": "1234567890abcdef",
                "iv": "abcdef1234567890",
                "algorithm": "aes-128-cbc"
            }),
        )
        .expect("aes128 dec");
        assert_eq!(dec128, json!(plaintext));

        let out192 = execute(
            "aes_encrypt",
            &json!({
                "plaintext": plaintext,
                "key": "1234567890abcdef12345678",
                "iv": "abcdef1234567890",
                "algorithm": "aes-192-cbc"
            }),
        )
        .expect("aes192 enc");
        let dec192 = execute(
            "aes_decrypt",
            &json!({
                "cipherTextBase64": out192,
                "key": "1234567890abcdef12345678",
                "iv": "abcdef1234567890",
                "algorithm": "aes-192-cbc"
            }),
        )
        .expect("aes192 dec");
        assert_eq!(dec192, json!(plaintext));

        // Unknown algorithm should fall back to aes-256-cbc.
        let out256 = execute(
            "aes_encrypt",
            &json!({
                "plaintext": plaintext,
                "key": "1234567890abcdef1234567890abcdef",
                "iv": "abcdef1234567890",
                "algorithm": "unknown"
            }),
        )
        .expect("aes256 enc");
        let dec256 = execute(
            "aes_decrypt",
            &json!({
                "cipherTextBase64": out256,
                "key": "1234567890abcdef1234567890abcdef",
                "iv": "abcdef1234567890",
                "algorithm": "aes-256-cbc"
            }),
        )
        .expect("aes256 dec");
        assert_eq!(dec256, json!(plaintext));
    }

    #[test]
    fn des_round_trip_for_des_and_3des() {
        let plaintext = "hello des";

        // In some OpenSSL builds (provider restrictions), DES-CBC may be unavailable.
        match execute(
            "des_encrypt",
            &json!({
                "plaintext": plaintext,
                "key": "12345678",
                "iv": "abcdefgh",
                "algorithm": "des-cbc"
            }),
        ) {
            Ok(out_des) => {
                let dec_des = execute(
                    "des_decrypt",
                    &json!({
                        "cipherTextBase64": out_des,
                        "key": "12345678",
                        "iv": "abcdefgh",
                        "algorithm": "des-cbc"
                    }),
                )
                .expect("des dec");
                assert_eq!(dec_des, json!(plaintext));
            }
            Err(err) => {
                assert!(err.contains("des encrypt failed"));
            }
        }

        let out_3des = execute(
            "des_encrypt",
            &json!({
                "plaintext": plaintext,
                "key": "123456789012345678901234",
                "iv": "abcdefgh",
                "algorithm": "des-ede3-cbc"
            }),
        )
        .expect("3des enc");
        let dec_3des = execute(
            "des_decrypt",
            &json!({
                "cipherTextBase64": out_3des,
                "key": "123456789012345678901234",
                "iv": "abcdefgh",
                "algorithm": "des-ede3-cbc"
            }),
        )
        .expect("3des dec");
        assert_eq!(dec_3des, json!(plaintext));
    }

    #[test]
    fn crypto_invalid_inputs_should_fail() {
        let err = execute(
            "rsa_encrypt",
            &json!({
                "plaintext": "x",
                "publicKeyPem": "bad pem"
            }),
        )
        .expect_err("invalid public key should fail");
        assert!(err.contains("invalid public key"));

        let err = execute(
            "aes_encrypt",
            &json!({
                "plaintext": "x",
                "key": "short",
                "iv": "short",
                "algorithm": "aes-256-cbc"
            }),
        )
        .expect_err("invalid key/iv should fail");
        assert!(err.contains("aes encrypt failed"));

        let err = execute(
            "aes_decrypt",
            &json!({
                "cipherTextBase64": "%%%bad-base64%%%",
                "key": "1234567890abcdef1234567890abcdef",
                "iv": "abcdef1234567890",
                "algorithm": "aes-256-cbc"
            }),
        )
        .expect_err("invalid base64 should fail");
        assert!(err.contains("invalid base64"));
    }

    #[test]
    fn bcrypt_hash_generates() {
        let r = execute("bcrypt_hash", &json!({"password": "test123", "cost": 4})).unwrap();
        let hash = r["hash"].as_str().unwrap();
        assert!(hash.starts_with("$2b$04$"));
    }

    #[test]
    fn bcrypt_verify_correct() {
        let r = execute("bcrypt_hash", &json!({"password": "test123", "cost": 4})).unwrap();
        let hash = r["hash"].as_str().unwrap();
        let v = execute(
            "bcrypt_verify",
            &json!({"password": "test123", "hash": hash}),
        )
        .unwrap();
        assert_eq!(v["valid"], true);
    }

    #[test]
    fn bcrypt_verify_wrong() {
        let r = execute("bcrypt_hash", &json!({"password": "test123", "cost": 4})).unwrap();
        let hash = r["hash"].as_str().unwrap();
        let v = execute("bcrypt_verify", &json!({"password": "wrong", "hash": hash})).unwrap();
        assert_eq!(v["valid"], false);
    }
}
