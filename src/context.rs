//! Context - The execution environment
//!
//! Simple: a shared dictionary of tools and optional capabilities.

use crate::error::{Error, Result};
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
    pub capabilities: Vec<String>,
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
    pub fn new() -> Self {
        Self {
            dict: Arc::new(RwLock::new(Dictionary::new())),
            capabilities: Vec::new(),
        }
    }

    /// Create a new context with a shared dictionary
    pub fn with_dict(dict: Arc<RwLock<Dictionary>>) -> Self {
        Self {
            dict,
            capabilities: Vec::new(),
        }
    }

    /// Add a capability
    pub fn with_capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }

    /// Check if context has a capability
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.iter().any(|c| c == cap)
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
            .with_capability("io")
            .with_capability("http");

        assert!(ctx.has_capability("io"));
        assert!(ctx.has_capability("http"));
        assert!(!ctx.has_capability("shell"));
    }
}
