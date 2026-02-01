//! Context - The execution environment
//!
//! The context carries everything needed for execution:
//! - A shared dictionary of available tools
//! - Caller and tenant identity for multi-tenancy
//! - Capabilities that gate what operations are allowed
//! - Resource limits for safety

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

    /// Caller identity
    pub caller: CallerId,

    /// Tenant isolation
    pub tenant: TenantId,

    /// Capabilities (what this execution can do)
    pub capabilities: Vec<String>,

    /// Resource limits
    pub limits: Limits,
}

/// Tool dictionary - name → tool mapping
#[derive(Debug, Default)]
pub struct Dictionary {
    /// Global tools (available to all tenants)
    global: HashMap<String, Tool>,

    /// Per-tenant tools
    tenant: HashMap<TenantId, HashMap<String, Tool>>,
}

impl Dictionary {
    /// Create a new empty dictionary
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a tool by name
    pub fn get(&self, name: &str, tenant: &TenantId) -> Result<Tool> {
        // 1. Try tenant-specific first
        if let Some(tenant_tools) = self.tenant.get(tenant) {
            if let Some(tool) = tenant_tools.get(name) {
                return Ok(tool.clone());
            }
        }

        // 2. Fall back to global
        if let Some(tool) = self.global.get(name) {
            return Ok(tool.clone());
        }

        // 3. Try with core/ prefix for unprefixed names
        if !name.contains('/') {
            let core_name = format!("core/{}", name);
            if let Some(tool) = self.global.get(&core_name) {
                return Ok(tool.clone());
            }
        }

        Err(Error::ToolNotFound(name.to_string()))
    }

    /// Register a global tool
    pub fn register_global(&mut self, tool: Tool) {
        self.global.insert(tool.name.clone(), tool);
    }

    /// Register a tool (alias for register_global)
    pub fn register(&mut self, tool: Tool) {
        self.register_global(tool);
    }

    /// Register a tenant-specific tool
    pub fn register_tenant(&mut self, tenant: TenantId, tool: Tool) {
        self.tenant
            .entry(tenant)
            .or_default()
            .insert(tool.name.clone(), tool);
    }

    /// Remove a tool
    pub fn remove(&mut self, name: &str, tenant: &TenantId) -> Option<Tool> {
        if let Some(tenant_tools) = self.tenant.get_mut(tenant) {
            if let Some(tool) = tenant_tools.remove(name) {
                return Some(tool);
            }
        }
        self.global.remove(name)
    }

    /// List all tool names for a tenant
    pub fn list(&self, tenant: &TenantId) -> Vec<String> {
        let mut names: Vec<_> = self.global.keys().cloned().collect();
        if let Some(tenant_tools) = self.tenant.get(tenant) {
            names.extend(tenant_tools.keys().cloned());
        }
        names.sort();
        names.dedup();
        names
    }
}

/// Caller identity - who is executing the program
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct CallerId(pub String);

impl CallerId {
    /// Create an anonymous caller (for unauthenticated requests)
    pub fn anonymous() -> Self {
        Self("anonymous".to_string())
    }

    /// Create a caller with a specific identity
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Tenant identity - isolation boundary for multi-tenant deployments
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TenantId(pub String);

impl TenantId {
    /// The default tenant for single-tenant deployments
    pub fn default_tenant() -> Self {
        Self("default".to_string())
    }

    /// Create a tenant with a specific identity
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Resource limits
#[derive(Debug, Clone)]
pub struct Limits {
    /// Maximum execution time (ms)
    pub timeout_ms: u64,

    /// Maximum stack depth
    pub max_stack_depth: usize,

    /// Maximum recursion depth
    pub max_recursion: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,      // 30 seconds
            max_stack_depth: 10_000, // 10k values
            max_recursion: 1_000,    // 1k calls deep
        }
    }
}

impl Context {
    /// Create a new context with default settings
    pub fn new() -> Self {
        Self {
            dict: Arc::new(RwLock::new(Dictionary::new())),
            caller: CallerId::anonymous(),
            tenant: TenantId::default_tenant(),
            capabilities: Vec::new(),
            limits: Limits::default(),
        }
    }

    /// Create a new context with a shared dictionary
    pub fn with_dict(dict: Arc<RwLock<Dictionary>>) -> Self {
        Self {
            dict,
            caller: CallerId::anonymous(),
            tenant: TenantId::default_tenant(),
            capabilities: Vec::new(),
            limits: Limits::default(),
        }
    }

    /// Create context with specific identity
    pub fn with_identity(mut self, caller: CallerId, tenant: TenantId) -> Self {
        self.caller = caller;
        self.tenant = tenant;
        self
    }

    /// Add a capability
    pub fn with_capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }

    /// Create context with specific limits
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
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

        // Add a global tool
        let tool = Tool::composed("math/double", None, vec![Op::call("dup"), Op::call("add")]);
        dict.register(tool);

        // Look it up
        let tenant = TenantId::default_tenant();
        let found = dict.get("math/double", &tenant).unwrap();
        assert_eq!(found.name, "math/double");

        // Not found
        assert!(dict.get("nonexistent", &tenant).is_err());
    }

    #[test]
    fn test_tenant_isolation() {
        let mut dict = Dictionary::new();
        let tenant_a = TenantId::new("tenant-a");
        let tenant_b = TenantId::new("tenant-b");

        // Register tenant-specific tool
        let tool = Tool::composed("my/tool", None, vec![]);
        dict.register_tenant(tenant_a.clone(), tool);

        // Tenant A can find it
        assert!(dict.get("my/tool", &tenant_a).is_ok());

        // Tenant B cannot
        assert!(dict.get("my/tool", &tenant_b).is_err());
    }

    #[test]
    fn test_context_capabilities() {
        let ctx = Context::new()
            .with_capability("io")
            .with_capability("http");

        assert!(ctx.capabilities.contains(&"io".to_string()));
        assert!(ctx.capabilities.contains(&"http".to_string()));
        assert!(!ctx.capabilities.contains(&"shell".to_string()));
    }
}
