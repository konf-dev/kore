//! Extension modules for the four pillars
//!
//! - Tensor: differentiable multi-dimensional arrays
//! - Fiber: reified computations (paused execution)
//! - Linear: values that cannot be duplicated or discarded
//! - Distribution: probability distributions
//!
//! All extension tools follow P1/P2/P3.

pub mod tensor;
pub mod linear;

use crate::context::Dictionary;

/// Register all extension tools
pub fn register_ext(dict: &mut Dictionary) {
    tensor::register(dict);
    linear::register(dict);
    // Future: fiber::register(dict);
    // Future: distribution::register(dict);
}
