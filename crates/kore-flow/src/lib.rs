//! # Kore Flow
//!
//! Workflow composition and control flow for Kore programs.
//!
//! ## Purpose
//!
//! Agents need robust workflow patterns:
//! - Retry with backoff
//! - Parallel execution
//! - Pipeline composition
//! - Conditional branching
//!
//! ## Modules
//!
//! | Module | Purpose | Key Tools |
//! |--------|---------|-----------|
//! | [`retry`] | Retry logic | `retry`, `retry-with` |
//! | [`parallel`] | Parallel execution | `par`, `par-map`, `race` |
//! | [`pipeline`] | Pipeline composition | `pipe`, `pipe-if`, `pipe-while` |
//!
//! ## Example
//!
//! ```kore
//! [ http-get ] { "attempts": 3, "delay": 1000 } retry
//! [ task1 task2 task3 ] par
//! [ fetch transform store ] pipe
//! ```

pub mod parallel;
pub mod pipeline;
pub mod retry;

pub use parallel::register_parallel_tools;
pub use pipeline::register_pipeline_tools;
pub use retry::register_retry_tools;

use kore::Context;

/// Register all flow tools into a context.
pub async fn register_all(ctx: &mut Context) {
    register_retry_tools(ctx).await;
    register_parallel_tools(ctx).await;
    register_pipeline_tools(ctx).await;
}
