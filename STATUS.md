# Kore Status

## Current State

**Tests**: 469 passing (255 lib + 214 integration)  
**Tools**: 186 total (87 core + 54 cap + 45 ext)  
**Clippy**: Clean  

---

## Architecture

### Core (87 tools)
Stack manipulation, arithmetic, logic, control flow, data structures, strings, types.

### Capabilities (54 tools)
Gated operations: fs, net, spawn, io, env, mem, rom, process, time, http, json.

### Extensions (45 tools)
- **Tensor (25+)**: Multi-dimensional arrays with autodiff support
- **Autodiff (5)**: Automatic differentiation (requires-grad, backward, grad-get, zero-grad, detach)
- **Linear (7+)**: Linear types (use-once, affine)

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

---

## Test Summary

```
lib.rs              255 tests
algebra_integration  18 tests
autodiff_tests       13 tests  
control_tests        34 tests
data_structure_tests 27 tests
stack_tests          23 tests
string_number_tests  44 tests
tensor_tests         68 tests
----------------------------------------
TOTAL               469 tests
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
