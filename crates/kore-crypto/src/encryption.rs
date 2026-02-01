//! # Symmetric Encryption
//!
//! AES-GCM encryption tools for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `key-gen` | `(-- key)` | Generate a random 256-bit key |
//! | `encrypt` | `(plaintext key -- ciphertext)` | Encrypt with AES-256-GCM |
//! | `decrypt` | `(ciphertext key -- plaintext)` | Decrypt with AES-256-GCM |
//!
//! ## Example
//!
//! ```kore
//! key-gen  -- ( key )
//! "secret message" swap encrypt  -- ( ciphertext )
//! key decrypt  -- ( plaintext )
//! ```
//!
//! ## Format
//!
//! Keys and ciphertexts are base64-encoded strings.
//! Ciphertext includes the nonce prepended to the encrypted data.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use kore::{Context, Stack, Tool, Value};
use rand::Rng;

/// Generate a random 256-bit encryption key.
pub fn generate_key() -> String {
    let mut key = [0u8; 32];
    rand::thread_rng().fill(&mut key);
    BASE64.encode(key)
}

/// Encrypt plaintext with AES-256-GCM.
pub fn encrypt_data(plaintext: &str, key_b64: &str) -> Result<String, String> {
    let key_bytes = BASE64
        .decode(key_b64)
        .map_err(|e| format!("Invalid key: {}", e))?;

    if key_bytes.len() != 32 {
        return Err("Key must be 256 bits (32 bytes)".to_string());
    }

    // Generate random nonce
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Create cipher and encrypt
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| format!("Invalid key: {}", e))?;

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

    // Prepend nonce to ciphertext
    let mut combined = nonce_bytes.to_vec();
    combined.extend(ciphertext);

    Ok(BASE64.encode(combined))
}

/// Decrypt ciphertext with AES-256-GCM.
pub fn decrypt_data(ciphertext_b64: &str, key_b64: &str) -> Result<String, String> {
    let key_bytes = BASE64
        .decode(key_b64)
        .map_err(|e| format!("Invalid key: {}", e))?;

    if key_bytes.len() != 32 {
        return Err("Key must be 256 bits (32 bytes)".to_string());
    }

    let combined = BASE64
        .decode(ciphertext_b64)
        .map_err(|e| format!("Invalid ciphertext: {}", e))?;

    if combined.len() < 12 {
        return Err("Ciphertext too short".to_string());
    }

    // Split nonce and ciphertext
    let nonce = Nonce::from_slice(&combined[..12]);
    let ciphertext_bytes = &combined[12..];

    // Create cipher and decrypt
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| format!("Invalid key: {}", e))?;

    let plaintext = cipher
        .decrypt(nonce, ciphertext_bytes)
        .map_err(|e| format!("Decryption failed: {}", e))?;

    String::from_utf8(plaintext).map_err(|e| format!("Invalid UTF-8: {}", e))
}

/// Register all encryption tools into a context.
pub async fn register_encryption_tools(ctx: &mut Context) {
    // key-gen: (-- text)
    ctx.dict.write().await.register(
        Tool::native("key-gen", "(-- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = generate_key();
                stack.push(Value::Text(key))?;
                Ok((stack, ctx))
            })
        }),
    );

    // encrypt: (text text -- text)
    ctx.dict.write().await.register(
        Tool::native("encrypt", "(text text -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                let plaintext = stack.pop()?.into_text()?;
                let ciphertext = encrypt_data(&plaintext, &key).map_err(kore::Error::Runtime)?;
                stack.push(Value::Text(ciphertext))?;
                Ok((stack, ctx))
            })
        }),
    );

    // decrypt: (text text -- text)
    ctx.dict.write().await.register(
        Tool::native("decrypt", "(text text -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                let ciphertext = stack.pop()?.into_text()?;
                let plaintext = decrypt_data(&ciphertext, &key).map_err(kore::Error::Runtime)?;
                stack.push(Value::Text(plaintext))?;
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_gen() {
        let key = generate_key();
        let decoded = BASE64.decode(&key).unwrap();
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = generate_key();
        let plaintext = "Hello, World!";

        let ciphertext = encrypt_data(plaintext, &key).unwrap();
        let decrypted = decrypt_data(&ciphertext, &key).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_wrong_key_fails() {
        let key1 = generate_key();
        let key2 = generate_key();

        let ciphertext = encrypt_data("secret", &key1).unwrap();
        let result = decrypt_data(&ciphertext, &key2);

        assert!(result.is_err());
    }
}
