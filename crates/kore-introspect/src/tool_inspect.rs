//! # Tool Introspection
//!
//! Inspect available tools in the dictionary.
//!
//! ## Purpose
//!
//! Agents need to discover what tools are available and understand
//! what each tool does before composing them into workflows.
//!
//! ## Tools
//!
//! | Tool | Effect | Description |
//! |------|--------|-------------|
//! | `tool-list` | `(-- list)` | List all available tool names |
//! | `tool-exists` | `(name -- bool)` | Check if a tool exists |
//! | `tool-doc` | `(name -- text)` | Get tool documentation |
//! | `tool-effect` | `(name -- text)` | Get tool effect signature |
//! | `tool-search` | `(pattern -- list)` | Search tools by glob pattern |

use kore::{Context, Stack, Tool, Value};

/// Register tool introspection tools into a context.
///
/// # Tools Registered
///
/// - `tool-list`: List all available tool names
/// - `tool-exists`: Check if a tool exists
/// - `tool-doc`: Get tool documentation
/// - `tool-effect`: Get tool effect signature
/// - `tool-search`: Search tools by glob pattern
pub async fn register_tool_tools(ctx: &mut Context) {
    // tool-list: (-- list)
    ctx.dict.write().await.register(
        Tool::native("tool-list", "(-- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let names = {
                    let dict = ctx.dict.read().await;
                    dict.list(&ctx.tenant)
                };
                let values: Vec<Value> = names.into_iter().map(Value::Text).collect();
                stack.push(Value::List(values))?;
                Ok((stack, ctx))
            })
        }),
    );

    // tool-exists: (name -- bool)
    ctx.dict.write().await.register(
        Tool::native("tool-exists", "(text -- bool)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name = stack.pop()?.into_text()?;
                let exists = {
                    let dict = ctx.dict.read().await;
                    dict.get(&name, &ctx.tenant).is_ok()
                };
                stack.push(Value::Bool(exists))?;
                Ok((stack, ctx))
            })
        }),
    );

    // tool-doc: (name -- text)
    ctx.dict.write().await.register(
        Tool::native("tool-doc", "(text -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name = stack.pop()?.into_text()?;
                let doc = {
                    let dict = ctx.dict.read().await;
                    let tool = dict.get(&name, &ctx.tenant)?;
                    tool.doc.clone().unwrap_or_else(|| "(no documentation)".to_string())
                };
                stack.push(Value::Text(doc))?;
                Ok((stack, ctx))
            })
        }),
    );

    // tool-effect: (name -- text)
    ctx.dict.write().await.register(
        Tool::native("tool-effect", "(text -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name = stack.pop()?.into_text()?;
                let effect = {
                    let dict = ctx.dict.read().await;
                    let tool = dict.get(&name, &ctx.tenant)?;
                    tool.effect
                        .as_ref()
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "(unknown)".to_string())
                };
                stack.push(Value::Text(effect))?;
                Ok((stack, ctx))
            })
        }),
    );

    // tool-search: (pattern -- list)
    ctx.dict.write().await.register(
        Tool::native("tool-search", "(text -- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let pattern = stack.pop()?.into_text()?;
                
                // Use glob pattern matching
                let glob_pattern = glob::Pattern::new(&pattern).map_err(|e| {
                    kore::Error::Runtime(format!("Invalid glob pattern: {}", e))
                })?;

                let matches: Vec<Value> = {
                    let dict = ctx.dict.read().await;
                    let all_names = dict.list(&ctx.tenant);
                    all_names
                        .into_iter()
                        .filter(|name| glob_pattern.matches(name))
                        .map(Value::Text)
                        .collect()
                };

                stack.push(Value::List(matches))?;
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use kore::register_builtins;

    #[tokio::test]
    async fn test_tool_list() {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        register_tool_tools(&mut ctx).await;

        let names = {
            let dict = ctx.dict.read().await;
            dict.list(&ctx.tenant)
        };

        // Should have builtins and our introspection tools
        assert!(names.contains(&"dup".to_string()));
        assert!(names.contains(&"tool-list".to_string()));
    }
}
