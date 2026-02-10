# Kore Bytecode Specification v0.1

## Overview

Kore bytecode is a minimal instruction set designed for:
- Maximum portability across hardware
- Simple implementation on any backend
- Direct mapping from Kore primitives

## Binary Format

### File Header
```
Bytes 0-3:   Magic number: 0x4B 0x4F 0x52 0x45 ("KORE")
Bytes 4-5:   Version: major.minor (u8, u8)
Bytes 6-7:   Flags (reserved)
Bytes 8-11:  Code section offset (u32 little-endian)
Bytes 12-15: Code section length (u32 little-endian)
Bytes 16-19: Data section offset (u32 little-endian)
Bytes 20-23: Data section length (u32 little-endian)
Bytes 24-27: Symbol table offset (u32 little-endian)
Bytes 28-31: Symbol table length (u32 little-endian)
```

### Instruction Encoding

Each instruction is 1 byte opcode + optional operands:

```
┌──────────┬────────┬─────────────────────────────────────┐
│  Opcode  │  Name  │  Operands                           │
├──────────┼────────┼─────────────────────────────────────┤
│   0x00   │  NOP   │  (none)                             │
│   0x01   │  DROP  │  (none)                             │
│   0x02   │  DUP   │  (none)                             │
│   0x03   │  SWAP  │  (none)                             │
│   0x04   │  ROT   │  (none)                             │
│   0x05   │  OVER  │  (none)                             │
├──────────┼────────┼─────────────────────────────────────┤
│   0x10   │  PAIR  │  (none)                             │
│   0x11   │ UNPAIR │  (none)                             │
│   0x12   │  LEFT  │  (none)                             │
│   0x13   │  RIGHT │  (none)                             │
│   0x14   │  CASE  │  u16 left_offset, u16 right_offset  │
├──────────┼────────┼─────────────────────────────────────┤
│   0x20   │  QUOTE │  u16 length, [length bytes of code] │
│   0x21   │  APPLY │  (none)                             │
│   0x22   │  CALL  │  u16 symbol_index                   │
│   0x23   │  RET   │  (none)                             │
├──────────┼────────┼─────────────────────────────────────┤
│   0x30   │  INT8  │  i8 value                           │
│   0x31   │  INT16 │  i16 value (little-endian)          │
│   0x32   │  INT32 │  i32 value (little-endian)          │
│   0x33   │  INT64 │  i64 value (little-endian)          │
│   0x34   │  F32   │  f32 value (IEEE 754)               │
│   0x35   │  F64   │  f64 value (IEEE 754)               │
│   0x36   │  STR   │  u16 length, [UTF-8 bytes]          │
│   0x37   │  NIL   │  (none)                             │
│   0x38   │  TRUE  │  (none)                             │
│   0x39   │  FALSE │  (none)                             │
├──────────┼────────┼─────────────────────────────────────┤
│   0x40   │  ADD   │  (none)                             │
│   0x41   │  SUB   │  (none)                             │
│   0x42   │  MUL   │  (none)                             │
│   0x43   │  DIV   │  (none)                             │
│   0x44   │  MOD   │  (none)                             │
│   0x45   │  NEG   │  (none)                             │
├──────────┼────────┼─────────────────────────────────────┤
│   0x50   │  EQ    │  (none)                             │
│   0x51   │  LT    │  (none)                             │
│   0x52   │  GT    │  (none)                             │
│   0x53   │  LE    │  (none)                             │
│   0x54   │  GE    │  (none)                             │
│   0x55   │  NE    │  (none)                             │
├──────────┼────────┼─────────────────────────────────────┤
│   0x60   │  AND   │  (none)                             │
│   0x61   │  OR    │  (none)                             │
│   0x62   │  NOT   │  (none)                             │
│   0x63   │  XOR   │  (none)                             │
├──────────┼────────┼─────────────────────────────────────┤
│   0x70   │  JMP   │  i16 relative_offset                │
│   0x71   │  JZ    │  i16 relative_offset (jump if zero) │
│   0x72   │  JNZ   │  i16 relative_offset (jump if !zero)│
├──────────┼────────┼─────────────────────────────────────┤
│   0x80   │  LIST  │  u16 count (collect N items)        │
│   0x81   │ UNLIST │  (none) (spread list to stack)      │
│   0x82   │  LEN   │  (none) (list/string length)        │
│   0x83   │  GET   │  (none) (list[index])               │
│   0x84   │  SET   │  (none) (list[index] = val)         │
├──────────┼────────┼─────────────────────────────────────┤
│   0xF0   │  EXT   │  u8 extension_id, ... (for tensors) │
│   0xFF   │  HALT  │  (none)                             │
└──────────┴────────┴─────────────────────────────────────┘
```

---

## Instruction Semantics

### Stack Operations

```
DROP:   (a -- )           Remove top element
DUP:    (a -- a a)        Duplicate top element
SWAP:   (a b -- b a)      Exchange top two elements
ROT:    (a b c -- b c a)  Rotate top three
OVER:   (a b -- a b a)    Copy second to top
```

### Data Construction

```
PAIR:   (a b -- (a,b))    Create pair from top two
UNPAIR: ((a,b) -- a b)    Destructure pair
LEFT:   (a -- Left(a))    Wrap in Left variant
RIGHT:  (a -- Right(a))   Wrap in Right variant
CASE:   (Sum [L] [R] --)  Branch on sum type
```

### Control Flow

```
QUOTE:  Push following bytecode as quoted value
APPLY:  ([code] -- ...)   Execute quote on stack
CALL:   Call named definition
RET:    Return from call
JMP:    Unconditional jump
JZ:     Jump if top is zero/false (pops)
JNZ:    Jump if top is non-zero/true (pops)
```

### Literals

```
INT8/16/32/64: Push integer
F32/F64:       Push float
STR:           Push string
NIL:           Push null/unit
TRUE/FALSE:    Push boolean
```

### Arithmetic

```
ADD: (a b -- a+b)
SUB: (a b -- a-b)
MUL: (a b -- a*b)
DIV: (a b -- a/b)
MOD: (a b -- a%b)
NEG: (a -- -a)
```

### Comparison

```
EQ: (a b -- a==b)
LT: (a b -- a<b)
GT: (a b -- a>b)
LE: (a b -- a<=b)
GE: (a b -- a>=b)
NE: (a b -- a!=b)
```

### List Operations

```
LIST:   Collect N items into list
UNLIST: Spread list onto stack
LEN:    Get length
GET:    (list i -- elem)
SET:    (list i v -- list')
```

### Extension Point

```
EXT: Extension opcode for hardware-specific ops
     0x01 = TENSOR_CREATE
     0x02 = TENSOR_ADD
     0x03 = TENSOR_MUL
     0x04 = TENSOR_MATMUL
     ... (defined per backend)
```

---

## Example: Factorial

Kore source:
```kore
def factorial
  dup 1 le
  [ drop 1 ]
  [ dup 1 sub factorial mul ]
  if
;

5 factorial
```

Bytecode (hex):
```
; Header
4B 4F 52 45  ; magic "KORE"
00 01        ; version 0.1
00 00        ; flags

; Code section
33 05        ; INT8 5
22 00 00     ; CALL 0 (factorial)
FF           ; HALT

; factorial definition at symbol 0
02           ; DUP
33 01        ; INT8 1
53           ; LE
71 XX XX     ; JZ skip_base_case
01           ; DROP
33 01        ; INT8 1
23           ; RET
; skip_base_case:
02           ; DUP
33 01        ; INT8 1
41           ; SUB
22 00 00     ; CALL 0 (factorial - recursion)
42           ; MUL
23           ; RET
```

---

## Backend Requirements

Any backend implementing Kore bytecode MUST:

1. **Implement all core opcodes (0x00-0x7F)**
2. **Handle stack underflow gracefully** (error, not crash)
3. **Support at least 1MB stack**
4. **Use IEEE 754 for floats**
5. **Use UTF-8 for strings**

Optional extensions (0x80+, 0xF0):
- Backends MAY implement extensions
- Unknown extensions MUST raise error (not silent ignore)

---

## Value Representation

Each backend chooses internal representation, but must preserve:

```
Type        | Tag  | Notes
------------|------|-------
Nil         | 0x00 | Unit type
Bool        | 0x01 | 0=false, 1=true
Int         | 0x02 | 64-bit signed
Float       | 0x03 | 64-bit IEEE 754
String      | 0x04 | UTF-8, immutable
List        | 0x05 | Dynamic array
Pair        | 0x06 | Two values
Left        | 0x07 | Sum type left
Right       | 0x08 | Sum type right
Quote       | 0x09 | Bytecode reference
```

---

## File Extension

- `.kore` - Kore source code
- `.korec` - Compiled Kore bytecode

---

## Versioning

- Major version: Breaking changes to opcode semantics
- Minor version: New opcodes, backward compatible

Current: **v0.1**

---

## Execution Backends

Kore bytecode can run on multiple backends:

### 1. Interpreter (Reference Implementation)
- Pure Rust stack machine
- Used for development and debugging
- Supports all opcodes
- Command: `korec run <file.korec>`

### 2. WebAssembly (WASM)
- Compiles to portable WASM binary
- Runs in browsers, Node.js, wasmtime, wasmer
- Uses i64 for integers
- Command: `korec wasm <file.korec>`

### 3. SPIR-V (GPU)
- Compiles to Vulkan compute shaders
- Runs on any GPU via wgpu:
  - Vulkan (Linux, Windows, Android)
  - Metal (macOS, iOS)
  - DirectX 12 (Windows)
  - WebGPU (browsers)
- Command: `korec spirv <file.korec>`
- Command: `korec gpu-run <file.spv>`

### Backend Verification

All backends produce identical results for identical inputs:

```
Program: 5 3 + 2 - 4 *
Expected: ((5 + 3) - 2) * 4 = 24

Interpreter:        24 ✓
WASM (Node.js):     24 ✓  
SPIR-V (GPU):       24 ✓
```
