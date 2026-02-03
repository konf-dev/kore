//! Environment Tools
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | env-get | (name -- value|null) | Get env var |
//! | env-set | (name value -- ) | Set env var |

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register environment tools (2)
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "env-get",
        "(name -- value|null)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name = stack.pop()?.into_text()?;
                if !ctx.caps.can_env_read() {
                    return Err(crate::error::Error::CapabilityDenied {
                        capability: "env:read".into(),
                        tool: "env-get".into(),
                    });
                }
                match std::env::var(&name) {
                    Ok(val) => stack.push(Value::Text(val))?,
                    Err(_) => stack.push(Value::Null)?,
                }
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "env-set",
        "(name value -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?.into_text()?;
                let name = stack.pop()?.into_text()?;
                if !ctx.caps.can_env_write() {
                    return Err(crate::error::Error::CapabilityDenied {
                        capability: "env:write".into(),
                        tool: "env-set".into(),
                    });
                }
                std::env::set_var(&name, &value);
                Ok((stack, ctx))
            })
        },
    ));
}
