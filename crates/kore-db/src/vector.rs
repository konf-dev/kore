//! # Vector Storage
//!
//! Vector storage and similarity search for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `vec-store` | `(id vector metadata --)` | Store a vector with metadata |
//! | `vec-search` | `(vector k -- results)` | Find k nearest neighbors |
//! | `vec-get` | `(id -- vector metadata)` | Get vector by ID |
//! | `vec-del` | `(id --)` | Delete a vector |
//!
//! ## Example
//!
//! ```kore
//! "doc1" [0.1, 0.2, 0.3] { "text": "Hello" } vec-store
//! "doc2" [0.15, 0.25, 0.35] { "text": "World" } vec-store
//! [0.12, 0.22, 0.32] 2 vec-search
//! -- [ { "id": "doc1", "score": 0.99, "metadata": {...} }, ... ]
//! ```
//!
//! ## Similarity
//!
//! Uses cosine similarity for vector comparison.

use kore::{Context, Stack, Tool, Value};
use indexmap::IndexMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A stored vector with metadata.
#[derive(Debug, Clone)]
struct VectorEntry {
    vector: Vec<f64>,
    metadata: Value,
}

type VectorStore = Arc<RwLock<IndexMap<String, VectorEntry>>>;

fn create_store() -> VectorStore {
    Arc::new(RwLock::new(IndexMap::new()))
}

/// Parse a Value list to Vec<f64>.
fn value_to_vector(value: &Value) -> Result<Vec<f64>, kore::Error> {
    match value {
        Value::List(list) => {
            list.iter()
                .map(|v| match v {
                    Value::Float(f) => Ok(*f),
                    Value::Int(i) => Ok(*i as f64),
                    _ => Err(kore::Error::Runtime("Vector elements must be numbers".to_string())),
                })
                .collect()
        }
        _ => Err(kore::Error::Runtime("Vector must be a list".to_string())),
    }
}

/// Compute cosine similarity between two vectors.
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

/// Register all vector tools into a context.
pub async fn register_vector_tools(ctx: &mut Context) {
    let store = create_store();

    // vec-store: (text list any --)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("vec-store", "(text list any --)", move |mut stack: Stack, ctx: Context| {
            let store: VectorStore = store_clone.clone();
            Box::pin(async move {
                let metadata = stack.pop()?;
                let vector = stack.pop()?;
                let id = stack.pop()?.into_text()?;

                let vec_f64 = value_to_vector(&vector)?;

                let entry = VectorEntry {
                    vector: vec_f64,
                    metadata,
                };

                store.write().await.insert(id, entry);
                Ok((stack, ctx))
            })
        }),
    );

    // vec-search: (list int -- list)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("vec-search", "(list int -- list)", move |mut stack: Stack, ctx: Context| {
            let store: VectorStore = store_clone.clone();
            Box::pin(async move {
                let k = stack.pop()?;
                let query = stack.pop()?;

                let k_val = match k {
                    Value::Int(n) => n as usize,
                    _ => return Err(kore::Error::Runtime("k must be an integer".to_string())),
                };

                let query_vec = value_to_vector(&query)?;

                let guard = store.read().await;

                // Compute similarities
                let mut scored: Vec<(String, f64, Value)> = guard
                    .iter()
                    .map(|(id, entry): (&String, &VectorEntry)| {
                        let score = cosine_similarity(&query_vec, &entry.vector);
                        (id.clone(), score, entry.metadata.clone())
                    })
                    .collect();

                // Sort by similarity descending
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                // Take top k
                let results: Vec<Value> = scored
                    .into_iter()
                    .take(k_val)
                    .map(|(id, score, metadata)| {
                        let mut map = IndexMap::new();
                        map.insert("id".to_string(), Value::Text(id));
                        map.insert("score".to_string(), Value::Float(score));
                        map.insert("metadata".to_string(), metadata);
                        Value::Map(map)
                    })
                    .collect();

                stack.push(Value::List(results))?;
                Ok((stack, ctx))
            })
        }),
    );

    // vec-get: (text -- map)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("vec-get", "(text -- map)", move |mut stack: Stack, ctx: Context| {
            let store: VectorStore = store_clone.clone();
            Box::pin(async move {
                let id = stack.pop()?.into_text()?;

                let guard = store.read().await;
                let entry = guard
                    .get(&id)
                    .ok_or_else(|| kore::Error::Runtime(format!("Vector not found: {}", id)))?;

                let vector_values: Vec<Value> = entry.vector.iter().map(|&f| Value::Float(f)).collect();
                let mut result = IndexMap::new();
                result.insert("vector".to_string(), Value::List(vector_values));
                result.insert("metadata".to_string(), entry.metadata.clone());

                stack.push(Value::Map(result))?;
                Ok((stack, ctx))
            })
        }),
    );

    // vec-del: (text --)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("vec-del", "(text --)", move |mut stack: Stack, ctx: Context| {
            let store: VectorStore = store_clone.clone();
            Box::pin(async move {
                let id = stack.pop()?.into_text()?;
                store.write().await.swap_remove(&id);
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vec_store_get() {
        let store = create_store();
        
        let entry = VectorEntry {
            vector: vec![0.1, 0.2, 0.3],
            metadata: Value::Map({
                let mut m = IndexMap::new();
                m.insert("text".to_string(), Value::Text("hello".to_string()));
                m
            }),
        };
        
        store.write().await.insert("doc1".to_string(), entry);
        
        let guard = store.read().await;
        let e = guard.get("doc1").unwrap();
        assert_eq!(e.vector, vec![0.1, 0.2, 0.3]);
    }

    #[tokio::test]
    async fn test_vec_search() {
        let store = create_store();

        // Store vectors
        store.write().await.insert("doc1".to_string(), VectorEntry {
            vector: vec![1.0, 0.0, 0.0],
            metadata: Value::Text("a".to_string()),
        });
        store.write().await.insert("doc2".to_string(), VectorEntry {
            vector: vec![0.9, 0.1, 0.0],
            metadata: Value::Text("b".to_string()),
        });
        store.write().await.insert("doc3".to_string(), VectorEntry {
            vector: vec![0.0, 1.0, 0.0],
            metadata: Value::Text("c".to_string()),
        });

        let query_vec = vec![1.0, 0.0, 0.0];
        let guard = store.read().await;
        
        let mut scored: Vec<(String, f64)> = guard
            .iter()
            .map(|(id, entry)| {
                let score = cosine_similarity(&query_vec, &entry.vector);
                (id.clone(), score)
            })
            .collect();
        
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // First result should be doc1 (exact match)
        assert_eq!(scored[0].0, "doc1");
    }

    #[tokio::test]
    async fn test_vec_del() {
        let store = create_store();
        
        store.write().await.insert("doc1".to_string(), VectorEntry {
            vector: vec![0.1, 0.2],
            metadata: Value::Null,
        });
        
        assert!(store.read().await.contains_key("doc1"));
        store.write().await.swap_remove("doc1");
        assert!(!store.read().await.contains_key("doc1"));
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.0001);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 0.0001);
    }
}
