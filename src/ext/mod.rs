//! Extension modules for the four pillars
//!
//! - Tensor: differentiable multi-dimensional arrays
//! - Autodiff: automatic differentiation (reverse-mode)
//! - Fiber: reified computations (suspended execution as values)
//! - Linear: values that cannot be duplicated or discarded
//! - Distribution: probability distributions
//!
//! All extension tools follow P1/P2/P3.
//!
//! # Algebraic Optimization
//!
//! Tensor operations are algebraically optimized at the Op level in `optimizer.rs`.
//! Due to Kore's immutability (P1), we can safely apply mathematical identities:
//! - log(exp(x)) = x
//! - exp(log(x)) = x (for x > 0)
//! - -(-x) = x
//!
//! This is impossible in von Neumann languages due to aliasing and side effects.
//!
//! # Fiber Semantics
//!
//! A Fiber is an **immutable value** F = (Stack, Code, Status).
//! All operations return NEW fibers; the original is unchanged.
//! This means:
//! - Fork = dup (fibers are values, cloning is free)
//! - Checkpoint = keep the value
//! - Restore = use the saved value
//!
//! This preserves determinism and compositionality.

pub mod tensor;
pub mod autodiff;
pub mod fiber;
pub mod linear;

use crate::context::Dictionary;

/// Register all extension tools
pub fn register_ext(dict: &mut Dictionary) {
    tensor::register(dict);
    autodiff::register_autodiff(dict);
    fiber::register(dict);
    linear::register(dict);
    // Future: distribution::register(dict);
}
