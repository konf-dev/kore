# Native Backend: Architecture & Status

**Updated**: 2025-07
**Status**: 96 tests, 50 JIT opcodes, 14 bugs found & fixed

---

## Overview

The Kore JIT compiler translates bytecode to native x86-64 machine code via Cranelift.
It has **two compilation paths** that share a single opcode registry:

```
                    ┌─────────────────┐
                    │  Kore Bytecode  │
                    │  134 opcodes    │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │  Op::jit_opcodes│
                    │  (50 opcodes)   │
                    └────────┬────────┘
                      ┌──────┴──────┐
                      ▼             ▼
               ┌────────────┐ ┌───────────┐
               │  compile() │ │ compile_  │
               │  SSA path  │ │ module()  │
               │  no CALL   │ │ CALL/RET  │
               │            │ │ SSA+mem   │
               └─────┬──────┘ └─────┬─────┘
                     │              │
                     └──────┬───────┘
                            ▼
                     ┌────────────┐
                     │  Cranelift │
                     │  → x86-64 │
                     └────────────┘
```

---

## Two Compilation Paths

### Path 1: `compile()` — SSA Variables (Fast Path)

- **Stack representation**: Cranelift SSA variables (`Variable`)
- **When used**: Programs with no function definitions
- **Supports**: All opcodes except CALL/RET
- **Advantage**: Cranelift can optimize through the stack (register allocation, dead code elimination)
- **Stack pointer**: Tracked at compile time (`sp` is a Rust variable, not runtime)

```rust
// Example: `5 3 +` compiles to:
let v0 = builder.ins().iconst(types::I64, 5);  // sp=1
builder.def_var(stack_vars[0], v0);
let v1 = builder.ins().iconst(types::I64, 3);  // sp=2
builder.def_var(stack_vars[1], v1);
let a = builder.use_var(stack_vars[0]);         // sp=1
let b = builder.use_var(stack_vars[1]);
let sum = builder.ins().iadd(a, b);
builder.def_var(stack_vars[0], sum);
```

### Path 2: `compile_module()` — SSA Within Functions + Memory at Boundaries

- **Stack representation**: SSA variables within function bodies, flushed to `StackSlot` memory at CALL/RET/jump-merge points
- **When used**: Programs with `: name ... ;` function definitions
- **Supports**: All 50 JIT opcodes including CALL/RET
- **Advantage**: Supports recursion, function calls, local save/restore. SSA within straight-line code reduces memory ops.
- **Stack pointer**: SSA-tracked (`ssa_sp: Option<usize>`) when compile-time known, falls back to runtime `sp_var` at branch merge points
- **CALL/RET**: Only saves/restores used locals (scanned by `scan_used_locals()`), not all 64 slots

```rust
// Example: stack operations go through memory:
let sp = builder.use_var(sp_var);
let addr = /* compute stack[sp-1] address */;
let val = builder.ins().load(types::I64, memflags, addr, 0);
```

### Why Two Paths?

| | `compile()` SSA | `compile_module()` SSA+Memory |
|---|---|---|
| Stack | SSA Variables | SSA within functions, memory at boundaries |
| SP tracking | Compile-time | SSA when known, runtime at merge points |
| Optimization | Full Cranelift opts | SSA opts within functions |
| CALL/RET | ❌ Not supported | ✅ Full recursion (used-locals-only save/restore) |
| Jump merge | SSA always | Flushes to memory, enters memory mode |

The system auto-selects: if the module has functions → `compile_module()`, else → `compile()`.

`compile_module()` uses the `flush_ssa_to_memory!` macro before CALL, RET, and jump targets
where multiple control flow paths merge. Within straight-line function code, all opcodes
use the SSA path (1-3 Cranelift instructions per opcode instead of 8-15 memory ops).

---

## Supported Opcodes (50)

All opcodes use symbolic names from `Op::jit_opcodes()` — no raw hex literals.

| Category | Opcodes | Count |
|----------|---------|-------|
| Stack | NOP, DROP, DUP, SWAP, ROT, OVER | 6 |
| Literals | INT8/16/32/64, F32, F64, NIL, TRUE, FALSE | 9 |
| Int Arithmetic | ADD, SUB, MUL, DIV, MOD, NEG | 6 |
| Float Arithmetic | FADD, FSUB, FMUL, FDIV, FNEG, FSQRT, FABS, I2F, F2I | 9 |
| Comparison | EQ, LT, GT, LE, GE, NE | 6 |
| Logic/Bitwise | AND, OR, NOT, XOR, BAND, BOR, BXOR, BNOT, SHL, SHR | 10 |
| Control Flow | JMP, JZ, JNZ | 3 |
| Functions | CALL, RET (module path only) | 2 |
| Locals | STORE, LOAD (64 slots per frame) | 2 |
| Reflection | FETCH, SIZE | 2 |
| Halt | HALT | 1 |
| | **Total** | **50** (of 134) |

### Not JIT-compiled (handled by interpreter)

Lists, maps, strings, fibers, channels, spawn, syscall, quotes, error handling,
linear/affine types, introspection — these require dynamic dispatch and are
handled by the interpreter. The JIT focuses on **tight computation**.

---

## Opcode Registry (Option F Architecture)

The JIT backend uses a **compile-time enforced opcode registry** to prevent bugs:

```rust
// bytecode.rs — single source of truth
impl Op {
    pub fn all_opcodes() -> &'static [Op] { /* 100+ opcodes */ }
    pub fn jit_opcodes() -> &'static [Op] { /* 50 JIT opcodes */ }
}

// native_backend.rs — symbolic references only
x if x == Op::Add as u8 => { /* ... */ }
// Never: 0x40 => { ... }
```

**What this prevents**: The 13-opcode class of bug where `compile()` and `compile_module()`
had different opcode values for the same operations. This was caused by copy-paste errors
with raw hex literals. With symbolic names + registry tests, both paths are guaranteed
to handle the same opcodes identically.

### Test Enforcement

```
test_jit_opcodes_registry_consistency    — roundtrip Op ↔ u8
test_all_opcodes_includes_jit_opcodes    — jit ⊆ all
test_compile_handles_all_jit_opcodes     — every opcode accepted by SSA path
test_compile_module_handles_all_jit_opcodes — every opcode accepted by module path
test_every_jit_opcode_cross_backend      — SSA = module = interpreter for 45+ programs
test_security_unknown_opcodes_always_error — unknown bytes always fail
```

---

## Performance

### Benchmarks (Intel Xeon E5-2690 v4, 62GB DDR4, Rust 1.93.0 release, 2025-07)

| Benchmark | Rust (ns) | JIT (ns) | Interp (ns) | JIT/Rust |
|-----------|-----------|----------|-------------|----------|
| Fib iter (78) | 0.8 | 620.7 | 24,141 | 769.5× |
| Sum i² (10K) | 9,509 | 55,331 | 2,658,965 | 5.8× |
| Collatz (10K) | 1,134,641 | 19,464,061 | 244,550,886 | 17.2× |
| Fib rec (25) | 2,705 | 1,358,960 | 49,479,641 | 502.4× |
| **GEO MEAN** | | | | **78.8×** |

JIT/Interpreter geo mean: ~30×.
Interpreter/Rust geo mean: 2,397×.

The recursive fibonacci benchmark (fib_rec) is the key test of CALL/RET efficiency.
After Fix 1 (used-locals-only save/restore), fib_rec(25) dropped from 15,912µs to 1,359µs.

---

## Bug History

14 bugs found and fixed through systematic testing:

| # | Bug | Where | How Found |
|---|-----|-------|-----------|
| 1 | Silent opcode ignore | compile() | Code review |
| 2-3 | Missing STORE/LOAD | compile() | Feature gap |
| 4-5 | Missing CALL/RET | compile_module() | New feature |
| 6-13 | Wrong opcode values | compile_module() | Code review (raw hex copy-paste errors) |
| 14 | NOT was logical not bitwise | compile_module() | Cross-backend test |

Bug #14 is notable: the cross-backend test suite caught it automatically.
`NOT` in `compile_module()` was comparing to zero (logical NOT → 0 or 1)
instead of using Cranelift's `bnot` (bitwise NOT → flip all bits).

---

## Function Calls (CALL/RET)

The module path supports full function calls with recursion:

```forth
: fib ->n
  n 2 < if  n
  else  n 1 - fib  n 2 - fib  +
  end ;

10 fib   -- → 55
```

### Implementation

1. **Pre-scan**: Find all function bodies via symbol table; `scan_used_locals()` identifies which local slots each function actually uses
2. **Block per function**: Each function becomes a Cranelift basic block
3. **CALL**: Save only used locals to frame stack (not all 64), flush SSA to memory, push return-site-ID, jump to function block
4. **RET**: Restore only used locals, use `br_table` to dispatch to return site
5. **Locals**: 64 slots available per frame, but only slots referenced by STORE/LOAD instructions are saved/restored

```
Main:                   Function "fib":
  push 10               load arg
  CALL fib  ────────►   n 2 <
  ◄──── return site     if/else
  (result on stack)     CALL fib (recursive)
                        ...
                        RET ────────► br_table → return site
```

---

## Files

| File | Lines | Tests |
|------|-------|-------|
| native_backend.rs | 4,105 | 96 |
| bytecode.rs (registry) | 1,081 | 2 |

---

## What's Next

The 84 opcodes not yet JIT-compiled (lists, maps, strings, etc.) are intentionally
left to the interpreter — they involve dynamic dispatch, heap allocation, and
string processing that don't benefit from JIT compilation.

The focus areas for the JIT are:
1. **Optimization**: Teach Cranelift more about stack patterns
2. **Arrays**: JIT-compile ArrayNew/Get/Set for O(1) data access
3. **Tail call optimization**: Detect `CALL; RET` and compile as `JMP`
