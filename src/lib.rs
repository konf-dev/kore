//! # Kore - The Fundamental Runtime
//!
//! Kore is a minimal, stack-based execution engine built on three postulates:
//!
//! ## The Three Postulates
//!
//! **Postulate 1: Everything is a Tool**
//! ```text
//! Tool : Stack → Stack
//! ```
//! Every operation, from arithmetic to I/O, is a tool that transforms a stack.
//!
//! **Postulate 2: Tools Transform Stacks**
//! ```text
//! execute(t, s) = s'
//! ```
//! Tools consume values from the stack and produce values onto the stack.
//!
//! **Postulate 3: Composition is Concatenation**  
//! ```text
//! (f ; g)(s) = g(f(s))
//! ```
//! Running tools in sequence is function composition, written by concatenation.
//!
//! ## Algebraic Foundations
//!
//! - **[algebra::CapSet]**: Capability lattice with ≤, ∧, ∨, attenuate
//! - **[algebra::Res]**: Resource monoid with +, split (conservation law)
//! - **[algebra::Trace]**: Execution traces (monoid under concatenation)
//!
//! ## Core Types
//!
//! - **[Value]**: 10 types - Null, Bool, Int, Float, Text, List, Map, Quote, Handle, Error
//! - **[Stack]**: LIFO data structure for passing values between tools
//! - **[Op]**: 2 operations - Push, Call (that's it!)
//! - **[Context]**: Execution sandbox with capabilities and resources
//!
//! ## Architecture
//!
//! - **Core** (27 primitives): Irreducible operations in `src/core/`
//! - **Stdlib**: Composed tools in `stdlib/*.kore`
//! - **Capability Tools**: External tools requiring permissions in `src/cap/`
//!
//! ## Design Principles
//!
//! 1. **Minimal**: 2 operations (Push, Call), 10 types, 27 core primitives
//! 2. **Formal**: Built on algebraic structures (lattice, monoid, category)
//! 3. **Secure**: Capability-based access control, resource conservation
//! 4. **Composable**: Tools are the only abstraction
//! 5. **Machine-readable**: Every tool has queryable manifest

// Algebraic foundations
pub mod algebra;

// Core: 57 irreducible primitives
pub mod core;

// Capability tools: OS, network, storage
pub mod cap;

// Extension tools: tensor, fiber, linear, distribution
pub mod ext;

// Legacy builtins (to be removed)
pub mod builtins;

// Fundamental types
pub mod capabilities;
pub mod context;
pub mod error;
pub mod executor;
pub mod memory;
pub mod meta;
pub mod op;
pub mod resources;
pub mod stack;
pub mod storage;
pub mod tool;
pub mod value;

// Optional modules
pub mod effect;
pub mod lookahead;
pub mod stdlib;

// Re-export algebra types
pub use algebra::{Cap, CapSet, Res, Trace, TraceStep};

// Re-export main types for convenience
pub use capabilities::Capabilities;
pub use context::Context;
pub use error::{Error, Result};
pub use executor::{execute, ExecFuture};
pub use memory::Memory;
pub use op::Op;
pub use resources::{ResourceQuota, Resources};
pub use stack::Stack;
pub use storage::Storage;
pub use tool::Tool;
pub use value::{ErrorValue, Handle, HandleKind, Value};

// Registration functions
pub use core::register_core;
pub use cap::register_cap;
pub use ext::register_ext;
pub use builtins::register_builtins;
