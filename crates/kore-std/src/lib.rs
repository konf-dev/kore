//! # Kore Standard Library
//!
//! Foundational components shared across all Kore tool libraries.
//!
//! ## Modules
//!
//! | Module | Purpose | Key Tools |
//! |--------|---------|-----------|
//! | [`handle`] | Lifecycle management for external resources | `handle-create`, `handle-get`, `handle-close` |
//! | [`capability`] | Gate access to sensitive operations | `cap-require`, `cap-has`, `cap-grant` |
//! | [`error_ext`] | Consistent error structure with context | `error-wrap`, `error-code`, `error-context` |
//! | [`async_rt`] | Concurrent execution primitives | `spawn`, `await`, `timeout`, `cancel` |
//! | [`serial`] | Value serialization | `to-json`, `from-json` |
//! | [`schema`] | JSON Schema validation | `schema-validate`, `schema-errors` |
//!
//! ## Design Principles
//!
//! 1. **Each module does one thing** - No feature overlap between modules
//! 2. **Dumb and deterministic** - No hidden intelligence or magic
//! 3. **Explicit over implicit** - Clear inputs and outputs

pub mod async_rt;
pub mod capability;
pub mod error_ext;
pub mod handle;
pub mod schema;
pub mod serial;

// Re-exports
pub use async_rt::register_async_tools;
pub use capability::register_capability_tools;
pub use error_ext::register_error_tools;
pub use handle::register_handle_tools;
pub use schema::register_schema_tools;
pub use serial::register_serial_tools;

use kore::Context;

/// Register all kore-std tools into a context.
///
/// This registers tools from all modules:
/// - Handle management (6 tools)
/// - Capability system (4 tools)
/// - Error enrichment (4 tools)
/// - Async runtime (4 tools)
/// - Serialization (2 tools)
/// - Schema validation (3 tools)
pub async fn register_all(ctx: &mut Context) {
    register_handle_tools(ctx).await;
    register_capability_tools(ctx).await;
    register_error_tools(ctx).await;
    register_async_tools(ctx).await;
    register_serial_tools(ctx).await;
    register_schema_tools(ctx).await;
}
