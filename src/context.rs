//! Context - The execution environment
//!
//! Contains:
//! - Tool dictionary
//! - Capabilities (what this execution can do)
//! - Resources (quotas and usage tracking)
//! - Memory (volatile session storage)
//! - Storage (persistent ROM)

use crate::capabilities::Capabilities;
use crate::error::{Error, Result};
use crate::memory::Memory;
use crate::resources::Resources;
use crate::storage::Storage;
use crate::tool::Tool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Execution context - everything a tool needs
#[derive(Clone)]
pub struct Context {
    /// Tool dictionary (shared, thread-safe)
    pub dict: Arc<RwLock<Dictionary>>,

    /// Capabilities (what this execution can do)
    pub caps: Arc<Capabilities>,

    /// Resource quotas and usage
    pub resources: Arc<RwLock<Resources>>,

    /// Session memory (volatile)
    pub memory: Memory,

    /// Persistent storage (optional, may not be available)
    pub storage: Option<Arc<Storage>>,
}

/// Tool dictionary - name → tool mapping
#[derive(Debug, Default)]
pub struct Dictionary {
    tools: HashMap<String, Tool>,
}

impl Dictionary {
    /// Create a new empty dictionary
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a tool by name
    pub fn get(&self, name: &str) -> Result<Tool> {
        if let Some(tool) = self.tools.get(name) {
            return Ok(tool.clone());
        }

        Err(Error::ToolNotFound(name.to_string()))
    }

    /// Register a tool
    pub fn register(&mut self, tool: Tool) {
        self.tools.insert(tool.name.clone(), tool);
    }

    /// Remove a tool
    pub fn remove(&mut self, name: &str) -> Option<Tool> {
        self.tools.remove(name)
    }

    /// List all tool names
    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<_> = self.tools.keys().cloned().collect();
        names.sort();
        names
    }

    /// Iterate over tool names
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.tools.keys().map(|s| s.as_str())
    }

    /// Check if a tool exists
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get number of tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Context {
    /// Create a new context with default settings
    /// Uses environment variables for configuration:
    /// - KORE_CAPS: capability string
    /// - KORE_MEM_LIMIT, KORE_ROM_LIMIT, etc: resource limits
    /// - KORE_STORAGE_PATH: persistent storage location
    pub fn new() -> Self {
        let storage = Storage::from_env().ok().map(Arc::new);

        Self {
            dict: Arc::new(RwLock::new(Dictionary::new())),
            caps: Arc::new(Capabilities::from_env()),
            resources: Arc::new(RwLock::new(Resources::from_env())),
            memory: Memory::new(),
            storage,
        }
    }

    /// Create with all capabilities (for trusted contexts)
    pub fn trusted() -> Self {
        let storage = Storage::from_env().ok().map(Arc::new);

        Self {
            dict: Arc::new(RwLock::new(Dictionary::new())),
            caps: Arc::new(Capabilities::all()),
            resources: Arc::new(RwLock::new(Resources::unlimited())),
            memory: Memory::new(),
            storage,
        }
    }

    /// Create a new context with a shared dictionary
    pub fn with_dict(dict: Arc<RwLock<Dictionary>>) -> Self {
        let storage = Storage::from_env().ok().map(Arc::new);

        Self {
            dict,
            caps: Arc::new(Capabilities::from_env()),
            resources: Arc::new(RwLock::new(Resources::from_env())),
            memory: Memory::new(),
            storage,
        }
    }

    /// Set capabilities
    pub fn with_caps(mut self, caps: Capabilities) -> Self {
        self.caps = Arc::new(caps);
        self
    }

    /// Set resources
    pub fn with_resources(mut self, resources: Resources) -> Self {
        self.resources = Arc::new(RwLock::new(resources));
        self
    }

    /// Set storage
    pub fn with_storage(mut self, storage: Storage) -> Self {
        self.storage = Some(Arc::new(storage));
        self
    }

    /// Check if context has a capability (legacy API)
    pub fn has_capability(&self, cap: &str) -> bool {
        self.caps.has(cap)
    }

    /// Add a capability (legacy API for compatibility)
    pub fn with_capability(self, cap: impl Into<String>) -> Self {
        // For backwards compatibility, parse the string
        let cap_str = cap.into();
        let mut new_caps = (*self.caps).clone();
        new_caps.add(&cap_str);
        Self {
            caps: Arc::new(new_caps),
            ..self
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::Op;

    #[test]
    fn test_dictionary_lookup() {
        let mut dict = Dictionary::new();

        // Add a tool
        let tool = Tool::composed("double", None, vec![Op::call("dup"), Op::call("add")]);
        dict.register(tool);

        // Look it up
        let found = dict.get("double").unwrap();
        assert_eq!(found.name, "double");

        // Not found
        assert!(dict.get("nonexistent").is_err());
    }

    #[test]
    fn test_context_capabilities() {
        let ctx = Context::new()
            .with_capability("exec")
            .with_capability("fs:read:/tmp");

        assert!(ctx.has_capability("exec"));
        assert!(ctx.has_capability("fs:read:/tmp"));
        assert!(!ctx.has_capability("shell"));
    }

    #[test]
    fn test_trusted_context() {
        let ctx = Context::trusted();

        assert!(ctx.caps.can_exec());
        assert!(ctx.caps.can_read_path(std::path::Path::new("/tmp")));
    }

    #[test]
    fn test_context_with_caps() {
        let caps = Capabilities::none()
            .with_fs_read("/workspace")
            .with_exec();

        let ctx = Context::new().with_caps(caps);

        assert!(ctx.caps.can_exec());
        assert!(ctx.caps.can_read_path(std::path::Path::new("/workspace/file.txt")));
        assert!(!ctx.caps.can_write_path(std::path::Path::new("/workspace/file.txt")));
    }
}
