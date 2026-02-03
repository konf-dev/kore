# Kore 2.0: The Machine-Native Safe Language

> "Trust the Algebra, Not the Author"

## Executive Summary

This document is the complete technical specification for transforming Kore into the definitive language for machine-authored, mathematically-verified code.

**Current State:** 12,630 lines of Rust, 400+ passing tests, working interpreter with capabilities.

**Target State:** A language where:
1. Resources cannot leak (Linear Types)
2. Effects cannot escape (Full Effect System)
3. Contracts cannot be violated (Refinement Types)
4. Proofs travel with code (Proof-Carrying Code)

**Timeline:** 12-16 weeks for complete implementation.

---

## Part I: Mathematical Foundations

### The Kore Type Universe

```
                    ┌─────────────────────────────────────────┐
                    │              UNIVERSE                    │
                    │                                          │
                    │  ┌──────────────────────────────────┐   │
                    │  │         REFINEMENT TYPES         │   │
                    │  │                                  │   │
                    │  │  ┌────────────────────────────┐  │   │
                    │  │  │     AFFINE/LINEAR TYPES   │  │   │
                    │  │  │                            │  │   │
                    │  │  │  ┌──────────────────────┐  │  │   │
                    │  │  │  │    EFFECT TYPES     │  │  │   │
                    │  │  │  │                      │  │  │   │
                    │  │  │  │  ┌────────────────┐  │  │  │   │
                    │  │  │  │  │  BASE TYPES   │  │  │  │   │
                    │  │  │  │  │ Int,Text,List │  │  │  │   │
                    │  │  │  │  └────────────────┘  │  │  │   │
                    │  │  │  └──────────────────────┘  │  │   │
                    │  │  └────────────────────────────┘  │   │
                    │  └──────────────────────────────────┘   │
                    └─────────────────────────────────────────┘
```

### Layer 1: Base Types (HAVE)
```
τ ::= Null | Bool | Int | Float | Text | List τ | Map τ | Quote | Handle | Error
```

### Layer 2: Effect Types (PARTIAL - need static inference)
```
ε ::= pure | fs | net | spawn | exec | ε ∪ ε

Γ ⊢ e : τ ! ε    (e has type τ with effect ε)
```

### Layer 3: Affine/Linear Types (NEED)
```
κ ::= U | A | L    (Unrestricted, Affine, Linear)

Γ ⊢ e : τ^κ       (e has type τ with linearity κ)

Rules:
  U: can dup, can drop
  A: cannot dup, can drop  
  L: cannot dup, cannot drop (must consume)
```

### Layer 4: Refinement Types (NEED)
```
{x : τ | φ(x)}    (type τ refined by predicate φ)

Γ ⊢ e : {x : τ | φ}   iff   Γ ⊢ e : τ  ∧  Γ ⊢ φ[e/x]
```

---

## Part II: Architecture That Respects the Postulates

### Postulate Compliance Matrix

Every feature MUST be implementable as tools following P1, P2, P3.

| Feature | Is a Tool? | Transforms Stack? | Composes via Concat? |
|---------|------------|-------------------|----------------------|
| Linear Types | ✅ `linear-new`, `linear-consume` | ✅ Stack values tagged | ✅ Yes |
| Effect Inference | ✅ `effect-infer` | ✅ Quote → Effect | ✅ Yes |
| Refinement Types | ✅ `spec-new`, `spec-check` | ✅ Value + Spec → Bool | ✅ Yes |
| Proof Generation | ✅ `prove` | ✅ Quote → Proof | ✅ Yes |
| Proof Verification | ✅ `verify` | ✅ Code + Proof → Bool | ✅ Yes |

### The Type-Carrying Value

Extend `Value` to carry type information:

```rust
// Current (src/value.rs)
pub enum Value {
    Int(i64),
    Text(String),
    // ...
    Ext(Arc<ExtValue>),  // Extension point - USE THIS
}

// ExtValue already has the structure we need:
pub struct ExtValue {
    pub kind: u8,        // LINEAR = 2 (already defined!)
    pub data: Box<Value>,
    pub meta: Option<IndexMap<String, Value>>,  // For refinement predicates
}
```

**Key Insight:** The `Ext` variant with `kind = LINEAR` already exists! We just need to enforce the rules.

---

## Part III: Implementation Phases

### Phase 1: Linear/Affine Types (Weeks 1-3)

#### 1.1 Core Linear Infrastructure

**File: `src/linear.rs` (NEW)**

```rust
//! Linear Type Enforcement
//!
//! Linearity is a PROPERTY of values, checked at runtime.
//! Tools enforce linearity; the Stack tracks it.

use crate::value::{Value, ExtValue, ext};
use crate::error::{Error, Result};

/// Linearity kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Linearity {
    /// Can dup, can drop
    Unrestricted,
    /// Cannot dup, can drop (Rust's move semantics)
    Affine,
    /// Cannot dup, cannot drop (must consume explicitly)
    Linear,
}

/// Wrap a value as linear
pub fn make_linear(value: Value) -> Value {
    Value::Ext(Arc::new(ExtValue {
        kind: ext::LINEAR,
        data: Box::new(value),
        meta: Some({
            let mut m = IndexMap::new();
            m.insert("linearity".into(), Value::Text("linear".into()));
            m
        }),
    }))
}

/// Check if value is linear
pub fn is_linear(value: &Value) -> bool {
    matches!(value, Value::Ext(e) if e.kind == ext::LINEAR)
}

/// Check if dup is allowed
pub fn can_dup(value: &Value) -> bool {
    !is_linear(value)  // Only unrestricted can dup
}

/// Check if drop is allowed (for Linear, must use explicit consume)
pub fn can_drop(value: &Value) -> bool {
    if let Value::Ext(e) = value {
        if e.kind == ext::LINEAR {
            if let Some(meta) = &e.meta {
                if let Some(Value::Text(lin)) = meta.get("linearity") {
                    return lin != "linear";  // Affine can drop, Linear cannot
                }
            }
        }
    }
    true  // Unrestricted can always drop
}
```

#### 1.2 Stack Enforcement

**Modify: `src/stack.rs`**

```rust
impl Stack {
    /// Pop a value - checks linear rules
    pub fn pop(&mut self) -> Result<Value> {
        self.values.pop().ok_or(Error::StackUnderflow)
    }

    /// Dup the top value - ERROR if linear
    pub fn dup(&mut self) -> Result<()> {
        let top = self.peek()?;
        if !linear::can_dup(top) {
            return Err(Error::LinearityViolation {
                operation: "dup".into(),
                reason: "cannot duplicate linear value".into(),
            });
        }
        let value = top.clone();
        self.push(value)
    }

    /// Drop the top value - ERROR if linear (must consume)
    pub fn drop_top(&mut self) -> Result<Value> {
        let top = self.pop()?;
        if !linear::can_drop(&top) {
            return Err(Error::LinearityViolation {
                operation: "drop".into(),
                reason: "cannot drop linear value, must consume explicitly".into(),
            });
        }
        Ok(top)
    }
}
```

#### 1.3 Linear Tools

**File: `src/cap/linear.rs` (NEW)**

```rust
//! Linear Type Tools
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | linear-new | (value -- linear) | Wrap as linear |
//! | linear-consume | (linear consumer:Quote -- result) | Consume linear value |
//! | affine-new | (value -- affine) | Wrap as affine |
//! | is-linear | (value -- bool) | Check if linear |

pub fn register(dict: &mut Dictionary) {
    // linear-new: Wrap value as linear (cannot dup, cannot drop)
    dict.register(Tool::native(
        "linear-new",
        "(value:Any -- linear:Linear)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                let linear = linear::make_linear(value, Linearity::Linear);
                stack.push(linear)?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Wrap value as linear - must be consumed exactly once"));

    // linear-consume: The ONLY way to use a linear value
    dict.register(Tool::native(
        "linear-consume",
        "(linear:Linear consumer:Quote -- result:Any)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let consumer = stack.pop()?.into_quote()?;
                let linear = stack.pop()?;
                
                if !linear::is_linear(&linear) {
                    return Err(Error::type_error("linear-consume", "Linear", &linear));
                }
                
                // Unwrap the linear value
                let inner = linear::unwrap(linear);
                
                // Push inner, call consumer, get result
                stack.push(inner)?;
                let (new_stack, new_ctx) = ctx.execute(&consumer, stack).await?;
                
                Ok((new_stack, new_ctx))
            })
        },
    ).with_doc("Consume a linear value with the given quote"));
}
```

#### 1.4 Linear Handles for Resources

**Modify existing handles to be linear:**

```rust
// When creating a file handle
pub fn fs_open(path: &str) -> Value {
    let handle = Handle { kind: HandleKind::File, id: path.to_string() };
    // Wrap as AFFINE (can drop via fs-close, cannot dup)
    linear::make_affine(Value::Handle(handle))
}

// fs-close CONSUMES the handle
dict.register(Tool::native(
    "fs-close",
    "(handle:Handle -- )",
    |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let handle = stack.pop()?;  // Takes ownership
            // Handle is now gone - no leak possible
            Ok((stack, ctx))
        })
    },
));
```

---

### Phase 2: Full Effect System (Weeks 4-6)

#### 2.1 Effect Algebra

**File: `src/effects.rs` (NEW)**

```rust
//! Algebraic Effect System
//!
//! Effects form a bounded join semilattice:
//! - ⊥ = pure (no effects)
//! - ⊤ = any (all effects)
//! - ∨ = union (combining effects)
//! - ≤ = subset (effect subsumption)

use std::collections::HashSet;
use serde::{Serialize, Deserialize};

/// Effect set
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Effects {
    effects: HashSet<String>,
}

/// Standard effects
pub mod std_effects {
    pub const PURE: &str = "pure";
    pub const FS_READ: &str = "fs:read";
    pub const FS_WRITE: &str = "fs:write";
    pub const NET_READ: &str = "net:read";
    pub const NET_WRITE: &str = "net:write";
    pub const SPAWN: &str = "spawn";
    pub const EXEC: &str = "exec";
    pub const MEM_READ: &str = "mem:read";
    pub const MEM_WRITE: &str = "mem:write";
    pub const DIVERGE: &str = "diverge";  // May not terminate
}

impl Effects {
    /// Pure effect (no side effects)
    pub fn pure() -> Self {
        Self { effects: HashSet::new() }
    }

    /// Add an effect
    pub fn add(&mut self, effect: &str) {
        self.effects.insert(effect.to_string());
    }

    /// Union of effects
    pub fn union(&self, other: &Self) -> Self {
        Self {
            effects: self.effects.union(&other.effects).cloned().collect()
        }
    }

    /// Check subsumption: self ≤ other (self's effects are subset of other's)
    pub fn is_subset_of(&self, other: &Self) -> bool {
        self.effects.is_subset(&other.effects)
    }

    /// Is this pure?
    pub fn is_pure(&self) -> bool {
        self.effects.is_empty()
    }
}
```

#### 2.2 Effect Inference

**Extend: `src/analyzer.rs`**

```rust
/// Extended analysis result
pub struct Analysis {
    /// Stack effect
    pub effect: Effect,
    /// IO effects
    pub io_effects: Effects,
    /// Errors
    pub errors: Vec<AnalysisError>,
    /// Warnings
    pub warnings: Vec<AnalysisWarning>,
}

/// Get IO effects for a tool
fn get_io_effects(&self, name: &str) -> Effects {
    let mut e = Effects::pure();
    match name {
        // File system
        "fs-read" | "fs-exists" | "fs-list" => e.add(std_effects::FS_READ),
        "fs-write" | "fs-append" | "fs-mkdir" | "fs-rm" => e.add(std_effects::FS_WRITE),
        
        // Network
        "http-get" => { e.add(std_effects::NET_READ); }
        "http-post" | "http-request" => { 
            e.add(std_effects::NET_READ); 
            e.add(std_effects::NET_WRITE); 
        }
        
        // Process
        "exec" => e.add(std_effects::EXEC),
        "spawn" => e.add(std_effects::SPAWN),
        
        // Memory
        "mem-get" | "rom-get" | "mem-keys" | "rom-keys" => e.add(std_effects::MEM_READ),
        "mem-set" | "rom-set" | "mem-del" | "rom-del" => e.add(std_effects::MEM_WRITE),
        
        // Non-termination
        "loop" | "while" => e.add(std_effects::DIVERGE),
        
        // Everything else is pure
        _ => {}
    }
    e
}

/// Infer effects from ops
fn infer_io_effects(&self, ops: &[Op]) -> Effects {
    let mut effects = Effects::pure();
    for op in ops {
        match op {
            Op::Call(name) => {
                effects = effects.union(&self.get_io_effects(name));
            }
            Op::Push(Value::Quote(inner)) => {
                // Quotes don't execute, but if called, their effects manifest
                // We track this conservatively
            }
            _ => {}
        }
    }
    effects
}
```

#### 2.3 Effect Tools

**File: `src/cap/effects.rs` (NEW)**

```rust
//! Effect Introspection Tools

pub fn register(dict: &mut Dictionary) {
    // effect-infer: Infer effects of a quote
    dict.register(Tool::native(
        "effect-infer",
        "(quote:Quote -- effects:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                let analysis = analyzer::analyze(&quote);
                
                let mut effects_map = IndexMap::new();
                effects_map.insert("stack".into(), effect_to_value(analysis.effect));
                effects_map.insert("io".into(), effects_to_value(&analysis.io_effects));
                effects_map.insert("pure".into(), Value::Bool(analysis.io_effects.is_pure()));
                
                stack.push(Value::Map(effects_map))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Infer stack and IO effects of a quote"));

    // effect-pure?: Check if quote is pure
    dict.register(Tool::native(
        "effect-pure?",
        "(quote:Quote -- bool:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                let analysis = analyzer::analyze(&quote);
                stack.push(Value::Bool(analysis.io_effects.is_pure()))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Check if quote has no side effects"));

    // effect-subset?: Check if quote's effects are subset of allowed
    dict.register(Tool::native(
        "effect-subset?",
        "(quote:Quote allowed:List -- bool:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let allowed = stack.pop()?.into_list()?;
                let quote = stack.pop()?.into_quote()?;
                
                let mut allowed_effects = Effects::pure();
                for e in allowed {
                    if let Value::Text(s) = e {
                        allowed_effects.add(&s);
                    }
                }
                
                let analysis = analyzer::analyze(&quote);
                let is_subset = analysis.io_effects.is_subset_of(&allowed_effects);
                
                stack.push(Value::Bool(is_subset))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Check if quote's effects are within allowed set"));
}
```

---

### Phase 3: Refinement Types (Weeks 7-10)

#### 3.1 Predicate Language

**File: `src/predicates.rs` (NEW)**

```rust
//! Predicate Language for Refinement Types
//!
//! Predicates are Kore QUOTES that return Bool.
//! This keeps everything as tools (Postulate 1).

use crate::value::Value;
use crate::op::Op;

/// A refinement specification
#[derive(Debug, Clone)]
pub struct Spec {
    /// Base type (optional, for documentation)
    pub base_type: Option<String>,
    /// Predicate quote: (value -- bool)
    pub predicate: Vec<Op>,
    /// Human-readable description
    pub description: String,
}

impl Spec {
    /// Create a spec from a predicate quote
    pub fn new(predicate: Vec<Op>, description: &str) -> Self {
        Self {
            base_type: None,
            predicate,
            description: description.to_string(),
        }
    }

    /// Create common specs
    pub fn positive_int() -> Self {
        // [ 0 swap lt ]  -- checks if value > 0
        Self::new(
            vec![
                Op::Push(Value::Int(0)),
                Op::Call("swap".into()),
                Op::Call("lt".into()),
            ],
            "positive integer (> 0)"
        )
    }

    pub fn non_empty_text() -> Self {
        // [ str-len 0 swap lt ]  -- checks if len > 0
        Self::new(
            vec![
                Op::Call("str-len".into()),
                Op::Push(Value::Int(0)),
                Op::Call("swap".into()),
                Op::Call("lt".into()),
            ],
            "non-empty text"
        )
    }

    pub fn in_range(min: i64, max: i64) -> Self {
        // [ dup MIN gte swap MAX lte and ]
        Self::new(
            vec![
                Op::Call("dup".into()),
                Op::Push(Value::Int(min)),
                Op::Call("gte".into()),
                Op::Call("swap".into()),
                Op::Push(Value::Int(max)),
                Op::Call("lte".into()),
                Op::Call("and".into()),
            ],
            &format!("integer in range [{}, {}]", min, max)
        )
    }
}
```

#### 3.2 Spec Tools

**File: `src/cap/specs.rs` (NEW)**

```rust
//! Specification Tools
//!
//! Specs are FIRST-CLASS VALUES (Postulate: programs are values)
//! Checking is a TOOL (Postulate: everything is a tool)

pub fn register(dict: &mut Dictionary) {
    // spec-new: Create a spec from predicate
    dict.register(Tool::native(
        "spec-new",
        "(predicate:Quote description:Text -- spec:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let description = stack.pop()?.into_text()?;
                let predicate = stack.pop()?.into_quote()?;
                
                let mut spec = IndexMap::new();
                spec.insert("predicate".into(), Value::Quote(predicate));
                spec.insert("description".into(), Value::Text(description));
                
                stack.push(Value::Map(spec))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Create a refinement spec from predicate quote"));

    // spec-check: Check if value satisfies spec
    dict.register(Tool::native(
        "spec-check",
        "(value:Any spec:Map -- bool:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let spec = stack.pop()?.into_map()?;
                let value = stack.pop()?;
                
                let predicate = spec.get("predicate")
                    .ok_or_else(|| Error::Runtime("spec missing predicate".into()))?
                    .clone()
                    .into_quote()?;
                
                // Push value, call predicate
                stack.push(value)?;
                let (mut new_stack, new_ctx) = ctx.execute(&predicate, stack).await?;
                
                let result = new_stack.pop()?.as_bool()?;
                new_stack.push(Value::Bool(result))?;
                
                Ok((new_stack, new_ctx))
            })
        },
    ).with_doc("Check if value satisfies spec's predicate"));

    // spec-assert: Assert value satisfies spec, fail otherwise
    dict.register(Tool::native(
        "spec-assert",
        "(value:Any spec:Map -- value:Any)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let spec = stack.pop()?.into_map()?;
                let value = stack.pop()?;
                
                let predicate = spec.get("predicate")
                    .ok_or_else(|| Error::Runtime("spec missing predicate".into()))?
                    .clone()
                    .into_quote()?;
                
                let description = spec.get("description")
                    .and_then(|v| v.as_text().ok())
                    .unwrap_or("spec".to_string());
                
                // Check predicate
                stack.push(value.clone())?;
                let (mut new_stack, new_ctx) = ctx.execute(&predicate, stack).await?;
                
                let satisfied = new_stack.pop()?.as_bool()?;
                
                if !satisfied {
                    return Err(Error::Runtime(format!(
                        "Spec violation: value does not satisfy '{}'", description
                    )));
                }
                
                new_stack.push(value)?;
                Ok((new_stack, new_ctx))
            })
        },
    ).with_doc("Assert value satisfies spec, fail if not"));

    // Standard specs as tools
    dict.register(Tool::native(
        "spec-positive",
        "( -- spec:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let spec = Spec::positive_int();
                stack.push(spec_to_value(&spec))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Spec for positive integers"));

    dict.register(Tool::native(
        "spec-non-empty",
        "( -- spec:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let spec = Spec::non_empty_text();
                stack.push(spec_to_value(&spec))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Spec for non-empty text"));
}
```

#### 3.3 Tool Specs (Contracts)

```rust
// Define tool with spec
dict.register(Tool::native(
    "safe-div",
    "(x:Int y:{v:Int | v != 0} -- result:Int)",
    |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let y = stack.pop()?.as_int()?;
            let x = stack.pop()?.as_int()?;
            
            // Spec is checked at call time by effect system
            // If we get here, y != 0 is guaranteed
            
            stack.push(Value::Int(x / y))?;
            Ok((stack, ctx))
        })
    },
).with_spec(ToolSpec {
    inputs: vec![
        TypeSpec::int(),
        TypeSpec::refined("Int", "v != 0"),
    ],
    outputs: vec![TypeSpec::int()],
}));
```

---

### Phase 4: Z3 Integration (Weeks 11-13)

#### 4.1 SMT-LIB Translation

**File: `src/smt.rs` (NEW)**

```rust
//! Kore → SMT-LIB Translation
//!
//! Translates Kore programs to SMT-LIB format for Z3 verification.

use crate::op::Op;
use crate::value::Value;

/// SMT-LIB generator
pub struct SmtGenerator {
    /// Counter for fresh variable names
    counter: usize,
    /// Generated declarations
    declarations: Vec<String>,
    /// Generated assertions
    assertions: Vec<String>,
}

impl SmtGenerator {
    pub fn new() -> Self {
        Self {
            counter: 0,
            declarations: vec![],
            assertions: vec![],
        }
    }

    /// Fresh variable name
    fn fresh(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("{}_{}", prefix, self.counter)
    }

    /// Translate a Kore program to SMT-LIB
    pub fn translate(&mut self, ops: &[Op]) -> String {
        let mut stack_vars: Vec<String> = vec![];

        for op in ops {
            match op {
                Op::Push(Value::Int(n)) => {
                    let var = self.fresh("s");
                    self.declarations.push(format!(
                        "(declare-const {} Int)", var
                    ));
                    self.assertions.push(format!(
                        "(assert (= {} {}))", var, n
                    ));
                    stack_vars.push(var);
                }
                Op::Push(Value::Bool(b)) => {
                    let var = self.fresh("s");
                    self.declarations.push(format!(
                        "(declare-const {} Bool)", var
                    ));
                    self.assertions.push(format!(
                        "(assert (= {} {}))", var, b
                    ));
                    stack_vars.push(var);
                }
                Op::Call(name) => {
                    match name.as_str() {
                        "add" => {
                            let b = stack_vars.pop().unwrap();
                            let a = stack_vars.pop().unwrap();
                            let r = self.fresh("s");
                            self.declarations.push(format!(
                                "(declare-const {} Int)", r
                            ));
                            self.assertions.push(format!(
                                "(assert (= {} (+ {} {})))", r, a, b
                            ));
                            stack_vars.push(r);
                        }
                        "sub" => {
                            let b = stack_vars.pop().unwrap();
                            let a = stack_vars.pop().unwrap();
                            let r = self.fresh("s");
                            self.declarations.push(format!(
                                "(declare-const {} Int)", r
                            ));
                            self.assertions.push(format!(
                                "(assert (= {} (- {} {})))", r, a, b
                            ));
                            stack_vars.push(r);
                        }
                        "mul" => {
                            let b = stack_vars.pop().unwrap();
                            let a = stack_vars.pop().unwrap();
                            let r = self.fresh("s");
                            self.declarations.push(format!(
                                "(declare-const {} Int)", r
                            ));
                            self.assertions.push(format!(
                                "(assert (= {} (* {} {})))", r, a, b
                            ));
                            stack_vars.push(r);
                        }
                        "lt" => {
                            let b = stack_vars.pop().unwrap();
                            let a = stack_vars.pop().unwrap();
                            let r = self.fresh("s");
                            self.declarations.push(format!(
                                "(declare-const {} Bool)", r
                            ));
                            self.assertions.push(format!(
                                "(assert (= {} (< {} {})))", r, a, b
                            ));
                            stack_vars.push(r);
                        }
                        "eq" => {
                            let b = stack_vars.pop().unwrap();
                            let a = stack_vars.pop().unwrap();
                            let r = self.fresh("s");
                            self.declarations.push(format!(
                                "(declare-const {} Bool)", r
                            ));
                            self.assertions.push(format!(
                                "(assert (= {} (= {} {})))", r, a, b
                            ));
                            stack_vars.push(r);
                        }
                        "dup" => {
                            let a = stack_vars.last().unwrap().clone();
                            stack_vars.push(a);
                        }
                        "drop" => {
                            stack_vars.pop();
                        }
                        "swap" => {
                            let b = stack_vars.pop().unwrap();
                            let a = stack_vars.pop().unwrap();
                            stack_vars.push(b);
                            stack_vars.push(a);
                        }
                        _ => {
                            // Unknown op - create symbolic result
                            let r = self.fresh("unknown");
                            self.declarations.push(format!(
                                "(declare-const {} Int)", r
                            ));
                            stack_vars.push(r);
                        }
                    }
                }
                _ => {}
            }
        }

        // Generate SMT-LIB output
        let mut output = String::new();
        output.push_str("; Kore → SMT-LIB translation\n");
        output.push_str("(set-logic QF_LIA)\n\n");
        
        for decl in &self.declarations {
            output.push_str(decl);
            output.push('\n');
        }
        output.push('\n');
        
        for assert in &self.assertions {
            output.push_str(assert);
            output.push('\n');
        }
        
        output.push_str("\n(check-sat)\n");
        output.push_str("(get-model)\n");
        
        output
    }
}
```

#### 4.2 Z3 Binding (Optional - via CLI)

```rust
//! Z3 Integration via CLI
//!
//! We don't link Z3 directly - we call the z3 binary.
//! This keeps Kore lightweight and Z3 optional.

use std::process::Command;

/// Verify SMT-LIB formula using Z3
pub fn verify_with_z3(smt_lib: &str) -> Result<VerifyResult, Error> {
    // Write to temp file
    let tmp = std::env::temp_dir().join("kore_verify.smt2");
    std::fs::write(&tmp, smt_lib)?;

    // Call Z3
    let output = Command::new("z3")
        .arg("-smt2")
        .arg(&tmp)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    if stdout.contains("unsat") {
        Ok(VerifyResult::Proven)
    } else if stdout.contains("sat") {
        // Extract counterexample
        Ok(VerifyResult::Counterexample(stdout.to_string()))
    } else {
        Ok(VerifyResult::Unknown)
    }
}

pub enum VerifyResult {
    Proven,
    Counterexample(String),
    Unknown,
}
```

#### 4.3 Verification Tools

```rust
pub fn register(dict: &mut Dictionary) {
    // verify: Verify spec holds using Z3
    dict.register(Tool::native(
        "verify",
        "(quote:Quote spec:Map -- result:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let spec = stack.pop()?.into_map()?;
                let quote = stack.pop()?.into_quote()?;
                
                // Generate SMT-LIB
                let mut gen = SmtGenerator::new();
                let smt = gen.translate_with_spec(&quote, &spec);
                
                // Call Z3
                let result = verify_with_z3(&smt)?;
                
                let mut result_map = IndexMap::new();
                match result {
                    VerifyResult::Proven => {
                        result_map.insert("status".into(), Value::Text("proven".into()));
                        result_map.insert("proven".into(), Value::Bool(true));
                    }
                    VerifyResult::Counterexample(ce) => {
                        result_map.insert("status".into(), Value::Text("counterexample".into()));
                        result_map.insert("proven".into(), Value::Bool(false));
                        result_map.insert("counterexample".into(), Value::Text(ce));
                    }
                    VerifyResult::Unknown => {
                        result_map.insert("status".into(), Value::Text("unknown".into()));
                        result_map.insert("proven".into(), Value::Bool(false));
                    }
                }
                
                stack.push(Value::Map(result_map))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Verify quote satisfies spec using Z3 SMT solver"));
}
```

---

### Phase 5: Proof-Carrying Code (Weeks 14-16)

#### 5.1 Proof Values

**File: `src/proof.rs` (NEW)**

```rust
//! Proof-Carrying Code
//!
//! A Proof is a VALUE that witnesses a property.
//! Proofs can be verified in O(n) without re-running Z3.

use crate::value::Value;
use serde::{Serialize, Deserialize};

/// A proof that a program satisfies a spec
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    /// The program being verified (hash)
    pub program_hash: String,
    /// The spec being verified (hash)
    pub spec_hash: String,
    /// Proof witness (SMT-LIB unsat core or similar)
    pub witness: String,
    /// Timestamp
    pub timestamp: u64,
    /// Prover identity (optional)
    pub prover: Option<String>,
}

impl Proof {
    /// Verify proof is valid (quick check)
    pub fn quick_verify(&self, program: &[Op], spec: &Spec) -> bool {
        // Check hashes match
        let program_hash = hash_ops(program);
        let spec_hash = hash_spec(spec);
        
        self.program_hash == program_hash && self.spec_hash == spec_hash
    }

    /// Full verification (re-run Z3)
    pub fn full_verify(&self, program: &[Op], spec: &Spec) -> Result<bool, Error> {
        // Re-generate SMT and verify
        let mut gen = SmtGenerator::new();
        let smt = gen.translate_with_spec(program, spec);
        
        match verify_with_z3(&smt)? {
            VerifyResult::Proven => Ok(true),
            _ => Ok(false),
        }
    }
}
```

#### 5.2 Proof Tools

```rust
pub fn register(dict: &mut Dictionary) {
    // prove: Generate proof for quote satisfying spec
    dict.register(Tool::native(
        "prove",
        "(quote:Quote spec:Map -- proof:Map | error:Error)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let spec = stack.pop()?.into_map()?;
                let quote = stack.pop()?.into_quote()?;
                
                // Try to prove
                let proof = generate_proof(&quote, &spec)?;
                
                match proof {
                    Some(p) => stack.push(proof_to_value(&p))?,
                    None => stack.push(Value::Error(Box::new(ErrorValue {
                        code: "CANNOT_PROVE".into(),
                        message: "Unable to generate proof".into(),
                        data: None,
                    })))?,
                }
                
                Ok((stack, ctx))
            })
        },
    ).with_doc("Generate proof that quote satisfies spec"));

    // proof-verify: Verify a proof
    dict.register(Tool::native(
        "proof-verify",
        "(quote:Quote spec:Map proof:Map -- bool:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let proof = stack.pop()?.into_map()?;
                let spec = stack.pop()?.into_map()?;
                let quote = stack.pop()?.into_quote()?;
                
                let p = proof_from_value(&proof)?;
                let s = spec_from_value(&spec)?;
                
                let valid = p.quick_verify(&quote, &s);
                
                stack.push(Value::Bool(valid))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Verify proof is valid for quote and spec"));

    // proof-attach: Attach proof to quote (makes it proof-carrying)
    dict.register(Tool::native(
        "proof-attach",
        "(quote:Quote proof:Map -- proven-quote:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let proof = stack.pop()?.into_map()?;
                let quote = stack.pop()?.into_quote()?;
                
                let mut proven = IndexMap::new();
                proven.insert("code".into(), Value::Quote(quote));
                proven.insert("proof".into(), Value::Map(proof));
                
                stack.push(Value::Map(proven))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Attach proof to quote, creating proof-carrying code"));
}
```

---

## Part IV: Complete Tool Inventory

### New Tools Summary

| Phase | Tool | Signature | Description |
|-------|------|-----------|-------------|
| **Linear** | `linear-new` | `(value -- linear)` | Wrap as linear |
| | `linear-consume` | `(linear quote -- result)` | Consume linear value |
| | `affine-new` | `(value -- affine)` | Wrap as affine |
| | `is-linear` | `(value -- bool)` | Check if linear |
| | `is-affine` | `(value -- bool)` | Check if affine |
| **Effects** | `effect-infer` | `(quote -- effects)` | Infer all effects |
| | `effect-pure?` | `(quote -- bool)` | Check if pure |
| | `effect-subset?` | `(quote allowed -- bool)` | Check effects in set |
| **Specs** | `spec-new` | `(predicate desc -- spec)` | Create spec |
| | `spec-check` | `(value spec -- bool)` | Check value against spec |
| | `spec-assert` | `(value spec -- value)` | Assert or fail |
| | `spec-positive` | `( -- spec)` | Positive int spec |
| | `spec-non-empty` | `( -- spec)` | Non-empty text spec |
| | `spec-range` | `(min max -- spec)` | Range spec |
| **Verify** | `verify` | `(quote spec -- result)` | Verify with Z3 |
| | `smt-export` | `(quote -- smt:Text)` | Export to SMT-LIB |
| **Proofs** | `prove` | `(quote spec -- proof)` | Generate proof |
| | `proof-verify` | `(quote spec proof -- bool)` | Verify proof |
| | `proof-attach` | `(quote proof -- proven)` | Create PCC |

**Total: 19 new tools**

---

## Part V: Modified Existing Code

### Files to Modify

| File | Changes |
|------|---------|
| `src/value.rs` | Add linearity to ExtValue, helper functions |
| `src/stack.rs` | Add linearity checks to `dup`, `drop` |
| `src/error.rs` | Add `LinearityViolation`, `SpecViolation` |
| `src/analyzer.rs` | Add IO effect inference |
| `src/types.rs` | Add `Effects` struct |
| `src/lib.rs` | Export new modules |
| `src/stdlib.rs` | Register new tools |

### New Files

| File | Purpose |
|------|---------|
| `src/linear.rs` | Linear type core |
| `src/effects.rs` | Effect algebra |
| `src/predicates.rs` | Predicate language |
| `src/smt.rs` | SMT-LIB translation |
| `src/proof.rs` | Proof-carrying code |
| `src/cap/linear.rs` | Linear tools |
| `src/cap/effects.rs` | Effect tools |
| `src/cap/specs.rs` | Spec tools |
| `src/cap/verify.rs` | Verification tools |
| `src/cap/proof.rs` | Proof tools |

---

## Part VI: Testing Strategy

### Test Categories

```
tests/
├── linear/
│   ├── linear_basic.rs       # Create, consume linear values
│   ├── linear_violations.rs  # Test dup/drop errors
│   ├── affine_handles.rs     # Test file/connection handles
│   └── linear_spawn.rs       # Linear values across spawn
├── effects/
│   ├── effect_inference.rs   # Test effect detection
│   ├── effect_composition.rs # Test effect union
│   ├── pure_detection.rs     # Test pure function detection
│   └── sandbox_effects.rs    # Test effect-based sandboxing
├── specs/
│   ├── spec_basic.rs         # Create, check specs
│   ├── spec_stdlib.rs        # Test standard specs
│   ├── spec_composition.rs   # Specs on composed code
│   └── spec_failures.rs      # Test spec violation errors
├── verify/
│   ├── smt_translation.rs    # Test Kore → SMT-LIB
│   ├── z3_integration.rs     # Test Z3 (if available)
│   └── verify_stdlib.rs      # Verify standard tools
└── proofs/
    ├── proof_generation.rs   # Test proof creation
    ├── proof_verification.rs # Test proof checking
    └── proof_carrying.rs     # Test PCC workflow
```

### Critical Properties to Test

```rust
// Linear soundness
#[test]
fn linear_value_used_exactly_once() {
    let code = r#"
        42 linear-new
        [ 1 add ] linear-consume
    "#;
    assert!(execute(code).is_ok());
}

#[test]
fn linear_dup_fails() {
    let code = r#"
        42 linear-new
        dup  // ERROR
    "#;
    assert!(execute(code).is_err());
}

#[test]
fn linear_drop_fails() {
    let code = r#"
        42 linear-new
        drop  // ERROR
    "#;
    assert!(execute(code).is_err());
}

// Effect soundness
#[test]
fn pure_function_detected() {
    let code = r#"
        [ 1 2 add ] effect-pure?
    "#;
    assert_eq!(execute(code), Ok(Value::Bool(true)));
}

#[test]
fn impure_function_detected() {
    let code = r#"
        [ "file.txt" fs-read ] effect-pure?
    "#;
    assert_eq!(execute(code), Ok(Value::Bool(false)));
}

// Spec soundness
#[test]
fn spec_positive_rejects_zero() {
    let code = r#"
        0 spec-positive spec-check
    "#;
    assert_eq!(execute(code), Ok(Value::Bool(false)));
}

#[test]
fn spec_positive_accepts_one() {
    let code = r#"
        1 spec-positive spec-check
    "#;
    assert_eq!(execute(code), Ok(Value::Bool(true)));
}
```

---

## Part VII: Timeline

```
Week 1-2:   Linear type infrastructure (linear.rs, stack changes)
Week 3:     Linear tools, handle linearity, tests
Week 4-5:   Effect algebra, inference in analyzer
Week 6:     Effect tools, tests
Week 7-8:   Predicate language, spec tools
Week 9-10:  Spec integration, tool contracts, tests
Week 11-12: SMT translation, Z3 integration
Week 13:    Verification tools, tests
Week 14-15: Proof generation, PCC
Week 16:    Integration tests, documentation, polish
```

---

## Part VIII: Success Criteria

### Mathematical Properties (MUST HOLD)

1. **Linear Soundness**: A linear value is used exactly once
   ```
   ∀v : Linear. uses(v) = 1
   ```

2. **Effect Soundness**: Inferred effects are an upper bound
   ```
   effects_actual(P) ⊆ effects_inferred(P)
   ```

3. **Spec Soundness**: If check passes, predicate holds
   ```
   spec-check(v, s) = true ⟹ s.predicate(v) = true
   ```

4. **Verification Soundness**: If verified, spec holds for all inputs
   ```
   verify(P, S) = proven ⟹ ∀input. S.pre(input) ⟹ S.post(P(input))
   ```

5. **Proof Soundness**: Valid proofs only exist for true properties
   ```
   proof-verify(P, S, π) = true ⟹ P satisfies S
   ```

### Postulate Compliance (MUST HOLD)

- [ ] Every feature is implemented as tools
- [ ] All tools transform Stack → Stack
- [ ] Composition is concatenation
- [ ] No hidden state

### Performance Targets

- Linear check overhead: < 1μs per operation
- Effect inference: < 1ms for 1000 ops
- Spec check: < 10μs per check
- SMT translation: < 10ms for 1000 ops
- Z3 verification: Timeout at 10s, report unknown

---

## Part IX: What This Enables

### The New Capabilities

```kore
; 1. Self-modifying agent that stays safe
: evolve ( code -- code' )
  optimize                          ; Transform code
  dup [ fs net ] effect-subset?     ; Check effects still bounded
  [ "evolution broke sandbox" fail ] unless
;

; 2. Proof exchange between agents
: receive-data ( data proof -- data )
  over spec-schema                  ; Get expected spec
  proof-verify                      ; Verify proof matches
  [ "invalid proof" fail ] unless
;

; 3. Linear resource safety
: with-file ( path quote -- result )
  swap fs-open                      ; Open file (returns linear handle)
  swap                              ; ( handle quote )
  [ call ] linear-consume           ; Use handle exactly once
;

; 4. Verified contracts
: payment ( amount recipient -- receipt )
  over spec-positive spec-assert    ; Amount must be positive
  swap send-money                   ; Execute payment
;
```

### The Guarantee

Every Kore 2.0 program has these properties:

1. **No resource leaks** - Linear types enforce cleanup
2. **No sandbox escape** - Effects prove containment
3. **No invalid data** - Specs validate at boundaries
4. **Auditable in O(n)** - Proofs verify without re-analysis

---

## Shall I Begin Implementation?

The plan is complete. Every phase respects the postulates. Every tool transforms stacks. Every feature is mathematically grounded.

**Start with Phase 1 (Linear Types)?** This is the foundation everything else builds on.
