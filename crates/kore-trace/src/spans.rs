//! # Tracing Spans
//!
//! Distributed tracing with spans for timing and causality.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `span-start` | `(text -- text)` | Start a new span, returns span-id |
//! | `span-end` | `(text --)` | End a span |
//! | `span-event` | `(text text --)` | Add event to span (span-id message) |
//! | `span-error` | `(text text --)` | Record error in span (span-id message) |
//!
//! ## Example
//!
//! ```kore
//! "process-request" span-start  -- ( span-id )
//! dup "Starting processing" span-event
//! -- do work here
//! span-end
//! ```
//!
//! ## Span Hierarchy
//!
//! Spans form a tree. If you start a span while another is active,
//! the new span becomes a child of the active span.

use kore::{Context, Stack, Tool, Value};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Instant;
use uuid::Uuid;

/// A span tracking timing and events.
#[derive(Debug)]
struct Span {
    name: String,
    start_time: Instant,
    events: Vec<SpanEvent>,
    parent_id: Option<String>,
}

#[derive(Debug)]
#[allow(dead_code)] // Fields reserved for future span analysis features
struct SpanEvent {
    message: String,
    is_error: bool,
    timestamp: Instant,
}

/// Global span storage.
static SPANS: std::sync::OnceLock<RwLock<HashMap<String, Span>>> = std::sync::OnceLock::new();

fn spans() -> &'static RwLock<HashMap<String, Span>> {
    SPANS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Current active span ID (for parent tracking).
static CURRENT_SPAN: std::sync::OnceLock<RwLock<Option<String>>> = std::sync::OnceLock::new();

fn current_span() -> &'static RwLock<Option<String>> {
    CURRENT_SPAN.get_or_init(|| RwLock::new(None))
}

/// Register all span tools into a context.
pub async fn register_span_tools(ctx: &mut Context) {
    // span-start: (text -- text)
    ctx.dict.write().await.register(
        Tool::native("span-start", "(text -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name = stack.pop()?.into_text()?;

                let span_id = Uuid::new_v4().to_string();

                // Get current span as parent
                let parent_id = current_span().read().unwrap().clone();

                let span = Span {
                    name: name.clone(),
                    start_time: Instant::now(),
                    events: Vec::new(),
                    parent_id,
                };

                // Store the span
                spans().write().unwrap().insert(span_id.clone(), span);

                // Set as current span
                *current_span().write().unwrap() = Some(span_id.clone());

                tracing::trace!(span_id = %span_id, name = %name, "span started");

                stack.push(Value::Text(span_id))?;
                Ok((stack, ctx))
            })
        }),
    );

    // span-end: (text --)
    ctx.dict.write().await.register(
        Tool::native("span-end", "(text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let span_id = stack.pop()?.into_text()?;

                let span = spans()
                    .write()
                    .unwrap()
                    .remove(&span_id)
                    .ok_or_else(|| kore::Error::Runtime(format!("Span not found: {}", span_id)))?;

                let duration = span.start_time.elapsed();

                tracing::info!(
                    span_id = %span_id,
                    name = %span.name,
                    duration_ms = %duration.as_millis(),
                    event_count = %span.events.len(),
                    "span ended"
                );

                // Restore parent span as current
                *current_span().write().unwrap() = span.parent_id;

                Ok((stack, ctx))
            })
        }),
    );

    // span-event: (text text --)
    ctx.dict.write().await.register(
        Tool::native("span-event", "(text text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let message = stack.pop()?.into_text()?;
                let span_id = stack.pop()?.into_text()?;

                let mut spans_guard = spans().write().unwrap();
                let span = spans_guard
                    .get_mut(&span_id)
                    .ok_or_else(|| kore::Error::Runtime(format!("Span not found: {}", span_id)))?;

                span.events.push(SpanEvent {
                    message: message.clone(),
                    is_error: false,
                    timestamp: Instant::now(),
                });

                tracing::debug!(span_id = %span_id, message = %message, "span event");

                Ok((stack, ctx))
            })
        }),
    );

    // span-error: (text text --)
    ctx.dict.write().await.register(
        Tool::native("span-error", "(text text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let message = stack.pop()?.into_text()?;
                let span_id = stack.pop()?.into_text()?;

                let mut spans_guard = spans().write().unwrap();
                let span = spans_guard
                    .get_mut(&span_id)
                    .ok_or_else(|| kore::Error::Runtime(format!("Span not found: {}", span_id)))?;

                span.events.push(SpanEvent {
                    message: message.clone(),
                    is_error: true,
                    timestamp: Instant::now(),
                });

                tracing::error!(span_id = %span_id, error = %message, "span error");

                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use kore::Value;

    #[tokio::test]
    async fn test_span_tools_registered() {
        let mut ctx = kore::Context::new();
        register_span_tools(&mut ctx).await;

        let dict = ctx.dict.read().await;
        let tools = dict.list(&ctx.tenant);

        assert!(tools.contains(&"span-start".to_string()));
        assert!(tools.contains(&"span-end".to_string()));
        assert!(tools.contains(&"span-event".to_string()));
        assert!(tools.contains(&"span-error".to_string()));
    }

    #[tokio::test]
    async fn test_span_lifecycle() {
        let mut ctx = kore::Context::new();
        register_span_tools(&mut ctx).await;

        // Start a span, returns span-id on stack
        let (mut stack, ctx) = kore::execute(
            &kore::Op::parse(r#""test-span" span-start"#).unwrap(),
            kore::Stack::new(),
            ctx,
        )
        .await
        .unwrap();

        // Should have span ID on stack
        assert_eq!(stack.depth(), 1);
        let span_id = stack.pop().unwrap();
        assert!(matches!(span_id, Value::Text(_)));

        // End the span using its ID
        stack.push(span_id).unwrap();
        let (result_stack, _) = kore::execute(
            &kore::Op::parse("span-end").unwrap(),
            stack,
            ctx,
        )
        .await
        .unwrap();

        assert_eq!(result_stack.depth(), 0);
    }

    #[tokio::test]
    async fn test_span_with_event() {
        let mut ctx = kore::Context::new();
        register_span_tools(&mut ctx).await;

        // Start span and get the span-id
        let (mut stack, ctx) = kore::execute(
            &kore::Op::parse(r#""event-span" span-start"#).unwrap(),
            kore::Stack::new(),
            ctx,
        )
        .await
        .unwrap();

        // Get span_id from stack, push it twice for event and end
        let span_id = stack.pop().unwrap();
        stack.push(span_id.clone()).unwrap();
        stack.push(Value::Text("Something happened".into())).unwrap();

        // Add event
        let (mut stack, ctx) = kore::execute(
            &kore::Op::parse("span-event").unwrap(),
            stack,
            ctx,
        )
        .await
        .unwrap();

        // End span
        stack.push(span_id).unwrap();
        let (stack, _) = kore::execute(
            &kore::Op::parse("span-end").unwrap(),
            stack,
            ctx,
        )
        .await
        .unwrap();

        assert_eq!(stack.depth(), 0);
    }

    #[tokio::test]
    async fn test_span_with_error() {
        let mut ctx = kore::Context::new();
        register_span_tools(&mut ctx).await;

        // Start span and get the span-id
        let (mut stack, ctx) = kore::execute(
            &kore::Op::parse(r#""error-span" span-start"#).unwrap(),
            kore::Stack::new(),
            ctx,
        )
        .await
        .unwrap();

        // Get span_id from stack, push it twice for error and end
        let span_id = stack.pop().unwrap();
        stack.push(span_id.clone()).unwrap();
        stack.push(Value::Text("Connection failed".into())).unwrap();

        // Record error
        let (mut stack, ctx) = kore::execute(
            &kore::Op::parse("span-error").unwrap(),
            stack,
            ctx,
        )
        .await
        .unwrap();

        // End span
        stack.push(span_id).unwrap();
        let (stack, _) = kore::execute(
            &kore::Op::parse("span-end").unwrap(),
            stack,
            ctx,
        )
        .await
        .unwrap();

        assert_eq!(stack.depth(), 0);
    }
}
