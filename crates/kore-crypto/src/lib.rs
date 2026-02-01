//! # Kore Cryptography
//!
//! Cryptographic tools for Kore programs.
//!
//! ## Purpose
//!
//! Agents need secure operations:
//! - Hashing data for integrity checks
//! - Encrypting sensitive data
//! - Signing messages for authentication
//!
//! ## Modules
//!
//! | Module | Purpose | Key Tools |
//! |--------|---------|-----------|
//! | [`hashing`] | Hash functions | `sha256`, `blake3`, `hash-verify` |
//! | [`encryption`] | Symmetric encryption | `encrypt`, `decrypt`, `key-gen` |
//! | [`signing`] | Digital signatures | `sign`, `verify`, `keypair-gen` |
//!
//! ## Security Note
//!
//! All cryptographic operations use well-audited libraries.
//! Keys should be managed securely - consider using capability
//! restrictions.

pub mod encryption;
pub mod hashing;
pub mod signing;

pub use encryption::register_encryption_tools;
pub use hashing::register_hashing_tools;
pub use signing::register_signing_tools;

use kore::Context;

/// Register all cryptographic tools into a context.
pub async fn register_all(ctx: &mut Context) {
    register_hashing_tools(ctx).await;
    register_encryption_tools(ctx).await;
    register_signing_tools(ctx).await;
}
