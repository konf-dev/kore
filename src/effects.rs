//! Static Effect Inference
//!
//! Tracks IO effects at static analysis time - which capabilities a program REQUIRES.
//! This enables sandboxing verification without runtime cost.
//!
//! ## Effect Categories
//!
//! | Effect | Capability | Operations |
//! |--------|------------|------------|
//! | `fs`   | fs:read, fs:write | fs-read, fs-write, fs-exists |
//! | `net`  | net:connect, net:listen | http-get, http-post, net-* |
//! | `spawn`| spawn | spawn |
//! | `time` | time | now, sleep |
//! | `io`   | io | print, println |
//! | `env`  | env:read, env:write | env-get, env-set |
//! | `exec` | exec | exec, shell |
//!
//! ## Mathematical Foundation
//!
//! Effects form a join-semilattice:
//! - ⊥ (bottom) = no effects (pure)
//! - E₁ ∪ E₂ = union of effects
//! - E ≤ E' iff E ⊆ E' (effect subsumption)
//!
//! For composition: effects(A ; B) = effects(A) ∪ effects(B)

use std::fmt;
use std::collections::HashSet;

/// The set of IO effects a program may perform
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectSet {
    /// File system access
    pub fs: bool,
    /// Network access
    pub net: bool,
    /// Spawn sub-agents
    pub spawn: bool,
    /// Time operations (now, sleep)
    pub time: bool,
    /// Console IO (print, println)
    pub io: bool,
    /// Environment variable access
    pub env: bool,
    /// Shell command execution
    pub exec: bool,
    /// Memory operations (non-deterministic state)
    pub mem: bool,
}

impl EffectSet {
    /// Create an empty effect set (pure)
    pub fn pure() -> Self {
        Self::default()
    }

    /// Create an effect set with all effects
    pub fn all() -> Self {
        Self {
            fs: true,
            net: true,
            spawn: true,
            time: true,
            io: true,
            env: true,
            exec: true,
            mem: true,
        }
    }

    /// Check if this is pure (no effects)
    pub fn is_pure(&self) -> bool {
        !self.fs && !self.net && !self.spawn && !self.time 
            && !self.io && !self.env && !self.exec && !self.mem
    }

    /// Union of two effect sets (join in the semilattice)
    pub fn union(&self, other: &Self) -> Self {
        Self {
            fs: self.fs || other.fs,
            net: self.net || other.net,
            spawn: self.spawn || other.spawn,
            time: self.time || other.time,
            io: self.io || other.io,
            env: self.env || other.env,
            exec: self.exec || other.exec,
            mem: self.mem || other.mem,
        }
    }

    /// Check if this effect set is a subset of another (⊆)
    pub fn subset_of(&self, other: &Self) -> bool {
        (!self.fs || other.fs) &&
        (!self.net || other.net) &&
        (!self.spawn || other.spawn) &&
        (!self.time || other.time) &&
        (!self.io || other.io) &&
        (!self.env || other.env) &&
        (!self.exec || other.exec) &&
        (!self.mem || other.mem)
    }

    /// Get the effect for a builtin tool name
    pub fn for_tool(name: &str) -> Self {
        let mut effects = Self::pure();
        
        match name {
            // File system
            "fs-read" | "fs-write" | "fs-exists" | "fs-delete" | "fs-list" | "fs-mkdir" => {
                effects.fs = true;
            }
            
            // Network
            "http-get" | "http-post" | "http-put" | "http-delete" |
            "net-connect" | "net-listen" | "net-send" | "net-recv" => {
                effects.net = true;
            }
            
            // Spawn
            "spawn" => {
                effects.spawn = true;
            }
            
            // Time
            "now" | "sleep" => {
                effects.time = true;
            }
            
            // Console IO
            "print" | "println" | "input" | "read-line" => {
                effects.io = true;
            }
            
            // Environment
            "env-get" | "env-set" | "env-has" => {
                effects.env = true;
            }
            
            // Execution
            "exec" | "shell" | "system" => {
                effects.exec = true;
            }
            
            // Memory (stateful, non-deterministic)
            "mem-get" | "mem-set" | "mem-del" | "mem-keys" |
            "rom-get" | "rom-set" | "rom-del" | "rom-keys" => {
                effects.mem = true;
            }
            
            // All other tools are pure
            _ => {}
        }
        
        effects
    }

    /// Convert to a list of effect names
    pub fn to_list(&self) -> Vec<&'static str> {
        let mut list = Vec::new();
        if self.fs { list.push("fs"); }
        if self.net { list.push("net"); }
        if self.spawn { list.push("spawn"); }
        if self.time { list.push("time"); }
        if self.io { list.push("io"); }
        if self.env { list.push("env"); }
        if self.exec { list.push("exec"); }
        if self.mem { list.push("mem"); }
        list
    }

    /// Parse from a list of effect names
    pub fn from_list(names: &[&str]) -> Self {
        let mut effects = Self::pure();
        for name in names {
            match *name {
                "fs" => effects.fs = true,
                "net" => effects.net = true,
                "spawn" => effects.spawn = true,
                "time" => effects.time = true,
                "io" => effects.io = true,
                "env" => effects.env = true,
                "exec" => effects.exec = true,
                "mem" => effects.mem = true,
                _ => {}
            }
        }
        effects
    }

    /// Create a HashSet of required capabilities from these effects
    pub fn required_capabilities(&self) -> HashSet<String> {
        let mut caps = HashSet::new();
        if self.fs { 
            caps.insert("fs:read".to_string()); 
            caps.insert("fs:write".to_string()); 
        }
        if self.net { 
            caps.insert("net:connect".to_string()); 
        }
        if self.spawn { 
            caps.insert("spawn".to_string()); 
        }
        if self.time { 
            caps.insert("time".to_string()); 
        }
        if self.io { 
            caps.insert("io".to_string()); 
        }
        if self.env { 
            caps.insert("env:read".to_string()); 
        }
        if self.exec { 
            caps.insert("exec".to_string()); 
        }
        if self.mem { 
            caps.insert("mem".to_string()); 
        }
        caps
    }
}

impl fmt::Display for EffectSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let list = self.to_list();
        if list.is_empty() {
            write!(f, "pure")
        } else {
            write!(f, "{{{}}}", list.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_effect() {
        let e = EffectSet::pure();
        assert!(e.is_pure());
        assert_eq!(format!("{}", e), "pure");
    }

    #[test]
    fn test_effect_union() {
        let e1 = EffectSet { fs: true, ..Default::default() };
        let e2 = EffectSet { net: true, ..Default::default() };
        let union = e1.union(&e2);
        assert!(union.fs);
        assert!(union.net);
        assert!(!union.spawn);
    }

    #[test]
    fn test_effect_subset() {
        let e1 = EffectSet { fs: true, ..Default::default() };
        let e2 = EffectSet { fs: true, net: true, ..Default::default() };
        assert!(e1.subset_of(&e2));
        assert!(!e2.subset_of(&e1));
    }

    #[test]
    fn test_for_tool_fs() {
        let e = EffectSet::for_tool("fs-read");
        assert!(e.fs);
        assert!(!e.net);
    }

    #[test]
    fn test_for_tool_pure() {
        let e = EffectSet::for_tool("add");
        assert!(e.is_pure());
    }

    #[test]
    fn test_to_list() {
        let e = EffectSet { fs: true, io: true, ..Default::default() };
        let list = e.to_list();
        assert!(list.contains(&"fs"));
        assert!(list.contains(&"io"));
        assert!(!list.contains(&"net"));
    }

    #[test]
    fn test_display() {
        let e = EffectSet { fs: true, net: true, ..Default::default() };
        let s = format!("{}", e);
        assert!(s.contains("fs"));
        assert!(s.contains("net"));
    }

    #[test]
    fn test_required_capabilities() {
        let e = EffectSet { fs: true, spawn: true, ..Default::default() };
        let caps = e.required_capabilities();
        assert!(caps.contains("fs:read"));
        assert!(caps.contains("spawn"));
        assert!(!caps.contains("net:connect"));
    }
}
