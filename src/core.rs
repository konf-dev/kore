//! Core Primitives
//!
//! These are the ONLY native tools. Every other operation is a composition.
//!
//! # Postulate Alignment
//!
//! **Postulate 1**: Everything is a Tool
//! - Each primitive is a Tool: Stack → Stack
//! - No special forms, no syntax sugar
//!
//! **Postulate 2**: Tools Transform Stacks
//! - Each primitive has a clear stack signature
//! - Input comes from stack, output goes to stack
//!
//! **Postulate 3**: Composition is Concatenation
//! - These primitives can be composed by concatenation
//! - Order matters: `f g` means "apply f, then apply g"
//!
//! # Primitive Categories
//!
//! ## Stack (9 primitives)
//! The pure mechanical operations. No side effects.
//!
//! | Tool    | Signature            | Description             |
//! |---------|---------------------|-------------------------|
//! | dup     | (a -- a a)          | Duplicate top           |
//! | drop    | (a -- )             | Remove top              |
//! | swap    | (a b -- b a)        | Swap top two            |
//! | rot     | (a b c -- b c a)    | Rotate third to top     |
//! | over    | (a b -- a b a)      | Copy second to top      |
//! | nip     | (a b -- b)          | Remove second           |
//! | tuck    | (a b -- b a b)      | Copy top below second   |
//! | pick    | (n -- x)            | Copy nth item to top    |
//! | depth   | ( -- n)             | Stack depth             |
//!
//! ## Arithmetic (9 primitives)
//! Pure mathematical operations.
//!
//! | Tool    | Signature            | Description             |
//! |---------|---------------------|-------------------------|
//! | add     | (a b -- a+b)        | Addition                |
//! | sub     | (a b -- a-b)        | Subtraction             |
//! | mul     | (a b -- a*b)        | Multiplication          |
//! | div     | (a b -- a/b)        | Division                |
//! | mod     | (a b -- a%b)        | Modulo                  |
//! | neg     | (a -- -a)           | Negation                |
//! | abs     | (a -- |a|)          | Absolute value          |
//! | min     | (a b -- min)        | Minimum                 |
//! | max     | (a b -- max)        | Maximum                 |
//!
//! ## Comparison (6 primitives)
//! Pure predicates.
//!
//! | Tool    | Signature            | Description             |
//! |---------|---------------------|-------------------------|
//! | eq      | (a b -- a=b)        | Equality                |
//! | lt      | (a b -- a<b)        | Less than               |
//! | gt      | (a b -- a>b)        | Greater than            |
//! | le      | (a b -- a≤b)        | Less or equal           |
//! | ge      | (a b -- a≥b)        | Greater or equal        |
//! | ne      | (a b -- a≠b)        | Not equal               |
//!
//! ## Logic (3 primitives)
//! Boolean operations.
//!
//! | Tool    | Signature            | Description             |
//! |---------|---------------------|-------------------------|
//! | and     | (a b -- a∧b)        | Logical and             |
//! | or      | (a b -- a∨b)        | Logical or              |
//! | not     | (a -- ¬a)           | Logical not             |
//!
//! ## Control (3 primitives)
//! The minimal control flow.
//!
//! | Tool    | Signature            | Description             |
//! |---------|---------------------|-------------------------|
//! | if      | (c t f -- r)        | Conditional             |
//! | call    | (q -- ...)          | Execute quotation       |
//! | loop    | (q -- ...)          | Loop while true on top  |
//!
//! ## Definition (3 primitives)
//! Tool creation.
//!
//! | Tool    | Signature            | Description             |
//! |---------|---------------------|-------------------------|
//! | def     | (q name -- )        | Define a new tool       |
//! | words   | ( -- list)          | List defined tools      |
//! | meta    | (name -- info)      | Get tool metadata       |
//!
//! ## Data (5 primitives)
//! Structured data operations.
//!
//! | Tool    | Signature            | Description             |
//! |---------|---------------------|-------------------------|
//! | list    | (... n -- list)     | Collect n items to list |
//! | unlist  | (list -- ...)       | Spread list to stack    |
//! | map-new | ( -- map)           | Create empty map        |
//! | map-get | (map k -- v)        | Get value from map      |
//! | map-set | (map k v -- map)    | Set value in map        |
//!
//! ## Capability (4 primitives)
//! Security primitives from the capability lattice.
//!
//! | Tool    | Signature            | Description             |
//! |---------|---------------------|-------------------------|
//! | cap-has | (cap -- bool)       | Check capability        |
//! | cap-list| ( -- caps)          | List capabilities       |
//! | cap-leq | (a b -- a≤b)        | Compare capabilities    |
//! | cap-attn| (caps -- caps')     | Attenuate (weaken)      |
//!
//! ## Resource (4 primitives)
//! Resource algebra operations.
//!
//! | Tool    | Signature            | Description             |
//! |---------|---------------------|-------------------------|
//! | res-avail| ( -- res)          | Available resources     |
//! | res-split| (ratio -- r1 r2)   | Split resources         |
//! | res-cons | (res -- )          | Consume resources       |
//! | res-has  | (res -- bool)      | Check if sufficient     |
//!
//! ## Spawn (1 primitive)
//! The ONLY way to create a new execution context.
//!
//! | Tool    | Signature            | Description             |
//! |---------|---------------------|-------------------------|
//! | spawn   | (q caps res -- ctx) | Spawn isolated context  |
//!
//! ## Error (2 primitives)
//! Error handling.
//!
//! | Tool    | Signature            | Description             |
//! |---------|---------------------|-------------------------|
//! | try     | (q h -- r)          | Try with handler        |
//! | fail    | (msg -- )           | Raise error             |
//!
//! ## Trace (2 primitives)
//! Execution traces for debugging/auditing.
//!
//! | Tool    | Signature            | Description             |
//! |---------|---------------------|-------------------------|
//! | trace-on| ( -- )              | Enable tracing          |
//! | trace   | ( -- trace)         | Get current trace       |
//!
//! # Total: 51 Core Primitives
//!
//! Stack(9) + Arithmetic(9) + Comparison(6) + Logic(3) + Control(3) +
//! Definition(3) + Data(5) + Capability(4) + Resource(4) + Spawn(1) +
//! Error(2) + Trace(2) = 51
//!
//! Everything else is either stdlib (can be built from primitives)
//! or system tools (native but not "primitives").

use crate::algebra::{CapSet, Res, Trace, TraceStep};
use crate::value::Value;
use std::collections::HashMap;

/// Result type for tool execution
pub type ToolResult = Result<(), ToolError>;

/// Tool execution error
#[derive(Debug, Clone)]
pub struct ToolError {
    pub tool: String,
    pub message: String,
}

impl ToolError {
    pub fn new(tool: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            tool: tool.into(),
            message: msg.into(),
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.tool, self.message)
    }
}

impl std::error::Error for ToolError {}

/// Execution context - the sandbox in which tools run
pub struct Context {
    /// The data stack
    pub stack: Vec<Value>,

    /// Available capabilities (can only go DOWN the lattice)
    pub caps: CapSet,

    /// Available resources (can only decrease via conservation)
    pub resources: Res,

    /// Defined tools (user-defined quotations)
    pub definitions: HashMap<String, Value>,

    /// Execution trace
    pub trace: Trace,

    /// Is tracing enabled?
    pub tracing: bool,
}

impl Context {
    /// Create a new context with given capabilities and resources
    pub fn new(caps: CapSet, resources: Res) -> Self {
        Self {
            stack: Vec::new(),
            caps,
            resources,
            definitions: HashMap::new(),
            trace: Trace::empty(),
            tracing: false,
        }
    }

    /// Create a child context with attenuated capabilities and split resources.
    /// 
    /// This is the ONLY way to create a new context, and it GUARANTEES:
    /// 1. child.caps ≤ self.caps (capability attenuation)
    /// 2. child.resources + remaining = self.resources (resource conservation)
    pub fn spawn(&mut self, requested_caps: &CapSet, resource_ratio: f64) -> Context {
        // Attenuate capabilities (can only go down the lattice)
        let child_caps = self.caps.attenuate(requested_caps);

        // Split resources (conservation law)
        let (child_res, remaining) = self.resources.split(resource_ratio);
        self.resources = remaining;

        Context::new(child_caps, child_res)
    }

    /// Push a value onto the stack
    pub fn push(&mut self, v: Value) {
        self.stack.push(v);
    }

    /// Pop a value from the stack
    pub fn pop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    /// Peek at the top of the stack
    pub fn peek(&self) -> Option<&Value> {
        self.stack.last()
    }

    /// Get stack depth
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Record a trace step
    pub fn record_step(&mut self, step: TraceStep) {
        if self.tracing {
            self.trace.push(step);
        }
    }

    /// Check if a capability is available
    pub fn has_cap(&self, cap: &str) -> bool {
        self.caps.allows_str(cap)
    }

    /// Check if resources are sufficient
    pub fn has_resources(&self, required: &Res) -> bool {
        self.resources.has(required)
    }

    /// Consume resources (for operations that cost)
    pub fn consume(&mut self, amount: &Res) -> Result<(), ToolError> {
        self.resources = self.resources.consume(amount).ok_or_else(|| {
            ToolError::new("resource", "insufficient resources")
        })?;
        Ok(())
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new(CapSet::top(), Res::UNLIMITED)
    }
}

/// List of all core primitives
pub const CORE_PRIMITIVES: &[&str] = &[
    // Stack (9)
    "dup", "drop", "swap", "rot", "over", "nip", "tuck", "pick", "depth",
    // Arithmetic (9)
    "add", "sub", "mul", "div", "mod", "neg", "abs", "min", "max",
    // Comparison (6)
    "eq", "lt", "gt", "le", "ge", "ne",
    // Logic (3)
    "and", "or", "not",
    // Control (3)
    "if", "call", "loop",
    // Definition (3)
    "def", "words", "meta",
    // Data (5)
    "list", "unlist", "map-new", "map-get", "map-set",
    // Capability (4)
    "cap-has", "cap-list", "cap-leq", "cap-attn",
    // Resource (4)
    "res-avail", "res-split", "res-cons", "res-has",
    // Spawn (1)
    "spawn",
    // Error (2)
    "try", "fail",
    // Trace (2)
    "trace-on", "trace",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_spawn_caps_attenuate() {
        // Parent has full access
        let mut parent = Context::new(
            CapSet::parse("fs:read,fs:write,net:connect"),
            Res::new(1000, 500, 2000, 100),
        );

        // Child asks for fs:read only
        let requested = CapSet::parse("fs:read");
        let child = parent.spawn(&requested, 0.5);

        // Child should have only fs:read
        assert!(child.has_cap("fs:read"));
        assert!(!child.has_cap("fs:write"));
        assert!(!child.has_cap("net:connect"));

        // Child caps ≤ parent caps
        assert!(child.caps.leq(&CapSet::parse("fs:read,fs:write,net:connect")));
    }

    #[test]
    fn test_context_spawn_resource_conservation() {
        let mut parent = Context::new(
            CapSet::top(),
            Res::new(1000, 500, 2000, 100),
        );

        let child = parent.spawn(&CapSet::top(), 0.3);

        // Child + parent = original
        let combined = child.resources.add(&parent.resources);
        assert_eq!(combined.mem, 1000);
        assert_eq!(combined.rom, 500);
        assert_eq!(combined.compute, 2000);
        assert_eq!(combined.net, 100);
    }

    #[test]
    fn test_core_primitives_count() {
        assert_eq!(CORE_PRIMITIVES.len(), 51);
    }
}
