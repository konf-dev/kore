//! Persistent Storage (ROM) Tools
//!
//! Persistent key-value storage that survives process restarts.
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | rom-set | (key value -- ) | Store value |
//! | rom-get | (key -- value) | Get value |
//! | rom-del | (key -- ) | Delete key |
//! | rom-has | (key -- bool) | Check if key exists |
//! | rom-keys | ( -- list) | List all keys |

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register ROM tools (5)
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "rom-set",
        "(key value -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                let key = stack.pop()?.into_text()?;
                let storage = ctx.storage.as_ref().ok_or_else(|| {
                    crate::error::Error::Runtime("rom-set: storage not initialized".into())
                })?;
                {
                    let mut res = ctx.resources.write().await;
                    storage
                        .set(&key, &value, &mut res.rom)
                        .map_err(|e| crate::error::Error::Runtime(format!("rom-set: {}", e)))?;
                }
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "rom-get",
        "(key -- value)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                let storage = ctx.storage.as_ref().ok_or_else(|| {
                    crate::error::Error::Runtime("rom-get: storage not initialized".into())
                })?;
                let value = storage
                    .get(&key)
                    .map_err(|e| crate::error::Error::Runtime(format!("rom-get: {}", e)))?
                    .unwrap_or(Value::Null);
                stack.push(value)?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "rom-del",
        "(key -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                let storage = ctx.storage.as_ref().ok_or_else(|| {
                    crate::error::Error::Runtime("rom-del: storage not initialized".into())
                })?;
                {
                    let mut res = ctx.resources.write().await;
                    storage
                        .del(&key, &mut res.rom)
                        .map_err(|e| crate::error::Error::Runtime(format!("rom-del: {}", e)))?;
                }
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "rom-has",
        "(key -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                let storage = ctx.storage.as_ref().ok_or_else(|| {
                    crate::error::Error::Runtime("rom-has: storage not initialized".into())
                })?;
                stack.push(Value::Bool(storage.has(&key)))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "rom-keys",
        "( -- list)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let storage = ctx.storage.as_ref().ok_or_else(|| {
                    crate::error::Error::Runtime("rom-keys: storage not initialized".into())
                })?;
                let keys = storage
                    .keys()
                    .map_err(|e| crate::error::Error::Runtime(format!("rom-keys: {}", e)))?;
                let list: Vec<Value> = keys.into_iter().map(Value::Text).collect();
                stack.push(Value::List(list))?;
                Ok((stack, ctx))
            })
        },
    ));
}
