//! Trace Tools
//!
//! Tools for execution tracing and debugging.
//! Traces form a monoid under concatenation.
//!
//! ## Stateless Design
//!
//! Following Postulate: tools are stateless. Trace storage uses Memory.
//! - `"__trace__" mem-get` to retrieve trace
//! - `"__trace__" mem-set` to store trace
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | trace-step | (name trace -- trace') | Append step to trace (pure) |
//! | trace-fingerprint | (trace -- hash) | Compute trace hash (pure) |
//! | trace-new | ( -- trace) | Create empty trace |

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;
use indexmap::IndexMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Register trace tools (3 pure tools)
pub fn register(dict: &mut Dictionary) {
    // trace-new: ( -- trace)
    // Create empty trace (pure)
    dict.register(Tool::native(
        "trace-new",
        "( -- trace:List)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                stack.push(Value::List(vec![]))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Create empty trace list"));

    // trace-step: (name trace -- trace')
    // Append a step to trace (pure - returns new trace)
    dict.register(Tool::native(
        "trace-step",
        "(name:Text trace:List -- trace':List)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let mut trace = stack.pop()?.into_list()?;
                let name = stack.pop()?.into_text()?;
                
                // Create step as a map
                let mut step = IndexMap::new();
                step.insert("tool".to_string(), Value::Text(name));
                step.insert("time".to_string(), Value::Int(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos() as i64)
                        .unwrap_or(0)
                ));
                
                trace.push(Value::Map(step));
                stack.push(Value::List(trace))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Append step to trace, return new trace (pure)"));

    // trace-fingerprint: (trace -- hash)
    // Compute fingerprint of trace (pure)
    dict.register(Tool::native(
        "trace-fingerprint",
        "(trace:List -- hash:Int)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let trace = stack.pop()?.into_list()?;
                
                let mut hasher = DefaultHasher::new();
                for step in &trace {
                    if let Value::Map(m) = step {
                        if let Some(Value::Text(name)) = m.get("tool") {
                            name.hash(&mut hasher);
                        }
                    }
                }
                
                stack.push(Value::Int(hasher.finish() as i64))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Compute fingerprint hash of trace (pure)"));
}
