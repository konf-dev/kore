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
//! ## Quick Example
//!
//! ```ignore
//! use kore::{Context, Stack, Op, Value, execute};
//!
//! #[tokio::main]
//! async fn main() {
//!     let ctx = Context::new();
//!     let stack = Stack::new();
//!     
//!     // Program: push 5, push 3, call "add"
//!     let ops = vec![
//!         Op::push(5),
//!         Op::push(3),
//!         Op::call("add"),
//!     ];
//!     
//!     let (result, _) = execute(&ops, stack, ctx).await.unwrap();
//!     assert_eq!(result.values()[0].as_int().unwrap(), 8);
//! }
//! ```
//!
//! ## Design Principles
//!
//! 1. **Minimal**: 2 operations (Push, Call), 10 types, ~200 lines of core logic
//! 2. **Formal**: Built on algebraic structures (lattice, monoid, category)
//! 3. **Secure**: Capability-based access control, resource conservation
//! 4. **Composable**: Tools are the only abstraction
//!
//! ## Core Primitives (~50)
//!
//! Stack, arithmetic, comparison, logic, control, definition, data,
//! capability, resource, spawn, error, and trace operations.
//! Everything else is in the stdlib.

// Algebraic foundations (new!)
pub mod algebra;
pub mod core;

// Existing modules
pub mod builtins;
pub mod capabilities;
pub mod context;
pub mod effect;
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

// Re-export algebra types
pub use algebra::{Cap, CapSet, Res, Trace, TraceStep};

// Re-export main types for convenience
pub use builtins::register_builtins;
pub use capabilities::Capabilities;
pub use context::Context;
pub use effect::{Effect, Type};
pub use error::{Error, Result};
pub use executor::{execute, ExecFuture};
pub use memory::Memory;
pub use op::Op;
pub use resources::{ResourceQuota, Resources};
pub use stack::Stack;
pub use storage::Storage;
pub use tool::Tool;
pub use value::{ErrorValue, Handle, HandleKind, Value};
