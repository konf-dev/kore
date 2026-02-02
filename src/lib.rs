//! # Kore - The Fundamental Runtime
//!
//! Kore is a minimal, stack-based execution engine where everything is a tool.
//!
//! ## Core Concepts
//!
//! - **[Value]**: 10 types - Null, Bool, Int, Float, Text, List, Map, Quote, Handle, Error
//! - **[Stack]**: LIFO data structure for passing values between tools
//! - **[Op]**: 4 operations - Push, Call, Quote, If
//! - **[Effect]**: Type signatures that describe what a tool consumes and produces
//! - **[Tool]**: Either native (Rust function) or composed (sequence of Ops)
//! - **[Context]**: Execution environment with dictionary, identity, and capabilities
//! - **[execute]**: The execution loop
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
//! 1. **Minimal**: 4 operations, 10 types, ~200 lines of core logic
//! 2. **Predictable**: No hidden state, no magic, no surprises
//! 3. **Secure**: Capability-based access control built in
//! 4. **Composable**: Tools are the only abstraction
//!
//! ## Built-in Tools
//!
//! Kore includes 9 built-in tools that make it a complete language:
//!
//! | Tool | Effect | Purpose |
//! |------|--------|---------|
//! | `call` | `(quote -- ...)` | Run a quote |
//! | `try` | `(quote -- value-or-error)` | Run, capture errors as values |
//! | `is-error` | `(value -- bool)` | Check if value is an Error |
//! | `unwrap` | `(value-or-error -- value)` | Extract or stop if Error |
//! | `dup` | `(a -- a a)` | Duplicate top value |
//! | `drop` | `(a -- )` | Remove top value |
//! | `swap` | `(a b -- b a)` | Swap top two |
//! | `over` | `(a b -- a b a)` | Copy second to top |
//! | `rot` | `(a b c -- b c a)` | Rotate top three |

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
