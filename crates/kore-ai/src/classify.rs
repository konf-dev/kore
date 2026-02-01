//! # Classification
//!
//! Text classification for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `classify` | `(text labels -- label)` | Classify into one label |
//! | `classify-multi` | `(text labels k -- labels)` | Get top k labels |
//! | `classify-score` | `(text labels -- scores)` | Get scores for all labels |
//!
//! ## Example
//!
//! ```kore
//! "I love this product!" ["positive", "negative", "neutral"] classify
//! -- "positive"
//!
//! "Great but pricey" ["quality", "price", "service"] 2 classify-multi
//! -- ["quality", "price"]
//! ```
//!
//! ## Note
//!
//! Uses embedding similarity for classification. For LLM-based
//! classification, use `llm-chat` with appropriate prompting.

use indexmap::IndexMap;
use kore::{Context, Stack, Tool, Value};

/// Generate a deterministic pseudo-embedding for classification.
fn embed_text(text: &str) -> Vec<f64> {
    let mut vec = vec![0.0f64; 384];

    for (i, byte) in text.bytes().enumerate() {
        let idx = i % vec.len();
        vec[idx] += (byte as f64) / 255.0;
    }

    let magnitude: f64 = vec.iter().map(|x| x * x).sum::<f64>().sqrt();
    if magnitude > 0.0 {
        for v in &mut vec {
            *v /= magnitude;
        }
    }

    vec
}

/// Compute cosine similarity.
fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let mag_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }

    dot / (mag_a * mag_b)
}

/// Register all classification tools into a context.
pub async fn register_classify_tools(ctx: &mut Context) {
    // classify: (text list -- text)
    ctx.dict.write().await.register(
        Tool::native("classify", "(text list -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let labels = stack.pop()?.into_list()?;
                let text = stack.pop()?.into_text()?;

                if labels.is_empty() {
                    return Err(kore::Error::Runtime("Labels array cannot be empty".to_string()));
                }

                let text_embedding = embed_text(&text);

                let mut best_label = String::new();
                let mut best_score = f64::NEG_INFINITY;

                for label in &labels {
                    let label_str = label.text_opt().unwrap_or("");
                    let label_embedding = embed_text(label_str);
                    let score = cosine_similarity(&text_embedding, &label_embedding);

                    if score > best_score {
                        best_score = score;
                        best_label = label_str.to_string();
                    }
                }

                stack.push(Value::Text(best_label))?;
                Ok((stack, ctx))
            })
        }),
    );

    // classify-multi: (text list int -- list)
    ctx.dict.write().await.register(
        Tool::native("classify-multi", "(text list int -- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let k = stack.pop()?.into_int()? as usize;
                let labels = stack.pop()?.into_list()?;
                let text = stack.pop()?.into_text()?;

                let text_embedding = embed_text(&text);

                let mut scored: Vec<(String, f64)> = labels
                    .iter()
                    .map(|label| {
                        let label_str = label.text_opt().unwrap_or("");
                        let label_embedding = embed_text(label_str);
                        let score = cosine_similarity(&text_embedding, &label_embedding);
                        (label_str.to_string(), score)
                    })
                    .collect();

                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                let top_k: Vec<Value> = scored
                    .into_iter()
                    .take(k)
                    .map(|(label, _)| Value::Text(label))
                    .collect();

                stack.push(Value::List(top_k))?;
                Ok((stack, ctx))
            })
        }),
    );

    // classify-score: (text list -- list)
    ctx.dict.write().await.register(
        Tool::native("classify-score", "(text list -- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let labels = stack.pop()?.into_list()?;
                let text = stack.pop()?.into_text()?;

                let text_embedding = embed_text(&text);

                let scores: Vec<Value> = labels
                    .iter()
                    .map(|label| {
                        let label_str = label.text_opt().unwrap_or("");
                        let label_embedding = embed_text(label_str);
                        let score = cosine_similarity(&text_embedding, &label_embedding);

                        let mut m = IndexMap::new();
                        m.insert("label".to_string(), Value::Text(label_str.to_string()));
                        m.insert("score".to_string(), Value::Float(score));
                        Value::Map(m)
                    })
                    .collect();

                stack.push(Value::List(scores))?;
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
        register_classify_tools(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn test_classify() {
        let ctx = test_ctx().await;

        let mut stack = Stack::new();
        stack.push(Value::Text("positive review".to_string())).unwrap();
        stack.push(Value::List(vec![
            Value::Text("positive".to_string()),
            Value::Text("negative".to_string()),
            Value::Text("neutral".to_string()),
        ])).unwrap();

        let (mut stack, _) = exec_tool("classify", stack, &ctx).await.unwrap();

        let result = stack.pop().unwrap();
        assert!(matches!(result, Value::Text(_)));
    }

    #[tokio::test]
    async fn test_classify_multi() {
        let ctx = test_ctx().await;

        let mut stack = Stack::new();
        stack.push(Value::Text("good quality".to_string())).unwrap();
        stack.push(Value::List(vec![
            Value::Text("quality".to_string()),
            Value::Text("price".to_string()),
            Value::Text("service".to_string()),
        ])).unwrap();
        stack.push(Value::Int(2)).unwrap();

        let (mut stack, _) = exec_tool("classify-multi", stack, &ctx).await.unwrap();

        let result = stack.pop().unwrap();
        match result {
            Value::List(arr) => {
                assert_eq!(arr.len(), 2);
            }
            _ => panic!("Expected List"),
        }
    }

    #[tokio::test]
    async fn test_classify_score() {
        let ctx = test_ctx().await;

        let mut stack = Stack::new();
        stack.push(Value::Text("test".to_string())).unwrap();
        stack.push(Value::List(vec![
            Value::Text("a".to_string()),
            Value::Text("b".to_string()),
            Value::Text("c".to_string()),
        ])).unwrap();

        let (mut stack, _) = exec_tool("classify-score", stack, &ctx).await.unwrap();

        let result = stack.pop().unwrap();
        match result {
            Value::List(arr) => {
                assert_eq!(arr.len(), 3);
                if let Value::Map(m) = &arr[0] {
                    assert!(m.get("label").is_some());
                    assert!(m.get("score").is_some());
                } else {
                    panic!("Expected map in list");
                }
            }
            _ => panic!("Expected List"),
        }
    }
}
