# Kore

**The fundamental runtime for agentic AI.**

Kore is a minimal, stack-based programming language built on three postulates. It provides exactly what's needed for agents to compose, execute, and reason about tools—nothing more.

## The Three Postulates

**Postulate 1: Everything is a Tool**
```
Tool : Stack → Stack
```
Every operation is a tool that transforms a stack.

**Postulate 2: Tools Transform Stacks**
```
execute(t, s) = s'
```
Tools consume values from the stack and produce values onto the stack.

**Postulate 3: Composition is Concatenation**
```
(f ; g)(s) = g(f(s))
```
Running tools in sequence is function composition.

## Architecture

### Three Tiers

```
┌─────────────────────────────────────────────┐
│           Capability Tools (cap/)            │
│  File I/O, Network, Shell - require perms    │
├─────────────────────────────────────────────┤
│              Stdlib (.kore)                  │
│  Composed tools: gt, over, map, filter, etc  │
├─────────────────────────────────────────────┤
│            Core (28 primitives)              │
│  Irreducible operations in Rust              │
└─────────────────────────────────────────────┘
```

### 28 Core Primitives

| Category | Primitives |
|----------|------------|
| **Execution** (4) | `call`, `spawn`, `if`, `loop` |
| **Definition** (2) | `def`, `words` |
| **Error** (3) | `try`, `fail`, `is-error` |
| **Stack** (5) | `dup`, `drop`, `swap`, `rot`, `depth` |
| **Arithmetic** (6) | `add`, `sub`, `mul`, `div`, `mod`, `neg` |
| **Comparison** (2) | `eq`, `lt` |
| **Logic** (3) | `and`, `or`, `not` |
| **Data** (3) | `list`, `unlist`, `map-new` |

Everything else is composed from these.

### 10 Value Types

| Type | Example | Purpose |
|------|---------|---------|
| Null | `null` | Absence of value |
| Bool | `true`, `false` | Logic |
| Int | `42`, `-7` | Whole numbers |
| Float | `3.14` | Decimals |
| Text | `"hello"` | Strings |
| List | `[1, 2, 3]` | Ordered collections |
| Map | `{"a": 1}` | Key-value pairs |
| Quote | `[ add 1 ]` | Deferred code |
| Handle | `@file:123` | External resources |
| Error | `Error(...)` | Captured failures |

### 2 Operations

| Op | Effect | Description |
|----|--------|-------------|
| Push | `( -- value)` | Put a value on the stack |
| Call | `(... -- ...)` | Look up and run a tool |

That's it. Conditionals (`if`) and quotations are tools, not special syntax.

## Example

```kore
# Define factorial
[ dup 1 le 
  [ drop 1 ] 
  [ dup 1 sub factorial mul ] 
  if 
] "factorial" def

# Compute 5!
5 factorial
# Stack: [120]
```

## Algebraic Foundations

Kore is built on formal algebraic structures:

- **CapSet**: Capability lattice with ≤, ∧, ∨ (only attenuation, no escalation)
- **Res**: Resource monoid (conservation law: resources can split but never increase)
- **Trace**: Execution trace monoid (append-only audit log)

The `spawn` primitive enforces:
```
spawn(q, caps', res') where caps' ≤ caps and res' ≤ res
```

## Design Principles

1. **Minimal**: 2 ops, 10 types, 28 primitives
2. **Formal**: Built on lattice, monoid, category theory
3. **Secure**: Capability-based access, resource conservation
4. **Composable**: Tools are the only abstraction
5. **Machine-readable**: Every tool has queryable manifest

## Quick Start

```bash
# Run tests
cargo test

# Run core tests only
cargo test core::

# Check all 28 primitives
cargo test proof_postulate_1_everything_is_tool
```

## File Structure

```
src/
├── core/           # 28 irreducible primitives
│   ├── mod.rs      # register_core()
│   ├── execution.rs
│   ├── arithmetic.rs
│   └── ...
├── algebra.rs      # CapSet, Res, Trace
├── executor.rs     # Main execution loop
└── ...

stdlib/
├── core-extensions.kore  # gt, le, ge, over, nip, etc.
├── prelude.kore         # Higher-level helpers
└── list.kore            # List operations
```

## License

MIT
