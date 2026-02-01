//! # Embeddings
//!
//! Vector embedding generation for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `embed` | `(text -- vector)` | Generate embedding for text |
//! | `embed-batch` | `(texts -- vectors)` | Generate embeddings for multiple texts |
//!
//! ## Example
//!
//! ```kore
//! "Hello, world!" embed
//! -- [0.1, 0.2, ...]
//!
//! ["Hello", "World"] embed-batch
//! -- [[0.1, ...], [0.2, ...]]
//! ```
//!
//! ## Providers
//!
//! Uses the AI provider configured via `ai-config`.
//! For `mock` provider, returns deterministic pseudo-embeddings.

use kore::{Context, Stack, Tool, Value};

/// Generate a deterministic pseudo-embedding for testing.
fn mock_embedding(text: &str) -> Vec<f64> {
    // Simple hash-based pseudo-embedding for consistency
    let mut vec = vec![0.0f64; 384]; // Common embedding dimension

    for (i, byte) in text.bytes().enumerate() {
        let idx = i % vec.len();
        vec[idx] += (byte as f64) / 255.0;
    }

    // Normalize
    let magnitude: f64 = vec.iter().map(|x| x * x).sum::<f64>().sqrt();
    if magnitude > 0.0 {
        for v in &mut vec {
            *v /= magnitude;
        }
    }

    vec
}

/// Register all embedding tools into a context.
pub async fn register_embedding_tools(ctx: &mut Context) {
    // embed: (text -- list)
    ctx.dict.write().await.register(
        Tool::native("embed", "(text -- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let text = stack.pop()?.into_text()?;

                // For now, use mock embeddings
                // Real implementation would use OpenAI, Anthropic, or local models
                let embedding = mock_embedding(&text);

                let values: Vec<Value> = embedding.into_iter().map(Value::Float).collect();
                stack.push(Value::List(values))?;
                Ok((stack, ctx))
            })
        }),
    );

    // embed-batch: (list -- list)
    ctx.dict.write().await.register(
        Tool::native("embed-batch", "(list -- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let texts = stack.pop()?.into_list()?;

                let embeddings: Vec<Value> = texts
                    .iter()
                    .map(|t| {
                        let text = t.text_opt().unwrap_or("");
                        let embedding = mock_embedding(text);
                        let values: Vec<Value> = embedding.into_iter().map(Value::Float).collect();
                        Value::List(values)
                    })
                    .collect();

                stack.push(Value::List(embeddings))?;
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use kore::tool::ToolBody;

    /// Helper to execute a tool by name from context
    async fn exec_tool(
        name: &str,
        stack: Stack,
        ctx: &Context,
    ) -> kore::Result<(Stack, Context)> {
        let dict = ctx.dict.read().await;
        let tool = dict.get(name, &ctx.tenant)?;
        drop(dict);

        match &tool.body {
            ToolBody::Native(native_fn) => native_fn.call(stack, ctx.clone()).await,
            ToolBody::Ops(ops) => kore::execute(ops, stack, ctx.clone()).await,
        }
    }

    async fn test_ctx() -> Context {
        let mut ctx = Context::new();
        register_embedding_tools(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn test_embed() {
        let ctx = test_ctx().await;

        let mut stack = Stack::new();
        stack.push(Value::Text("Hello, world!".to_string())).unwrap();

        let (mut stack, _) = exec_tool("embed", stack, &ctx).await.unwrap();

        let result = stack.pop().unwrap();
        match result {
            Value::List(arr) => {
                assert_eq!(arr.len(), 384);
            }
            _ => panic!("Expected List"),
        }
    }

    #[tokio::test]
    async fn test_embed_batch() {
        let ctx = test_ctx().await;

        let mut stack = Stack::new();
        stack.push(Value::List(vec![
            Value::Text("Hello".to_string()),
            Value::Text("World".to_string()),
        ])).unwrap();

        let (mut stack, _) = exec_tool("embed-batch", stack, &ctx).await.unwrap();

        let result = stack.pop().unwrap();
        match result {
            Value::List(arr) => {
                assert_eq!(arr.len(), 2);
                if let Value::List(inner) = &arr[0] {
                    assert_eq!(inner.len(), 384);
                } else {
                    panic!("Expected inner list");
                }
            }
            _ => panic!("Expected List"),
        }
    }

    #[tokio::test]
    async fn test_embed_deterministic() {
        let ctx = test_ctx().await;

        let mut stack = Stack::new();
        stack.push(Value::Text("test".to_string())).unwrap();

        let (mut stack, _) = exec_tool("embed", stack, &ctx).await.unwrap();
        let result1 = stack.pop().unwrap();

        let mut stack = Stack::new();
        stack.push(Value::Text("test".to_string())).unwrap();

        let (mut stack, _) = exec_tool("embed", stack, &ctx).await.unwrap();
        let result2 = stack.pop().unwrap();

        assert_eq!(result1, result2);
    }
}
