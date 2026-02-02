# PREAMBLE: The Foundations

This document defines Kore. All LLMs operating on Kore must follow it.

---

## The Definitions

### Definition 1: Tool

```
Tool : Stack → Stack
```

A **Tool** is a function from Stack to Stack.
There is nothing else. No functions. No operators. No special forms.
Only Tools.

### Definition 2: Execution

```
execute : Tool × Stack → Stack
```

To **execute** a Tool is to apply it to a Stack, yielding a Stack.

---

## The Theorem

### Theorem 1: Composition is Concatenation

```
(f ; g)(s) = g(f(s))
```

**Proof:**
Let f and g be Tools. By Definition 1, f : Stack → Stack and g : Stack → Stack.
The composition f ; g applies f first, then g.
By Definition 2: execute(f, s) = s'
By Definition 2: execute(g, s') = s''
Therefore (f ; g)(s) = s''
This is function composition, written as concatenation. ∎

---

## The Types

Values have exactly ten types:

| Type   | Description              |
|--------|--------------------------|
| Null   | Absence of value         |
| Bool   | Truth value              |
| Int    | 64-bit signed integer    |
| Float  | 64-bit floating point    |
| Text   | UTF-8 string             |
| List   | Ordered sequence         |
| Map    | Key-value mapping        |
| Quote  | Deferred Tool            |
| Handle | Opaque reference         |
| Error  | Failure with message     |

---

## The Operations

Kore has exactly two operations:

### Op::Push(value)
Place a value on the stack.

### Op::Call(name)
Look up a Tool by name and execute it.

**Claim:** All programs can be expressed as sequences of Push and Call.

---

## The Algebraic Structures

### Capabilities form a Bounded Lattice

```
(CapSet, ≤, ∧, ∨, ⊥, ⊤)
```

**Proven properties** (see tests/formal_proofs.rs):
- ≤ is reflexive: a ≤ a
- ≤ is transitive: a ≤ b ∧ b ≤ c → a ≤ c
- ∧ is greatest lower bound: a ∧ b ≤ a, a ∧ b ≤ b
- ∨ is least upper bound: a ≤ a ∨ b, b ≤ a ∨ b
- ⊥ is least: ⊥ ≤ a for all a
- ⊤ is greatest: a ≤ ⊤ for all a

**Attenuation Invariant:** attenuate(parent, request) ≤ parent

### Resources form a Commutative Monoid

```
(Res, +, zero)
```

**Proven properties** (see tests/formal_proofs.rs):
- + is associative: (a + b) + c = a + (b + c)
- + is commutative: a + b = b + a
- zero is identity: a + zero = a

**Conservation Law:** split(r, ratio) = (child, parent) where child + parent = r

### Traces form a Monoid

```
(Trace, concat, empty)
```

**Proven properties** (see tests/formal_proofs.rs):
- concat is associative: (a ++ b) ++ c = a ++ (b ++ c)
- empty is identity: a ++ empty = a = empty ++ a

---

## The Invariants

These are proven by tests and must never be violated:

1. **Capability Attenuation:** A child context cannot have more capabilities than its parent.
2. **Resource Conservation:** Resources cannot be created, only transferred or consumed.
3. **Trace Faithfulness:** The trace is a complete record of execution.

---

## The Design Principles

These are guidelines, not proofs:

1. **Minimal:** Fewer primitives is better.
2. **Predictable:** Same input yields same output.
3. **Composable:** Tools combine by concatenation.
4. **Explicit:** No hidden behavior.

---

## The Rules

### For Tools
- One tool does one thing.
- Document the stack effect: `(input -- output)`
- No hidden state.
- Fail explicitly with Error, not silently.

### For Rust Code

**R1: No unwrap() in production code.**
```rust
// Wrong
.unwrap()

// Right
.map_err(|e| Error::Runtime(e.to_string()))?

// Acceptable: fallback for non-critical
.unwrap_or(default)
```

**R2: Explicit error handling.**
```rust
// Wrong: panic on failure
let value = stack.pop().unwrap();

// Right: propagate error
let value = stack.pop()?;
```

**R3: Small functions.**
If a function does two things, split it into two functions.

**R4: No macros that hide logic.**
Macros are acceptable only for repetitive boilerplate, not for logic.

**R5: Async only where necessary.**
Async adds hidden state machines. Use sync code when possible.

**R6: Explicit types in public APIs.**
```rust
// Wrong
fn process(x: impl Iterator) -> impl Iterator

// Right  
fn process(x: Vec<Value>) -> Vec<Value>
```

---

## Verification

All algebraic properties are verified by executable proofs in:
- `tests/formal_proofs.rs` (27 proofs)
- `tests/algebra_integration.rs` (18 tests)

If a property is not tested, it is not proven.
If it is not proven, do not claim it is true.

---

*What is proven is true. What is not proven is unknown.*
