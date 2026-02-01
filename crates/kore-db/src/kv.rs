//! # Key-Value Store
//!
//! Simple key-value storage for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `kv-get` | `(key -- value)` | Get value by key |
//! | `kv-set` | `(key value --)` | Set key to value |
//! | `kv-del` | `(key --)` | Delete a key |
//! | `kv-list` | `(prefix -- keys)` | List keys matching prefix |
//! | `kv-has` | `(key -- bool)` | Check if key exists |
//!
//! ## Example
//!
//! ```kore
//! "user:1" { "name": "Alice" } kv-set
//! "user:1" kv-get  -- { "name": "Alice" }
//! "user:" kv-list  -- [ "user:1" ]
//! "user:1" kv-del
//! ```
//!
//! ## Storage
//!
//! This implementation uses in-memory storage. Values are stored
//! as JSON for serialization flexibility.

use kore::{Context, Stack, Tool, Value};
use indexmap::IndexMap;
use std::sync::Arc;
use tokio::sync::RwLock;

type KvStore = Arc<RwLock<IndexMap<String, Value>>>;

fn create_store() -> KvStore {
    Arc::new(RwLock::new(IndexMap::new()))
}

/// Register all KV tools into a context.
pub async fn register_kv_tools(ctx: &mut Context) {
    let store = create_store();

    // kv-get: (text -- any)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("kv-get", "(text -- any)", move |mut stack: Stack, ctx: Context| {
            let store: KvStore = store_clone.clone();
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                let guard = store.read().await;
                match guard.get(&key) {
                    Some(value) => {
                        stack.push(value.clone())?;
                        Ok((stack, ctx))
                    }
                    None => Err(kore::Error::Runtime(format!("Key not found: {}", key))),
                }
            })
        }),
    );

    // kv-set: (text any --)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("kv-set", "(text any --)", move |mut stack: Stack, ctx: Context| {
            let store: KvStore = store_clone.clone();
            Box::pin(async move {
                let value = stack.pop()?;
                let key = stack.pop()?.into_text()?;
                store.write().await.insert(key, value);
                Ok((stack, ctx))
            })
        }),
    );

    // kv-del: (text --)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("kv-del", "(text --)", move |mut stack: Stack, ctx: Context| {
            let store: KvStore = store_clone.clone();
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                store.write().await.swap_remove(&key);
                Ok((stack, ctx))
            })
        }),
    );

    // kv-list: (text -- list)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("kv-list", "(text -- list)", move |mut stack: Stack, ctx: Context| {
            let store: KvStore = store_clone.clone();
            Box::pin(async move {
                let prefix = stack.pop()?.into_text()?;
                let guard = store.read().await;
                let keys: Vec<Value> = guard
                    .keys()
                    .filter(|k: &&String| k.starts_with(&prefix))
                    .map(|k: &String| Value::Text(k.clone()))
                    .collect();
                stack.push(Value::List(keys))?;
                Ok((stack, ctx))
            })
        }),
    );

    // kv-has: (text -- bool)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("kv-has", "(text -- bool)", move |mut stack: Stack, ctx: Context| {
            let store: KvStore = store_clone.clone();
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                let guard = store.read().await;
                let exists = guard.contains_key(&key);
                stack.push(Value::Bool(exists))?;
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_kv_operations() {
        // Basic test that store creation works
        let store = create_store();
        store.write().await.insert("test".to_string(), Value::Text("value".to_string()));
        assert_eq!(store.read().await.get("test"), Some(&Value::Text("value".to_string())));
    }

    #[tokio::test]
    async fn test_kv_list_filter() {
        let store = create_store();
        store.write().await.insert("user:1".to_string(), Value::Text("alice".to_string()));
        store.write().await.insert("user:2".to_string(), Value::Text("bob".to_string()));
        store.write().await.insert("other:1".to_string(), Value::Text("data".to_string()));

        let guard = store.read().await;
        let keys: Vec<&String> = guard.keys().filter(|k| k.starts_with("user:")).collect();
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn test_kv_has() {
        let store = create_store();
        store.write().await.insert("exists".to_string(), Value::Text("value".to_string()));
        
        assert!(store.read().await.contains_key("exists"));
        assert!(!store.read().await.contains_key("missing"));
    }
}
