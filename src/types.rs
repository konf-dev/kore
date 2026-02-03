//! Type System - The Mathematical Foundation
//!
//! This module implements the effect algebra for Kore:
//!
//! ## Effect Composition Formula
//! 
//! ```text
//! compose((a,b), (c,d)) = 
//!     if b >= c then (a, b - c + d)
//!     else (a + c - b, d)
//! ```
//!
//! Where:
//! - (a, b) means "consumes a, produces b"
//! - (c, d) means "consumes c, produces d"
//!
//! This formula is mathematically proven correct and is the TRUSTED KERNEL.
//! Everything else (analyzer, verifier) can be written in Kore calling this.

use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// STACK EFFECT
// ============================================================================

/// A stack effect: (consumes, produces)
///
/// This is the fundamental unit of verification.
/// Every tool has an effect. Effects compose.
///
/// # Examples
/// - `dup`: (1, 2) - consumes 1, produces 2
/// - `drop`: (1, 0) - consumes 1, produces 0  
/// - `add`: (2, 1) - consumes 2 (both operands), produces 1 (sum)
/// - `swap`: (2, 2) - consumes 2, produces 2 (same count, different order)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Effect {
    /// Number of values consumed from stack
    pub consumes: u32,
    /// Number of values produced onto stack
    pub produces: u32,
}

impl Effect {
    /// Create a new effect
    pub const fn new(consumes: u32, produces: u32) -> Self {
        Self { consumes, produces }
    }

    /// Identity effect: (0, 0) - does nothing
    pub const fn identity() -> Self {
        Self::new(0, 0)
    }

    /// Push effect: (0, 1) - pushes one value
    pub const fn push() -> Self {
        Self::new(0, 1)
    }

    /// Pop effect: (1, 0) - pops one value
    pub const fn pop() -> Self {
        Self::new(1, 0)
    }

    /// Net change in stack depth
    #[inline]
    pub const fn net(&self) -> i32 {
        self.produces as i32 - self.consumes as i32
    }

    /// Compose two effects: self ; other
    ///
    /// This is the MATHEMATICAL CORE of verification.
    ///
    /// ```text
    /// compose((a,b), (c,d)) = 
    ///     if b >= c then (a, b - c + d)
    ///     else (a + c - b, d)
    /// ```
    ///
    /// Intuition:
    /// - First tool produces b values
    /// - Second tool needs c values
    /// - If b >= c: second tool has enough, surplus = b - c, total out = surplus + d
    /// - If b < c: second tool needs more from original stack, extra in = c - b
    #[inline]
    pub const fn compose(self, other: Self) -> Self {
        let (a, b) = (self.consumes, self.produces);
        let (c, d) = (other.consumes, other.produces);

        if b >= c {
            // Second tool has enough from first tool's outputs
            // Total input: a (what first needs)
            // Total output: b - c + d (leftover from first + second's output)
            Effect::new(a, b - c + d)
        } else {
            // Second tool needs more than first produces
            // Must consume extra (c - b) from original stack
            // Total input: a + (c - b)
            // Total output: d (only second tool's output remains)
            Effect::new(a + c - b, d)
        }
    }

    /// Check if this effect is valid given minimum stack depth
    #[inline]
    pub const fn is_valid_at(&self, depth: u32) -> bool {
        depth >= self.consumes
    }

    /// Parse from signature string: "(2 -- 1)" or "(a b -- sum)"
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();

        // Remove outer parens
        let s = s
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or("Effect must be wrapped in ()")?;

        // Split on --
        let parts: Vec<&str> = s.split("--").collect();
        if parts.len() != 2 {
            return Err("Effect must contain --".into());
        }

        // Count inputs and outputs (ignore type annotations for now)
        let inputs = parts[0].split_whitespace().count() as u32;
        let outputs = parts[1].split_whitespace().count() as u32;

        Ok(Self::new(inputs, outputs))
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({} -- {})", self.consumes, self.produces)
    }
}

// ============================================================================
// TYPE (for future typed effects)
// ============================================================================

/// Value types in Kore
///
/// For Phase 1, we only track stack depth (Effect).
/// Future phases will add full type checking.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Type {
    /// Matches any type
    Any,
    /// Null
    Null,
    /// Boolean
    Bool,
    /// Integer
    Int,
    /// Float
    Float,
    /// Number (Int | Float)
    Num,
    /// String
    Text,
    /// List
    List,
    /// Map
    Map,
    /// Quote (deferred code)
    Quote,
    /// Handle (opaque resource)
    Handle,
    /// Error
    Error,
    /// Tensor (extension)
    Tensor,
    /// Fiber (extension)
    Fiber,
}

impl Type {
    /// Parse type from string
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "Any" | "*" => Ok(Type::Any),
            "Null" => Ok(Type::Null),
            "Bool" => Ok(Type::Bool),
            "Int" => Ok(Type::Int),
            "Float" => Ok(Type::Float),
            "Num" => Ok(Type::Num),
            "Text" | "Str" => Ok(Type::Text),
            "List" => Ok(Type::List),
            "Map" => Ok(Type::Map),
            "Quote" => Ok(Type::Quote),
            "Handle" => Ok(Type::Handle),
            "Error" => Ok(Type::Error),
            "Tensor" => Ok(Type::Tensor),
            "Fiber" => Ok(Type::Fiber),
            _ => Err(format!("Unknown type: {}", s)),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Any => write!(f, "Any"),
            Type::Null => write!(f, "Null"),
            Type::Bool => write!(f, "Bool"),
            Type::Int => write!(f, "Int"),
            Type::Float => write!(f, "Float"),
            Type::Num => write!(f, "Num"),
            Type::Text => write!(f, "Text"),
            Type::List => write!(f, "List"),
            Type::Map => write!(f, "Map"),
            Type::Quote => write!(f, "Quote"),
            Type::Handle => write!(f, "Handle"),
            Type::Error => write!(f, "Error"),
            Type::Tensor => write!(f, "Tensor"),
            Type::Fiber => write!(f, "Fiber"),
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_compose_basic() {
        // dup: (1,2) ; drop: (1,0) = (1,1) ← identity-like
        let dup = Effect::new(1, 2);
        let drop = Effect::new(1, 0);
        assert_eq!(dup.compose(drop), Effect::new(1, 1));

        // push: (0,1) ; push: (0,1) = (0,2)
        let push = Effect::push();
        assert_eq!(push.compose(push), Effect::new(0, 2));

        // pop: (1,0) ; pop: (1,0) = (2,0)
        let pop = Effect::pop();
        assert_eq!(pop.compose(pop), Effect::new(2, 0));
    }

    #[test]
    fn test_effect_compose_formula() {
        // The formula from docs:
        // compose((a,b), (c,d)) = if b >= c then (a, b-c+d) else (a+c-b, d)

        // Case 1: b >= c → (a, b-c+d)
        // (1,3) ; (2,1) → b=3 >= c=2 → (1, 3-2+1) = (1, 2)
        assert_eq!(
            Effect::new(1, 3).compose(Effect::new(2, 1)),
            Effect::new(1, 2)
        );

        // Case 2: b < c → (a+c-b, d)
        // (1,1) ; (3,2) → b=1 < c=3 → (1+3-1, 2) = (3, 2)
        assert_eq!(
            Effect::new(1, 1).compose(Effect::new(3, 2)),
            Effect::new(3, 2)
        );

        // Edge case: b == c → (a, d)
        // (2,2) ; (2,3) → b=2 == c=2 → (2, 2-2+3) = (2, 3)
        assert_eq!(
            Effect::new(2, 2).compose(Effect::new(2, 3)),
            Effect::new(2, 3)
        );
    }

    #[test]
    fn test_effect_compose_associative() {
        // Composition should be associative: (a;b);c = a;(b;c)
        let a = Effect::new(1, 2);
        let b = Effect::new(2, 1);
        let c = Effect::new(1, 3);

        let left = a.compose(b).compose(c);
        let right = a.compose(b.compose(c));
        assert_eq!(left, right);
    }

    #[test]
    fn test_effect_identity() {
        let id = Effect::identity();
        let eff = Effect::new(2, 3);

        // Identity laws
        assert_eq!(id.compose(eff), eff);
        assert_eq!(eff.compose(id), eff);
    }

    #[test]
    fn test_effect_parse() {
        assert_eq!(Effect::parse("(-- )").unwrap(), Effect::new(0, 0));
        assert_eq!(Effect::parse("(a -- b)").unwrap(), Effect::new(1, 1));
        assert_eq!(Effect::parse("(a b -- sum)").unwrap(), Effect::new(2, 1));
        assert_eq!(Effect::parse("(x -- a b)").unwrap(), Effect::new(1, 2));
        assert_eq!(Effect::parse("(a:Int b:Int -- sum:Int)").unwrap(), Effect::new(2, 1));
    }

    #[test]
    fn test_effect_net() {
        assert_eq!(Effect::new(0, 0).net(), 0);
        assert_eq!(Effect::new(0, 1).net(), 1);
        assert_eq!(Effect::new(1, 0).net(), -1);
        assert_eq!(Effect::new(2, 3).net(), 1);
        assert_eq!(Effect::new(3, 1).net(), -2);
    }

    #[test]
    fn test_type_parse() {
        assert_eq!(Type::parse("Any").unwrap(), Type::Any);
        assert_eq!(Type::parse("Int").unwrap(), Type::Int);
        assert_eq!(Type::parse("Text").unwrap(), Type::Text);
        assert_eq!(Type::parse("Tensor").unwrap(), Type::Tensor);
        assert!(Type::parse("Unknown").is_err());
    }
}
