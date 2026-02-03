# Kore Refactoring Summary

## What Changed

### Core Primitives: 143 → 28

We reduced from 143 builtins to **28 core primitives**, organized in 8 categories:

| Category | Count | Primitives |
|----------|-------|------------|
| Execution | 4 | `call`, `spawn`, `if`, `loop` |
| Definition | 2 | `def`, `words` |
| Error | 3 | `try`, `fail`, `is-error` |
| Stack | 5 | `dup`, `drop`, `swap`, `rot`, `depth` |
| Arithmetic | 6 | `add`, `sub`, `mul`, `div`, `mod`, `neg` |
| Comparison | 2 | `eq`, `lt` |
| Logic | 3 | `and`, `or`, `not` |
| Data | 3 | `list`, `unlist`, `map-new` |

### Why 28?

Each primitive is **irreducible** - it cannot be composed from simpler operations:

- `eq` and `lt` are sufficient for all comparison (gt, le, ge, ne compose from these)
- `dup`, `drop`, `swap`, `rot` are the minimal stack alphabet (over, nip, tuck compose from these)
- `add`, `sub`, `mul`, `div`, `mod`, `neg` are the arithmetic closure
- `and`, `or`, `not` are the boolean closure
- `call`, `if`, `loop` are the control minimal set
- `spawn` is required for capability attenuation (cannot be composed)
- `list`, `unlist`, `map-new` are the data constructors

### File Structure

```
src/
├── core/           # NEW: 28 primitives
│   ├── mod.rs      # Module root, register_core()
│   ├── execution.rs # call, spawn, if, loop
│   ├── definition.rs # def, words
│   ├── error.rs    # try, fail, is-error
│   ├── stack.rs    # dup, drop, swap, rot, depth
│   ├── arithmetic.rs # add, sub, mul, div, mod, neg
│   ├── comparison.rs # eq, lt
│   ├── logic.rs    # and, or, not
│   └── data.rs     # list, unlist, map-new
├── algebra.rs      # CapSet, Res, Trace (unchanged)
├── builtins.rs     # Legacy (kept for compatibility)
└── ...

stdlib/
├── core-extensions.kore  # NEW: gt, le, ge, ne, over, nip, etc.
├── prelude.kore         # Higher-level compositions
└── list.kore            # List operations
```

### Design Principles Applied

1. **Everything is a Tool** - All 28 primitives are registered via `Tool::native()`
2. **Tools Transform Stacks** - Every primitive has a clear `(inputs -- outputs)` signature
3. **Composition is Concatenation** - Complex tools built by concatenating simpler ones
4. **No Magic** - Each primitive has one clear purpose
5. **Explicit Dependencies** - Core primitives have no dependencies on stdlib

### Backward Compatibility

- `register_builtins()` still exists in `builtins.rs` (143 builtins)
- `register_core()` is the new minimal registration
- Tests pass for both paths

### What's Composed Now (was primitive before)

| Old Primitive | Now Composed As |
|--------------|-----------------|
| `gt` | `swap lt` |
| `le` | `swap lt not` |
| `ge` | `lt not` |
| `ne` | `eq not` |
| `over` | `swap dup rot swap` |
| `nip` | `swap drop` |
| `tuck` | `dup rot swap` |
| `abs` | `dup 0 lt [ neg ] [ ] if` |
| `min` | `dup rot dup rot lt [ drop ] [ swap drop ] if` |
| `max` | `dup rot dup rot lt [ swap drop ] [ drop ] if` |

### Verification

```bash
# All tests pass
cargo test

# Core module tests
cargo test core::

# Check primitive count
cargo test proof_postulate_1_everything_is_tool
```

### Next Steps

1. Remove `builtins.rs` entirely once all consumers migrated
2. Add capability tools in `src/cap/` for fs, net, etc.
3. Create TOML manifests for all tools
4. Document each primitive with formal stack effect

## Mathematical Foundation

The 28 primitives form a **complete basis** for:

- Stack manipulation (Church encoding via list/unlist + stack ops)
- Arithmetic (integers and rationals via div)
- Control flow (Turing complete via if + loop + call)
- Sandboxing (capability lattice via spawn)
- Error handling (try/fail for control transfer)

Any computable function can be expressed as a composition of these 28 tools.
