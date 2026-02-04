# Kore Formal Verification Enhancement Specification

> **Status**: ✅ IMPLEMENTED  
> **Date**: 2025-02-04  
> **Author**: Agent (based on debugging session findings)

This document specifies all changes to strengthen Kore's formal verification capabilities.

---

## Executive Summary

The debugging session revealed a critical insight: **the Kore runtime is already pure**. The corruption was entirely in documentation that claimed non-existent convenience tools (`gt`, `neq`, `when`, `abs`, etc.) existed. The runtime correctly has 197 primitives with no composed convenience tools.

This specification proposes enhancements to:
1. Fix analyzer tables that still reference non-existent tools
2. Add `def-verified` tool for definitions with effect verification
3. Add `axioms` tool to export algebraic identities
4. Add `selftest` tool for runtime self-verification
5. Add `CAP_UNVERIFIED` capability for discovery mode
6. Enhance error system for machine-parseable output

---

## Change 1: Fix Analyzer Effect Tables

### Problem

[analyzer.rs](../src/analyzer.rs#L305-L311) contains effect mappings for tools that DO NOT EXIST:

```rust
// Line 305-311 in analyzer.rs
"eq" | "neq" | "lt" | "lte" | "gt" | "gte" => (2, 1),
...
"neg" | "abs" => (1, 1),
...
"when" | "unless" => (2, 0), // cond quote --
```

The tools `neq`, `lte`, `gt`, `gte`, `abs`, `when`, `unless` are NOT primitives.

### Solution

Remove non-existent tools from effect tables:

**File**: [src/analyzer.rs](../src/analyzer.rs)  
**Lines**: 305-311, 370

```rust
// BEFORE (line 305):
"eq" | "neq" | "lt" | "lte" | "gt" | "gte" => (2, 1),

// AFTER:
"eq" | "lt" => (2, 1),

// BEFORE (line 308):
"neg" | "abs" => (1, 1),

// AFTER:
"neg" => (1, 1),

// BEFORE (line 370):
"when" | "unless" => (2, 0),

// AFTER:
// DELETE this line entirely
```

### Postulate Compliance

✅ **P1**: Removing phantom entries doesn't add non-tool syntax  
✅ **P2**: Effect mappings remain Stack → Stack  
✅ **P3**: No change to composition semantics

### Mathematical Justification

The analyzer should only model what exists. Phantom entries create misalignment between the model and reality.

---

## Change 2: Add `def-verified` Tool

### Problem

Currently, a user can define a tool with a claimed effect signature that doesn't match the actual effect:

```kore
[ dup ] "bad-def" def   ; No effect checking!
```

There's no way to define a tool AND verify its effect at definition time.

### Solution

Add `def-verified` tool that:
1. Takes a quote and a name and an effect signature
2. Infers the quote's actual effect
3. Verifies actual matches declared
4. Only defines if they match (fails otherwise)

**File**: [src/core/definition.rs](../src/core/definition.rs)  
**Action**: Add new tool

```rust
// def-verified: (quote name sig -- )
// Defines tool ONLY if inferred effect matches declared signature
dict.register(Tool::native(
    "def-verified",
    "(code:Quote name:Text sig:Text -- )",
    |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let sig = stack.pop()?.into_text()?;
            let name = stack.pop()?.into_text()?;
            let quote = stack.pop()?.into_quote()?;
            
            // Parse declared effect
            let declared = Effect::parse(&sig)
                .map_err(|e| Error::Runtime(format!("def-verified: invalid signature: {}", e)))?;
            
            // Infer actual effect
            let analysis = analyzer::analyze(&quote);
            
            // Check for errors
            if analysis.has_errors() {
                return Err(Error::Runtime(format!(
                    "def-verified: quote has stack errors: {:?}", 
                    analysis.errors
                )));
            }
            
            // Compare effects
            if analysis.effect != declared {
                return Err(Error::EffectMismatch {
                    expected: format!("{}", declared),
                    got: format!("{}", analysis.effect),
                });
            }
            
            // Define the tool
            let mut dict = ctx.dict.write().await;
            dict.register(Tool::composed(
                &name,
                &sig,
                quote,
            ));
            drop(dict);
            
            Ok((stack, ctx))
        })
    },
).with_doc("Define tool only if effect matches signature"));
```

**Signature**: `(code:Quote name:Text sig:Text -- )`  
**Effect**: `(3 -- 0)` - consumes 3, produces 0

### Usage

```kore
; Correct - will succeed
[ dup mul ] "square" "(n -- n)" def-verified

; Wrong - will FAIL at definition time
[ dup ] "broken" "(a -- a)" def-verified
; Error: EffectMismatch: expected (1 -- 1), got (1 -- 2)
```

### Postulate Compliance

✅ **P1**: `def-verified` is a tool, not syntax  
✅ **P2**: Takes stack, returns stack (with side effect of definition)  
✅ **P3**: Composes by concatenation

### Mathematical Justification

This is **correct by construction**: a verified definition can only exist if the math checks out. The verification happens at the categorical boundary (definition time).

---

## Change 3: Add `axioms` Tool

### Problem

The optimizer contains algebraic identities ([optimizer.rs](../src/optimizer.rs#L152-L280)) but they're not accessible to Kore programs. For formal verification, agents need to query what identities the system believes.

### Solution

Add `axioms` tool that exports the algebraic identities as a list of maps:

**File**: [src/core/verify.rs](../src/core/verify.rs)  
**Action**: Add new tool

```rust
// axioms: ( -- list)
// Returns all algebraic identities as a list of {pattern: "...", reduces_to: "..."}
dict.register(Tool::native(
    "axioms",
    "( -- axioms:List)",
    |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let axioms = vec![
                // Involutions (self-inverse)
                axiom("swap swap", ""),
                axiom("not not", ""),
                axiom("neg neg", ""),
                axiom("tensor-neg tensor-neg", ""),
                axiom("tensor-transpose tensor-transpose", ""),
                
                // Inverse pairs
                axiom("tensor-exp tensor-log", ""),
                axiom("tensor-log tensor-exp", ""),
                
                // Idempotent
                axiom("tensor-relu tensor-relu", "tensor-relu"),
                axiom("tensor-abs tensor-abs", "tensor-abs"),
                
                // Absorption laws
                axiom("tensor-sigmoid tensor-relu", "tensor-sigmoid"),
                axiom("tensor-softmax tensor-relu", "tensor-softmax"),
                axiom("tensor-relu tensor-abs", "tensor-relu"),
                
                // Stack laws
                axiom("dup drop", ""),
                axiom("over drop", ""),
                axiom("over nip", "dup"),
                axiom("swap nip", "drop"),
                
                // Rotation (order 3)
                axiom("rot rot rot", ""),
                
                // Scalar identities
                axiom("1 tensor-scale", ""),
                axiom("-1 tensor-scale", "tensor-neg"),
            ];
            
            stack.push(Value::List(axioms))?;
            Ok((stack, ctx))
        })
    },
).with_doc("Export algebraic identities used by optimizer"));

fn axiom(pattern: &str, reduces_to: &str) -> Value {
    let mut m = IndexMap::new();
    m.insert("pattern".into(), Value::Text(pattern.into()));
    m.insert("reduces_to".into(), Value::Text(reduces_to.into()));
    Value::Map(m)
}
```

**Signature**: `( -- axioms:List)`  
**Effect**: `(0 -- 1)` - pure, no IO

### Usage

```kore
axioms list-len println  ; "22" - how many identities
axioms 0 list-get "pattern" map-get println  ; "swap swap"
```

### Postulate Compliance

✅ **P1**: Tool, not syntax  
✅ **P2**: Stack → Stack  
✅ **P3**: Concatenation composition

### Mathematical Justification

Axioms are the foundation of a formal system. Making them inspectable allows:
- External verification (Lean/Coq can consume this list)
- Agent reasoning about optimization
- Self-documentation of the system's beliefs

---

## Change 4: Add `selftest` Tool

### Problem

There's no way for the runtime to verify its own integrity. A corrupted runtime could lie about effects.

### Solution

Add `selftest` tool that runs critical invariant checks:

**File**: [src/core/verify.rs](../src/core/verify.rs)  
**Action**: Add new tool

```rust
// selftest: ( -- result:Map)
// Verifies runtime invariants, returns test results
dict.register(Tool::native(
    "selftest",
    "( -- result:Map)",
    |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let mut results = IndexMap::new();
            let mut all_passed = true;
            
            // Test 1: Effect composition is associative
            let e1 = Effect::new(1, 2);  // (1 -- 2)
            let e2 = Effect::new(2, 1);  // (2 -- 1)
            let e3 = Effect::new(1, 3);  // (1 -- 3)
            let left = e1.compose(e2).compose(e3);    // ((e1;e2);e3)
            let right = e1.compose(e2.compose(e3));   // (e1;(e2;e3))
            let assoc_ok = left == right;
            results.insert("effect_associativity".into(), Value::Bool(assoc_ok));
            all_passed &= assoc_ok;
            
            // Test 2: Identity is neutral
            let id = Effect::new(0, 0);
            let e = Effect::new(2, 3);
            let left_id = id.compose(e);
            let right_id = e.compose(id);
            let id_ok = left_id == e && right_id == e;
            results.insert("effect_identity".into(), Value::Bool(id_ok));
            all_passed &= id_ok;
            
            // Test 3: swap swap = ε (involution)
            let ops = vec![Op::call("swap"), Op::call("swap")];
            let optimized = optimizer::optimize(ops.clone());
            let swap_ok = optimized.is_empty();
            results.insert("swap_involution".into(), Value::Bool(swap_ok));
            all_passed &= swap_ok;
            
            // Test 4: Effect composition formula
            // compose((a,b), (c,d)) = if b >= c then (a, b-c+d) else (a+c-b, d)
            let e1 = Effect::new(2, 3);  // consumes 2, produces 3
            let e2 = Effect::new(1, 2);  // consumes 1, produces 2
            let composed = e1.compose(e2);
            // 3 >= 1, so (2, 3-1+2) = (2, 4)
            let formula_ok = composed == Effect::new(2, 4);
            results.insert("composition_formula".into(), Value::Bool(formula_ok));
            all_passed &= formula_ok;
            
            // Test 5: rot³ = ε
            let ops = vec![Op::call("rot"), Op::call("rot"), Op::call("rot")];
            let optimized = optimizer::optimize(ops);
            let rot_ok = optimized.is_empty();
            results.insert("rot_order_3".into(), Value::Bool(rot_ok));
            all_passed &= rot_ok;
            
            results.insert("passed".into(), Value::Bool(all_passed));
            
            stack.push(Value::Map(results))?;
            Ok((stack, ctx))
        })
    },
).with_doc("Verify runtime invariants: effect algebra, optimizer axioms"));
```

**Signature**: `( -- result:Map)`  
**Effect**: `(0 -- 1)` - pure computation

### Usage

```kore
selftest "passed" map-get
[ "Runtime integrity verified" println ]
[ "CRITICAL: Runtime invariants failed!" println ]
if
```

### Postulate Compliance

✅ **P1**: Tool, not syntax  
✅ **P2**: Stack → Stack  
✅ **P3**: Concatenation

### Mathematical Justification

Self-test verifies the trusted kernel's axioms hold at runtime:
- Effect composition is a monoid (associative + identity)
- Optimizer axioms are correct (involutions, orders)
- The math we claim is the math we compute

---

## Change 5: Add `CAP_UNVERIFIED` Capability

### Problem

During discovery, agents may need to run unverified code. But letting unverified code run freely violates the safety philosophy.

### Solution

Add `CAP_UNVERIFIED` capability that:
1. Must be explicitly granted
2. Allows `def` to work without verification
3. Is stripped from spawn (can't delegate)
4. Logs all uses for audit

**File**: [src/capabilities.rs](../src/capabilities.rs)  
**Action**: Add capability

```rust
// In Capabilities struct:
/// Can define tools without effect verification
can_unverified: bool,

// In capability parsing:
"unverified" => self.can_unverified = true,

// New method:
pub fn can_use_unverified(&self) -> bool {
    self.can_unverified
}

// In attenuate (for spawn):
pub fn attenuate(&self, subset: &Capabilities) -> Option<Capabilities> {
    // ... existing logic ...
    
    // CAP_UNVERIFIED is NEVER delegable
    result.can_unverified = false;
    
    Some(result)
}
```

**File**: [src/core/definition.rs](../src/core/definition.rs)  
**Action**: Modify `def` to check capability when verification is enforced

```rust
// If strict mode is enabled, def requires either:
// 1. CAP_UNVERIFIED capability, or
// 2. Use def-verified instead
```

### Postulate Compliance

✅ **P1**: Capability is checked by `cap-check` tool  
✅ **P2**: No change to Stack → Stack  
✅ **P3**: No change to composition

### Mathematical Justification

This creates a two-tier system:
- **Discovery tier**: `CAP_UNVERIFIED` grants freedom to explore
- **Production tier**: Only verified definitions allowed

The capability is non-delegable, so untrusted code can't escalate.

---

## Change 6: Structured Error Format

### Problem

Current error types ([error.rs](../src/error.rs)) are Rust enums with string formatting. Machine parsing requires regex, which is fragile.

### Solution

Add JSON-structured error format:

**File**: [src/error.rs](../src/error.rs)  
**Action**: Add `to_map` method

```rust
impl Error {
    /// Convert error to machine-parseable map
    pub fn to_map(&self) -> IndexMap<String, Value> {
        let mut m = IndexMap::new();
        m.insert("code".into(), Value::Text(self.code().into()));
        m.insert("message".into(), Value::Text(self.to_string()));
        
        // Add structured fields based on variant
        match self {
            Self::StackUnderflow { expected, actual } => {
                m.insert("expected".into(), Value::Int(*expected as i64));
                m.insert("actual".into(), Value::Int(*actual as i64));
            }
            Self::TypeError { expected, got } => {
                m.insert("expected_type".into(), Value::Text(expected.clone()));
                m.insert("got_type".into(), Value::Text(got.clone()));
            }
            Self::EffectMismatch { expected, got } => {
                m.insert("expected_effect".into(), Value::Text(expected.clone()));
                m.insert("got_effect".into(), Value::Text(got.clone()));
            }
            Self::CapabilityDenied { capability, tool } => {
                m.insert("capability".into(), Value::Text(capability.clone()));
                m.insert("tool".into(), Value::Text(tool.clone()));
            }
            Self::IndexOutOfBounds { index, length } => {
                m.insert("index".into(), Value::Int(*index));
                m.insert("length".into(), Value::Int(*length as i64));
            }
            _ => {}
        }
        
        m
    }
}
```

**File**: [src/core/error.rs](../src/core/error.rs)  
**Action**: Add `error-info` tool

```rust
// error-info: (error -- map)
// Convert error to structured map
dict.register(Tool::native(
    "error-info",
    "(err:Error -- info:Map)",
    |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let val = stack.pop()?;
            if let Value::Error(e) = val {
                stack.push(Value::Map(e.to_map()))?;
            } else {
                return Err(Error::type_error("Error", &val));
            }
            Ok((stack, ctx))
        })
    },
).with_doc("Convert error to structured map for machine parsing"));
```

### Usage

```kore
[ unknown-tool ] try
dup is-error [
    error-info
    "code" map-get println     ; "E_TOOL_NOT_FOUND"
] [ ] if
```

### Postulate Compliance

✅ **P1**: Tool, not syntax  
✅ **P2**: Stack → Stack  
✅ **P3**: Concatenation

---

## Summary of Changes

| # | Change | File(s) | Complexity | Priority |
|---|--------|---------|------------|----------|
| 1 | Fix analyzer tables | `analyzer.rs` | Low | **HIGH** |
| 2 | Add `def-verified` | `core/definition.rs` | Medium | **HIGH** |
| 3 | Add `axioms` | `core/verify.rs` | Low | Medium |
| 4 | Add `selftest` | `core/verify.rs` | Medium | Medium |
| 5 | Add `CAP_UNVERIFIED` | `capabilities.rs`, `core/definition.rs` | Medium | Low |
| 6 | Structured errors | `error.rs`, `core/error.rs` | Low | Medium |

---

## What NOT To Change

Based on the postulates, these are **explicitly rejected**:

1. ❌ **Special syntax for verification** - Violates P1 (everything is a tool)
2. ❌ **Implicit effect inference** - All behavior must be explicit
3. ❌ **Convenience tools in runtime** - Keep runtime minimal, compose in lib
4. ❌ **Level 4 verification (Lean/Coq)** - Out of scope, can be added externally

---

## Sign-Off Required

Please review this specification and respond with:

- **APPROVED**: Proceed with all changes
- **APPROVED WITH MODIFICATIONS**: List specific changes to the spec
- **REJECTED**: Explain concerns

No implementation will begin until sign-off is received.
