//! Core Primitives - The minimal irreducible set
//!
//! These are atomic operations that cannot be composed from anything simpler.
//! Each primitive does exactly ONE thing. No magic. No hidden behavior.
//!
//! ## Postulate Compliance
//!
//! Every primitive is a Tool (P1), transforms Stack → Stack (P2),
//! and composes by concatenation (P3).
//!
//! ## Categories (99 total)
//!
//! ### Control (4)
//! - `call`: Execute a quote
//! - `spawn`: Create sandboxed execution context
//! - `if`: Conditional execution
//! - `loop`: Iterate while condition true
//!
//! ### Definition (3)
//! - `def`: Define a new tool
//! - `words`: List all tool names
//! - `describe`: Get tool signature
//!
//! ### Error (3)
//! - `try`: Execute with error capture
//! - `fail`: Raise an error
//! - `is-error`: Check if value is error
//!
//! ### Stack (7)
//! - `dup`, `drop`, `swap`, `rot`, `over`, `dip`, `depth`
//!
//! ### Arithmetic (6)
//! - `add`, `sub`, `mul`, `div`, `mod`, `neg`
//!
//! ### Comparison (2)
//! - `eq`, `lt` (gt, le, ge, ne compose from these)
//!
//! ### Logic (3)
//! - `and`, `or`, `not`
//!
//! ### Data (4)
//! - `list`: Collect n items into list
//! - `unlist`: Spread list onto stack
//! - `map-new`: Create empty map
//! - `emptylist`: Create empty list
//!
//! ### Compose (1)
//! - `compose`: Concatenate two quotes
//!
//! ### Locals (16)
//! - `store0`..`store7`: Pop TOS into local slot
//! - `load0`..`load7`: Push local slot onto stack
//!
//! ### String (10)
//! - `str-len`, `str-get`, `str-slice`, `str-concat`, `str-split`
//! - `str-join`, `str-find`, `str-starts`, `str-ends`, `str-replace`
//!
//! ### List (8)
//! - `list-len`, `list-get`, `list-set`, `list-push`, `list-pop`
//! - `list-slice`, `list-concat`, `list-reverse`
//!
//! ### Map (6)
//! - `map-get`, `map-set`, `map-del`, `map-has`, `map-keys`, `map-vals`
//!
//! ### Type (14)
//! - `type-of`, `to-int`, `to-float`, `to-text`, `to-bool`
//! - `is-null`, `is-bool`, `is-int`, `is-float`, `is-text`
//! - `is-list`, `is-map`, `is-quote`, `unwrap`
//!
//! ### Combinators (4)
//! - `map`, `filter`, `fold`, `each`
//!
//! ### Verification (5) - THE TRUSTED KERNEL
//! - `effect-compose`, `effect-parse`, `effect-net`, `effect-valid?`, `effect-new`

mod arithmetic;
mod combinators;
mod comparison;
mod compose;
mod data;
mod definition;
mod error;
mod execution;
mod list;
mod locals;
mod logic;
mod map;
mod stack;
mod string;
mod types;
mod verify;

use crate::context::Context;

/// Register all core primitives
///
/// This is the ONLY way to populate a context with primitives.
/// Call this before executing any Kore programs.
pub async fn register_core(ctx: &mut Context) {
    let mut dict = ctx.dict.write().await;
    
    // Control flow (4)
    execution::register(&mut dict);
    
    // Definition (3)
    definition::register(&mut dict);
    
    // Error handling (3)
    error::register(&mut dict);
    
    // Stack manipulation (7)
    stack::register(&mut dict);
    
    // Arithmetic (6)
    arithmetic::register(&mut dict);
    
    // Comparison (2)
    comparison::register(&mut dict);
    
    // Logic (3)
    logic::register(&mut dict);
    
    // Data construction (4)
    data::register(&mut dict);
    
    // Composition (1)
    compose::register(&mut dict);
    
    // Local variables (16)
    locals::register(&mut dict);
    
    // String operations (10)
    string::register(&mut dict);
    
    // List operations (8)
    list::register(&mut dict);
    
    // Map operations (6)
    map::register(&mut dict);
    
    // Type operations (14)
    types::register(&mut dict);
    
    // Combinators (4)
    combinators::register(&mut dict);
    
    // Verification - THE TRUSTED KERNEL (5)
    verify::register(&mut dict);
}

/// Total count of core primitives
pub const CORE_COUNT: usize = 99;

/// All primitive names for introspection
pub const CORE_PRIMITIVES: [&str; 99] = [
    // Execution (4)
    "call", "spawn", "if", "loop",
    // Definition (3)
    "def", "words", "describe",
    // Error (3)
    "try", "fail", "is-error",
    // Stack (7)
    "dup", "drop", "swap", "rot", "over", "dip", "depth",
    // Arithmetic (6)
    "add", "sub", "mul", "div", "mod", "neg",
    // Comparison (2)
    "eq", "lt",
    // Logic (3)
    "and", "or", "not",
    // Data (4)
    "list", "unlist", "map-new", "emptylist",
    // Compose (1)
    "compose",
    // Locals (16)
    "store0", "store1", "store2", "store3",
    "store4", "store5", "store6", "store7",
    "load0", "load1", "load2", "load3",
    "load4", "load5", "load6", "load7",
    // String (13)
    "str-len", "str-get", "str-slice", "str-concat", "str-split",
    "str-join", "str-find", "str-starts", "str-ends", "str-replace",
    "str-trim", "char-code", "code-char",
    // List (8)
    "list-len", "list-get", "list-set", "list-push", "list-pop",
    "list-slice", "list-concat", "list-reverse",
    // Map (6)
    "map-get", "map-set", "map-del", "map-has", "map-keys", "map-vals",
    // Type (14)
    "type-of", "to-int", "to-float", "to-text", "to-bool",
    "is-null", "is-bool", "is-int", "is-float", "is-text",
    "is-list", "is-map", "is-quote", "unwrap",
    // Combinators (4)
    "map", "filter", "fold", "each",
    // Verification - THE TRUSTED KERNEL (5)
    "effect-compose", "effect-parse", "effect-net", "effect-valid?", "effect-new",
];
