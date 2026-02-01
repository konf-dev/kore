//! # Kore Tracing
//!
//! Observability tools for Kore programs.
//!
//! ## Purpose
//!
//! Agents and operators need visibility into execution:
//! - What happened during execution?
//! - Where did things go wrong?
//! - How long did operations take?
//!
//! ## Modules
//!
//! | Module | Purpose | Key Tools |
//! |--------|---------|-----------|
//! | [`logging`] | Structured logging | `log-debug`, `log-info`, `log-warn`, `log-error` |
//! | [`spans`] | Distributed tracing | `span-start`, `span-end`, `span-event` |
//! | [`metrics`] | Counters and gauges | `counter-inc`, `gauge-set`, `histogram-rec` |

pub mod logging;
pub mod metrics;
pub mod spans;

pub use logging::register_logging_tools;
pub use metrics::register_metrics_tools;
pub use spans::register_span_tools;

use kore::Context;

/// Register all tracing tools into a context.
pub async fn register_all(ctx: &mut Context) {
    register_logging_tools(ctx).await;
    register_span_tools(ctx).await;
    register_metrics_tools(ctx).await;
}
