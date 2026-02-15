# Kore Upgrade Plan v2: Universal AI Training Substrate

> **Goal:** Make korec a universal, minimal, perfectly uniform execution engine
> that ANY search/training algorithm (MCTS, RL, LLM, genetic, beam search) can
> drive to teach AI to solve ANY task expressible as code — starting with ARC-AGI,
> extensible to anything.

> **Constraint:** Every change must honor the Three Postulates, preserve all
> safety/math guarantees, and pass all existing tests. No ML, no heuristics,
> no decision-making inside Kore. Pure mechanism. The intelligence lives outside.

---

## Table of Contents

1. [Design Philosophy](#1-design-philosophy)
2. [Current State Analysis](#2-current-state-analysis)
3. [Phase 0: Language Uniformity](#3-phase-0-language-uniformity)
4. [Phase A: Simulation Primitives](#4-phase-a-simulation-primitives)
5. [Phase B: Session Protocol](#5-phase-b-session-protocol)
6. [Phase C: Performance](#6-phase-c-performance)
7. [Training Library Architecture](#7-training-library-architecture)
8. [Implementation Details](#8-implementation-details)
9. [Postulate Compliance Audit](#9-postulate-compliance-audit)
10. [Testing Strategy](#10-testing-strategy)
11. [What We Are NOT Doing](#11-what-we-are-not-doing)

---

## 1. Design Philosophy

### The Uniformity Principle

A model (of any kind) interacting with Kore sees this at every step:

```
1. Observe the stack (list of typed values)
2. Pick ONE action from a FIXED vocabulary
3. The action transforms the stack (S → S)
4. Repeat
```

**No brackets. No delimiters. No mode switches. No special syntax. No variable-
length constructs. No lookahead. No matching.** Every action is atomic, every
action has the same shape: one token in, stack transforms, done.

### Why This Matters

Any training algorithm — MCTS tree search, PPO policy gradient, LLM token
generation, evolutionary search — needs a fixed action space with uniform
structure. Special cases (bracket matching, literal operands, mode switches)
force the training algorithm to learn language grammar instead of task logic.

By making every action identical in shape, we let the training algorithm focus
entirely on: "given this stack state, which action gets me closer to the goal?"

### The Training Library Approach

**We do NOT rewrite Kore.** Kore stays exactly as it is — a full-featured
programming language for humans. Instead, we create a **training library** (a
`.kore` file) that:

1. Defines task-specific helper functions (e.g., `grid-map2d`, `transpose`)
2. Is loaded via `korec serve --prelude training.kore`
3. Exposes ONLY the functions + built-ins we want via `words` introspection
4. The model generates code that calls these functions + core opcodes

This way:
- Kore's full power remains available for human programming
- The model sees a curated, uniform subset
- Different tasks get different training libraries (different `words`)
- Zero Kore language changes needed for vocabulary curation

**What DOES change in Kore:** We add new opcodes (`compose`, `words`, `emptylist`,
`store0`–`store7`, `load0`–`load7`) and simulation infrastructure (snapshot,
step, sessions). The existing opcodes and syntax remain untouched.

---

## 2. Current State Analysis

### Architecture (25,585 lines across 13 files)

| File | Lines | Role |
|------|-------|------|
| `interpreter.rs` | 3,043 | Core VM — `execute_op()` dispatch loop |
| `parser.rs` | 6,222 | Source → bytecode compiler |
| `bytecode.rs` | 1,147 | Opcode definitions, assembler, module format |
| `proof_checker.rs` | 1,793 | Static type/stack-effect verification |
| `optimizer.rs` | 1,339 | Algebraic identity elimination |
| `pruner.rs` | 1,206 | Dead code removal via stack-effect analysis |
| `main.rs` | 1,417 | CLI: compile/run/serve/eval/repl/prune |
| `native_backend.rs` | 4,143 | Cranelift JIT |
| `gpu_runtime.rs` | 320 | WebGPU compute shaders |
| `spirv_backend.rs` | 1,470 | SPIR-V codegen |
| `wasm_backend.rs` | 844 | WebAssembly codegen |
| `benchmarks.rs` | 1,151 | Microbenchmarks |
| `industrial_benchmarks.rs` | 1,490 | Real-world benchmarks |

### Current `korec serve` Protocol

```
stdin  → "3 5 +"
         OR {"source": "3 5 +", "max_steps": 10000}

stdout ← {"ok":true, "stage":"run", "result":"Int(8)",
           "stack":["Int(8)"], "steps":5, ...}
```

Each request is **fully independent**: parse → compile → optimize → proof-check → run.
No state between requests. No way to resume, fork, or step.

**Key existing feature:** `korec serve --prelude file.kore` prepends a prelude
to every program. This is already the mechanism for training libraries.

**Key existing feature:** Serve mode runs with `allowed_caps: 0` (pure computation,
no IO/FS/NET/EXEC). Perfect for training — the model cannot escape the sandbox.

### Interpreter State (what needs snapshot)

```
code: &[u8]              — borrowed bytecode (immutable, shared)
pc: usize                — program counter
stack: Vec<Value>        — THE observable state
call_stack: Vec<usize>   — return addresses
locals: Vec<Value>       — local variable slots
local_frames: Vec<...>   — saved locals for nested calls
allowed_caps: u8         — capability bitmask (immutable after init)
output: Vec<String>      — captured print output
channels: Vec<...>       — inter-fiber channels
steps: usize             — step counter
max_stack_depth: usize   — peak depth tracker
```

### What Currently Works (Verified by Testing)

```kore
3 4 +                              ✅  → Int(7)
: double 2 * ; 5 double            ✅  → Int(10)
( 1 2 3 ) [ 2 * ] map             ✅  → List([2, 4, 6])
( ) 1 append 2 append 3 append    ✅  → List([1, 2, 3])
true [ 1 ] [ 2 ] cond             ✅  → Int(1)
10 0 gt [ 1 ] [ 2 ] cond          ✅  → Int(1)
42 -> a 10 -> b a b +             ✅  → Int(52)
map-new "x" 42 map-set "x" map-get ✅ → Int(42)
"drop" describe                    ✅  → Str("(a --)")
3 type-of                          ✅  → Str("int") (non-destructive)
3 5 depth                          ✅  → Int(2)
```

### What Does NOT Work

```kore
nil 1 append        ✗  Nil is not List — append rejects it
@name / !name       ✗  Parser doesn't support @ or ! (stdlib files broken)
words               ✗  Not implemented (promised in POSTULATES.md)
compose             ✗  Doesn't exist
```

### Non-Uniformities That Break Model Training

| Construct | Problem | Tokens |
|-----------|---------|--------|
| `[ ... ]` | Variable-length, nested delimiters | 2 + N |
| `( ... )` | Variable-length, parser counts elements | 2 + N |
| `#( ... )` | Two-char opener `#(`, variable-length | 2 + N |
| `-> name` | Arrow syntax, name is compile-time | 2 |
| `collect N` | Next token must be literal integer | 2 |
| `if...else...end` | Multi-token, paired, compiler jumps | 3–5 |
| `while...do...end` | Multi-token, paired, compiler jumps | 3–5 |
| `"string"` | Delimiter matching, escape sequences | 1 (variable) |
| `123456` | Unbounded integer range | 1 (infinite) |
| 14 synonym pairs | `+`/`add`, `ret`/`return`, etc. | ×2 vocab |

---

## 3. Phase 0: Language Uniformity

These changes add new opcodes and parser words to Kore. They do NOT remove or
change any existing syntax — everything that works today continues to work.

### 0.1: `compose` — Quote Concatenation

**The single most important addition.** Makes Postulate 3 a runtime tool.

```
compose  ( quote1 quote2 -- quote3 )
```

Applying `quote3` is equivalent to applying `quote1` then `quote2`.

This eliminates multi-token `[ ... ]` for the model. Instead of:
```kore
[ dup 0 gt 1 sub ]     -- 7 tokens, bracket matching
```

The model generates:
```kore
[dup] [0] compose [gt] compose [1] compose [sub] compose
-- 9 flat atomic actions, zero brackets to match
```

Where `[dup]`, `[0]`, `[gt]`, `[1]`, `[sub]` are each ONE atomic action in the
model's vocabulary — a single-opcode quote. The model never generates `[` or `]`
as separate tokens.

#### Implementation

**New opcode:** `Op::Compose = 0xD3` in `bytecode.rs` (slot available in
introspection range 0xD0–0xDF).

**New parser word:** `"compose" => self.asm.emit(Op::Compose)` in `parser.rs`
`emit_word()`.

**Interpreter logic in `interpreter.rs`:**

```rust
// COMPOSE: (quote1 quote2 -- quote3)
// Lazy implementation: store as ComposedQuote pair.
// When applied, run quote1 then quote2.
0xD3 => {
    let q2 = self.stack.pop()?;
    let q1 = self.stack.pop()?;
    match (&q1, &q2) {
        (Value::Quote { .. }, Value::Quote { .. }) => {
            // Store composed quote as a List of the two quotes
            // apply will detect this pattern and run both in sequence
            self.stack.push(Value::List(Box::new(vec![q1, q2])));
        }
        // Also handle composing already-composed quotes (List of quotes)
        (Value::List(existing), Value::Quote { .. }) => {
            let mut combined = existing.as_ref().clone();
            combined.push(q2);
            self.stack.push(Value::List(Box::new(combined)));
        }
        _ => return Err(format!("COMPOSE expects two quotes")),
    }
}
```

**Modify `apply` (0x21)** to handle composed quotes (List of quotes):

```rust
0x21 => {
    match self.stack.pop()? {
        Value::Quote { offset, len: _ } => {
            // Existing behavior: single quote
            self.call_stack.push(self.pc);
            self.local_frames.push(self.locals.clone());
            self.pc = offset as usize;
        }
        Value::List(items) => {
            // Composed quote: run each sub-quote in sequence
            // Verify all items are quotes first
            for item in items.iter() {
                if !matches!(item, Value::Quote { .. }) {
                    return Err("APPLY on list: not all elements are quotes".into());
                }
            }
            // Execute each quote inline
            for item in items.iter() {
                if let Value::Quote { offset, .. } = item {
                    self.run_quote_inline(*offset as usize)?;
                }
            }
        }
        v => return Err(format!("APPLY expects quote, got {}", v.type_name())),
    }
}
```

**Alternative (cleaner):** Instead of overloading `apply` on List, add a new
Value variant `ComposedQuote`. But this changes the 16-byte Value layout.
Decision: use the List approach first (zero layout changes), optimize later if
profiling shows composed-quote overhead matters.

#### Proof Checker

Add `Compose` to `proof_checker.rs`: pops two values (expected: Quote types),
pushes one (Quote type). Stack effect: `(2, 1)`.

#### Optimizer

`compose` of two identity quotes can be eliminated. But this is a future
optimization — correctness first.

---

### 0.2: `emptylist` — Bracket-Free List Construction

**Problem:** `( )` requires brackets. `nil` pushes Nil, not an empty List.
`nil 1 append` fails because `append` expects List.

**Fix:** New opcode `emptylist` that pushes `Value::List(Box::new(vec![]))`.

```
emptylist  ( -- list )   push empty list
```

Now the model builds lists without brackets:
```kore
emptylist 1 append 2 append 3 append  →  [1, 2, 3]
```

Every token is a regular stack op. No delimiters, no counting.

#### Implementation

**New opcode:** `Op::EmptyList = 0xCE` in `bytecode.rs` (slot available in
training primitives range 0xC5–0xCF).

**Parser:** `"emptylist" => self.asm.emit(Op::EmptyList)` in `emit_word()`.

**Interpreter:**
```rust
0xCE => {
    self.stack.push(Value::List(Box::new(vec![])));
}
```

Trivial. One line.

---

### 0.3: `store0`–`store7` / `load0`–`load7` — Numbered Local Slots

**Problem:** `-> name` requires special arrow syntax + a compile-time name.
The model must learn "after `->`, next token is a name, not an opcode."

**Fix:** 16 new keywords that map directly to `STORE slot` / `LOAD slot`:

```
store0 (a --)    store TOS in slot 0
load0  (-- a)    push slot 0 onto stack
store1 (a --)    store TOS in slot 1
load1  (-- a)    push slot 1 onto stack
...
store7 (a --)
load7  (-- a)
```

Each is one atomic action. No arrow, no name, no mode switch.

#### Implementation

**No new opcodes needed.** These just emit the existing `STORE`/`LOAD` opcodes
with hardcoded slot numbers:

```rust
// In parser.rs emit_word():
"store0" => self.asm.emit_store(0),
"store1" => self.asm.emit_store(1),
"store2" => self.asm.emit_store(2),
"store3" => self.asm.emit_store(3),
"store4" => self.asm.emit_store(4),
"store5" => self.asm.emit_store(5),
"store6" => self.asm.emit_store(6),
"store7" => self.asm.emit_store(7),
"load0"  => self.asm.emit_load(0),
"load1"  => self.asm.emit_load(1),
"load2"  => self.asm.emit_load(2),
"load3"  => self.asm.emit_load(3),
"load4"  => self.asm.emit_load(4),
"load5"  => self.asm.emit_load(5),
"load6"  => self.asm.emit_load(6),
"load7"  => self.asm.emit_load(7),
```

The existing `-> name` syntax stays for human programmers. The numbered
variants are just uniform aliases for the model.

#### Describe

Add entries to `describe_word()` for all 16:
```
"store0" => "(a --)  store top of stack in slot 0"
"load0"  => "(-- a)  push slot 0 onto stack"
```

---

### 0.4: `words` — Tool Discovery

**Promised in POSTULATES.md D3 but never implemented.**

```
words  ( -- list )   push list of all known word names as strings
```

This is how the model discovers what tools are available. Different training
libraries expose different word sets.

#### Implementation

**New opcode:** `Op::Words = 0xD4` in `bytecode.rs`.

**Interpreter:** Must know what words exist. Two sources:
1. **Built-in words:** Hardcoded list (same words in `describe_word()`)
2. **User-defined functions:** From the symbol table

```rust
// In interpreter.rs:
0xD4 => {
    let mut word_list: Vec<Value> = Vec::new();

    // Built-in words (from describe_word registry)
    for name in builtin_word_names() {
        word_list.push(Value::Str(Box::new(name.to_string())));
    }

    // User-defined functions (from symbol table names)
    // Requires storing function names alongside symbol indices.
    // Currently symbol_table is HashMap<u16, usize> (index → offset).
    // We need a reverse map or a separate name table.
    // See implementation notes below.

    self.stack.push(Value::List(Box::new(word_list)));
}
```

**Symbol name table:** Currently `BytecodeModule` stores `symbol_table:
HashMap<u16, usize>` (index → code offset) but NOT the original function names.
The parser's `functions: HashMap<String, usize>` (name → label) is discarded
after compilation.

**Fix:** Add `symbol_names: Vec<String>` to `BytecodeModule`. The parser
populates it during the function-collection pass. The interpreter reads it
for `words`. This is purely additive — no existing behavior changes.

```rust
// In bytecode.rs, add to BytecodeModule:
pub symbol_names: Vec<String>,

// In parser.rs, during function collection pass:
// After registering each `: name`, push name to symbol_names.
```

---

### 0.5: Enhanced `describe` — User Functions Too

Currently `describe` only knows built-in words (hardcoded in `describe_word()`).
For user-defined functions, it returns `"unknown word: X"`.

**Fix:** If the word isn't a built-in, check the symbol name table. If found,
return `"(user-defined function)"`. Future: support user-provided stack-effect
annotations via comments.

#### Implementation

Modify the `Describe` handler in `interpreter.rs`:

```rust
0xD2 => {
    match self.stack.pop()? {
        Value::Str(name) => {
            let desc = describe_word(&name);
            if desc.starts_with("unknown word:") {
                // Check if it's a user-defined function
                if self.symbol_names.contains(&*name) {
                    self.stack.push(Value::Str(Box::new(
                        "(user-defined function)".into()
                    )));
                } else {
                    self.stack.push(Value::Str(Box::new(desc)));
                }
            } else {
                self.stack.push(Value::Str(Box::new(desc)));
            }
        }
        v => return Err(format!("DESCRIBE expects string, got {}", v.type_name())),
    }
}
```

---

## 4. Phase A: Simulation Primitives

These are the core primitives that enable external search systems to drive Kore
efficiently. All are host-side infrastructure — they never appear on the Kore
stack or affect Kore semantics.

### A.1: Interpreter State Snapshot/Restore

```rust
/// Complete snapshot of interpreter state. Owned, cloneable, no borrows.
///
/// P1-safe: NOT a Kore value. Never on the stack. Host-side tooling.
/// P2-safe: No tool behavior changes. Pure observation + rewind.
/// P3-safe: snapshot(after A) + run(B) = run(A·B).
#[derive(Clone)]
pub struct InterpreterSnapshot {
    pub pc: usize,
    pub stack: Vec<Value>,
    pub call_stack: Vec<usize>,
    pub locals: Vec<Value>,
    pub local_frames: Vec<Vec<Value>>,
    pub steps: usize,
    pub max_stack_depth: usize,
    pub output: Vec<String>,
    pub channels: Vec<ChannelState>,
}

impl<'a> Interpreter<'a> {
    /// Capture current state. O(s) where s = stack + locals size.
    pub fn snapshot(&self) -> InterpreterSnapshot { ... }

    /// Restore to a previously captured state. O(s).
    pub fn restore(&mut self, snap: &InterpreterSnapshot) { ... }
}
```

#### Where in Code

File: `interpreter.rs`. Add `InterpreterSnapshot` struct after `ChannelState`.
Add `snapshot()` and `restore()` methods to `impl<'a> Interpreter<'a>`.

#### Performance Target

Typical MCTS state (stack depth 5, small lists): **< 100ns** per snapshot.
16,000 snapshots per MCTS move: **~1.6ms**. Negligible.

---

### A.2: Incremental Execution (Step Mode)

```rust
pub enum StepResult {
    Continue,           // Opcode executed, interpreter advanced
    Halted,             // Hit HALT or end of code
    Error(String),      // Runtime error
    StepLimitReached,   // Gas exhausted
}

impl<'a> Interpreter<'a> {
    /// Execute exactly one bytecode instruction.
    pub fn step(&mut self) -> StepResult { ... }

    /// Execute until returning to current call depth (one "tool application").
    pub fn step_over(&mut self) -> StepResult { ... }

    /// Execute up to N instructions.
    pub fn step_n(&mut self, n: usize) -> (usize, StepResult) { ... }
}
```

#### Where in Code

File: `interpreter.rs`. Extract one iteration of the `run()` loop into `step()`.
`step_over()` wraps `step()` with call-depth tracking. `step_n()` wraps `step()`
in a counted loop.

---

### A.3: `push_value` — Host-Side Stack Setup

```rust
impl<'a> Interpreter<'a> {
    /// Push a value onto the stack from the host.
    /// D1: Configuration is initial state.
    pub fn push_value(&mut self, v: Value) { ... }

    /// Push multiple values.
    pub fn push_values(&mut self, values: &[Value]) { ... }
}
```

#### Where in Code

File: `interpreter.rs`. Trivial: `self.stack.push(v)`.

---

### A.4: Value JSON Serialization

Replace Rust Debug format `"Int(8)"` with structured JSON that round-trips.

| Kore Value | JSON | Example |
|-----------|------|---------|
| `Int(n)` | number | `42` |
| `Float(f)` | `{"f": n}` | `{"f": 3.14}` |
| `Bool(b)` | boolean | `true` |
| `Str(s)` | string | `"hello"` |
| `Nil` | `null` | `null` |
| `List([...])` | array | `[1, 2, 3]` |
| `Pair(a,b)` | `{"pair": [a, b]}` | `{"pair": [1, "x"]}` |
| `Left(v)` | `{"left": v}` | `{"left": 42}` |
| `Right(v)` | `{"right": v}` | `{"right": "err"}` |
| `Error(s)` | `{"error": s}` | `{"error": "overflow"}` |
| `Quote{..}` | `{"quote": [off, len]}` | `{"quote": [12, 5]}` |
| `Map({..})` | `{"map": {...}}` | `{"map": {"x": 1}}` |
| `Array([..])` | `{"array": [...]}` | `{"array": [1, 2, 3]}` |

#### Where in Code

File: `interpreter.rs`. Add `to_json()` and `from_json()` to `impl Value`.
Hand-written (no serde dependency, matching existing design philosophy).

---

### A.5: SharedBytecode

`Interpreter<'a>` borrows `&'a [u8]` code. For sessions that outlive a single
request, we need owned bytecode.

```rust
/// Immutable bytecode that can be shared across interpreters.
pub struct SharedBytecode {
    code: Arc<Vec<u8>>,
    symbol_table: HashMap<u16, usize>,
    symbol_names: Vec<String>,
    cap_flags: u8,
}
```

#### Where in Code

File: `bytecode.rs` (struct definition). File: `interpreter.rs` (add
`Interpreter::from_shared()` constructor).

---

## 5. Phase B: Session Protocol

Build on Phase A primitives. Extend `korec serve` with session management.

### Protocol Commands

All JSON, one per line, backward-compatible (bare strings still work).

#### `compile` — Compile source, store as named program

```json
→ {"cmd":"compile", "id":"prog1", "source":": double 2 * ; : inc 1 + ;"}
← {"ok":true, "id":"prog1", "bytecode_len":42, "functions":["double","inc"]}
```

#### `session` — Create execution session from compiled program

```json
→ {"cmd":"session", "sid":"s1", "program":"prog1", "max_steps":100000}
← {"ok":true, "sid":"s1"}
```

#### `push` — Push values onto session stack

```json
→ {"cmd":"push", "sid":"s1", "values":[[3,1,2]]}
← {"ok":true, "sid":"s1", "stack_depth":1}
```

#### `compile_op` — Compile + append one operation

This is the key primitive for incremental generation. The model emits one
action, the controller sends it as `compile_op`, Kore compiles and appends it.

```json
→ {"cmd":"compile_op", "sid":"s1", "op":"dup"}
← {"ok":true, "sid":"s1", "bytecode_appended":1}

→ {"cmd":"compile_op", "sid":"s1", "op":"[dup]"}
← {"ok":true, "sid":"s1", "bytecode_appended":5}

→ {"cmd":"compile_op", "sid":"s1", "op":"compose"}
← {"ok":true, "sid":"s1", "bytecode_appended":1}
```

Note: `compile_op` accepts any valid Kore expression that compiles to a
complete bytecode unit. Single words like `dup`, `compose`, `store0`. Single-op
quotes like `[dup]`, `[0]`, `[add]`. Even multi-word expressions like
`10 0 gt` — though the model will typically send one action at a time.

**Implementation:** Create a mini-parser that has access to the session's
function table (from the initial `compile`). It compiles the expression string
into bytecode bytes and appends them to the session's code buffer. The session's
PC doesn't advance — `step` does that.

#### `step` — Execute one operation

```json
→ {"cmd":"step", "sid":"s1"}
← {"ok":true, "sid":"s1", "stack":[...], "steps":1, "status":"continue"}
```

#### `step_n` — Execute up to N instructions

```json
→ {"cmd":"step_n", "sid":"s1", "n":10}
← {"ok":true, "sid":"s1", "stack":[...], "steps":10, "status":"continue"}
```

#### `run` — Execute until halt or error

```json
→ {"cmd":"run", "sid":"s1"}
← {"ok":true, "sid":"s1", "stack":[...], "steps":42, "status":"halted"}
```

#### `snap` — Save session state

```json
→ {"cmd":"snap", "sid":"s1", "name":"after_setup"}
← {"ok":true, "sid":"s1", "name":"after_setup"}
```

#### `restore` — Restore to snapshot

```json
→ {"cmd":"restore", "sid":"s1", "name":"after_setup"}
← {"ok":true, "sid":"s1", "stack":[...]}
```

#### `fork` — Clone session

```json
→ {"cmd":"fork", "sid":"s1", "new_sid":"s2"}
← {"ok":true, "sid":"s2", "stack":[...]}
```

#### `drop_session` / `drop_snap` — Free resources

```json
→ {"cmd":"drop_session", "sid":"s1"}
← {"ok":true}
```

#### `batch_eval` — Run program against many inputs

```json
→ {"cmd":"batch_eval", "program":"prog1",
   "inputs":[[[3,1,2]], [[5,4,3,2,1]], [[1]]],
   "max_steps":100000}
← {"results":[
     {"ok":true, "stack":[...], "steps":15},
     {"ok":true, "stack":[...], "steps":42},
     {"ok":true, "stack":[...], "steps":3}
   ]}
```

#### `eval` — Backward-compatible stateless eval

```json
→ {"cmd":"eval", "source":"3 5 +", "max_steps":100000}
← {"ok":true, "result":"Int(8)", "stack":["Int(8)"], ...}
```

Also, bare source strings continue to work for full backward compatibility.

### Serve State

```rust
struct ServeState {
    programs: HashMap<String, SharedBytecode>,
    sessions: HashMap<String, Session>,
}

struct Session {
    interpreter: Interpreter<'static>,  // uses SharedBytecode via Arc
    snapshots: HashMap<String, InterpreterSnapshot>,
    program_id: String,
    // For compile_op: the function table from the initial compile
    function_table: HashMap<String, usize>,
}
```

#### Where in Code

File: `main.rs`. New function `run_serve_v2()` alongside existing `run_serve()`.
Existing serve mode stays unchanged. New protocol detected by JSON `"cmd"` field.

---

## 6. Phase C: Performance

### C.1: mimalloc

```rust
// main.rs
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;
```

One line. Zero risk. Typically 10–20% speedup for alloc-heavy workloads.

### C.2: Benchmarks

Add snapshot/restore and step throughput benchmarks to `benchmarks.rs`.
Measure real MCTS-pattern workloads: 800 simulations × 20 depth.

### C.3: Compile-Once Cache

The `compile_op` command maintains a dictionary for the session. Each word
is compiled once. Repeated uses of the same word in different sessions reuse
the bytecode.

---

## 7. Training Library Architecture

### The Key Insight

Kore has ~140 built-in keywords + synonyms. A model training on ARC-AGI needs
~50. A model training on text processing needs a different ~50. A model training
on numerical algorithms needs yet another ~50.

Rather than hardcoding which keywords the model sees, we define **training
libraries** — `.kore` files loaded via `--prelude` that:

1. Define task-specific helper functions
2. Serve as the source of truth for `words` (what the model can discover)

### How `words` Works With Training Libraries

`words` returns ALL available words: built-in opcodes + prelude-defined
functions. The training controller on the Python side filters this to define
the actual action vocabulary for the model. This filtering is outside Kore.

Alternatively, for a more integrated approach, we can add a `restrict-words`
mechanism at the interpreter level that limits which opcodes/functions are
accessible. But the simpler approach is: the Python controller defines the
action vocabulary, and only sends `compile_op` for words in that vocabulary.
If the model picks action #37 and that maps to `"fold"`, the controller sends
`compile_op("fold")`. The model never sees the full word list — only the
controller does.

### Example: ARC-AGI Training Library

```kore
-- stdlib/training/arc-agi.kore
-- Loaded via: korec serve --prelude stdlib/training/arc-agi.kore

-- Grid = List of Lists of Ints (0-9)
-- e.g., ((0 0 1) (1 0 0) (0 1 0)) is a 3x3 grid

-- grid-height: (grid -- grid n)
: grid-height dup len ;

-- grid-width: (grid -- grid n)
: grid-width dup head len ;

-- grid-get: (grid row col -- grid val)
-- Get cell value at (row, col). Non-destructive on grid.
: grid-get
  -> col -> row
  dup row get col get ;

-- grid-set: (grid row col val -- grid')
-- Set cell value at (row, col).
: grid-set
  -> val -> col -> row
  dup row get col val set
  swap row rot set ;

-- grid-map-cells: (grid quote -- grid')
-- Apply quote to every cell value. Quote: (int -- int).
: grid-map-cells
  -> f
  [ [ f apply ] map ] map ;

-- grid-transpose: (grid -- grid')
-- Transpose rows and columns.
: grid-transpose
  dup grid-width swap -> w
  dup grid-height swap -> h
  emptylist
  0 -> col
  [ col w lt ]
  [
    emptylist
    0 -> row
    [ row h lt ]
    [
      dup row get col get append
      row 1 add -> row
    ] compose loop              -- inner loop builds one column
    append
    col 1 add -> col
  ] compose loop ;              -- outer loop collects columns

-- grid-rotate-cw: (grid -- grid')
-- Rotate 90° clockwise = transpose then reverse each row.
: grid-rotate-cw grid-transpose [ reverse ] map ;

-- grid-rotate-ccw: (grid -- grid')
-- Rotate 90° counter-clockwise = transpose then reverse the rows.
: grid-rotate-ccw grid-transpose reverse ;

-- grid-flip-h: (grid -- grid')
-- Flip horizontally (mirror each row).
: grid-flip-h [ reverse ] map ;

-- grid-flip-v: (grid -- grid')
-- Flip vertically (reverse row order).
: grid-flip-v reverse ;

-- grid-count: (grid val -- grid n)
-- Count occurrences of val in grid. Non-destructive.
: grid-count
  -> target
  dup 0 -> count
  [ [ dup target eq [ drop count 1 add -> count ] [ drop ] cond ] map drop ] map
  drop count ;

-- grid-replace: (grid from to -- grid')
-- Replace all occurrences of 'from' with 'to'.
: grid-replace
  -> to -> from
  [ [ dup from eq [ drop to ] [ ] cond ] map ] map ;

-- grid-subgrid: (grid r1 c1 r2 c2 -- grid subgrid)
-- Extract rectangular region [r1,r2) × [c1,c2). Non-destructive.
: grid-subgrid
  -> c2 -> r2 -> c1 -> r1
  dup -> g
  emptylist
  r1 -> row
  [ row r2 lt ]
  [
    g row get -> rowdata
    emptylist
    c1 -> col
    [ col c2 lt ]
    [
      rowdata col get append
      col 1 add -> col
    ] compose loop
    append
    row 1 add -> row
  ] compose loop ;

-- grid-equal: (grid1 grid2 -- bool)
-- Check if two grids are identical.
: grid-equal
  zip [ unpair zip [ unpair eq ] map [ and ] fold ] map [ and ] fold ;
```

### Action Vocabulary for ARC-AGI

The Python controller defines the model's action space. Each action maps to
a `compile_op` string:

```python
ARC_AGI_ACTIONS = [
    # Literals (13)
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
    "true", "false", "nil",

    # Stack (5)
    "dup", "drop", "swap", "rot", "over",

    # Arithmetic (5)
    "add", "sub", "mul", "mod", "neg",

    # Comparison (6)
    "eq", "ne", "lt", "gt", "le", "ge",

    # Logic (3)
    "and", "or", "not",

    # Control (4)
    "apply", "cond", "loop", "nop",

    # Lists (10)
    "emptylist", "append", "len", "get", "set",
    "head", "tail", "reverse", "map", "fold",

    # Helpers (5)
    "times", "filter", "range", "empty?", "zip",

    # Compose (1)
    "compose",

    # Structures (4)
    "pair", "unpair", "type-of", "depth",

    # Locals (16)
    "store0", "store1", "store2", "store3",
    "store4", "store5", "store6", "store7",
    "load0", "load1", "load2", "load3",
    "load4", "load5", "load6", "load7",

    # Single-opcode quotes (matching core ops) (~40)
    "[dup]", "[drop]", "[swap]", "[add]", "[sub]", "[mul]",
    "[eq]", "[ne]", "[lt]", "[gt]", "[le]", "[ge]",
    "[and]", "[or]", "[not]",
    "[0]", "[1]", "[2]", "[3]", "[4]", "[5]", "[6]", "[7]", "[8]", "[9]",
    "[append]", "[head]", "[tail]", "[reverse]", "[len]", "[get]",
    "[pair]", "[unpair]", "[true]", "[false]",
    "[apply]", "[nop]",
    "[neg]", "[mod]",

    # Task-specific functions (from prelude)
    "grid-height", "grid-width", "grid-get", "grid-set",
    "grid-map-cells", "grid-transpose", "grid-rotate-cw",
    "grid-rotate-ccw", "grid-flip-h", "grid-flip-v",
    "grid-count", "grid-replace", "grid-subgrid", "grid-equal",

    # Discovery (2)
    "words", "describe",

    # Error handling (2)
    "try", "is-error",
]
# Total: ~125 fixed actions
```

### Other Task Libraries (Future)

```
stdlib/training/arc-agi.kore     — grid manipulation
stdlib/training/text.kore        — string processing + map operations
stdlib/training/math.kore        — float ops + geometry
stdlib/training/list-algo.kore   — sorting, searching, graph algorithms
stdlib/training/general.kore     — full vocabulary for open-ended tasks
```

Each library defines task-specific functions. The Python controller picks the
library and defines the action vocabulary. Kore doesn't know or care — it just
compiles and runs whatever it receives.

---

## 8. Implementation Details

### Files Changed Per Phase

#### Phase 0 (Language Uniformity)

| Change | File(s) | Lines Est. |
|--------|---------|-----------|
| `Op::Compose = 0xD3` | `bytecode.rs` | +3 (enum, all_opcodes, from_byte) |
| `compose` interpreter logic | `interpreter.rs` | +20 |
| Modify `apply` for composed quotes | `interpreter.rs` | +15 |
| `compose` parser word | `parser.rs` | +1 |
| `compose` proof checker | `proof_checker.rs` | +5 |
| `compose` describe entry | `interpreter.rs` | +1 |
| `Op::EmptyList = 0xCE` | `bytecode.rs` | +3 |
| `emptylist` interpreter logic | `interpreter.rs` | +3 |
| `emptylist` parser word | `parser.rs` | +1 |
| `emptylist` proof checker | `proof_checker.rs` | +3 |
| `emptylist` describe entry | `interpreter.rs` | +1 |
| `store0`–`store7` parser words | `parser.rs` | +8 |
| `load0`–`load7` parser words | `parser.rs` | +8 |
| `store0`–`load7` describe entries | `interpreter.rs` | +16 |
| `Op::Words = 0xD4` | `bytecode.rs` | +3 |
| `words` interpreter logic | `interpreter.rs` | +25 |
| `words` parser word | `parser.rs` | +1 |
| `symbol_names` in BytecodeModule | `bytecode.rs` | +5 |
| Populate symbol_names in parser | `parser.rs` | +10 |
| Pass symbol_names to interpreter | `interpreter.rs` | +10 |
| Enhanced `describe` for user funcs | `interpreter.rs` | +10 |
| **Total Phase 0** | | **~152 lines** |

#### Phase A (Simulation Primitives)

| Change | File(s) | Lines Est. |
|--------|---------|-----------|
| `InterpreterSnapshot` struct | `interpreter.rs` | +20 |
| `snapshot()` method | `interpreter.rs` | +20 |
| `restore()` method | `interpreter.rs` | +20 |
| `step()` method | `interpreter.rs` | +30 |
| `step_over()` method | `interpreter.rs` | +25 |
| `step_n()` method | `interpreter.rs` | +15 |
| `push_value()` / `push_values()` | `interpreter.rs` | +10 |
| `Value::to_json()` | `interpreter.rs` | +80 |
| `Value::from_json()` | `interpreter.rs` | +120 |
| `SharedBytecode` struct | `bytecode.rs` | +20 |
| `Interpreter::from_shared()` | `interpreter.rs` | +20 |
| Tests for all above | `interpreter.rs` | +200 |
| **Total Phase A** | | **~580 lines** |

#### Phase B (Session Protocol)

| Change | File(s) | Lines Est. |
|--------|---------|-----------|
| `ServeState`, `Session` structs | `main.rs` | +30 |
| Command parser (JSON) | `main.rs` | +60 |
| compile command handler | `main.rs` | +30 |
| session/push/step/run handlers | `main.rs` | +100 |
| snap/restore/fork/drop handlers | `main.rs` | +80 |
| compile_op handler | `main.rs` + `parser.rs` | +60 |
| batch_eval handler | `main.rs` | +40 |
| Backward compat (bare strings) | `main.rs` | +20 |
| Integration tests | `main.rs` or new file | +200 |
| **Total Phase B** | | **~620 lines** |

#### Phase C (Performance)

| Change | File(s) | Lines Est. |
|--------|---------|-----------|
| mimalloc integration | `Cargo.toml`, `main.rs` | +3 |
| Snapshot/restore benchmarks | `benchmarks.rs` | +50 |
| Step throughput benchmarks | `benchmarks.rs` | +50 |
| **Total Phase C** | | **~103 lines** |

#### Training Library

| File | Lines Est. |
|------|-----------|
| `stdlib/training/arc-agi.kore` | ~200 |

### Total Estimated Lines

```
Phase 0:  ~152 lines (across 4 files)
Phase A:  ~580 lines (interpreter.rs + bytecode.rs)
Phase B:  ~620 lines (main.rs + parser.rs)
Phase C:  ~103 lines (Cargo.toml + main.rs + benchmarks.rs)
Library:  ~200 lines (training/arc-agi.kore)
──────────────────
Total:    ~1,655 lines of new/modified code
```

### Implementation Order

```
Phase 0 (3 days):
  Day 1: compose + emptylist (opcodes, parser, interpreter, proof checker)
  Day 2: store0-7/load0-7 + words + symbol_names
  Day 3: Enhanced describe + tests for all Phase 0

Phase A (4 days):
  Day 4: InterpreterSnapshot + snapshot/restore
  Day 5: step/step_over/step_n + push_value
  Day 6: Value JSON serialization (to_json + from_json)
  Day 7: SharedBytecode + comprehensive tests

Phase B (5 days):
  Day 8:  ServeState + Session structs + command parser
  Day 9:  compile/session/push/step commands
  Day 10: snap/restore/fork/drop commands
  Day 11: compile_op + batch_eval
  Day 12: Backward compat + integration tests

Phase C (2 days):
  Day 13: mimalloc + benchmarks
  Day 14: Training library (arc-agi.kore) + end-to-end test

Total: ~14 days of focused work
```

---

## 9. Postulate Compliance Audit

### Phase 0 Changes

| Change | P1 (Tool) | P2 (S→S) | P3 (Concat) |
|--------|-----------|----------|-------------|
| `compose` | ✅ It's a tool | ✅ (q q -- q) | ✅ **IS P3 as a tool** |
| `emptylist` | ✅ It's a tool | ✅ (-- list) | ✅ Flat concatenation |
| `store0`–`store7` | ✅ Tools | ✅ (a --) | ✅ Flat actions |
| `load0`–`load7` | ✅ Tools | ✅ (-- a) | ✅ Flat actions |
| `words` | ✅ Tool (D3) | ✅ (-- list) | ✅ Flat action |
| Enhanced `describe` | ✅ Tool (D3) | ✅ (str -- str) | ✅ Flat action |

### Phase A Changes

| Change | P1 | P2 | P3 | Safety |
|--------|----|----|----|----|
| Snapshot | ✅ Not a Kore value | ✅ No tool changes | ✅ snap(A)+run(B)=run(AB) | ✅ Caps preserved |
| Step | ✅ Host API | ✅ Same as run() | ✅ step(A);step(B)=run(AB) | ✅ Deterministic |
| push_value | ✅ D1: initial state | ✅ Sets up stack | ✅ Prepend values | ✅ No cap bypass |
| JSON serial | ✅ Observation | ✅ Round-trips | ✅ No semantic change | ✅ Lossless |
| SharedBytecode | ✅ Impl detail | ✅ Same behavior | ✅ Same behavior | ✅ Immutable |

### Phase B Changes

| Change | P1 | P2 | P3 |
|--------|----|----|-----|
| Sessions | ✅ Host-side | ✅ Each session normal P2 | ✅ No composition change |
| compile_op | ✅ Host-side | ✅ Appends bytecode normally | ✅ Incremental = concat |
| batch_eval | ✅ Convenience | ✅ Same as N separate runs | ✅ No change |

---

## 10. Testing Strategy

### Phase 0 Tests

```rust
// compose: basic
"[1] [2] compose apply"  →  stack: [1, 2]

// compose: chain
"[1] [add] compose [2] compose [mul] compose"  →  quote that does: 1 add 2 mul
"5 [1] [add] compose apply"  →  stack: [6]

// compose: used with cond
"true [1] [add] compose [2] [mul] compose cond"
// with 5 on stack: true → 1 add → 6

// compose: used with loop
"10 [dup] [0] compose [gt] compose [1] [sub] compose compose loop"
// counts down from 10 to 0

// emptylist
"emptylist"  →  List([])
"emptylist 1 append"  →  List([1])
"emptylist 1 append 2 append 3 append"  →  List([1, 2, 3])

// store/load numbered
"42 store0 load0"  →  42
"1 store0 2 store1 load0 load1 add"  →  3

// words
"words"  →  List(["drop", "dup", ..., user-functions...])
"words len"  →  Int(N)  where N > 100

// describe for user functions
": double 2 * ; \"double\" describe"  →  "(user-defined function)"
```

### Phase A Tests

```rust
// Snapshot fidelity: snap after A, restore, run B = run AB
"5 3 +"  snapshot after "5 3" → restore → run "+"  →  8

// Step equivalence: step N times = run
for every test program: step-by-step == run()

// Fork isolation: fork → step on fork doesn't affect original

// JSON round-trip: from_json(to_json(v)) == v for all Value variants
```

### Phase B Tests

```json
// End-to-end MCTS simulation pattern:
compile("dup 1 add")
session(s1, prog)
push(s1, [5])           // stack: [5]
step(s1)                // dup → [5, 5]
snap(s1, "after_dup")   // save
step(s1)                // 1 → [5, 5, 1]
step(s1)                // add → [5, 6]
// result: 6 ✓

restore(s1, "after_dup") // back to [5, 5]
// try different continuation...
```

---

## 11. What We Are NOT Doing

### ❌ No Changes to Existing Kore Syntax
`if/else/end`, `while/do/end`, `[ ... ]`, `( ... )`, `-> name`, all synonyms —
everything stays. Human programmers use whatever syntax they prefer. The model
uses the uniform subset via the training library + Python controller.

### ❌ No Removal of Any Existing Opcode
All ~140 opcodes remain. The model's action vocabulary is a SUBSET defined by
the Python controller, not by Kore.

### ❌ No ML/Neural Net Code in Kore
Kore doesn't know MCTS exists. Kore doesn't know about policies, values, or
training. It's a programming language interpreter. Period.

### ❌ No Search Algorithm in Kore
MCTS, beam search, PPO, LLM decoding — all external. Kore is the physics
engine. Search is the brain. Separate concerns.

### ❌ No Breaking Changes to Existing API
All current `korec` commands work identically. The serve protocol is backward-
compatible. The new session protocol is opt-in via `"cmd"` field.

### ❌ No serde Dependency
Hand-written JSON serialization. Matching existing design philosophy.

### ❌ No Multithreading (Phase A/B)
Each session is single-threaded. The MCTS system can launch multiple `korec
serve` processes for parallelism. Internal threading is a Phase C concern.

### ❌ No "Smart" Features
Kore will not suggest opcodes, prune invalid moves, rank actions, or do anything
"intelligent." It compiles, it runs, it reports the stack. That's it.

---

## Appendix A: Complete Model Action Space Design

### At Every Step, the Model Sees:

```
Observation: [stack contents as JSON array]
Valid actions: [same fixed vocabulary — always all available]
```

### Action Execution Flow:

```
Model picks action #42 → Python maps to "fold" → compile_op("fold")
  → parser emits Op::Fold (1 byte) → appended to session bytecode
  → step() executes it → stack transforms → new observation
```

### Invalid Actions:

If the model picks `"add"` but the stack has `[List, Bool]`, the `step()` will
return `Error("ADD expects numbers")`. The training algorithm treats this as a
negative reward. Kore doesn't prevent it — it just reports the error honestly.
The model learns to avoid type errors through training.

---

## Appendix B: Per-Move MCTS Cost (Kore Side)

```
Snapshots (16K):     ~1.6 ms
Step execution (16K): ~0.8 ms
JSON serialize (800): ~0.2 ms
compile_op (16K):     ~0.5 ms
─────────────────────────────
Total:                ~3.1 ms per MCTS move
```

Neural network forward pass (~5–10ms per batch) is the bottleneck. Kore is not.

---

## Appendix C: Serve Protocol Quick Reference

```
COMPILE:      {"cmd":"compile", "id":"...", "source":"..."}
SESSION:      {"cmd":"session", "sid":"...", "program":"...", "max_steps":N}
PUSH:         {"cmd":"push", "sid":"...", "values":[...]}
STEP:         {"cmd":"step", "sid":"..."}
STEP_N:       {"cmd":"step_n", "sid":"...", "n":N}
RUN:          {"cmd":"run", "sid":"..."}
SNAP:         {"cmd":"snap", "sid":"...", "name":"..."}
RESTORE:      {"cmd":"restore", "sid":"...", "name":"..."}
FORK:         {"cmd":"fork", "sid":"...", "new_sid":"..."}
DROP_SESSION: {"cmd":"drop_session", "sid":"..."}
DROP_SNAP:    {"cmd":"drop_snap", "sid":"...", "name":"..."}
COMPILE_OP:   {"cmd":"compile_op", "sid":"...", "op":"dup"}
BATCH_EVAL:   {"cmd":"batch_eval", "program":"...", "inputs":[...], "max_steps":N}
EVAL:         {"cmd":"eval", "source":"...", "max_steps":N}
```
