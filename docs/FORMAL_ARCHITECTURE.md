# Kore Formal Architecture

> *"Simplicity is the ultimate sophistication"* — Leonardo da Vinci

Kore is built on three postulates and three algebraic structures that **guarantee** safety properties by construction.

## The Three Postulates

### Postulate 1: Everything is a Tool

```
Tool : Stack → Stack
```

Every operation—from basic arithmetic to file I/O to spawning processes—is a **tool**. There are no special forms, no hidden state, no magic. A tool takes a stack and returns a transformed stack.

**Consequence**: The system has exactly ONE abstraction. You learn tools, you know everything.

### Postulate 2: Tools Transform Stacks

```
execute(t, s) = s'
```

Tools consume values **from the stack** and produce values **onto the stack**. There is no other input mechanism. No globals, no implicit parameters, no ambient authority.

**Consequence**: Every tool is a pure function over stacks (modulo capability effects).

### Postulate 3: Composition is Concatenation

```
(f ; g)(s) = g(f(s))
```

Running tools in sequence is function composition. The composition of `f` and `g` is written by **concatenation**: `f g`. This is the only way to build programs.

**Consequence**: Programs are just lists of tool names. No syntax, no parsing complexity.

---

## The Op Enum: Only 2 Operations

```rust
pub enum Op {
    Push(Value),  // Put a value on the stack
    Call(String), // Execute a tool by name
}
```

That's it. Everything else—conditionals, loops, definitions—is implemented as **tools**.

| Operation | What it does |
|-----------|--------------|
| `Push(v)` | Push value `v` onto the stack |
| `Call(n)` | Look up tool `n` and execute it |

---

## The Three Algebraic Structures

### 1. Capability Lattice

Capabilities form a **bounded lattice** `(L, ≤, ∧, ∨, ⊥, ⊤)`:

```
        ⊤ (can do everything)
       /|\
      / | \
   fs  net spawn
     \ | /
      \|/
        ⊥ (can do nothing)
```

**Operations**:

| Operation | Symbol | Meaning |
|-----------|--------|---------|
| `leq(a, b)` | a ≤ b | "a is weaker than b" |
| `meet(a, b)` | a ∧ b | Greatest lower bound (intersection) |
| `join(a, b)` | a ∨ b | Least upper bound (union) |
| `attenuate(c, mask)` | c ↓ mask | Weaken to subset |

**Key Property**: Attenuation can only move **DOWN** the lattice.

```
attenuate(c) ≤ c  (always)
```

This means a child process can never have more capabilities than its parent.

### 2. Resource Monoid

Resources form a **commutative monoid** `(R, ⊕, 0)` with splitting:

```rust
struct Res {
    mem: u64,      // Memory units
    rom: u64,      // Storage units
    compute: u64,  // CPU budget
    net: u64,      // Network budget
}
```

**Operations**:

| Operation | Meaning |
|-----------|---------|
| `add(r1, r2)` | Combine resources |
| `split(r, ratio)` | Divide resources |
| `consume(r, amount)` | Use resources |

**Conservation Law**:

```
split(r, p) = (r₁, r₂)  where  r₁ ⊕ r₂ = r
```

Resources are **never created**, only split or consumed. This prevents resource amplification attacks.

### 3. Trace Algebra

Execution traces form a **monoid** `(T, ·, ε)`:

```rust
struct Trace {
    steps: Vec<TraceStep>
}

struct TraceStep {
    tool: String,
    input_hash: u64,
    output_hash: u64,
    caps_used: Vec<Cap>,
    res_consumed: Res,
}
```

**Key Property**:

```
trace(f ; g) = trace(f) · trace(g)
```

This is **Postulate 3** manifesting in traces: composition traces concatenate.

---

## Tool Categories (186 total)

Tools are organized into three layers:

- **Core (87 tools)**: Primitives, stack, arithmetic, logic, string, control, data structures
- **Cap (54 tools)**: Capability-gated operations (fs, net, spawn, io, env, mem, rom)
- **Ext (45 tools)**: Extensions (tensor, autodiff, linear types)

Everything is a tool, and tools compose by concatenation.

### Stack Operations (9)

| Tool | Signature | Description |
|------|-----------|-------------|
| `dup` | (a -- a a) | Duplicate top |
| `drop` | (a -- ) | Remove top |
| `swap` | (a b -- b a) | Swap top two |
| `rot` | (a b c -- b c a) | Rotate third to top |
| `over` | (a b -- a b a) | Copy second to top |
| `nip` | (a b -- b) | Remove second |
| `tuck` | (a b -- b a b) | Copy top below second |
| `pick` | (n -- x) | Copy nth item |
| `depth` | ( -- n) | Stack depth |

### Arithmetic (9)

| Tool | Signature | Description |
|------|-----------|-------------|
| `add` | (a b -- a+b) | Addition |
| `sub` | (a b -- a-b) | Subtraction |
| `mul` | (a b -- a*b) | Multiplication |
| `div` | (a b -- a/b) | Division |
| `mod` | (a b -- a%b) | Modulo |
| `neg` | (a -- -a) | Negation |
| `abs` | (a -- \|a\|) | Absolute value |
| `min` | (a b -- min) | Minimum |
| `max` | (a b -- max) | Maximum |

### Comparison (6)

| Tool | Signature | Description |
|------|-----------|-------------|
| `eq` | (a b -- bool) | Equal |
| `lt` | (a b -- bool) | Less than |
| `gt` | (a b -- bool) | Greater than |
| `le` | (a b -- bool) | Less or equal |
| `ge` | (a b -- bool) | Greater or equal |
| `ne` | (a b -- bool) | Not equal |

### Logic (3)

| Tool | Signature | Description |
|------|-----------|-------------|
| `and` | (a b -- a∧b) | Logical and |
| `or` | (a b -- a∨b) | Logical or |
| `not` | (a -- ¬a) | Logical not |

### Control (3)

| Tool | Signature | Description |
|------|-----------|-------------|
| `if` | (c t f -- r) | Conditional |
| `call` | (q -- ...) | Execute quotation |
| `loop` | (q -- ...) | Loop while true |

### Definition (3)

| Tool | Signature | Description |
|------|-----------|-------------|
| `def` | (q name -- ) | Define a tool |
| `words` | ( -- list) | List tools |
| `meta` | (name -- info) | Get metadata |

### Data (5)

| Tool | Signature | Description |
|------|-----------|-------------|
| `list` | (... n -- list) | Collect to list |
| `unlist` | (list -- ...) | Spread to stack |
| `map-new` | ( -- map) | Empty map |
| `map-get` | (m k -- v) | Get from map |
| `map-set` | (m k v -- m) | Set in map |

### Capability (4)

| Tool | Signature | Description |
|------|-----------|-------------|
| `cap-has` | (cap -- bool) | Check capability |
| `cap-list` | ( -- caps) | List capabilities |
| `cap-leq` | (a b -- bool) | Compare (a ≤ b) |
| `cap-attn` | (caps -- caps') | Attenuate |

### Resource (4)

| Tool | Signature | Description |
|------|-----------|-------------|
| `res-avail` | ( -- res) | Available resources |
| `res-split` | (ratio -- r1 r2) | Split resources |
| `res-cons` | (res -- ) | Consume resources |
| `res-has` | (res -- bool) | Check if sufficient |

### Spawn (1)

| Tool | Signature | Description |
|------|-----------|-------------|
| `spawn` | (q caps res -- ctx) | Spawn isolated context |

**Spawn** is the crown jewel. It creates a child context with:
- **Attenuated capabilities**: Child can only have caps ≤ parent caps
- **Split resources**: Child + parent = original (conservation)
- **Isolated execution**: Child cannot affect parent's state

### Error (2)

| Tool | Signature | Description |
|------|-----------|-------------|
| `try` | (q h -- r) | Try with handler |
| `fail` | (msg -- ) | Raise error |

### Trace (2)

| Tool | Signature | Description |
|------|-----------|-------------|
| `trace-on` | ( -- ) | Enable tracing |
| `trace` | ( -- trace) | Get current trace |

---

## Safety Guarantees

The formal foundations give us **theorems**, not just hopes:

### Theorem 1: Capability Monotonicity

> A spawned process can never have more capabilities than its parent.

**Proof**: `spawn` uses `attenuate`, which by construction satisfies `attenuate(c) ≤ c`.

### Theorem 2: Resource Conservation

> The sum of resources in all contexts equals the initial resource pool.

**Proof**: `split(r, p) = (r₁, r₂)` where `r₁ + r₂ = r`. No operation creates resources.

### Theorem 3: Trace Faithfulness

> The trace of a composed program equals the concatenation of component traces.

**Proof**: `trace(f ; g) = trace(f) · trace(g)` by Postulate 3.

---

## Directory Structure

```
kore/
├── src/
│   ├── lib.rs          # Module exports
│   ├── op.rs           # Push, Call operations
│   ├── executor.rs     # Execution engine
│   ├── value.rs        # Value types
│   ├── stack.rs        # LIFO stack
│   ├── context.rs      # Execution context
│   ├── tool.rs         # Tool abstraction
│   ├── core/           # Core primitives (87 tools)
│   │   ├── stack.rs    # Stack manipulation
│   │   ├── arith.rs    # Arithmetic
│   │   ├── logic.rs    # Boolean logic
│   │   ├── string.rs   # String operations
│   │   └── ...
│   ├── cap/            # Capability tools (54 tools)
│   │   ├── fs.rs       # File system
│   │   ├── io.rs       # Standard I/O
│   │   ├── process.rs  # Process management
│   │   └── ...
│   └── ext/            # Extensions (45 tools)
│       ├── tensor.rs   # Tensors (25+ ops)
│       ├── autodiff.rs # Automatic differentiation
│       └── linear.rs   # Linear types
└── docs/               # Documentation
```

---

## Philosophy

> "Make the easy things easy, and the hard things possible." — Larry Wall

Kore makes the **simple things obvious** (stack operations, arithmetic) and the **dangerous things explicit** (capabilities, resources, spawning). You can't accidentally get more power than you were given.

This is **security by construction**, not security by audit.
