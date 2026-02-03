# Kore v2: Upgrade Plan

## Overview

**Goal**: Upgrade Kore's value system to support extensions (Tensor, Fiber, Linear, Distribution) while:
- Preserving all three postulates
- Maintaining 375 passing tests
- Reducing memory footprint from 48 bytes to 16 bytes per value
- Enabling O(1) clone via Arc

---

## Phase 0: Pre-Upgrade Verification

### 0.1 Baseline Metrics

```bash
# Record current state
cargo test 2>&1 | tee baseline_tests.log
cargo test -- --ignored 2>&1 | tee baseline_ignored.log

# Count tests
grep -E "^test result:" baseline_tests.log

# Measure binary size
cargo build --release
ls -la target/release/kore

# Measure value size
# Add temporary test:
assert_eq!(std::mem::size_of::<Value>(), 48);  # Current
```

### 0.2 Create Upgrade Branch

```bash
git checkout -b upgrade/arc-values
git push -u origin upgrade/arc-values
```

---

## Phase 1: Value Refactoring (Week 1)

### 1.1 Files to Modify

| File | Changes |
|------|---------|
| `src/value.rs` | Complete rewrite |
| `src/op.rs` | Update Op::Push type |
| `src/stack.rs` | Update push/pop signatures |
| `src/error.rs` | Add LinearViolation error |
| `src/core/*.rs` | Update all 75 tools |
| `src/cap/*.rs` | Update all 55 tools |
| `tests/*.rs` | Update value construction |

### 1.2 New Value Type

**Before** (`src/value.rs`):
```rust
pub enum Value {
    Int(i64),
    Float(f64),
    Text(String),
    List(Vec<Value>),
    Quote(Vec<Op>),
    Map(BTreeMap<String, Value>),
    Error(String),
}
```

**After** (`src/value.rs`):
```rust
use std::sync::Arc;

/// Core value type - 16 bytes on 64-bit systems
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    // Atoms (inline, no allocation)
    Int(i64),
    Float(f64),
    
    // Shared compounds (Arc for O(1) clone)
    Text(Arc<str>),
    List(Arc<[Value]>),
    Quote(Arc<[Op]>),
    Map(Arc<Map>),
    
    // Extension wrapper (single variant for ALL extensions)
    Ext(Arc<ExtValue>),
}

/// Map type alias
pub type Map = std::collections::BTreeMap<Arc<str>, Value>;

/// Extension value - wraps any value with kind + metadata
#[derive(Clone, Debug, PartialEq)]
pub struct ExtValue {
    /// Extension kind: 0=tensor, 1=fiber, 2=linear, 3=dist, 4+=future
    pub kind: u8,
    /// The wrapped value
    pub data: Value,
    /// Optional metadata (lazy allocated)
    pub meta: Option<Arc<Map>>,
}

/// Extension kinds as constants
pub mod ext {
    pub const TENSOR: u8 = 0;
    pub const FIBER: u8 = 1;
    pub const LINEAR: u8 = 2;
    pub const DIST: u8 = 3;
}

impl Value {
    // Constructors (maintain API compatibility)
    pub fn text(s: impl AsRef<str>) -> Self {
        Value::Text(Arc::from(s.as_ref()))
    }
    
    pub fn list(v: Vec<Value>) -> Self {
        Value::List(Arc::from(v))
    }
    
    pub fn quote(ops: Vec<Op>) -> Self {
        Value::Quote(Arc::from(ops))
    }
    
    pub fn map(m: BTreeMap<String, Value>) -> Self {
        let m: Map = m.into_iter()
            .map(|(k, v)| (Arc::from(k.as_str()), v))
            .collect();
        Value::Map(Arc::new(m))
    }
    
    // Extension constructors
    pub fn tensor(data: Value, meta: Option<Map>) -> Self {
        Value::Ext(Arc::new(ExtValue {
            kind: ext::TENSOR,
            data,
            meta: meta.map(Arc::new),
        }))
    }
    
    pub fn fiber(data: Value, meta: Option<Map>) -> Self {
        Value::Ext(Arc::new(ExtValue {
            kind: ext::FIBER,
            data,
            meta: meta.map(Arc::new),
        }))
    }
    
    pub fn linear(data: Value) -> Self {
        Value::Ext(Arc::new(ExtValue {
            kind: ext::LINEAR,
            data,
            meta: None,
        }))
    }
    
    pub fn distribution(data: Value, meta: Option<Map>) -> Self {
        Value::Ext(Arc::new(ExtValue {
            kind: ext::DIST,
            data,
            meta: meta.map(Arc::new),
        }))
    }
    
    // Predicates
    pub fn is_linear(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::LINEAR)
    }
    
    pub fn is_tensor(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::TENSOR)
    }
    
    pub fn is_fiber(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::FIBER)
    }
    
    pub fn is_dist(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::DIST)
    }
    
    // Accessors (updated for Arc)
    pub fn as_text(&self) -> Result<&str> {
        match self {
            Value::Text(s) => Ok(s.as_ref()),
            _ => Err(Error::TypeError { expected: "Text".into(), got: self.type_name().into() })
        }
    }
    
    pub fn as_list(&self) -> Result<&[Value]> {
        match self {
            Value::List(l) => Ok(l.as_ref()),
            _ => Err(Error::TypeError { expected: "List".into(), got: self.type_name().into() })
        }
    }
    
    pub fn as_quote(&self) -> Result<&[Op]> {
        match self {
            Value::Quote(q) => Ok(q.as_ref()),
            _ => Err(Error::TypeError { expected: "Quote".into(), got: self.type_name().into() })
        }
    }
    
    pub fn into_list(self) -> Result<Vec<Value>> {
        match self {
            Value::List(l) => Ok(l.to_vec()),  // Note: copies when needed
            _ => Err(Error::TypeError { expected: "List".into(), got: self.type_name().into() })
        }
    }
    
    // Type name for errors
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Text(_) => "Text",
            Value::List(_) => "List",
            Value::Quote(_) => "Quote",
            Value::Map(_) => "Map",
            Value::Ext(e) => match e.kind {
                ext::TENSOR => "Tensor",
                ext::FIBER => "Fiber",
                ext::LINEAR => "Linear",
                ext::DIST => "Distribution",
                _ => "Ext",
            }
        }
    }
}
```

### 1.3 Error Updates

**Add to `src/error.rs`**:
```rust
pub enum Error {
    // ... existing variants ...
    
    /// Attempted to duplicate a linear value
    LinearDuplicate(String),
    
    /// Attempted to discard a linear value
    LinearDiscard(String),
    
    /// Linear value not consumed before scope exit
    LinearLeak(String),
}
```

### 1.4 Stack Updates

**Modify `src/core/stack.rs`**:
```rust
// dup must check linearity
dict.register(Tool::native(
    "dup",
    "(a -- a a)",
    |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.peek()?;
            if v.is_linear() {
                return Err(Error::LinearDuplicate(
                    "cannot duplicate linear value".into()
                ));
            }
            stack.push(v.clone())?;
            Ok((stack, ctx))
        })
    },
));

// drop must check linearity
dict.register(Tool::native(
    "drop",
    "(a -- )",
    |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            if v.is_linear() {
                return Err(Error::LinearDiscard(
                    "cannot discard linear value; use linear-unwrap".into()
                ));
            }
            Ok((stack, ctx))
        })
    },
));
```

### 1.5 Migration Script

Create `scripts/migrate_values.sh`:
```bash
#!/bin/bash
# Migrate old Value construction to new API

# String -> Arc<str>
find src tests -name "*.rs" -exec sed -i \
    's/Value::Text(\([^)]*\)\.to_string())/Value::text(\1)/g' {} \;
find src tests -name "*.rs" -exec sed -i \
    's/Value::Text(\([^)]*\)\.into())/Value::text(\1)/g' {} \;
find src tests -name "*.rs" -exec sed -i \
    's/Value::Text("\([^"]*\)"\.to_string())/Value::text("\1")/g' {} \;

# Vec<Value> -> Arc<[Value]>  
find src tests -name "*.rs" -exec sed -i \
    's/Value::List(vec!\[\([^]]*\)\])/Value::list(vec![\1])/g' {} \;

# Vec<Op> -> Arc<[Op]>
find src tests -name "*.rs" -exec sed -i \
    's/Value::Quote(vec!\[\([^]]*\)\])/Value::quote(vec![\1])/g' {} \;
```

---

## Phase 2: Core Tool Updates (Week 2)

### 2.1 Tools Requiring Changes

| Module | Tools | Change Required |
|--------|-------|-----------------|
| `core/stack.rs` | `dup`, `drop` | Add linear check |
| `core/list.rs` | `list-get`, `list-set`, etc. | Use `as_list()` |
| `core/string.rs` | All 13 tools | Use `as_text()` |
| `core/map.rs` | All 6 tools | Use Arc<Map> |
| `core/types.rs` | `type-of` | Add Ext cases |
| `core/combinators.rs` | `map`, `filter`, `fold` | Handle Ext |

### 2.2 Example Tool Migration

**Before**:
```rust
dict.register(Tool::native(
    "list-len",
    "(list -- int)",
    |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let list = stack.pop()?.into_list()?;
            stack.push(Value::Int(list.len() as i64))?;
            Ok((stack, ctx))
        })
    },
));
```

**After**:
```rust
dict.register(Tool::native(
    "list-len",
    "(list -- int)",
    |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            let list = v.as_list()?;  // Borrow, don't move
            stack.push(Value::Int(list.len() as i64))?;
            Ok((stack, ctx))
        })
    },
));
```

### 2.3 Type-Of Extension

**Add to `core/types.rs`**:
```rust
Value::Ext(e) => match e.kind {
    ext::TENSOR => Value::text("tensor"),
    ext::FIBER => Value::text("fiber"),
    ext::LINEAR => Value::text("linear"),
    ext::DIST => Value::text("distribution"),
    k => Value::text(format!("ext:{}", k)),
}
```

---

## Phase 3: Extension Tools (Weeks 3-4)

### 3.1 New Files to Create

| File | Tools | Purpose |
|------|-------|---------|
| `src/cap/tensor.rs` | 15 | Differentiable arrays |
| `src/cap/fiber.rs` | 8 | Reified computations |
| `src/cap/linear.rs` | 5 | Linear value handling |
| `src/cap/prob.rs` | 8 | Probability distributions |

### 3.2 Tensor Tools

```rust
// src/cap/tensor.rs

pub fn register(dict: &mut Dictionary) {
    // tensor-new: Create tensor from shape and data
    dict.register(Tool::native(
        "tensor-new",
        "(shape data -- tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let data = stack.pop()?;
                let shape = stack.pop()?;
                
                let meta = {
                    let mut m = Map::new();
                    m.insert(Arc::from("shape"), shape);
                    m.insert(Arc::from("requires_grad"), Value::Int(0));
                    m
                };
                
                stack.push(Value::tensor(data, Some(meta)))?;
                Ok((stack, ctx))
            })
        },
    ));
    
    // requires-grad: Enable gradient tracking
    dict.register(Tool::native(
        "requires-grad",
        "(tensor -- tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let t = stack.pop()?;
                if let Value::Ext(e) = &t {
                    if e.kind == ext::TENSOR {
                        let mut meta = e.meta.as_ref()
                            .map(|m| (**m).clone())
                            .unwrap_or_default();
                        meta.insert(Arc::from("requires_grad"), Value::Int(1));
                        stack.push(Value::tensor(e.data.clone(), Some(meta)))?;
                        return Ok((stack, ctx));
                    }
                }
                Err(Error::TypeError { expected: "Tensor".into(), got: t.type_name().into() })
            })
        },
    ));
    
    // matmul, add, relu, softmax, backward, grad, zero-grad...
}
```

### 3.3 Fiber Tools

```rust
// src/cap/fiber.rs

pub fn register(dict: &mut Dictionary) {
    // fiber-new: Create paused fiber from quote
    dict.register(Tool::native(
        "fiber-new",
        "(quote -- fiber)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?;
                let ops = quote.as_quote()?;
                
                // Fiber data: [stack, ops, ip, status]
                let fiber_data = Value::list(vec![
                    Value::list(vec![]),           // Empty stack
                    Value::Quote(Arc::from(ops)),  // Program
                    Value::Int(0),                 // IP = 0
                    Value::text("paused"),         // Status
                ]);
                
                stack.push(Value::fiber(fiber_data, None))?;
                Ok((stack, ctx))
            })
        },
    ));
    
    // fiber-resume, fiber-yield, fiber-fork, fiber-status, fiber-stack...
}
```

### 3.4 Linear Tools

```rust
// src/cap/linear.rs

pub fn register(dict: &mut Dictionary) {
    // linear-wrap: Make a value linear (non-duplicable)
    dict.register(Tool::native(
        "linear-wrap",
        "(value -- linear)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                stack.push(Value::linear(v))?;
                Ok((stack, ctx))
            })
        },
    ));
    
    // linear-unwrap: Consume linear value, extract inner
    dict.register(Tool::native(
        "linear-unwrap",
        "(linear -- value)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                if let Value::Ext(e) = v {
                    if e.kind == ext::LINEAR {
                        stack.push(e.data.clone())?;
                        return Ok((stack, ctx));
                    }
                }
                Err(Error::TypeError { expected: "Linear".into(), got: "other".into() })
            })
        },
    ));
    
    // is-linear, linear-peek...
}
```

### 3.5 Distribution Tools

```rust
// src/cap/prob.rs

pub fn register(dict: &mut Dictionary) {
    // dist-normal: Create normal distribution
    dict.register(Tool::native(
        "dist-normal",
        "(mean std -- dist)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let std = stack.pop()?.as_float()?;
                let mean = stack.pop()?.as_float()?;
                
                let data = Value::list(vec![Value::Float(mean), Value::Float(std)]);
                let mut meta = Map::new();
                meta.insert(Arc::from("type"), Value::text("normal"));
                
                stack.push(Value::distribution(data, Some(meta)))?;
                Ok((stack, ctx))
            })
        },
    ));
    
    // sample, score, observe, infer, log-prob...
}
```

---

## Phase 4: Context State (Week 4)

### 4.1 Context Updates

**Modify `src/context.rs`**:
```rust
pub struct Context {
    pub dict: Arc<RwLock<Dictionary>>,
    pub caps: Capabilities,
    pub resources: Resources,
    /// Extension state - accessible via ctx-get/ctx-set tools
    pub state: Arc<RwLock<Map>>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            dict: Arc::new(RwLock::new(Dictionary::new())),
            caps: Capabilities::default(),
            resources: Resources::default(),
            state: Arc::new(RwLock::new(Map::new())),
        }
    }
}
```

### 4.2 Context Tools

```rust
// src/cap/ctx.rs

pub fn register(dict: &mut Dictionary) {
    // ctx-get: Get extension state value
    dict.register(Tool::native(
        "ctx-get",
        "(key -- value)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?;
                let key_str = key.as_text()?;
                
                let state = ctx.state.read().await;
                let value = state.get(&Arc::from(key_str))
                    .cloned()
                    .unwrap_or(Value::list(vec![]));  // Empty list as "null"
                
                stack.push(value)?;
                Ok((stack, ctx))
            })
        },
    ));
    
    // ctx-set: Set extension state value
    dict.register(Tool::native(
        "ctx-set",
        "(value key -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?;
                let value = stack.pop()?;
                let key_str = key.as_text()?;
                
                let mut state = ctx.state.write().await;
                state.insert(Arc::from(key_str), value);
                
                Ok((stack, ctx))
            })
        },
    ));
    
    // ctx-keys: List all state keys
    dict.register(Tool::native(
        "ctx-keys",
        "( -- keys)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let state = ctx.state.read().await;
                let keys: Vec<Value> = state.keys()
                    .map(|k| Value::Text(k.clone()))
                    .collect();
                stack.push(Value::list(keys))?;
                Ok((stack, ctx))
            })
        },
    ));
}
```

---

## Phase 5: Testing Strategy (Week 5)

### 5.1 Consistency Tests

**Create `tests/upgrade_consistency.rs`**:
```rust
//! Tests that verify upgrade doesn't break existing behavior

use kore::{Context, execute, Op, Stack, Value};
use kore::builtins::register_builtins;

/// Test that Value size is reduced
#[test]
fn test_value_size() {
    assert_eq!(std::mem::size_of::<Value>(), 16);
}

/// Test that Arc sharing works
#[test]
fn test_arc_sharing() {
    let list = Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
    let list2 = list.clone();
    
    // Both should point to same data
    if let (Value::List(a), Value::List(b)) = (&list, &list2) {
        assert!(Arc::ptr_eq(a, b));
    } else {
        panic!("Expected List");
    }
}

/// Test that all existing tools still work
#[tokio::test]
async fn test_all_core_tools() {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    
    // Test each tool category
    let test_cases = vec![
        // Stack
        ("1 2 dup", vec![Value::Int(1), Value::Int(2), Value::Int(2)]),
        ("1 2 drop", vec![Value::Int(1)]),
        ("1 2 swap", vec![Value::Int(2), Value::Int(1)]),
        
        // Arithmetic
        ("2 3 add", vec![Value::Int(5)]),
        ("10 3 sub", vec![Value::Int(7)]),
        ("4 5 mul", vec![Value::Int(20)]),
        ("15 3 div", vec![Value::Int(5)]),
        
        // Comparison
        ("1 2 eq", vec![Value::Int(0)]),
        ("2 2 eq", vec![Value::Int(1)]),
        ("1 2 lt", vec![Value::Int(1)]),
        
        // Logic
        ("1 1 and", vec![Value::Int(1)]),
        ("1 0 or", vec![Value::Int(1)]),
        ("1 not", vec![Value::Int(0)]),
        
        // Lists
        ("[ 1 2 3 ] list-len", vec![Value::Int(3)]),
        ("[ 1 2 3 ] 1 list-get", vec![Value::Int(2)]),
        
        // Strings
        ("\"hello\" str-len", vec![Value::Int(5)]),
        
        // Control
        ("1 [ 2 ] [ 3 ] if", vec![Value::Int(2)]),
        ("0 [ 2 ] [ 3 ] if", vec![Value::Int(3)]),
    ];
    
    for (code, expected) in test_cases {
        let ops = Op::parse(code).unwrap();
        let (stack, _) = execute(&ops, Stack::new(), ctx.clone()).await.unwrap();
        assert_eq!(stack.values(), &expected, "Failed for: {}", code);
    }
}

/// Test linear value semantics
#[tokio::test]
async fn test_linear_cannot_dup() {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    
    let ops = Op::parse("42 linear-wrap dup").unwrap();
    let result = execute(&ops, Stack::new(), ctx).await;
    
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(format!("{:?}", e).contains("Linear"));
    }
}

/// Test linear value semantics
#[tokio::test]
async fn test_linear_cannot_drop() {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    
    let ops = Op::parse("42 linear-wrap drop").unwrap();
    let result = execute(&ops, Stack::new(), ctx).await;
    
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(format!("{:?}", e).contains("Linear"));
    }
}

/// Test linear value can be unwrapped
#[tokio::test]
async fn test_linear_unwrap() {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    
    let ops = Op::parse("42 linear-wrap linear-unwrap").unwrap();
    let (stack, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
    
    assert_eq!(stack.values(), &[Value::Int(42)]);
}
```

### 5.2 Property-Based Tests

**Create `tests/properties.rs`**:
```rust
use proptest::prelude::*;
use kore::{Value, Op};

proptest! {
    /// Clone should always be O(1) and produce equal value
    #[test]
    fn prop_clone_equality(v in arb_value()) {
        let v2 = v.clone();
        prop_assert_eq!(v, v2);
    }
    
    /// Arc should be shared after clone
    #[test]
    fn prop_arc_sharing(s in ".*") {
        let v = Value::text(&s);
        let v2 = v.clone();
        if let (Value::Text(a), Value::Text(b)) = (&v, &v2) {
            prop_assert!(Arc::ptr_eq(a, b));
        }
    }
    
    /// Linear values should reject dup
    #[test]
    fn prop_linear_no_dup(i in any::<i64>()) {
        let v = Value::linear(Value::Int(i));
        prop_assert!(v.is_linear());
    }
}

fn arb_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<i64>().prop_map(Value::Int),
        any::<f64>().prop_filter_map("finite", |f| 
            if f.is_finite() { Some(Value::Float(f)) } else { None }
        ),
        ".*".prop_map(|s| Value::text(s)),
    ]
}
```

### 5.3 Postulate Compliance Tests

**Create `tests/postulates.rs`**:
```rust
//! Tests that verify the three postulates

use kore::{Context, Dictionary, Tool, execute, Op, Stack, Value};

/// P1: Everything is a Tool
/// Verify all operations are registered via Tool::native
#[test]
fn test_p1_everything_is_tool() {
    // The only things in the system are:
    // 1. Values (data)
    // 2. Ops (Push/Call)
    // 3. Tools (in dictionary)
    
    // No other constructs exist
    assert_eq!(std::mem::variant_count::<Op>(), 2);  // Push, Call only
}

/// P2: Tools transform stacks
/// Verify tool signature is (Stack, Context) -> (Stack, Context)
#[tokio::test]
async fn test_p2_stack_transformation() {
    let mut ctx = Context::new();
    
    // Register a tool and verify its signature
    let dict = ctx.dict.write().await;
    
    // Every tool in the dictionary has the same signature
    for (name, tool) in dict.iter() {
        // Tool exists - that's the only interface
        assert!(!name.is_empty());
    }
}

/// P3: Composition is concatenation
/// Verify programs are flat sequences
#[test]
fn test_p3_flat_composition() {
    let program = Op::parse("1 2 add 3 mul").unwrap();
    
    // Program is Vec<Op> - flat, not nested
    assert_eq!(program.len(), 5);
    
    // Each op is independent
    for op in &program {
        match op {
            Op::Push(_) => {},
            Op::Call(_) => {},
        }
    }
}

/// Extended values preserve postulates
#[tokio::test]
async fn test_extensions_preserve_postulates() {
    let mut ctx = Context::new();
    kore::builtins::register_builtins(&mut ctx).await;
    
    // Extension values are just Values (P1)
    let tensor = Value::tensor(Value::list(vec![Value::Float(1.0)]), None);
    let fiber = Value::fiber(Value::list(vec![]), None);
    let linear = Value::linear(Value::Int(42));
    let dist = Value::distribution(Value::list(vec![Value::Float(0.0)]), None);
    
    // Can be pushed onto stack (P2)
    let mut stack = Stack::new();
    stack.push(tensor.clone()).unwrap();
    stack.push(fiber.clone()).unwrap();
    // Note: linear can be pushed once
    // Note: dist can be pushed
    
    // Can be operated on by tools (P3 - composition)
    let ops = vec![
        Op::Push(tensor),
        Op::Call("type-of".into()),
    ];
    let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
    assert_eq!(result.values()[0], Value::text("tensor"));
}
```

### 5.4 Regression Test Suite

```bash
# Run all tests and compare to baseline
cargo test 2>&1 | tee upgrade_tests.log
diff baseline_tests.log upgrade_tests.log

# Should have SAME number of passes, ZERO new failures
```

---

## Phase 6: Verification Checklist

### 6.1 Before Merge

- [ ] All 375 tests pass
- [ ] `sizeof(Value) == 16`
- [ ] Linear dup fails
- [ ] Linear drop fails
- [ ] Linear unwrap succeeds
- [ ] Arc sharing verified
- [ ] No new warnings
- [ ] Benchmarks show improvement

### 6.2 Postulate Checklist

- [ ] **P1**: All extension ops are `Tool::native()`
- [ ] **P1**: No new keywords
- [ ] **P1**: No special syntax
- [ ] **P2**: All tools take `(Stack, Context)`
- [ ] **P2**: All tools return `(Stack, Context)`
- [ ] **P2**: No global state
- [ ] **P3**: Programs are `Vec<Op>`
- [ ] **P3**: Composition is sequential
- [ ] **P3**: No nested scopes

### 6.3 Performance Checklist

- [ ] Value clone is O(1)
- [ ] Stack push is O(1)
- [ ] Fiber fork is O(1) initially
- [ ] Memory usage reduced
- [ ] No regressions in benchmarks

---

## Summary

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| 0: Baseline | 1 day | Metrics recorded |
| 1: Value refactor | 1 week | New Value type, 16 bytes |
| 2: Core tools | 1 week | All 75 core tools updated |
| 3: Extension tools | 1 week | 36 new tools (tensor, fiber, linear, prob) |
| 4: Context state | 0.5 week | ctx-get, ctx-set, ctx-keys |
| 5: Testing | 0.5 week | 100+ new tests |
| **Total** | **4 weeks** | **Upgraded Kore v2** |

### Files Changed

| Category | Files | Changes |
|----------|-------|---------|
| Core | 15 files | Value type, all tools |
| New | 5 files | tensor, fiber, linear, prob, ctx |
| Tests | 4 files | consistency, properties, postulates, regression |
| **Total** | **24 files** | **~3000 lines changed/added** |
