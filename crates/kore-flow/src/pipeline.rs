//! # Pipeline Composition
//!
//! Pipeline patterns for chaining operations.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `pipe` | `(value quotes -- result)` | Chain operations |
//! | `pipe-if` | `(value cond then else -- result)` | Conditional pipe |
//! | `pipe-while` | `(value cond body -- result)` | Loop while condition |
//! | `tap` | `(value quote -- value)` | Execute for side effect |
//!
//! ## Example
//!
//! ```kore
//! "hello" [ upper ] [ reverse ] [ print ] pipe
//!
//! 5 [ 10 < ] [ 2 * ] [ ] pipe-if  -- 10
//!
//! 1 [ 100 < ] [ 2 * ] pipe-while  -- 128
//! ```
//!
//! ## Patterns
//!
//! Pipes pass results between stages automatically.
//! Each stage receives the output of the previous stage.

use kore::{execute, Context, Stack, Tool, Value};

/// Register all pipeline tools into a context.
pub async fn register_pipeline_tools(ctx: &mut Context) {
    // pipe: (value quotes -- result)
    ctx.dict.write().await.register(
        Tool::native("pipe", "(any list -- any)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quotes = stack.pop()?.into_list()?;
                // Value is already on stack

                // Execute each quote in sequence, passing result to next
                let mut current_stack = stack;
                let mut current_ctx = ctx;

                for quote_val in quotes {
                    let quote = quote_val.into_quote().map_err(|_| {
                        kore::Error::Runtime("Expected list of quotes".into())
                    })?;
                    
                    let (new_stack, new_ctx) = execute(&quote, current_stack, current_ctx).await?;
                    current_stack = new_stack;
                    current_ctx = new_ctx;
                }

                Ok((current_stack, current_ctx))
            })
        }),
    );

    // pipe-if: (value cond then else -- result)
    ctx.dict.write().await.register(
        Tool::native("pipe-if", "(any quote quote quote -- any)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let else_quote = stack.pop()?.into_quote()?;
                let then_quote = stack.pop()?.into_quote()?;
                let cond_quote = stack.pop()?.into_quote()?;
                // Value is on stack

                // Duplicate value for condition check
                let value = stack.peek()?.clone();

                // Evaluate condition
                let (cond_stack, _) = execute(&cond_quote, stack, ctx.clone()).await?;
                let mut result_stack = cond_stack;
                let cond_result = result_stack.pop()?;

                let condition = match cond_result {
                    Value::Bool(b) => b,
                    _ => return Err(kore::Error::Runtime("Condition must return boolean".into())),
                };

                // Restore value and execute appropriate branch
                result_stack.push(value)?;
                let branch = if condition { then_quote } else { else_quote };

                execute(&branch, result_stack, ctx).await
            })
        }),
    );

    // pipe-while: (value cond body -- result)
    ctx.dict.write().await.register(
        Tool::native("pipe-while", "(any quote quote -- any)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let body_quote = stack.pop()?.into_quote()?;
                let cond_quote = stack.pop()?.into_quote()?;
                // Value is on stack

                let max_iterations = 10000; // Safety limit
                let mut current_stack = stack;
                let mut current_ctx = ctx;

                for _ in 0..max_iterations {
                    // Duplicate value for condition check
                    let value = current_stack.peek()?.clone();

                    // Check condition
                    let (cond_stack, _) = execute(&cond_quote, current_stack, current_ctx.clone()).await?;
                    let mut result_stack = cond_stack;
                    let cond_result = result_stack.pop()?;

                    let should_continue = match cond_result {
                        Value::Bool(b) => b,
                        _ => return Err(kore::Error::Runtime("Condition must return boolean".into())),
                    };

                    if !should_continue {
                        // Push value back and return
                        result_stack.push(value)?;
                        return Ok((result_stack, current_ctx));
                    }

                    // Value is still on stack, execute body
                    result_stack.push(value)?;
                    let (new_stack, new_ctx) = execute(&body_quote, result_stack, current_ctx).await?;
                    current_stack = new_stack;
                    current_ctx = new_ctx;
                }

                Err(kore::Error::Runtime("pipe-while exceeded maximum iterations".into()))
            })
        }),
    );

    // tap: (value quote -- value)
    ctx.dict.write().await.register(
        Tool::native("tap", "(any quote -- any)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;

                // Get value but keep it for later
                let value = stack.peek()?.clone();

                // Execute quote for side effect
                let (mut result_stack, new_ctx) = execute(&quote, stack, ctx).await?;

                // Discard result, restore original value
                let _ = result_stack.pop();
                result_stack.push(value)?;

                Ok((result_stack, new_ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pipeline_tools_registered() {
        let mut ctx = Context::new();
        register_pipeline_tools(&mut ctx).await;

        let dict = ctx.dict.read().await;
        let tenant = &ctx.tenant;
        assert!(dict.get("pipe", tenant).is_ok());
        assert!(dict.get("pipe-if", tenant).is_ok());
        assert!(dict.get("pipe-while", tenant).is_ok());
        assert!(dict.get("tap", tenant).is_ok());
    }
}
