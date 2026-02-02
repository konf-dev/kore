//! Meta - Metadata for any Thing
//!
//! Everything in Kore is a "Thing" with:
//! - A value (the actual code/data)
//! - Metadata (documentation, stats, health)
//!
//! Meta is the same for everything - tools, workflows, data.
//! No special treatment. Compose for power.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Metadata for any Thing in the system
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    /// Name (e.g., "math/square")
    pub name: String,

    /// Type signature (e.g., "(n -- n)")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sig: Option<String>,

    /// Human-readable documentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,

    /// Health status: "ok", "warn", "error"
    #[serde(default = "default_status")]
    pub status: String,

    /// Warning messages
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,

    /// Error messages
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,

    /// Tags for discovery
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Call statistics
    #[serde(default)]
    pub stats: Stats,

    /// Custom metadata fields
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub extra: IndexMap<String, String>,
}

fn default_status() -> String {
    "ok".to_string()
}

/// Call statistics - auto-tracked
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Stats {
    /// Total successful calls
    pub calls: u64,

    /// Total failures
    pub failures: u64,

    /// Total execution time (milliseconds)
    pub time_ms: u64,

    /// Last call timestamp (Unix epoch millis)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_call: Option<u64>,

    /// Last failure timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure: Option<u64>,
}

impl Meta {
    /// Create minimal metadata with just a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sig: None,
            doc: None,
            status: "ok".to_string(),
            warnings: Vec::new(),
            errors: Vec::new(),
            tags: Vec::new(),
            stats: Stats::default(),
            extra: IndexMap::new(),
        }
    }

    /// Create with signature
    pub fn with_sig(mut self, sig: impl Into<String>) -> Self {
        self.sig = Some(sig.into());
        self
    }

    /// Add documentation
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Record a successful call
    pub fn record_call(&mut self, duration: Duration) {
        self.stats.calls += 1;
        self.stats.time_ms += duration.as_millis() as u64;
        self.stats.last_call = Some(now_millis());
        self.update_status();
    }

    /// Record a failure
    pub fn record_failure(&mut self) {
        self.stats.failures += 1;
        self.stats.last_failure = Some(now_millis());
        self.update_status();
    }

    /// Add a warning
    pub fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
        self.update_status();
    }

    /// Add an error
    pub fn error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
        self.update_status();
    }

    /// Clear warnings
    pub fn clear_warnings(&mut self) {
        self.warnings.clear();
        self.update_status();
    }

    /// Clear errors
    pub fn clear_errors(&mut self) {
        self.errors.clear();
        self.update_status();
    }

    /// Update status based on current state
    fn update_status(&mut self) {
        if !self.errors.is_empty() {
            self.status = "error".to_string();
        } else if !self.warnings.is_empty() {
            self.status = "warn".to_string();
        } else if self.stats.failures > 0 {
            let failure_rate = self.failure_rate();
            if failure_rate > 0.1 {
                self.status = "warn".to_string();
            } else {
                self.status = "ok".to_string();
            }
        } else {
            self.status = "ok".to_string();
        }
    }

    /// Calculate failure rate (0.0 - 1.0)
    pub fn failure_rate(&self) -> f64 {
        let total = self.stats.calls + self.stats.failures;
        if total == 0 {
            0.0
        } else {
            self.stats.failures as f64 / total as f64
        }
    }

    /// Average call time in milliseconds
    pub fn avg_time_ms(&self) -> f64 {
        if self.stats.calls == 0 {
            0.0
        } else {
            self.stats.time_ms as f64 / self.stats.calls as f64
        }
    }

    /// Set a custom field
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.extra.insert(key.into(), value.into());
    }

    /// Get a custom field
    pub fn get(&self, key: &str) -> Option<&str> {
        self.extra.get(key).map(|s| s.as_str())
    }

    /// Convert to a Map Value for introspection
    pub fn to_value(&self) -> crate::value::Value {
        let mut map = IndexMap::new();

        map.insert("name".to_string(), crate::value::Value::Text(self.name.clone()));

        if let Some(ref sig) = self.sig {
            map.insert("sig".to_string(), crate::value::Value::Text(sig.clone()));
        }

        if let Some(ref doc) = self.doc {
            map.insert("doc".to_string(), crate::value::Value::Text(doc.clone()));
        }

        map.insert("status".to_string(), crate::value::Value::Text(self.status.clone()));

        if !self.warnings.is_empty() {
            map.insert(
                "warnings".to_string(),
                crate::value::Value::List(
                    self.warnings.iter().map(|w| crate::value::Value::Text(w.clone())).collect(),
                ),
            );
        }

        if !self.errors.is_empty() {
            map.insert(
                "errors".to_string(),
                crate::value::Value::List(
                    self.errors.iter().map(|e| crate::value::Value::Text(e.clone())).collect(),
                ),
            );
        }

        if !self.tags.is_empty() {
            map.insert(
                "tags".to_string(),
                crate::value::Value::List(
                    self.tags.iter().map(|t| crate::value::Value::Text(t.clone())).collect(),
                ),
            );
        }

        // Stats as a nested map
        let mut stats_map = IndexMap::new();
        stats_map.insert("calls".to_string(), crate::value::Value::Int(self.stats.calls as i64));
        stats_map.insert("failures".to_string(), crate::value::Value::Int(self.stats.failures as i64));
        stats_map.insert("time_ms".to_string(), crate::value::Value::Int(self.stats.time_ms as i64));
        if let Some(last) = self.stats.last_call {
            stats_map.insert("last_call".to_string(), crate::value::Value::Int(last as i64));
        }
        if let Some(last) = self.stats.last_failure {
            stats_map.insert("last_failure".to_string(), crate::value::Value::Int(last as i64));
        }
        map.insert("stats".to_string(), crate::value::Value::Map(stats_map));

        // Extra fields
        for (k, v) in &self.extra {
            map.insert(k.clone(), crate::value::Value::Text(v.clone()));
        }

        crate::value::Value::Map(map)
    }
}

/// Get current time as Unix millis
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_creation() {
        let meta = Meta::new("square")
            .with_sig("(n -- n)")
            .with_doc("Squares a number")
            .with_tag("math");

        assert_eq!(meta.name, "square");
        assert_eq!(meta.sig, Some("(n -- n)".to_string()));
        assert_eq!(meta.doc, Some("Squares a number".to_string()));
        assert_eq!(meta.tags, vec!["math".to_string()]);
        assert_eq!(meta.status, "ok");
    }

    #[test]
    fn test_stats_tracking() {
        let mut meta = Meta::new("test");

        meta.record_call(Duration::from_millis(10));
        meta.record_call(Duration::from_millis(20));
        meta.record_failure();

        assert_eq!(meta.stats.calls, 2);
        assert_eq!(meta.stats.failures, 1);
        assert_eq!(meta.stats.time_ms, 30);
        assert!(meta.stats.last_call.is_some());
        assert!(meta.stats.last_failure.is_some());
    }

    #[test]
    fn test_failure_rate() {
        let mut meta = Meta::new("test");

        // 8 calls, 2 failures = 20% failure rate
        for _ in 0..8 {
            meta.record_call(Duration::from_millis(1));
        }
        for _ in 0..2 {
            meta.record_failure();
        }

        assert!((meta.failure_rate() - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_status_updates() {
        let mut meta = Meta::new("test");
        assert_eq!(meta.status, "ok");

        meta.warn("Something suspicious");
        assert_eq!(meta.status, "warn");

        meta.error("Something bad");
        assert_eq!(meta.status, "error");

        meta.clear_errors();
        assert_eq!(meta.status, "warn");

        meta.clear_warnings();
        assert_eq!(meta.status, "ok");
    }

    #[test]
    fn test_to_value() {
        let meta = Meta::new("square")
            .with_sig("(n -- n)")
            .with_doc("Squares a number");

        let value = meta.to_value();
        assert!(matches!(value, crate::value::Value::Map(_)));
    }

    #[test]
    fn test_custom_fields() {
        let mut meta = Meta::new("test");
        meta.set("author", "Alice");
        meta.set("version", "1.0.0");

        assert_eq!(meta.get("author"), Some("Alice"));
        assert_eq!(meta.get("version"), Some("1.0.0"));
        assert_eq!(meta.get("nonexistent"), None);
    }
}
