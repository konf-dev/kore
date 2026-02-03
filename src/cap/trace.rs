//! Trace Tools
//!
//! Tools for execution tracing and debugging.
//! Traces form a monoid under concatenation.
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | trace-on | (bool -- ) | Enable/disable tracing |
//! | trace | ( -- list) | Get current trace |
//! | trace-step | (name -- ) | Record a trace step |
//! | trace-fingerprint | ( -- hash) | Get execution fingerprint |

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Register trace tools (4)
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "trace-on",
        "(bool -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let _enabled = stack.pop()?.as_bool()?;
                // TODO: Store trace state in Context
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "trace",
        "( -- list)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                // TODO: Return actual trace from Context
                stack.push(Value::List(vec![]))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "trace-step",
        "(name -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name = stack.pop()?.into_text()?;
                // Record step (currently a no-op)
                let _ = crate::algebra::TraceStep::new(&name);
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "trace-fingerprint",
        "( -- hash)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let mut hasher = DefaultHasher::new();
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
                    .hash(&mut hasher);
                stack.push(Value::Int(hasher.finish() as i64))?;
                Ok((stack, ctx))
            })
        },
    ));
}
