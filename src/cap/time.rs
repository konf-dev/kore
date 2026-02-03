//! Time Tools
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | now | ( -- ms) | Unix timestamp in ms |
//! | sleep | (ms -- ) | Sleep for ms |

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register time tools (2)
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "now",
        "( -- ms)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                stack.push(Value::Int(ms))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "sleep",
        "(ms -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let ms = stack.pop()?.as_int()?;
                if ms < 0 {
                    return Err(crate::error::Error::Runtime(
                        "sleep: duration cannot be negative".into(),
                    ));
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(ms as u64)).await;
                Ok((stack, ctx))
            })
        },
    ));
}
