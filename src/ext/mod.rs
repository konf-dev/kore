//! Extension modules for the four pillars
//!
//! - Tensor: differentiable multi-dimensional arrays
//! - Autodiff: automatic differentiation (reverse-mode)
//! - Fiber: reified computations (paused execution)
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

pub mod tensor;
pub mod autodiff;
pub mod linear;

use crate::context::Dictionary;

/// Register all extension tools
pub fn register_ext(dict: &mut Dictionary) {
    tensor::register(dict);
    autodiff::register_autodiff(dict);
    linear::register(dict);
    // Future: fiber::register(dict);
    // Future: distribution::register(dict);
}
