//! # Parallel Execution
//!
//! Concurrent execution patterns for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `par` | `(quotes -- results)` | Execute quotes in parallel |
//! | `par-map` | `(items quote -- results)` | Map in parallel |
//! | `race` | `(quotes -- first-result)` | Return first to complete |
//!
//! ## Example
//!
//! ```kore
//! [ [ task1 ] [ task2 ] [ task3 ] ] par  -- run all, get all results
//!
//! [ 1 2 3 ] [ 2 * ] par-map  -- [ 2 4 6 ]
//!
//! [ [ slow-task ] [ fast-task ] ] race  -- first result wins
//! ```
//!
//! ## Note
//!
//! Parallel execution uses tokio for async coordination.
//! Each quote runs in its own task.

use indexmap::IndexMap;
use kore::{execute, Context, Stack, Tool, Value};

/// Register all parallel tools into a context.
pub async fn register_parallel_tools(ctx: &mut Context) {
    // par: (quotes -- results)
    ctx.dict.write().await.register(
        Tool::native("par", "(list -- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quotes = stack.pop()?.into_list()?;

                // Spawn all tasks
                let mut handles = Vec::new();
                for quote_val in quotes {
                    let quote = quote_val.into_quote().map_err(|_| {
                        kore::Error::Runtime("Expected list of quotes".into())
                    })?;
                    let ctx_clone = ctx.clone();
                    let task_stack = Stack::new();

                    let handle = tokio::spawn(async move {
                        execute(&quote, task_stack, ctx_clone).await
                    });
                    handles.push(handle);
                }

                // Collect results
                let mut results = Vec::new();
                for handle in handles {
                    match handle.await {
                        Ok(Ok((result_stack, _))) => {
                            // Get top of result stack or null
                            let result = result_stack.values().last().cloned().unwrap_or(Value::Null);
                            results.push(result);
                        }
                        Ok(Err(e)) => {
                            // Store error as map
                            let mut err_map = IndexMap::new();
                            err_map.insert("error".to_string(), Value::Text(e.to_string()));
                            results.push(Value::Map(err_map));
                        }
                        Err(e) => {
                            // Join error
                            let mut err_map = IndexMap::new();
                            err_map.insert("error".to_string(), Value::Text(format!("Task panicked: {}", e)));
                            results.push(Value::Map(err_map));
                        }
                    }
                }

                stack.push(Value::List(results))?;
                Ok((stack, ctx))
            })
        }),
    );

    // par-map: (items quote -- results)
    ctx.dict.write().await.register(
        Tool::native("par-map", "(list quote -- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                let items = stack.pop()?.into_list()?;

                // Spawn task for each item
                let mut handles = Vec::new();
                for item in items {
                    let quote_clone = quote.clone();
                    let ctx_clone = ctx.clone();

                    // Create a stack with the item
                    let mut task_stack = Stack::new();
                    task_stack.push(item)?;

                    let handle = tokio::spawn(async move {
                        execute(&quote_clone, task_stack, ctx_clone).await
                    });
                    handles.push(handle);
                }

                // Collect results
                let mut results = Vec::new();
                for handle in handles {
                    match handle.await {
                        Ok(Ok((result_stack, _))) => {
                            let result = result_stack.values().last().cloned().unwrap_or(Value::Null);
                            results.push(result);
                        }
                        Ok(Err(e)) => {
                            let mut err_map = IndexMap::new();
                            err_map.insert("error".to_string(), Value::Text(e.to_string()));
                            results.push(Value::Map(err_map));
                        }
                        Err(e) => {
                            let mut err_map = IndexMap::new();
                            err_map.insert("error".to_string(), Value::Text(format!("Task panicked: {}", e)));
                            results.push(Value::Map(err_map));
                        }
                    }
                }

                stack.push(Value::List(results))?;
                Ok((stack, ctx))
            })
        }),
    );

    // race: (quotes -- first-result)
    ctx.dict.write().await.register(
        Tool::native("race", "(list -- any)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quotes = stack.pop()?.into_list()?;

                if quotes.is_empty() {
                    return Err(kore::Error::Runtime("Cannot race empty list".into()));
                }

                // Use tokio::select! with spawned tasks
                let mut handles = Vec::new();
                for quote_val in quotes {
                    let quote = quote_val.into_quote().map_err(|_| {
                        kore::Error::Runtime("Expected list of quotes".into())
                    })?;
                    let ctx_clone = ctx.clone();
                    let task_stack = Stack::new();

                    let handle = tokio::spawn(async move {
                        execute(&quote, task_stack, ctx_clone).await
                    });
                    handles.push(handle);
                }

                // Wait for first to complete using select_all
                let (result, _index, remaining) = futures::future::select_all(handles).await;

                // Cancel remaining tasks
                for handle in remaining {
                    handle.abort();
                }

                match result {
                    Ok(Ok((result_stack, _))) => {
                        let result = result_stack.values().last().cloned().unwrap_or(Value::Null);
                        stack.push(result)?;
                        Ok((stack, ctx))
                    }
                    Ok(Err(e)) => Err(e),
                    Err(e) => Err(kore::Error::Runtime(format!("Task panicked: {}", e))),
                }
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parallel_tools_registered() {
        let mut ctx = Context::new();
        register_parallel_tools(&mut ctx).await;

        let dict = ctx.dict.read().await;
        let tenant = &ctx.tenant;
        assert!(dict.get("par", tenant).is_ok());
        assert!(dict.get("par-map", tenant).is_ok());
        assert!(dict.get("race", tenant).is_ok());
    }
}
