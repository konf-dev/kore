//! # Kore Registry
//!
//! Package management for Kore programs.
//!
//! ## Purpose
//!
//! Share and discover Kore packages:
//! - Publish packages to registry
//! - Search for packages
//! - Install dependencies
//!
//! ## Modules
//!
//! | Module | Purpose | Key Tools |
//! |--------|---------|-----------|
//! | [`registry`] | Registry client | `pkg-search`, `pkg-info`, `pkg-list` |
//! | [`package`] | Package management | `pkg-install`, `pkg-publish`, `pkg-uninstall` |
//!
//! ## Example
//!
//! ```kore
//! "http-utils" pkg-search  -- find packages
//! "http-utils" pkg-install  -- install package
//! "my-package" { "version": "1.0.0" } pkg-publish
//! ```

pub mod package;
pub mod registry;

pub use package::register_package_tools;
pub use registry::register_registry_tools;

use kore::Context;

/// Register all registry tools into a context.
pub async fn register_all(ctx: &mut Context) {
    register_registry_tools(ctx).await;
    register_package_tools(ctx).await;
}
