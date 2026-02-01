//! # Kore Resolve
//!
//! Tool resolution from multiple sources.
//!
//! ## Purpose
//!
//! Agents may need tools from various sources:
//! - Local definitions
//! - Kore packages
//! - MCP servers
//! - Remote registries
//!
//! ## Modules
//!
//! | Module | Purpose | Key Tools |
//! |--------|---------|-----------|
//! | [`loader`] | Load tools | `load`, `load-file`, `load-package` |
//! | [`resolver`] | Resolve tools | `resolve`, `resolve-all` |
//!
//! ## Resolution Order
//!
//! 1. Local dictionary
//! 2. Loaded packages
//! 3. Connected MCP servers
//! 4. Remote registries (if configured)

pub mod loader;
pub mod resolver;

pub use loader::register_loader_tools;
pub use resolver::register_resolver_tools;

use kore::Context;

/// Register all resolution tools into a context.
pub async fn register_all(ctx: &mut Context) {
    register_loader_tools(ctx).await;
    register_resolver_tools(ctx).await;
}
