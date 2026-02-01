//! # Structured Logging
//!
//! Level-based structured logging for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `log-debug` | `(text --)` | Debug-level log |
//! | `log-info` | `(text --)` | Info-level log |
//! | `log-warn` | `(text --)` | Warning-level log |
//! | `log-error` | `(text --)` | Error-level log |
//!
//! ## Example
//!
//! ```kore
//! "User logged in" log-info
//! "Failed to connect" log-error
//! ```

use kore::{Context, Stack, Tool};

/// Register all logging tools into a context.
pub async fn register_logging_tools(ctx: &mut Context) {
    // log-debug: (text --)
    ctx.dict.write().await.register(
        Tool::native("log-debug", "(text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let message = stack.pop()?.into_text()?;
                tracing::debug!(message = %message);
                Ok((stack, ctx))
            })
        }),
    );

    // log-info: (text --)
    ctx.dict.write().await.register(
        Tool::native("log-info", "(text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let message = stack.pop()?.into_text()?;
                tracing::info!(message = %message);
                Ok((stack, ctx))
            })
        }),
    );

    // log-warn: (text --)
    ctx.dict.write().await.register(
        Tool::native("log-warn", "(text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let message = stack.pop()?.into_text()?;
                tracing::warn!(message = %message);
                Ok((stack, ctx))
            })
        }),
    );

    // log-error: (text --)
    ctx.dict.write().await.register(
        Tool::native("log-error", "(text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let message = stack.pop()?.into_text()?;
                tracing::error!(message = %message);
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_logging_tools_registered() {
        let mut ctx = kore::Context::new();
        register_logging_tools(&mut ctx).await;

        let dict = ctx.dict.read().await;
        let tools = dict.list(&ctx.tenant);

        assert!(tools.contains(&"log-debug".to_string()));
        assert!(tools.contains(&"log-info".to_string()));
        assert!(tools.contains(&"log-warn".to_string()));
        assert!(tools.contains(&"log-error".to_string()));
    }

    #[tokio::test]
    async fn test_log_debug_execution() {
        let mut ctx = kore::Context::new();
        register_logging_tools(&mut ctx).await;

        let (stack, _) = kore::execute(
            &kore::Op::parse(r#""Debug message" log-debug"#).unwrap(),
            kore::Stack::new(),
            ctx,
        )
        .await
        .unwrap();

        assert_eq!(stack.depth(), 0);
    }

    #[tokio::test]
    async fn test_log_info_execution() {
        let mut ctx = kore::Context::new();
        register_logging_tools(&mut ctx).await;

        let (stack, _) = kore::execute(
            &kore::Op::parse(r#""Info message" log-info"#).unwrap(),
            kore::Stack::new(),
            ctx,
        )
        .await
        .unwrap();

        assert_eq!(stack.depth(), 0);
    }

    #[tokio::test]
    async fn test_log_warn_execution() {
        let mut ctx = kore::Context::new();
        register_logging_tools(&mut ctx).await;

        let (stack, _) = kore::execute(
            &kore::Op::parse(r#""Warning message" log-warn"#).unwrap(),
            kore::Stack::new(),
            ctx,
        )
        .await
        .unwrap();

        assert_eq!(stack.depth(), 0);
    }

    #[tokio::test]
    async fn test_log_error_execution() {
        let mut ctx = kore::Context::new();
        register_logging_tools(&mut ctx).await;

        let (stack, _) = kore::execute(
            &kore::Op::parse(r#""Error message" log-error"#).unwrap(),
            kore::Stack::new(),
            ctx,
        )
        .await
        .unwrap();

        assert_eq!(stack.depth(), 0);
    }
}
