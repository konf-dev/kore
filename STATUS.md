# Kore Status

## Current State

**Tests**: 475 passing  
**Tools**: 186 total  
**Clippy**: Clean  
**Optimizer**: Integrated (algebraic rewrites)

---

## Architecture

### Core Tools
Stack manipulation, arithmetic, logic, control flow, data structures, strings, types.

### Capability Tools
Gated operations: fs, net, spawn, io, env, mem, rom, process, time, http, json.

### Extensions
- **Tensor (33)**: Multi-dimensional arrays with full autodiff support
- **Autodiff (5)**: Automatic differentiation (requires-grad, backward, grad-get, zero-grad, detach)
- **Linear (7)**: Linear types (linear-new, linear-unwrap, affine-new, affine-unwrap, is-linear, is-affine, linearity)

---

## Features

| Feature | Status |
|---------|--------|
| Stack-based execution | ✓ |
| 3 postulates | ✓ |
| Static stack analysis | ✓ |
| IO effect inference | ✓ |
| Capability system | ✓ |
| Tensor operations | ✓ |
| Automatic differentiation | ✓ |
| Linear types | ✓ |
| Algebraic optimizer | ✓ |

---

## Test Summary

```
lib.rs                  261 tests
algebra_integration      18 tests
algorithms               34 tests  
formal_proofs            27 tests
integration_tests        23 tests
language_semantics       44 tests
primitives               68 tests
----------------------------------------
TOTAL                   475 tests
```

---

## Performance

See [docs/BENCHMARKS.md](docs/BENCHMARKS.md) for detailed benchmarks.

| Operation | Rate |
|-----------|------|
| Loop overhead | 1.15M ops/s |
| Stack operations | 474K ops/s |
| Tensor add (100-elem) | 101K ops/s |
| Autodiff backward | 36K ops/s |

---

## Documentation

- [POSTULATES.md](docs/POSTULATES.md) - The three axioms
- [PHILOSOPHY.md](docs/PHILOSOPHY.md) - Design rationale  
- [PRIMITIVES.md](docs/PRIMITIVES.md) - Core tool reference
- [REFERENCE.md](docs/REFERENCE.md) - Complete tool reference
- [QUICKSTART.md](docs/QUICKSTART.md) - Getting started
- [AUTODIFF_DESIGN.md](docs/AUTODIFF_DESIGN.md) - Autodiff implementation
- [FORMAL_FOUNDATIONS.md](docs/FORMAL_FOUNDATIONS.md) - Mathematical foundations
