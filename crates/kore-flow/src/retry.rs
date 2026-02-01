//! # Retry Logic
//!
//! Retry patterns for resilient operations.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `retry` | `(quote n -- result)` | Retry n times |
//! | `retry-with` | `(quote config -- result)` | Retry with config |
//! | `retry-exp` | `(quote config -- result)` | Exponential backoff |
//!
//! ## Example
//!
//! ```kore
//! [ "http://api.example.com" http-get ] 3 retry
//!
//! [ do-something ] { "attempts": 5, "delay_ms": 100 } retry-with
//! ```
//!
//! ## Config Options
//!
//! - `attempts`: Maximum attempts (default: 3)
//! - `delay_ms`: Delay between attempts in ms (default: 100)
//! - `multiplier`: Backoff multiplier for exponential (default: 2.0)

use kore::{execute, Context, Stack, Tool, Value};
use std::time::Duration;

/// Register all retry tools into a context.
pub async fn register_retry_tools(ctx: &mut Context) {
    // retry: (quote n -- result)
    ctx.dict.write().await.register(
        Tool::native("retry", "(quote int -- any)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let n = stack.pop()?.as_int()?;
                let quote = stack.pop()?.into_quote()?;

                let max_attempts = n.max(1) as usize;
                let mut last_error: Option<kore::Error> = None;

                for attempt in 1..=max_attempts {
                    match execute(&quote, stack.clone(), ctx.clone()).await {
                        Ok((new_stack, new_ctx)) => {
                            tracing::debug!(attempt = attempt, "retry succeeded");
                            return Ok((new_stack, new_ctx));
                        }
                        Err(e) => {
                            tracing::debug!(attempt = attempt, error = %e, "retry failed");
                            last_error = Some(e);

                            // Small delay between attempts
                            if attempt < max_attempts {
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                        }
                    }
                }

                Err(last_error.unwrap_or_else(|| kore::Error::Runtime("Retry failed".into())))
            })
        }),
    );

    // retry-with: (quote config -- result)
    ctx.dict.write().await.register(
        Tool::native("retry-with", "(quote map -- any)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let config = stack.pop()?;
                let quote = stack.pop()?.into_quote()?;

                let (max_attempts, delay_ms) = match &config {
                    Value::Map(m) => {
                        let attempts = m
                            .get("attempts")
                            .and_then(|v| v.as_int().ok())
                            .unwrap_or(3) as usize;
                        let delay = m
                            .get("delay_ms")
                            .and_then(|v| v.as_int().ok())
                            .unwrap_or(100) as u64;
                        (attempts, delay)
                    }
                    _ => return Err(kore::Error::Runtime("Config must be a map".into())),
                };

                let mut last_error: Option<kore::Error> = None;

                for attempt in 1..=max_attempts {
                    match execute(&quote, stack.clone(), ctx.clone()).await {
                        Ok((new_stack, new_ctx)) => {
                            return Ok((new_stack, new_ctx));
                        }
                        Err(e) => {
                            last_error = Some(e);

                            if attempt < max_attempts {
                                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                            }
                        }
                    }
                }

                Err(last_error.unwrap_or_else(|| kore::Error::Runtime("Retry failed".into())))
            })
        }),
    );

    // retry-exp: (quote config -- result)
    ctx.dict.write().await.register(
        Tool::native("retry-exp", "(quote map -- any)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let config = stack.pop()?;
                let quote = stack.pop()?.into_quote()?;

                let (max_attempts, initial_delay_ms, multiplier) = match &config {
                    Value::Map(m) => {
                        let attempts = m
                            .get("attempts")
                            .and_then(|v| v.as_int().ok())
                            .unwrap_or(3) as usize;
                        let delay = m
                            .get("delay_ms")
                            .and_then(|v| v.as_int().ok())
                            .unwrap_or(100) as u64;
                        let mult = m
                            .get("multiplier")
                            .and_then(|v| v.as_float().ok())
                            .unwrap_or(2.0);
                        (attempts, delay, mult)
                    }
                    _ => return Err(kore::Error::Runtime("Config must be a map".into())),
                };

                let mut last_error: Option<kore::Error> = None;
                let mut delay_ms = initial_delay_ms;

                for attempt in 1..=max_attempts {
                    match execute(&quote, stack.clone(), ctx.clone()).await {
                        Ok((new_stack, new_ctx)) => {
                            return Ok((new_stack, new_ctx));
                        }
                        Err(e) => {
                            last_error = Some(e);

                            if attempt < max_attempts {
                                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                                delay_ms = (delay_ms as f64 * multiplier) as u64;
                            }
                        }
                    }
                }

                Err(last_error.unwrap_or_else(|| kore::Error::Runtime("Retry failed".into())))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_tools_registered() {
        let mut ctx = Context::new();
        register_retry_tools(&mut ctx).await;

        let dict = ctx.dict.read().await;
        let tenant = &ctx.tenant;
        assert!(dict.get("retry", tenant).is_ok());
        assert!(dict.get("retry-with", tenant).is_ok());
        assert!(dict.get("retry-exp", tenant).is_ok());
    }
}
