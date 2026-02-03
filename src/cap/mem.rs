//! Session Memory Tools
//!
//! In-memory key-value storage for the current session.
//! Data is lost when the process exits.
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | mem-set | (key value -- ) | Store value |
//! | mem-get | (key -- value) | Get value |
//! | mem-del | (key -- ) | Delete key |
//! | mem-has | (key -- bool) | Check if key exists |
//! | mem-keys | ( -- list) | List all keys |

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register memory tools (5)
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "mem-set",
        "(key value -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                let key = stack.pop()?.into_text()?;
                {
                    let mut res = ctx.resources.write().await;
                    ctx.memory
                        .set(key, value, &mut res.mem)
                        .await
                        .map_err(|e| crate::error::Error::Runtime(format!("mem-set: {}", e)))?;
                }
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "mem-get",
        "(key -- value)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                let value = ctx.memory.get(&key).await.unwrap_or(Value::Null);
                stack.push(value)?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "mem-del",
        "(key -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                {
                    let mut res = ctx.resources.write().await;
                    ctx.memory.del(&key, &mut res.mem).await;
                }
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "mem-has",
        "(key -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                let has = ctx.memory.has(&key).await;
                stack.push(Value::Bool(has))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "mem-keys",
        "( -- list)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let keys = ctx.memory.keys().await;
                let list: Vec<Value> = keys.into_iter().map(Value::Text).collect();
                stack.push(Value::List(list))?;
                Ok((stack, ctx))
            })
        },
    ));
}
