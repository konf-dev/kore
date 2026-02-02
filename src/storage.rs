//! Persistent Storage - ROM for agents
//!
//! Data survives restarts. Uses filesystem-based JSON storage.
//! Simple, no dependencies beyond std.
//!
//! ## Design
//! - Each key is a file
//! - Values are JSON-serialized
//! - Quota tracked by file sizes
//! - Storage location configurable

use crate::resources::ResourceQuota;
use crate::value::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Persistent storage handle
#[derive(Debug, Clone)]
pub struct Storage {
    /// Directory where data is stored
    path: PathBuf,
}

impl Storage {
    /// Create storage at the given directory
    pub fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Create directory if it doesn't exist
        if !path.exists() {
            fs::create_dir_all(&path)?;
        }

        Ok(Self { path })
    }

    /// Create from environment variable KORE_STORAGE_PATH
    /// Defaults to ".kore_storage" in current directory
    pub fn from_env() -> io::Result<Self> {
        let path = std::env::var("KORE_STORAGE_PATH")
            .unwrap_or_else(|_| ".kore_storage".to_string());
        Self::new(path)
    }

    /// Get file path for a key
    fn key_path(&self, key: &str) -> PathBuf {
        // Sanitize key to be safe as filename
        let safe_key: String = key
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect();
        self.path.join(format!("{}.json", safe_key))
    }

    /// Calculate units for a value (matches memory calculation)
    fn value_units(value: &Value) -> u64 {
        match value {
            Value::Null => 1,
            Value::Bool(_) => 1,
            Value::Int(_) => 1,
            Value::Float(_) => 1,
            Value::Text(s) => 1 + (s.len() as u64 / 100),
            Value::List(v) => 1 + v.iter().map(Self::value_units).sum::<u64>(),
            Value::Map(m) => 1 + m.values().map(Self::value_units).sum::<u64>(),
            Value::Quote(ops) => 1 + ops.len() as u64,
            Value::Handle(_) => 1,
            Value::Error(_) => 1,
        }
    }

    /// Set a value
    pub fn set(
        &self,
        key: &str,
        value: &Value,
        quota: &mut ResourceQuota,
    ) -> Result<(), StorageError> {
        let new_size = Self::value_units(value);

        // Check existing size
        let old_size = self.get_size(key);
        let delta = new_size as i64 - old_size as i64;

        // Check quota if growing
        if delta > 0 {
            let needed = delta as u64;
            if !quota.can_allocate(needed) {
                return Err(StorageError::QuotaExceeded {
                    needed,
                    available: quota.free(),
                });
            }
            quota.try_allocate(needed);
        } else if delta < 0 {
            quota.release((-delta) as u64);
        }

        // Serialize and write
        let json = serde_json::to_string_pretty(value)
            .map_err(|e| StorageError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;

        let path = self.key_path(key);
        fs::write(&path, json).map_err(StorageError::Io)?;

        Ok(())
    }

    /// Get a value
    pub fn get(&self, key: &str) -> Result<Option<Value>, StorageError> {
        let path = self.key_path(key);

        if !path.exists() {
            return Ok(None);
        }

        let json = fs::read_to_string(&path).map_err(StorageError::Io)?;
        let value: Value = serde_json::from_str(&json)
            .map_err(|e| StorageError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;

        Ok(Some(value))
    }

    /// Check if key exists
    pub fn has(&self, key: &str) -> bool {
        self.key_path(key).exists()
    }

    /// Delete a value
    pub fn del(&self, key: &str, quota: &mut ResourceQuota) -> Result<bool, StorageError> {
        let path = self.key_path(key);

        if !path.exists() {
            return Ok(false);
        }

        // Release quota
        let size = self.get_size(key);
        quota.release(size);

        fs::remove_file(&path).map_err(StorageError::Io)?;
        Ok(true)
    }

    /// List all keys
    pub fn keys(&self) -> Result<Vec<String>, StorageError> {
        let mut keys = Vec::new();

        for entry in fs::read_dir(&self.path).map_err(StorageError::Io)? {
            let entry = entry.map_err(StorageError::Io)?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "json") {
                if let Some(stem) = path.file_stem() {
                    keys.push(stem.to_string_lossy().to_string());
                }
            }
        }

        keys.sort();
        Ok(keys)
    }

    /// Get size of a key in units
    fn get_size(&self, key: &str) -> u64 {
        match self.get(key) {
            Ok(Some(value)) => Self::value_units(&value),
            _ => 0,
        }
    }

    /// Calculate total usage
    pub fn usage(&self) -> Result<u64, StorageError> {
        let mut total = 0u64;

        for key in self.keys()? {
            total += self.get_size(&key);
        }

        Ok(total)
    }

    /// Number of keys
    pub fn len(&self) -> Result<usize, StorageError> {
        Ok(self.keys()?.len())
    }

    /// Is empty?
    pub fn is_empty(&self) -> Result<bool, StorageError> {
        Ok(self.len()? == 0)
    }

    /// Clear all data
    pub fn clear(&self, quota: &mut ResourceQuota) -> Result<(), StorageError> {
        for key in self.keys()? {
            self.del(&key, quota)?;
        }
        Ok(())
    }
}

/// Storage operation errors
#[derive(Debug)]
pub enum StorageError {
    /// IO error
    Io(io::Error),

    /// Quota exceeded
    QuotaExceeded { needed: u64, available: u64 },
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Io(e) => write!(f, "storage IO error: {}", e),
            StorageError::QuotaExceeded { needed, available } => {
                write!(
                    f,
                    "storage quota exceeded: need {} units, only {} available",
                    needed, available
                )
            }
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StorageError::Io(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_basic_set_get() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        let mut quota = ResourceQuota::new(100);

        storage
            .set("key1", &Value::Int(42), &mut quota)
            .unwrap();

        assert_eq!(storage.get("key1").unwrap(), Some(Value::Int(42)));
    }

    #[test]
    fn test_persistence() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        let mut quota = ResourceQuota::new(100);

        // Write with one instance
        {
            let storage = Storage::new(&path).unwrap();
            storage
                .set("key1", &Value::Text("hello".to_string()), &mut quota)
                .unwrap();
        }

        // Read with another instance
        {
            let storage = Storage::new(&path).unwrap();
            assert_eq!(
                storage.get("key1").unwrap(),
                Some(Value::Text("hello".to_string()))
            );
        }
    }

    #[test]
    fn test_quota_enforcement() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        let mut quota = ResourceQuota::new(2);

        // Small value should fit
        storage.set("key1", &Value::Int(1), &mut quota).unwrap();

        // Large value should fail
        let result = storage.set(
            "key2",
            &Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
            &mut quota,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_keys() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        let mut quota = ResourceQuota::new(100);

        storage.set("a", &Value::Int(1), &mut quota).unwrap();
        storage.set("b", &Value::Int(2), &mut quota).unwrap();
        storage.set("c", &Value::Int(3), &mut quota).unwrap();

        let keys = storage.keys().unwrap();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn test_delete() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        let mut quota = ResourceQuota::new(100);

        storage.set("key1", &Value::Int(42), &mut quota).unwrap();
        let used_before = quota.used;

        storage.del("key1", &mut quota).unwrap();

        assert!(!storage.has("key1"));
        assert!(quota.used < used_before);
    }
}
