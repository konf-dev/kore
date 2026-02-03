//! Algebraic Foundations of Kore
//!
//! This module defines the mathematical structures that guarantee safety.
//!
//! # Capability Lattice
//!
//! Capabilities form a bounded lattice (L, ≤, ∧, ∨, ⊥, ⊤) where:
//! - L is the set of all capabilities
//! - ≤ is the "weaker than" partial order
//! - ∧ (meet) is the greatest lower bound (intersection)
//! - ∨ (join) is the least upper bound (union)
//! - ⊥ (bottom) is the empty capability (can do nothing)
//! - ⊤ (top) is the full capability (can do everything)
//!
//! Key property: Attenuation can only move DOWN the lattice.
//! `attenuate(c) ≤ c` always holds.
//!
//! # Resource Monoid
//!
//! Resources form a commutative monoid (R, ⊕, 0) with:
//! - R is the set of resource bundles
//! - ⊕ is resource addition
//! - 0 is the empty resource bundle
//!
//! Additionally, resources support splitting:
//! `split(r, p) = (r₁, r₂)` where `r₁ ⊕ r₂ = r` and |r₁|/|r| ≈ p
//!
//! Key property: Conservation. Resources are never created, only split or consumed.
//!
//! # Trace Algebra
//!
//! Execution traces form a monoid (T, ·, ε) where:
//! - T is the set of all traces
//! - · is trace concatenation
//! - ε is the empty trace
//!
//! Traces satisfy: `trace(f ; g) = trace(f) · trace(g)`
//! (Postulate 3: Composition is concatenation)

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

// ============================================================================
// CAPABILITY LATTICE
// ============================================================================

/// A capability is an atomic permission.
/// 
/// Capabilities follow the principle of least privilege.
/// Format: `domain:action:target`
/// 
/// Examples:
/// - `fs:read:/home/user` - read files under /home/user
/// - `net:connect:api.example.com:443` - connect to specific host
/// - `spawn` - create child contexts
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Cap(String);

impl Cap {
    pub fn new(s: impl Into<String>) -> Self {
        Cap(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parse capability into (domain, action, target)
    pub fn parts(&self) -> (&str, Option<&str>, Option<&str>) {
        let mut parts = self.0.splitn(3, ':');
        let domain = parts.next().unwrap_or("");
        let action = parts.next();
        let target = parts.next();
        (domain, action, target)
    }

    /// Check if this capability implies another.
    /// `a.implies(b)` means "if you have a, you can do b"
    /// 
    /// Rules:
    /// - `fs:*` implies `fs:read`, `fs:write`, etc.
    /// - `fs:read:/home` implies `fs:read:/home/user`
    /// - `*` implies everything
    pub fn implies(&self, other: &Cap) -> bool {
        // Universal capability implies everything
        if self.0 == "*" {
            return true;
        }

        let (d1, a1, t1) = self.parts();
        let (d2, a2, t2) = other.parts();

        // Domain must match (or be wildcard)
        if d1 != "*" && d1 != d2 {
            return false;
        }

        // If no action specified in self, we only care about domain
        let Some(action1) = a1 else {
            return a2.is_none();
        };

        // Action must match (or be wildcard)
        let Some(action2) = a2 else {
            return false;
        };

        if action1 != "*" && action1 != action2 {
            return false;
        }

        // Target matching (path prefix for fs, exact for others)
        match (t1, t2) {
            (None, None) => true,
            (Some(_), None) => false,
            (None, Some(_)) => true, // No target restriction
            (Some(target1), Some(target2)) => {
                if d1 == "fs" {
                    // Path prefix matching
                    target2.starts_with(target1)
                } else {
                    // Exact match or wildcard
                    target1 == "*" || target1 == target2
                }
            }
        }
    }
}

impl fmt::Display for Cap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Cap {
    fn from(s: &str) -> Self {
        Cap::new(s)
    }
}

impl From<String> for Cap {
    fn from(s: String) -> Self {
        Cap(s)
    }
}

/// A capability set forms a lattice.
/// 
/// Ordering: A ≤ B iff every capability in A is implied by some capability in B.
/// Meet: A ∧ B = capabilities implied by both A and B
/// Join: A ∨ B = union of capabilities
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CapSet {
    caps: BTreeSet<Cap>,
}

impl CapSet {
    /// Bottom element: empty capabilities (can do nothing)
    pub fn bottom() -> Self {
        Self { caps: BTreeSet::new() }
    }

    /// Top element: universal capability (can do everything)
    pub fn top() -> Self {
        let mut caps = BTreeSet::new();
        caps.insert(Cap::new("*"));
        Self { caps }
    }

    /// Create from a list of capabilities
    pub fn from_caps(caps: impl IntoIterator<Item = Cap>) -> Self {
        Self {
            caps: caps.into_iter().collect(),
        }
    }

    /// Parse from comma-separated string
    pub fn parse(s: &str) -> Self {
        let caps = s
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(Cap::new)
            .collect();
        Self { caps }
    }

    /// Add a capability
    pub fn add(&mut self, cap: impl Into<Cap>) {
        self.caps.insert(cap.into());
    }

    /// Check if this set allows a specific capability
    pub fn allows(&self, cap: &Cap) -> bool {
        self.caps.iter().any(|c| c.implies(cap))
    }

    /// Check if this set allows a capability string
    pub fn allows_str(&self, cap: &str) -> bool {
        self.allows(&Cap::new(cap))
    }

    /// Lattice ordering: self ≤ other
    /// True if everything self can do, other can also do.
    pub fn leq(&self, other: &CapSet) -> bool {
        self.caps.iter().all(|c| other.allows(c))
    }

    /// Lattice meet: greatest lower bound (intersection of powers)
    /// Returns capabilities that both sets can do.
    pub fn meet(&self, other: &CapSet) -> CapSet {
        let caps = self
            .caps
            .iter()
            .filter(|c| other.allows(c))
            .cloned()
            .collect();
        CapSet { caps }
    }

    /// Lattice join: least upper bound (union of powers)
    pub fn join(&self, other: &CapSet) -> CapSet {
        let caps = self.caps.union(&other.caps).cloned().collect();
        CapSet { caps }
    }

    /// Attenuate: create a subset of capabilities.
    /// 
    /// CRITICAL PROPERTY: The result is ALWAYS ≤ self.
    /// This is the only way to create child capabilities.
    /// 
    /// If `allowed` contains capabilities not in `self`, they are ignored.
    pub fn attenuate(&self, allowed: &CapSet) -> CapSet {
        // Only keep capabilities that self actually has
        let caps = allowed
            .caps
            .iter()
            .filter(|c| self.allows(c))
            .cloned()
            .collect();
        CapSet { caps }
    }

    /// List all capabilities
    pub fn list(&self) -> Vec<&Cap> {
        self.caps.iter().collect()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.caps.is_empty()
    }

    /// Number of capabilities
    pub fn len(&self) -> usize {
        self.caps.len()
    }

    /// Create from a Capabilities struct
    pub fn from_capabilities(caps: &crate::capabilities::Capabilities) -> Self {
        let cap_strings = caps.list();
        Self::from_caps(cap_strings.into_iter().map(Cap::new))
    }

    /// Convert to a Capabilities struct
    pub fn to_capabilities(&self) -> crate::capabilities::Capabilities {
        let cap_str = self
            .caps
            .iter()
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join(",");
        crate::capabilities::Capabilities::parse(&cap_str)
    }
}

impl fmt::Display for CapSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let caps: Vec<_> = self.caps.iter().map(|c| c.as_str()).collect();
        write!(f, "{{{}}}", caps.join(", "))
    }
}

// ============================================================================
// RESOURCE MONOID
// ============================================================================

/// A resource bundle with conservation laws.
/// 
/// Resources obey: `split(r) = (r1, r2)` where `r1 + r2 = r`
/// This ensures resources are never created, only divided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Res {
    /// Memory units (volatile)
    pub mem: u64,
    /// Storage units (persistent)
    pub rom: u64,
    /// Compute units (execution budget)
    pub compute: u64,
    /// Network units (transfer budget)
    pub net: u64,
}

impl Res {
    /// Zero resources (monoid identity)
    pub const ZERO: Res = Res {
        mem: 0,
        rom: 0,
        compute: 0,
        net: 0,
    };

    /// Unlimited resources (for trusted contexts)
    pub const UNLIMITED: Res = Res {
        mem: u64::MAX,
        rom: u64::MAX,
        compute: u64::MAX,
        net: u64::MAX,
    };

    /// Create with specific amounts
    pub fn new(mem: u64, rom: u64, compute: u64, net: u64) -> Self {
        Self { mem, rom, compute, net }
    }

    /// Monoid addition: r1 ⊕ r2
    /// Saturates at u64::MAX to prevent overflow
    pub fn add(&self, other: &Res) -> Res {
        Res {
            mem: self.mem.saturating_add(other.mem),
            rom: self.rom.saturating_add(other.rom),
            compute: self.compute.saturating_add(other.compute),
            net: self.net.saturating_add(other.net),
        }
    }

    /// Subtraction (for consumption)
    /// Returns None if any component would go negative
    pub fn sub(&self, other: &Res) -> Option<Res> {
        Some(Res {
            mem: self.mem.checked_sub(other.mem)?,
            rom: self.rom.checked_sub(other.rom)?,
            compute: self.compute.checked_sub(other.compute)?,
            net: self.net.checked_sub(other.net)?,
        })
    }

    /// Check if we have at least this much
    pub fn has(&self, required: &Res) -> bool {
        self.mem >= required.mem
            && self.rom >= required.rom
            && self.compute >= required.compute
            && self.net >= required.net
    }

    /// Split resources by ratio (0.0 to 1.0).
    /// 
    /// CONSERVATION LAW: split(r, p) = (r1, r2) where r1 + r2 = r
    /// 
    /// Returns (portion for child, remaining for parent)
    pub fn split(&self, ratio: f64) -> (Res, Res) {
        let ratio = ratio.clamp(0.0, 1.0);

        // Calculate child's portion
        let child = Res {
            mem: ((self.mem as f64) * ratio) as u64,
            rom: ((self.rom as f64) * ratio) as u64,
            compute: ((self.compute as f64) * ratio) as u64,
            net: ((self.net as f64) * ratio) as u64,
        };

        // Parent gets the rest (conservation!)
        let parent = Res {
            mem: self.mem - child.mem,
            rom: self.rom - child.rom,
            compute: self.compute - child.compute,
            net: self.net - child.net,
        };

        (child, parent)
    }

    /// Split with specific amounts for child.
    /// 
    /// Returns None if child would get more than available.
    /// Returns (child_resources, remaining_parent_resources)
    pub fn split_exact(&self, child_request: &Res) -> Option<(Res, Res)> {
        let parent = self.sub(child_request)?;
        Some((*child_request, parent))
    }

    /// Consume resources (reduce available).
    /// Returns the new resource state, or None if insufficient.
    pub fn consume(&self, amount: &Res) -> Option<Res> {
        self.sub(amount)
    }

    /// Total resource units (for rough comparison)
    pub fn total(&self) -> u64 {
        self.mem
            .saturating_add(self.rom)
            .saturating_add(self.compute)
            .saturating_add(self.net)
    }

    /// Check if unlimited
    pub fn is_unlimited(&self) -> bool {
        self.mem == u64::MAX
            && self.rom == u64::MAX
            && self.compute == u64::MAX
            && self.net == u64::MAX
    }
}

impl Default for Res {
    fn default() -> Self {
        Self::UNLIMITED
    }
}

impl fmt::Display for Res {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_unlimited() {
            write!(f, "Res(∞)")
        } else {
            write!(
                f,
                "Res(mem={}, rom={}, compute={}, net={})",
                self.mem, self.rom, self.compute, self.net
            )
        }
    }
}

// ============================================================================
// TRACE ALGEBRA
// ============================================================================

/// A single execution step in a trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    /// Tool that executed
    pub tool: String,

    /// Hash of input stack (for verification)
    pub input_hash: u64,

    /// Hash of output stack
    pub output_hash: u64,

    /// Capabilities used
    pub caps_used: Vec<Cap>,

    /// Resources consumed
    pub res_consumed: Res,

    /// Timestamp (nanoseconds since epoch)
    pub timestamp: u64,
}

impl TraceStep {
    pub fn new(tool: impl Into<String>) -> Self {
        Self {
            tool: tool.into(),
            input_hash: 0,
            output_hash: 0,
            caps_used: Vec::new(),
            res_consumed: Res::ZERO,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
        }
    }

    /// Compute a fingerprint of this step
    pub fn fingerprint(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.tool.hash(&mut hasher);
        self.input_hash.hash(&mut hasher);
        self.output_hash.hash(&mut hasher);
        hasher.finish()
    }
}

/// An execution trace (monoid under concatenation).
/// 
/// Traces form a monoid:
/// - Identity: empty trace
/// - Operation: concatenation
/// 
/// Property: trace(f ; g) = trace(f) · trace(g)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Trace {
    steps: Vec<TraceStep>,
}

impl Trace {
    /// Empty trace (monoid identity)
    pub fn empty() -> Self {
        Self { steps: Vec::new() }
    }

    /// Create trace with a single step
    pub fn single(step: TraceStep) -> Self {
        Self { steps: vec![step] }
    }

    /// Monoid operation: concatenate two traces
    pub fn concat(&self, other: &Trace) -> Trace {
        let mut steps = self.steps.clone();
        steps.extend(other.steps.iter().cloned());
        Trace { steps }
    }

    /// Append a step (mutating version)
    pub fn push(&mut self, step: TraceStep) {
        self.steps.push(step);
    }

    /// Get all steps
    pub fn steps(&self) -> &[TraceStep] {
        &self.steps
    }

    /// Number of steps
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Compute a fingerprint of the entire trace
    pub fn fingerprint(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        for step in &self.steps {
            step.fingerprint().hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Total resources consumed
    pub fn total_resources(&self) -> Res {
        self.steps.iter().fold(Res::ZERO, |acc, step| acc.add(&step.res_consumed))
    }

    /// All capabilities used
    pub fn all_caps_used(&self) -> Vec<&Cap> {
        self.steps.iter().flat_map(|s| s.caps_used.iter()).collect()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cap_implies() {
        // Wildcard implies everything
        assert!(Cap::new("*").implies(&Cap::new("fs:read:/tmp")));
        assert!(Cap::new("*").implies(&Cap::new("net:connect:example.com:80")));

        // Domain wildcard
        assert!(Cap::new("fs:*").implies(&Cap::new("fs:read")));
        assert!(Cap::new("fs:*").implies(&Cap::new("fs:write:/tmp")));
        assert!(!Cap::new("fs:*").implies(&Cap::new("net:connect")));

        // Path prefix
        assert!(Cap::new("fs:read:/home").implies(&Cap::new("fs:read:/home/user")));
        assert!(Cap::new("fs:read:/home").implies(&Cap::new("fs:read:/home/user/file.txt")));
        assert!(!Cap::new("fs:read:/home/user").implies(&Cap::new("fs:read:/home")));

        // Exact match
        assert!(Cap::new("exec").implies(&Cap::new("exec")));
        assert!(!Cap::new("exec").implies(&Cap::new("spawn")));
    }

    #[test]
    fn test_capset_attenuate() {
        let parent = CapSet::parse("fs:read:/home,fs:write:/tmp,net:connect:*:443");

        // Child requests subset - gets what they asked for
        let child_request = CapSet::parse("fs:read:/home/user");
        let child = parent.attenuate(&child_request);
        assert!(child.allows_str("fs:read:/home/user"));

        // Child requests something parent doesn't have - doesn't get it
        let greedy_request = CapSet::parse("fs:read:/home,exec");
        let child = parent.attenuate(&greedy_request);
        assert!(child.allows_str("fs:read:/home"));
        assert!(!child.allows_str("exec")); // Parent doesn't have exec!

        // Attenuate is always ≤ parent
        assert!(child.leq(&parent));
    }

    #[test]
    fn test_capset_lattice() {
        let a = CapSet::parse("fs:read,net:connect");
        let b = CapSet::parse("fs:write,net:connect");

        // Meet: what both can do
        let meet = a.meet(&b);
        assert!(meet.allows_str("net:connect"));
        assert!(!meet.allows_str("fs:read"));
        assert!(!meet.allows_str("fs:write"));

        // Join: union
        let join = a.join(&b);
        assert!(join.allows_str("fs:read"));
        assert!(join.allows_str("fs:write"));
        assert!(join.allows_str("net:connect"));
    }

    #[test]
    fn test_res_conservation() {
        let r = Res::new(1000, 500, 2000, 100);

        // Split preserves total
        let (child, parent) = r.split(0.3);
        let recombined = child.add(&parent);
        assert_eq!(r.mem, recombined.mem);
        assert_eq!(r.rom, recombined.rom);
        assert_eq!(r.compute, recombined.compute);
        assert_eq!(r.net, recombined.net);
    }

    #[test]
    fn test_res_split_exact() {
        let r = Res::new(1000, 500, 2000, 100);

        // Valid split
        let request = Res::new(300, 200, 500, 50);
        let (child, parent) = r.split_exact(&request).unwrap();
        assert_eq!(child, request);
        assert_eq!(parent.mem, 700);
        assert_eq!(parent.rom, 300);

        // Invalid split (too greedy)
        let greedy = Res::new(2000, 0, 0, 0);
        assert!(r.split_exact(&greedy).is_none());
    }

    #[test]
    fn test_trace_monoid() {
        let t1 = Trace::single(TraceStep::new("dup"));
        let t2 = Trace::single(TraceStep::new("add"));

        // Concatenation
        let t3 = t1.concat(&t2);
        assert_eq!(t3.len(), 2);
        assert_eq!(t3.steps()[0].tool, "dup");
        assert_eq!(t3.steps()[1].tool, "add");

        // Identity
        let empty = Trace::empty();
        let t4 = t1.concat(&empty);
        assert_eq!(t4.len(), 1);
    }
}
