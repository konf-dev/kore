# Kore Status

> **Branch:** `proof-based-refactor`
> **Version:** v0.3.0 (in progress)
> **Philosophy:** Proof-based, not trust-based

## The Three Postulates

1. **Everything is a Tool** - No special constructs
2. **Tools Transform Stacks** - Stack in → Stack out
3. **Composition is Concatenation** - P; Q = run P then Q

## Architecture

### Two Operations

```rust
enum Op {
    Push(Value),    // Put value on stack
    Call(String),   // Execute a tool
}
```

That's it. Everything else is a tool.

### Ten Value Types

```
Null, Bool, Int, Float, Text, List, Map, Quote, Handle, Error
```

### Essential Tools (~40)

| Category | Tools |
|----------|-------|
| **Core** | `call`, `try`, `if`, `def` |
| **Stack** | `dup`, `drop`, `swap`, `over`, `rot`, `depth` |
| **Math** | `add`, `sub`, `mul`, `div`, `mod`, `neg` |
| **Compare** | `eq`, `neq`, `lt`, `gt`, `le`, `ge` |
| **Logic** | `and`, `or`, `not` |
| **List** | `list-empty`, `list-len`, `list-get`, `list-push`, `list-concat`, `collect` |
| **Map** | `map-empty`, `map-get`, `map-set`, `map-has`, `map-keys` |
| **Text** | `str-len`, `str-concat`, `str-split`, `str-join` |
| **Type** | `type-of`, `is-error`, `unwrap` |
| **Introspection** | `words`, `describe`, `meta` |
| **Capability** | `cap-has`, `cap-list`, `cap-leq`, `cap-attenuate` |
| **Resource** | `res-status`, `res-check`, `res-consume`, `res-split` |
| **Spawn** | `spawn`, `await`, `channel`, `send`, `recv` |
| **Trace** | `trace-start`, `trace-stop`, `trace-hash` |
| **I/O** | `print`, `fs-read`, `fs-write`, `http-get`, `exec` |

## Safety Model

### Capabilities (Lattice)

```
spawn attenuates: child_caps ⊆ parent_caps
```

Cannot give what you don't have. Mathematically enforced.

### Resources (Monoid)

```
spawn splits: child_res + parent_remaining = parent_original
```

Conservation law. Resources transfer, never created.

### Traces (Proof Objects)

```
same input + deterministic tools = same trace fingerprint
```

Execution is provable.

## Refactor Progress

- [x] Create proof-based-refactor branch
- [x] Document postulates
- [x] Document formal foundations
- [x] Plan refactor
- [x] Remove dead weight docs
- [ ] Simplify Op to Push/Call only
- [ ] Implement `if` as tool
- [ ] Implement capability lattice
- [ ] Implement resource algebra
- [ ] Implement trace system
- [ ] Implement spawn with safety
- [ ] Reduce builtins from 143 to ~40
- [ ] Validate with tests

## Core Documents

| Document | Purpose |
|----------|---------|
| [POSTULATES.md](POSTULATES.md) | The three inviolable axioms |
| [PHILOSOPHY.md](PHILOSOPHY.md) | The five design principles |
| [FORMAL_FOUNDATIONS.md](FORMAL_FOUNDATIONS.md) | Mathematical treatment |
| [REFACTOR_PLAN.md](REFACTOR_PLAN.md) | Implementation roadmap |
| [PRIMITIVES.md](PRIMITIVES.md) | Tool reference (to be updated) |

## Design Principle

> **Security is not a feature. It's the absence of tools.**

Want sandboxing? Don't include `fs-write` or `exec`.
Want unlimited compute? Set resources to unlimited.
Different configs, same rules.
