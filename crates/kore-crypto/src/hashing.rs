//! # Cryptographic Hashing
//!
//! Hash function tools for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `sha256` | `(data -- hash)` | SHA-256 hash |
//! | `blake3` | `(data -- hash)` | BLAKE3 hash |
//! | `hash-verify` | `(data expected-hash -- bool)` | Verify hash matches |
//!
//! ## Example
//!
//! ```kore
//! "hello world" sha256
//! -- "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
//!
//! "hello world" blake3
//! -- "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24"
//! ```
//!
//! ## Output Format
//!
//! All hashes are returned as lowercase hexadecimal strings.

use kore::{Context, Stack, Tool, Value};
use sha2::{Digest, Sha256};

/// Compute SHA-256 hash of data.
pub fn sha256_hash(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

/// Compute BLAKE3 hash of data.
pub fn blake3_hash(data: &str) -> String {
    blake3::hash(data.as_bytes()).to_hex().to_string()
}

/// Verify that data matches an expected SHA-256 hash.
pub fn verify_hash(data: &str, expected: &str) -> bool {
    let computed = sha256_hash(data);
    computed == expected.to_lowercase()
}

/// Register all hashing tools into a context.
pub async fn register_hashing_tools(ctx: &mut Context) {
    // sha256: (text -- text)
    ctx.dict.write().await.register(
        Tool::native("sha256", "(text -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let data = stack.pop()?.into_text()?;
                let hash = sha256_hash(&data);
                stack.push(Value::Text(hash))?;
                Ok((stack, ctx))
            })
        }),
    );

    // blake3: (text -- text)
    ctx.dict.write().await.register(
        Tool::native("blake3", "(text -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let data = stack.pop()?.into_text()?;
                let hash = blake3_hash(&data);
                stack.push(Value::Text(hash))?;
                Ok((stack, ctx))
            })
        }),
    );

    // hash-verify: (text text -- bool)
    ctx.dict.write().await.register(
        Tool::native("hash-verify", "(text text -- bool)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let expected = stack.pop()?.into_text()?;
                let data = stack.pop()?.into_text()?;
                let matches = verify_hash(&data, &expected);
                stack.push(Value::Bool(matches))?;
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256() {
        let hash = sha256_hash("hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_blake3() {
        let hash = blake3_hash("hello world");
        assert_eq!(
            hash,
            "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24"
        );
    }

    #[test]
    fn test_hash_verify_success() {
        let result = verify_hash(
            "hello world",
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
        );
        assert!(result);
    }

    #[test]
    fn test_hash_verify_failure() {
        let result = verify_hash("hello world", "wrong_hash");
        assert!(!result);
    }
}
