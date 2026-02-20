use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use openssl::pkey::{Private, Public};
use openssl::rsa::{Padding, Rsa};
use openssl::symm::{decrypt, encrypt, Cipher};
use serde_json::{json, Value};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "rsa_encrypt" => {
            let plaintext = payload["plaintext"].as_str().unwrap_or_default().as_bytes().to_vec();
            let public_pem = payload["publicKeyPem"].as_str().unwrap_or_default();
            let rsa: Rsa<Public> =
                Rsa::public_key_from_pem(public_pem.as_bytes()).map_err(|e| format!("invalid public key: {e}"))?;
            let mut buf = vec![0; rsa.size() as usize];
            let len = rsa
                .public_encrypt(&plaintext, &mut buf, Padding::PKCS1_OAEP)
                .map_err(|e| format!("rsa encrypt failed: {e}"))?;
            buf.truncate(len);
            Ok(json!(BASE64.encode(buf)))
        }
        "rsa_decrypt" => {
            let cipher = payload["cipherTextBase64"].as_str().unwrap_or_default();
            let data = BASE64.decode(cipher).map_err(|e| format!("invalid base64: {e}"))?;
            let private_pem = payload["privateKeyPem"].as_str().unwrap_or_default();
            let rsa: Rsa<Private> =
                Rsa::private_key_from_pem(private_pem.as_bytes()).map_err(|e| format!("invalid private key: {e}"))?;
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
            let out = encrypt(cipher, key, Some(iv), plaintext).map_err(|e| format!("aes encrypt failed: {e}"))?;
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
            let out = decrypt(cipher, key, Some(iv), &cipher_data).map_err(|e| format!("aes decrypt failed: {e}"))?;
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
            let out = encrypt(cipher, key, Some(iv), plaintext).map_err(|e| format!("des encrypt failed: {e}"))?;
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
            let out = decrypt(cipher, key, Some(iv), &cipher_data).map_err(|e| format!("des decrypt failed: {e}"))?;
            Ok(json!(String::from_utf8_lossy(&out).to_string()))
        }
        _ => Err(format!("unsupported crypto action: {action}")),
    }
}
