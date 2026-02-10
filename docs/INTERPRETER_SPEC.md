# Kore Bytecode Interpreter Specification

**Version**: 0.1  
**Status**: ✅ Working  
**Last Updated**: Following BYTECODE_SPEC.md

## Overview

The interpreter is a **deterministic** execution engine that transforms bytecode into stack operations. It follows all four postulates mechanically, with no decision points.

```
                    Postulate Compliance
┌─────────────────────────────────────────────────────────────┐
│ P1: Every value on the stack is a tool (S → S)              │
│ P2: APPLY is the only "real" operation                      │
│ P3: Execution is sequential concatenation                   │
│ P4: Types are checked at runtime (P4 = compile-time check)  │
└─────────────────────────────────────────────────────────────┘
```

## Architecture

```
┌──────────────────────────────────────────────┐
│                 Interpreter                   │
├──────────────────────────────────────────────┤
│  code: &[u8]       ← bytecode being executed │
│  pc: usize         ← program counter         │
│  stack: Stack      ← THE state               │
│  call_stack: Vec   ← return addresses        │
│  symbols: HashMap  ← name → offset           │
│  trace: bool       ← debugging               │
└──────────────────────────────────────────────┘
                     │
                     ▼
        ┌────────────────────────┐
        │   run() → step()       │
        │   (P2: apply loop)     │
        └────────────────────────┘
```

## Value Representation

```rust
pub enum Value {
    Nil,                           // () - unit type
    Bool(bool),                    // truth values
    Int(i64),                      // integers
    Float(f64),                    // floating point
    Str(String),                   // strings
    Pair(Box<Value>, Box<Value>),  // products: A × B
    Left(Box<Value>),              // sums: A + B (left injection)
    Right(Box<Value>),             // sums: A + B (right injection)
    Quote { offset, len },         // deferred computation
    List(Vec<Value>),              // homogeneous sequences
}
```

### Type Lattice (P4)

```
              ⊤ (Any)
           /    |    \
       Number  Bool  Container
       /   \          /   \
     Int  Float    Pair   List
                   / \
               Left  Right
                    |
                   ⊥ (Nil)
```

## Execution Model

### The Step Function (P2 in Action)

```
step():
    1. FETCH:   op = code[pc]; pc++
    2. DECODE:  match op to operation
    3. EXECUTE: transform stack
    
This is the ONLY operation - everything else is composition (P3)
```

### Stack Effects

Each opcode has a deterministic stack effect:

| Category | Opcode | Stack Before | Stack After |
|----------|--------|--------------|-------------|
| Stack | `drop` | `a` | `` |
| Stack | `dup` | `a` | `a a` |
| Stack | `swap` | `a b` | `b a` |
| Stack | `rot` | `a b c` | `b c a` |
| Stack | `over` | `a b` | `a b a` |
| Data | `pair` | `a b` | `(a,b)` |
| Data | `unpair` | `(a,b)` | `a b` |
| Data | `left` | `a` | `Left(a)` |
| Data | `right` | `a` | `Right(a)` |
| Arith | `add` | `a b` | `(a+b)` |
| Arith | `sub` | `a b` | `(a-b)` |
| Arith | `mul` | `a b` | `(a*b)` |
| Arith | `div` | `a b` | `(a/b)` |
| Compare | `eq` | `a b` | `(a==b)` |
| Compare | `lt` | `a b` | `(a<b)` |
| Compare | `le` | `a b` | `(a≤b)` |
| Logic | `and` | `a b` | `(a∧b)` |
| Logic | `or` | `a b` | `(a∨b)` |
| Logic | `not` | `a` | `¬a` |
| Control | `jmp` | `` | `` (pc=addr) |
| Control | `jz` | `bool` | `` (jump if false) |
| Control | `call` | `` | `` (push return) |
| Control | `ret` | `` | `` (pop return) |
| Control | `halt` | `` | `` (stop) |

## Error Handling

Errors are type mismatches (P4 violations at runtime):

```
Type Error        → "expected X, got Y"
Stack Underflow   → "stack underflow"
Division by Zero  → "division by zero"
Index Out of Bounds → "index out of bounds"
Unknown Opcode    → "unknown opcode: 0xXX"
```

## Example: Factorial Trace

```
factorial(5):

0000: stack=[]              ; start
0002: stack=[5]             ; push 5
0006: stack=[5]             ; call factorial
0007: stack=[5, 5]          ; dup
0009: stack=[5, 5, 1]       ; push 1
000A: stack=[5, false]      ; le: 5 ≤ 1 = false
0011: stack=[5]             ; jz taken (false)
0012: stack=[5, 5]          ; dup
0014: stack=[5, 5, 1]       ; push 1
0015: stack=[5, 4]          ; sub: 5-1=4
[... recurse until n=1 ...]
0010: stack=[..., 1]        ; base case returns 1
0018: stack=[..., 2, 1]     ; back up call stack
0019: stack=[..., 2]        ; mul: 2*1=2
[... unwind multiplying ...]
0019: stack=[120]           ; final: 5*4*3*2*1=120
```

## CLI Usage

```bash
# Disassemble bytecode
korec disasm file.korec

# Run bytecode
korec run file.korec

# Run with execution trace
korec run file.korec --trace

# Run factorial example
korec example

# Run test suite
korec test
```

## Test Results

```
✓ Test 1: 3 + 4 = 7
✓ Test 2: factorial(5) = 120
✓ Test 3: unpair(pair(1,2)) then add = 3
✓ Test 4: 7 dup mul = 49
✓ Test 5: 10 3 swap sub = 7
✓ Test 6: Left(42)
✓ Test 7: 5 < 10 = true
✓ Test 8: Complex (3,4) magnitude² = 25

Passed: 8, Failed: 0
```

## What's Next

```
┌─────────────────────────────────────────┐
│         IMPLEMENTATION STACK            │
├─────────────────────────────────────────┤
│  ✅ BYTECODE_SPEC.md    (specification) │
│  ✅ bytecode.rs         (assembler)     │
│  ✅ interpreter.rs      (executor)      │
│  ✅ main.rs             (CLI tool)      │
│  ✅ proof_checker.rs    (P4 at compile) │
│  ✅ wasm_backend.rs     (browser)       │
│  ✅ spirv_backend.rs    (GPU)           │
│  ✅ native_backend.rs   (Cranelift JIT) │
│  ⏳ parser.rs           (Kore syntax)   │
└─────────────────────────────────────────┘
```

## Execution Backends

Kore has multiple execution backends optimized for different use cases:

| Backend | Good For | Limitations |
|---------|----------|-------------|
| **Interpreter** | Everything, debugging | Slower execution |
| **Native (Cranelift)** | Arithmetic, loops | No recursion |
| **WASM** | Browser, portable | Some overhead |
| **SPIR-V** | Parallel GPU ops | No branching |

See [NATIVE_BACKEND.md](NATIVE_BACKEND.md) for JIT compilation details.

## Postulate Compliance Summary

| Postulate | Implementation |
|-----------|----------------|
| P1: Everything is a Tool | Every `Value` can be on stack, quotes enable code-as-data |
| P2: One Operation | `step()` is the only operation, everything else is composition |
| P3: Composition = Concatenation | Bytecode is sequential; execution follows linearly |
| P4: Constraints Attenuate | Type errors halt execution (runtime check, compile-time via proof checker) |

---

*No decisions were made. Just blocks on top of blocks.*
