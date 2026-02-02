//! Capabilities - What an agent is allowed to do
//!
//! No ambient authority. Every capability must be explicitly granted.
//!
//! ## Design Principles
//! - Deny by default
//! - Capabilities are checked BEFORE every operation
//! - Each capability is specific and minimal
//! - Capabilities can be queried by the agent
//!
//! ## Capability Types
//! - `fs:read:/path` - Can read files under /path
//! - `fs:write:/path` - Can write files under /path
//! - `net:connect:host:port` - Can connect to host:port
//! - `net:listen:port` - Can listen on port
//! - `exec` - Can run shell commands
//! - `spawn` - Can spawn sub-agents
//! - `env:read` - Can read environment variables
//! - `env:write` - Can write environment variables

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Capability set for an execution context
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Capabilities {
    /// Raw capability strings
    caps: HashSet<String>,

    /// Parsed file read paths
    fs_read: Vec<PathBuf>,

    /// Parsed file write paths
    fs_write: Vec<PathBuf>,

    /// Parsed network connect patterns (host:port or *:port)
    net_connect: Vec<NetPattern>,

    /// Ports allowed to listen on
    net_listen: Vec<u16>,

    /// Can execute shell commands
    can_exec: bool,

    /// Can spawn sub-agents
    can_spawn: bool,

    /// Can read environment variables
    can_env_read: bool,

    /// Can write environment variables
    can_env_write: bool,
}

/// Network connection pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetPattern {
    /// Host pattern ("*" for any, or specific host)
    pub host: String,

    /// Port (0 for any)
    pub port: u16,
}

impl NetPattern {
    /// Parse from "host:port" string
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.rsplitn(2, ':').collect();
        if parts.len() == 2 {
            let port = if parts[0] == "*" {
                0
            } else {
                parts[0].parse().ok()?
            };
            Some(Self {
                host: parts[1].to_string(),
                port,
            })
        } else {
            None
        }
    }

    /// Check if pattern matches a host:port
    pub fn matches(&self, host: &str, port: u16) -> bool {
        let host_matches = self.host == "*" || self.host == host;
        let port_matches = self.port == 0 || self.port == port;
        host_matches && port_matches
    }
}

impl Capabilities {
    /// Create empty capabilities (deny all)
    pub fn none() -> Self {
        Self::default()
    }

    /// Create with all capabilities (for trusted contexts)
    pub fn all() -> Self {
        Self {
            caps: HashSet::new(),
            fs_read: vec![PathBuf::from("/")],
            fs_write: vec![PathBuf::from("/")],
            net_connect: vec![NetPattern {
                host: "*".to_string(),
                port: 0,
            }],
            net_listen: vec![0], // 0 means any port
            can_exec: true,
            can_spawn: true,
            can_env_read: true,
            can_env_write: true,
        }
    }

    /// Create from environment variable KORE_CAPS
    /// Format: "fs:read:/path,fs:write:/path,net:connect:*:80,exec"
    pub fn from_env() -> Self {
        let caps_str = std::env::var("KORE_CAPS").unwrap_or_default();
        Self::parse(&caps_str)
    }

    /// Parse capability string
    pub fn parse(s: &str) -> Self {
        let mut caps = Self::none();

        for cap in s.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            caps.add(cap);
        }

        caps
    }

    /// Add a capability
    pub fn add(&mut self, cap: &str) {
        self.caps.insert(cap.to_string());

        let parts: Vec<&str> = cap.splitn(3, ':').collect();

        match parts.as_slice() {
            ["fs", "read", path] => {
                self.fs_read.push(PathBuf::from(path));
            }
            ["fs", "write", path] => {
                self.fs_write.push(PathBuf::from(path));
            }
            ["net", "connect", pattern] => {
                if let Some(p) = NetPattern::parse(pattern) {
                    self.net_connect.push(p);
                }
            }
            ["net", "listen", port] => {
                if let Ok(p) = port.parse() {
                    self.net_listen.push(p);
                }
            }
            ["exec"] => {
                self.can_exec = true;
            }
            ["spawn"] => {
                self.can_spawn = true;
            }
            ["env", "read"] => {
                self.can_env_read = true;
            }
            ["env", "write"] => {
                self.can_env_write = true;
            }
            ["all"] => {
                *self = Self::all();
            }
            _ => {
                // Unknown capability - still store in caps set
            }
        }
    }

    /// Check if has a specific raw capability
    pub fn has(&self, cap: &str) -> bool {
        // Check for "all" first
        if self.caps.contains("all") {
            return true;
        }
        self.caps.contains(cap)
    }

    /// List all raw capability strings
    pub fn list(&self) -> Vec<String> {
        let mut v: Vec<_> = self.caps.iter().cloned().collect();
        v.sort();
        v
    }

    // === Specific capability checks ===

    /// Can read this file path?
    pub fn can_read_path(&self, path: &Path) -> bool {
        if self.caps.contains("all") {
            return true;
        }

        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        self.fs_read.iter().any(|allowed| {
            let allowed = allowed.canonicalize().unwrap_or_else(|_| allowed.clone());
            path.starts_with(&allowed)
        })
    }

    /// Can write this file path?
    pub fn can_write_path(&self, path: &Path) -> bool {
        if self.caps.contains("all") {
            return true;
        }

        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        self.fs_write.iter().any(|allowed| {
            let allowed = allowed.canonicalize().unwrap_or_else(|_| allowed.clone());
            path.starts_with(&allowed)
        })
    }

    /// Can connect to this host:port?
    pub fn can_connect(&self, host: &str, port: u16) -> bool {
        if self.caps.contains("all") {
            return true;
        }

        self.net_connect.iter().any(|p| p.matches(host, port))
    }

    /// Can listen on this port?
    pub fn can_listen(&self, port: u16) -> bool {
        if self.caps.contains("all") {
            return true;
        }

        self.net_listen.iter().any(|&p| p == 0 || p == port)
    }

    /// Can execute shell commands?
    pub fn can_exec(&self) -> bool {
        self.caps.contains("all") || self.can_exec
    }

    /// Can spawn sub-agents?
    pub fn can_spawn(&self) -> bool {
        self.caps.contains("all") || self.can_spawn
    }

    /// Can read environment variables?
    pub fn can_env_read(&self) -> bool {
        self.caps.contains("all") || self.can_env_read
    }

    /// Can write environment variables?
    pub fn can_env_write(&self) -> bool {
        self.caps.contains("all") || self.can_env_write
    }

    // === Builders ===

    /// Add file read capability
    pub fn with_fs_read(mut self, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        self.add(&format!("fs:read:{}", path.display()));
        self
    }

    /// Add file write capability
    pub fn with_fs_write(mut self, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        self.add(&format!("fs:write:{}", path.display()));
        self
    }

    /// Add network connect capability
    pub fn with_net_connect(mut self, host: &str, port: u16) -> Self {
        self.add(&format!("net:connect:{}:{}", host, port));
        self
    }

    /// Add network listen capability
    pub fn with_net_listen(mut self, port: u16) -> Self {
        self.add(&format!("net:listen:{}", port));
        self
    }

    /// Add exec capability
    pub fn with_exec(mut self) -> Self {
        self.add("exec");
        self
    }

    /// Add spawn capability
    pub fn with_spawn(mut self) -> Self {
        self.add("spawn");
        self
    }

    /// Add env read capability
    pub fn with_env_read(mut self) -> Self {
        self.add("env:read");
        self
    }

    /// Add env write capability
    pub fn with_env_write(mut self) -> Self {
        self.add("env:write");
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_caps() {
        let caps = Capabilities::none();
        assert!(!caps.can_exec());
        assert!(!caps.can_read_path(Path::new("/tmp/test")));
    }

    #[test]
    fn test_all_caps() {
        let caps = Capabilities::all();
        assert!(caps.can_exec());
        assert!(caps.can_read_path(Path::new("/tmp/test")));
        assert!(caps.can_connect("example.com", 80));
    }

    #[test]
    fn test_parse_caps() {
        let caps = Capabilities::parse("fs:read:/tmp,net:connect:*:80,exec");

        assert!(caps.can_exec());
        assert!(caps.can_read_path(Path::new("/tmp/test")));
        assert!(!caps.can_write_path(Path::new("/tmp/test")));
        assert!(caps.can_connect("example.com", 80));
        assert!(!caps.can_connect("example.com", 443));
    }

    #[test]
    fn test_net_pattern() {
        let p = NetPattern::parse("*:80").unwrap();
        assert!(p.matches("example.com", 80));
        assert!(!p.matches("example.com", 443));

        let p = NetPattern::parse("example.com:*").unwrap();
        assert!(p.matches("example.com", 80));
        assert!(p.matches("example.com", 443));
        assert!(!p.matches("other.com", 80));
    }

    #[test]
    fn test_builder() {
        let caps = Capabilities::none()
            .with_fs_read("/workspace")
            .with_net_connect("api.example.com", 443)
            .with_exec();

        assert!(caps.can_exec());
        assert!(caps.can_connect("api.example.com", 443));
        assert!(!caps.can_connect("api.example.com", 80));
    }
}
