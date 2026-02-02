//! Resources - Abstract resource accounting
//!
//! Resources are abstract units, not bytes or MHz.
//! The host runtime maps units to real resources.
//!
//! ## Resource Types
//! - `mem`: Volatile session memory (cleared on restart)
//! - `rom`: Persistent storage (survives restart)
//! - `compute`: Execution budget (time/instructions)
//! - `net`: Network transfer budget
//!
//! ## Design
//! - Each resource has: total, used, reserved
//! - Operations check quota before proceeding
//! - Quota exceeded = Error (not panic)

use serde::{Deserialize, Serialize};
use std::fmt;

/// Abstract resource quotas and usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resources {
    /// Volatile memory units
    pub mem: ResourceQuota,

    /// Persistent storage units
    pub rom: ResourceQuota,

    /// Compute units (optional metering)
    pub compute: ResourceQuota,

    /// Network transfer units
    pub net: ResourceQuota,
}

/// A single resource's quota and usage
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ResourceQuota {
    /// Maximum units available
    pub total: u64,

    /// Units currently used
    pub used: u64,

    /// Units reserved but not yet used
    pub reserved: u64,
}

impl ResourceQuota {
    /// Create with a total quota
    pub fn new(total: u64) -> Self {
        Self {
            total,
            used: 0,
            reserved: 0,
        }
    }

    /// Create unlimited (u64::MAX)
    pub fn unlimited() -> Self {
        Self::new(u64::MAX)
    }

    /// Free units available
    pub fn free(&self) -> u64 {
        self.total.saturating_sub(self.used).saturating_sub(self.reserved)
    }

    /// Can we allocate this many units?
    pub fn can_allocate(&self, units: u64) -> bool {
        self.free() >= units
    }

    /// Try to allocate units. Returns false if not enough.
    pub fn try_allocate(&mut self, units: u64) -> bool {
        if self.can_allocate(units) {
            self.used = self.used.saturating_add(units);
            true
        } else {
            false
        }
    }

    /// Release units back to pool
    pub fn release(&mut self, units: u64) {
        self.used = self.used.saturating_sub(units);
    }

    /// Reserve units (but don't use yet)
    pub fn reserve(&mut self, units: u64) -> bool {
        if self.free() >= units {
            self.reserved = self.reserved.saturating_add(units);
            true
        } else {
            false
        }
    }

    /// Commit reserved units to used
    pub fn commit_reserved(&mut self, units: u64) {
        let to_commit = units.min(self.reserved);
        self.reserved = self.reserved.saturating_sub(to_commit);
        self.used = self.used.saturating_add(to_commit);
    }

    /// Cancel reservation
    pub fn cancel_reserved(&mut self, units: u64) {
        self.reserved = self.reserved.saturating_sub(units);
    }

    /// Usage as percentage (0-100)
    pub fn usage_percent(&self) -> u8 {
        if self.total == 0 || self.total == u64::MAX {
            0
        } else {
            ((self.used * 100) / self.total).min(100) as u8
        }
    }
}

impl Resources {
    /// Create with default unlimited resources
    pub fn unlimited() -> Self {
        Self {
            mem: ResourceQuota::unlimited(),
            rom: ResourceQuota::unlimited(),
            compute: ResourceQuota::unlimited(),
            net: ResourceQuota::unlimited(),
        }
    }

    /// Create with specific limits
    pub fn with_limits(mem: u64, rom: u64, compute: u64, net: u64) -> Self {
        Self {
            mem: ResourceQuota::new(mem),
            rom: ResourceQuota::new(rom),
            compute: ResourceQuota::new(compute),
            net: ResourceQuota::new(net),
        }
    }

    /// Create from environment variables
    /// KORE_MEM_LIMIT, KORE_ROM_LIMIT, KORE_COMPUTE_LIMIT, KORE_NET_LIMIT
    pub fn from_env() -> Self {
        let mem = std::env::var("KORE_MEM_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(u64::MAX);

        let rom = std::env::var("KORE_ROM_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(u64::MAX);

        let compute = std::env::var("KORE_COMPUTE_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(u64::MAX);

        let net = std::env::var("KORE_NET_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(u64::MAX);

        Self::with_limits(mem, rom, compute, net)
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::from_env()
    }
}

impl fmt::Display for ResourceQuota {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.total == u64::MAX {
            write!(f, "{} used (unlimited)", self.used)
        } else {
            write!(f, "{}/{} ({}% used)", self.used, self.total, self.usage_percent())
        }
    }
}

impl fmt::Display for Resources {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "mem: {}", self.mem)?;
        writeln!(f, "rom: {}", self.rom)?;
        writeln!(f, "compute: {}", self.compute)?;
        write!(f, "net: {}", self.net)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quota_allocation() {
        let mut q = ResourceQuota::new(100);

        assert!(q.can_allocate(50));
        assert!(q.try_allocate(50));
        assert_eq!(q.used, 50);
        assert_eq!(q.free(), 50);

        assert!(q.try_allocate(50));
        assert_eq!(q.free(), 0);

        // Can't allocate more
        assert!(!q.can_allocate(1));
        assert!(!q.try_allocate(1));

        // Release
        q.release(30);
        assert_eq!(q.free(), 30);
    }

    #[test]
    fn test_reservation() {
        let mut q = ResourceQuota::new(100);

        // Reserve 60
        assert!(q.reserve(60));
        assert_eq!(q.reserved, 60);
        assert_eq!(q.free(), 40);

        // Can only allocate 40 more
        assert!(q.can_allocate(40));
        assert!(!q.can_allocate(41));

        // Commit reservation
        q.commit_reserved(60);
        assert_eq!(q.used, 60);
        assert_eq!(q.reserved, 0);
    }

    #[test]
    fn test_unlimited() {
        let q = ResourceQuota::unlimited();
        assert!(q.can_allocate(u64::MAX - 1));
    }
}
