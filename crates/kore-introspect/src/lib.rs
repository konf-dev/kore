//! # Kore Introspection
//!
//! Tools for inspecting the runtime: tools, quotes, and stack.
//!
//! ## Purpose
//!
//! Agents need to understand what's available:
//! - What tools exist?
//! - What does a tool do?
//! - What's in this quote?
//! - What's on the stack?
//!
//! ## Modules
//!
//! | Module | Purpose | Key Tools |
//! |--------|---------|-----------|
//! | [`tool_inspect`] | Inspect available tools | `tool-list`, `tool-doc`, `tool-effect` |
//! | [`quote_inspect`] | Inspect and construct quotes | `quote-ops`, `ops-quote` |
//! | [`stack_inspect`] | Inspect the stack | `depth`, `stack-list` |

pub mod quote_inspect;
pub mod stack_inspect;
pub mod tool_inspect;

pub use quote_inspect::register_quote_tools;
pub use stack_inspect::register_stack_tools;
pub use tool_inspect::register_tool_tools;

use kore::Context;

/// Register all introspection tools into a context.
pub async fn register_all(ctx: &mut Context) {
    register_tool_tools(ctx).await;
    register_quote_tools(ctx).await;
    register_stack_tools(ctx).await;
}
