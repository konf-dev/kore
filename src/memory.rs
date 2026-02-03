//! Session Memory - Volatile key-value storage
//!
//! RAM for agents. Cleared on restart.
//! Each value consumes resource units.
//!
//! ## Design
//! - Simple key-value store
//! - Values are measured in units (1 unit per "complexity")
//! - Quota enforced on set operations
//! - No magic - just store and retrieve

use crate::resources::ResourceQuota;
use crate::value::Value;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Session memory - volatile key-value store
#[derive(Debug, Clone)]
pub struct Memory {
    /// The actual storage
    store: Arc<RwLock<MemoryStore>>,
}

/// Inner storage
#[derive(Debug, Default, Serialize, Deserialize)]
struct MemoryStore {
    /// Key-value pairs
    data: IndexMap<String, Value>,

    /// Size of each key in units (for quota tracking)
    sizes: IndexMap<String, u64>,

    /// Total units used
    used: u64,
}

impl Memory {
    /// Create new empty memory
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(MemoryStore::default())),
        }
    }

    /// Calculate units for a value (simple: depth-based)
    fn value_units(value: &Value) -> u64 {
        match value {
            Value::Null => 1,
            Value::Bool(_) => 1,
            Value::Int(_) => 1,
            Value::Float(_) => 1,
            Value::Text(s) => 1 + (s.len() as u64 / 100), // 1 unit per 100 chars
            Value::List(v) => 1 + v.iter().map(Self::value_units).sum::<u64>(),
            Value::Map(m) => 1 + m.values().map(Self::value_units).sum::<u64>(),
            Value::Quote(ops) => 1 + ops.len() as u64,
            Value::Handle(_) => 1,
            Value::Error(_) => 1,
            Value::Ext(e) => 1 + Self::value_units(&e.data),
        }
    }

    /// Set a value, checking quota
    /// Returns: (old_value, units_delta)
    pub async fn set(
        &self,
        key: String,
        value: Value,
        quota: &mut ResourceQuota,
    ) -> Result<Option<Value>, MemoryError> {
        let new_size = Self::value_units(&value);

        let mut store = self.store.write().await;

        // Calculate delta
        let old_size = store.sizes.get(&key).copied().unwrap_or(0);
        let delta = new_size as i64 - old_size as i64;

        // Check quota if growing
        if delta > 0 {
            let needed = delta as u64;
            if !quota.can_allocate(needed) {
                return Err(MemoryError::QuotaExceeded {
                    needed,
                    available: quota.free(),
                });
            }
            quota.try_allocate(needed);
        } else if delta < 0 {
            // Release units if shrinking
            quota.release((-delta) as u64);
        }

        // Update tracking
        store.sizes.insert(key.clone(), new_size);
        store.used = (store.used as i64 + delta).max(0) as u64;

        // Store value
        let old = store.data.insert(key, value);
        Ok(old)
    }

    /// Get a value
    pub async fn get(&self, key: &str) -> Option<Value> {
        let store = self.store.read().await;
        store.data.get(key).cloned()
    }

    /// Check if key exists
    pub async fn has(&self, key: &str) -> bool {
        let store = self.store.read().await;
        store.data.contains_key(key)
    }

    /// Delete a value
    /// Returns: (removed_value, units_freed)
    pub async fn del(&self, key: &str, quota: &mut ResourceQuota) -> Option<Value> {
        let mut store = self.store.write().await;

        if let Some(value) = store.data.shift_remove(key) {
            let size = store.sizes.shift_remove(key).unwrap_or(0);
            store.used = store.used.saturating_sub(size);
            quota.release(size);
            Some(value)
        } else {
            None
        }
    }

    /// List all keys
    pub async fn keys(&self) -> Vec<String> {
        let store = self.store.read().await;
        store.data.keys().cloned().collect()
    }

    /// Get current usage in units
    pub async fn usage(&self) -> u64 {
        let store = self.store.read().await;
        store.used
    }

    /// Number of keys
    pub async fn len(&self) -> usize {
        let store = self.store.read().await;
        store.data.len()
    }

    /// Is empty?
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Clear all data
    pub async fn clear(&self, quota: &mut ResourceQuota) {
        let mut store = self.store.write().await;
        quota.release(store.used);
        store.data.clear();
        store.sizes.clear();
        store.used = 0;
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory operation errors
#[derive(Debug, Clone)]
pub enum MemoryError {
    /// Quota exceeded
    QuotaExceeded { needed: u64, available: u64 },

    /// Key not found
    NotFound { key: String },
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryError::QuotaExceeded { needed, available } => {
                write!(
                    f,
                    "memory quota exceeded: need {} units, only {} available",
                    needed, available
                )
            }
            MemoryError::NotFound { key } => {
                write!(f, "key not found: {}", key)
            }
        }
    }
}

impl std::error::Error for MemoryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_set_get() {
        let mem = Memory::new();
        let mut quota = ResourceQuota::new(100);

        mem.set("key1".to_string(), Value::Int(42), &mut quota)
            .await
            .unwrap();

        assert_eq!(mem.get("key1").await, Some(Value::Int(42)));
        assert!(quota.used > 0);
    }

    #[tokio::test]
    async fn test_quota_enforcement() {
        let mem = Memory::new();
        let mut quota = ResourceQuota::new(2);

        // First value should fit
        mem.set("key1".to_string(), Value::Int(1), &mut quota)
            .await
            .unwrap();

        // Second might not
        let result = mem
            .set(
                "key2".to_string(),
                Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
                &mut quota,
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_releases_quota() {
        let mem = Memory::new();
        let mut quota = ResourceQuota::new(10);

        mem.set("key1".to_string(), Value::Int(42), &mut quota)
            .await
            .unwrap();

        let used_before = quota.used;

        mem.del("key1", &mut quota).await;

        assert!(quota.used < used_before);
    }

    #[tokio::test]
    async fn test_keys() {
        let mem = Memory::new();
        let mut quota = ResourceQuota::new(100);

        mem.set("a".to_string(), Value::Int(1), &mut quota)
            .await
            .unwrap();
        mem.set("b".to_string(), Value::Int(2), &mut quota)
            .await
            .unwrap();
        mem.set("c".to_string(), Value::Int(3), &mut quota)
            .await
            .unwrap();

        let keys = mem.keys().await;
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"a".to_string()));
        assert!(keys.contains(&"b".to_string()));
        assert!(keys.contains(&"c".to_string()));
    }
}
