//! # Capability System
//!
//! Gates access to sensitive operations.
//!
//! ## Purpose
//!
//! Agents should not have unrestricted access to all tools.
//! Capabilities provide fine-grained control over what operations
//! are allowed in a given execution context.
//!
//! ## Standard Capabilities
//!
//! | Capability | Grants Access To |
//! |------------|------------------|
//! | `network` | HTTP, WebSocket, DNS |
//! | `filesystem` | File read/write |
//! | `storage` | Database operations |
//! | `crypto` | Encryption/signing |
//! | `ai` | LLM/embedding calls |
//! | `mcp` | MCP server connections |
//! | `shell` | Command execution |
//!
//! ## Tools
//!
//! | Tool | Effect | Description |
//! |------|--------|-------------|
//! | `cap-require` | `(capability --)` | Assert capability or fail |
//! | `cap-has` | `(capability -- bool)` | Check if capability is granted |
//! | `cap-list` | `(-- list)` | List all granted capabilities |
//! | `cap-with` | `(capabilities quote -- result)` | Execute with temporary capabilities |

use kore::{Context, Stack, Tool, Value};

/// Check if a capability is granted in the context.
///
/// Returns true if the capability is in the context's capabilities list.
pub fn has_capability(ctx: &Context, capability: &str) -> bool {
    ctx.capabilities.contains(&capability.to_string())
}

/// Assert that a capability is granted.
///
/// Returns an error if the capability is not granted.
pub fn require_capability(ctx: &Context, capability: &str) -> kore::Result<()> {
    if has_capability(ctx, capability) {
        Ok(())
    } else {
        Err(kore::Error::Runtime(format!(
            "Capability required: {}",
            capability
        )))
    }
}

/// Register capability tools into a context.
///
/// # Tools Registered
///
/// - `cap-require`: Assert capability or fail
/// - `cap-has`: Check if capability is granted
/// - `cap-list`: List all granted capabilities
/// - `cap-with`: Execute with temporary capabilities
pub async fn register_capability_tools(ctx: &mut Context) {
    // cap-require: (capability --)
    ctx.dict.write().await.register(
        Tool::native("cap-require", "(text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let capability = stack.pop()?.into_text()?;
                require_capability(&ctx, &capability)?;
                Ok((stack, ctx))
            })
        }),
    );

    // cap-has: (capability -- bool)
    ctx.dict.write().await.register(
        Tool::native("cap-has", "(text -- bool)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let capability = stack.pop()?.into_text()?;
                let has = has_capability(&ctx, &capability);
                stack.push(Value::Bool(has))?;
                Ok((stack, ctx))
            })
        }),
    );

    // cap-list: (-- list)
    ctx.dict.write().await.register(
        Tool::native("cap-list", "(-- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let caps: Vec<Value> = ctx
                    .capabilities
                    .iter()
                    .map(|c| Value::Text(c.clone()))
                    .collect();
                stack.push(Value::List(caps))?;
                Ok((stack, ctx))
            })
        }),
    );

    // cap-with: (capabilities quote -- result)
    ctx.dict.write().await.register(
        Tool::native("cap-with", "(list quote -- any)", |mut stack: Stack, mut ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                let caps_list = stack.pop()?.into_list()?;

                // Parse capabilities from list
                let mut new_caps: Vec<String> = Vec::new();
                for cap in caps_list {
                    new_caps.push(cap.into_text()?);
                }

                // Save original capabilities
                let original_caps = ctx.capabilities.clone();

                // Grant temporary capabilities (additive)
                for cap in new_caps {
                    if !ctx.capabilities.contains(&cap) {
                        ctx.capabilities.push(cap);
                    }
                }

                // Execute the quote
                let result = kore::execute(&quote, stack, ctx.clone()).await;

                // Restore original capabilities
                ctx.capabilities = original_caps;

                match result {
                    Ok((new_stack, _)) => Ok((new_stack, ctx)),
                    Err(e) => Err(e),
                }
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_capability() {
        let mut ctx = Context::new();
        ctx.capabilities.push("network".into());
        ctx.capabilities.push("storage".into());

        assert!(has_capability(&ctx, "network"));
        assert!(has_capability(&ctx, "storage"));
        assert!(!has_capability(&ctx, "shell"));
    }

    #[test]
    fn test_require_capability_success() {
        let mut ctx = Context::new();
        ctx.capabilities.push("network".into());

        assert!(require_capability(&ctx, "network").is_ok());
    }

    #[test]
    fn test_require_capability_failure() {
        let ctx = Context::new();

        let result = require_capability(&ctx, "network");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Capability required"));
    }
}
