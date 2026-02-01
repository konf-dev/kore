//! # Digital Signatures
//!
//! Ed25519 signing tools for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `keypair-gen` | `(-- keypair)` | Generate Ed25519 keypair |
//! | `sign` | `(message keypair -- signature)` | Sign a message |
//! | `verify` | `(message signature pubkey -- bool)` | Verify signature |
//! | `pubkey` | `(keypair -- pubkey)` | Extract public key |
//!
//! ## Example
//!
//! ```kore
//! keypair-gen  -- ( keypair )
//! dup pubkey   -- ( keypair pubkey )
//! swap "hello" swap sign  -- ( pubkey signature )
//! "hello" rot rot verify  -- ( bool )
//! ```
//!
//! ## Format
//!
//! Keypairs, public keys, and signatures are base64-encoded strings.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use indexmap::IndexMap;
use kore::{Context, Stack, Tool, Value};
use rand::{rngs::OsRng, Rng};

/// Generate an Ed25519 signing keypair.
/// Returns a map with "private" and "public" keys as base64 strings.
pub fn generate_keypair() -> IndexMap<String, Value> {
    let mut secret = [0u8; 32];
    OsRng.fill(&mut secret);
    let signing_key = SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();

    let private_b64 = BASE64.encode(signing_key.to_bytes());
    let public_b64 = BASE64.encode(verifying_key.to_bytes());

    let mut keypair = IndexMap::new();
    keypair.insert("private".to_string(), Value::Text(private_b64));
    keypair.insert("public".to_string(), Value::Text(public_b64));
    keypair
}

/// Sign a message with an Ed25519 private key.
pub fn sign_message(message: &str, private_key_b64: &str) -> Result<String, String> {
    let key_bytes = BASE64
        .decode(private_key_b64)
        .map_err(|e| format!("Invalid keypair: {}", e))?;

    if key_bytes.len() != 32 {
        return Err("Keypair must be 32 bytes".to_string());
    }

    let signing_key = SigningKey::from_bytes(
        key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid keypair length".to_string())?,
    );

    let signature = signing_key.sign(message.as_bytes());
    Ok(BASE64.encode(signature.to_bytes()))
}

/// Verify an Ed25519 signature.
pub fn verify_signature(message: &str, signature_b64: &str, pubkey_b64: &str) -> Result<bool, String> {
    let pubkey_bytes = BASE64
        .decode(pubkey_b64)
        .map_err(|e| format!("Invalid public key: {}", e))?;

    let signature_bytes = BASE64
        .decode(signature_b64)
        .map_err(|e| format!("Invalid signature: {}", e))?;

    if pubkey_bytes.len() != 32 {
        return Err("Public key must be 32 bytes".to_string());
    }

    if signature_bytes.len() != 64 {
        return Err("Signature must be 64 bytes".to_string());
    }

    let verifying_key = VerifyingKey::from_bytes(
        pubkey_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid public key length".to_string())?,
    )
    .map_err(|e| format!("Invalid public key: {}", e))?;

    let signature = Signature::from_bytes(
        signature_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid signature length".to_string())?,
    );

    Ok(verifying_key.verify(message.as_bytes(), &signature).is_ok())
}

/// Extract public key from private key.
pub fn extract_pubkey(private_key_b64: &str) -> Result<String, String> {
    let key_bytes = BASE64
        .decode(private_key_b64)
        .map_err(|e| format!("Invalid keypair: {}", e))?;

    if key_bytes.len() != 32 {
        return Err("Keypair must be 32 bytes".to_string());
    }

    let signing_key = SigningKey::from_bytes(
        key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid keypair length".to_string())?,
    );

    let verifying_key = signing_key.verifying_key();
    Ok(BASE64.encode(verifying_key.to_bytes()))
}

/// Register all signing tools into a context.
pub async fn register_signing_tools(ctx: &mut Context) {
    // keypair-gen: (-- map)
    ctx.dict.write().await.register(
        Tool::native("keypair-gen", "(-- map)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let keypair = generate_keypair();
                stack.push(Value::Map(keypair))?;
                Ok((stack, ctx))
            })
        }),
    );

    // sign: (text text -- text)
    ctx.dict.write().await.register(
        Tool::native("sign", "(text text -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let private_key = stack.pop()?.into_text()?;
                let message = stack.pop()?.into_text()?;
                let signature = sign_message(&message, &private_key).map_err(kore::Error::Runtime)?;
                stack.push(Value::Text(signature))?;
                Ok((stack, ctx))
            })
        }),
    );

    // verify: (text text text -- bool)
    ctx.dict.write().await.register(
        Tool::native("verify", "(text text text -- bool)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let pubkey = stack.pop()?.into_text()?;
                let signature = stack.pop()?.into_text()?;
                let message = stack.pop()?.into_text()?;
                let is_valid = verify_signature(&message, &signature, &pubkey).map_err(kore::Error::Runtime)?;
                stack.push(Value::Bool(is_valid))?;
                Ok((stack, ctx))
            })
        }),
    );

    // pubkey: (text -- text)
    ctx.dict.write().await.register(
        Tool::native("pubkey", "(text -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let private_key = stack.pop()?.into_text()?;
                let pubkey = extract_pubkey(&private_key).map_err(kore::Error::Runtime)?;
                stack.push(Value::Text(pubkey))?;
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_gen() {
        let keypair = generate_keypair();
        assert!(keypair.contains_key("private"));
        assert!(keypair.contains_key("public"));

        if let Value::Text(private) = &keypair["private"] {
            let decoded = BASE64.decode(private).unwrap();
            assert_eq!(decoded.len(), 32);
        } else {
            panic!("Expected text");
        }

        if let Value::Text(public) = &keypair["public"] {
            let decoded = BASE64.decode(public).unwrap();
            assert_eq!(decoded.len(), 32);
        } else {
            panic!("Expected text");
        }
    }

    #[test]
    fn test_sign_verify_roundtrip() {
        let keypair = generate_keypair();
        let private_key = match &keypair["private"] {
            Value::Text(s) => s.clone(),
            _ => panic!("Expected text"),
        };
        let public_key = match &keypair["public"] {
            Value::Text(s) => s.clone(),
            _ => panic!("Expected text"),
        };

        let message = "Hello, World!";
        let signature = sign_message(message, &private_key).unwrap();
        let is_valid = verify_signature(message, &signature, &public_key).unwrap();

        assert!(is_valid);
    }

    #[test]
    fn test_verify_wrong_message_fails() {
        let keypair = generate_keypair();
        let private_key = match &keypair["private"] {
            Value::Text(s) => s.clone(),
            _ => panic!("Expected text"),
        };
        let public_key = match &keypair["public"] {
            Value::Text(s) => s.clone(),
            _ => panic!("Expected text"),
        };

        let signature = sign_message("original message", &private_key).unwrap();
        let is_valid = verify_signature("different message", &signature, &public_key).unwrap();

        assert!(!is_valid);
    }

    #[test]
    fn test_extract_pubkey() {
        let keypair = generate_keypair();
        let private_key = match &keypair["private"] {
            Value::Text(s) => s.clone(),
            _ => panic!("Expected text"),
        };
        let expected_public = match &keypair["public"] {
            Value::Text(s) => s.clone(),
            _ => panic!("Expected text"),
        };

        let extracted = extract_pubkey(&private_key).unwrap();
        assert_eq!(extracted, expected_public);
    }
}
