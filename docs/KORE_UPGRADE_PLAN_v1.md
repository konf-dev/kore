# Kore Upgrade Plan: Simulation-Optimized Execution Engine

> **Goal:** Make korec the fastest possible simulator for external search systems
> (MCTS, RL, genetic search — whatever), without Kore knowing or caring what's
> driving it. Kore remains a dumb, deterministic, postulate-compliant programming
> language. The intelligence lives outside.

> **Constraint:** Every change must honor the Three Postulates, preserve all
> safety/math guarantees, and pass all existing tests. No ML, no heuristics,
> no decision-making. Pure mechanism.

---

## Table of Contents

1. [Current State Analysis](#1-current-state-analysis)
2. [Bottleneck Identification](#2-bottleneck-identification)
3. [Upgrade 1: Interpreter State Snapshot/Restore](#3-upgrade-1-interpreter-state-snapshotrestore)
4. [Upgrade 2: Incremental Execution (Step Mode)](#4-upgrade-2-incremental-execution-step-mode)
5. [Upgrade 3: Session-Based Serve Protocol](#5-upgrade-3-session-based-serve-protocol)
6. [Upgrade 4: Batch Evaluation Mode](#6-upgrade-4-batch-evaluation-mode)
7. [Upgrade 5: Value Serialization (Structured JSON)](#7-upgrade-5-value-serialization-structured-json)
8. [Upgrade 6: Arena Allocator for Simulation Workloads](#8-upgrade-6-arena-allocator-for-simulation-workloads)
9. [Upgrade 7: Compile-Once Cache](#9-upgrade-7-compile-once-cache)
10. [Implementation Order](#10-implementation-order)
11. [Postulate Compliance Audit](#11-postulate-compliance-audit)
12. [Testing Strategy](#12-testing-strategy)
13. [What We Are NOT Doing](#13-what-we-are-not-doing)

---

## 1. Current State Analysis

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
stdin  → "3 5 +"                          (raw source string)
         OR {"source": "3 5 +", "max_steps": 10000}   (JSON)

stdout ← {"ok":true, "stage":"run", "result":"Int(8)",
           "stack":["Int(8)"], "steps":5, "bytecode_len":4,
           "max_stack_depth":2, "compile_ok":true, "typecheck_ok":true}
```

Each request is **fully independent**: parse → compile → optimize → proof-check → run.
No state carries between requests. No way to:
- Resume a previous execution
- Fork/clone a mid-execution state
- Execute one opcode at a time
- Avoid re-compilation of the same prefix

### Interpreter State (what needs to be captured for snapshot)

From `Interpreter<'a>` struct:
```
code: &'a [u8]           — borrowed bytecode (immutable, shared)
pc: usize                — program counter
stack: Stack             — Vec<Value> (THE observable state)
call_stack: Vec<usize>   — return addresses
symbols: HashMap<u16, usize> — symbol table (immutable after compile)
locals: Vec<Value>       — local variable slots
local_frames: Vec<Vec<Value>> — saved locals for nested calls
allowed_caps: u8         — capability bitmask (immutable)
output: Vec<String>      — captured print output
channels: Vec<ChannelState> — inter-fiber channels
trace: bool              — trace flag
steps: usize             — step counter
max_steps: usize         — gas limit
max_stack_depth: usize   — peak depth tracker
```

### Value Layout

`Value` is 16 bytes (compact enum). `Clone` is derived. Heap-allocated variants
(Str, List, Map, Pair, etc.) do deep copies on clone. This is the dominant cost
for state snapshots.

---

## 2. Bottleneck Identification

### The MCTS Access Pattern

An external search system (MCTS, beam search, whatever) does this:

```
1. Start with initial state S₀ (input on stack)
2. Pick opcode A₁, execute it → state S₁
3. Pick opcode A₂, execute it → state S₂
...
N. Check if Sₙ matches expected output

But ALSO:
- Fork S₃ → try different opcodes from same state
- Fork S₃ again → try yet another branch
- Fork S₇ from a different branch → etc.
- Run 800 such simulations per "move"
```

### Current Cost Per Simulation Node

With today's `korec serve`, every node expansion requires:

| Step | Cost | Notes |
|------|------|-------|
| Send full source string | O(n) | n = program length so far |
| Lex + parse | O(n) | Re-parses the entire prefix |
| Compile to bytecode | O(n) | Regenerates all bytecode |
| Optimize | O(n²) worst | Fixed-point algebraic passes |
| Proof-check | O(n) | Re-checks entire program |
| Execute from scratch | O(n) | Re-runs all n opcodes |
| Serialize result | O(s) | s = stack size |
| **Total** | **O(n²)** | For program of length n |

For MCTS at depth 20 with 800 simulations: up to **800 × 20 × O(n²)** work.
Most of this is redundant — we're re-doing work that was already done for the
parent node.

### What It Should Be

| Step | Cost | How |
|------|------|-----|
| Restore parent snapshot | O(s) | Clone the stack (s = stack size) |
| Compile 1 opcode | O(1) | Single opcode → 1-9 bytes |
| Execute 1 opcode | O(1) | One step of the interpreter |
| Read stack state | O(s) | Serialize current stack |
| **Total** | **O(s)** | Independent of program length |

This is the target. Every upgrade below moves toward this.

---

## 3. Upgrade 1: Interpreter State Snapshot/Restore

### What

Add the ability to capture the interpreter's full state as an owned value,
and restore it later. This is the fundamental primitive that enables tree search.

### Design

```rust
/// A complete snapshot of interpreter state.
/// Owns all data — no borrows, freely cloneable.
/// 
/// Postulate compliance:
///   P1: Snapshot is not a Kore value, not on the stack. It's host-side tooling.
///   P2: Does not affect stack semantics. Snapshot = external observation.
///   P3: Restoring a snapshot + continuing = concatenating from that point.
///       The composition property is preserved.
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
```

New methods on `Interpreter`:

```rust
impl<'a> Interpreter<'a> {
    /// Capture current state as an owned snapshot.
    /// O(s) where s = total size of stack + locals.
    pub fn snapshot(&self) -> InterpreterSnapshot { ... }
    
    /// Restore interpreter to a previously captured state.
    /// O(s) — replaces current state with snapshot's state.
    pub fn restore(&mut self, snap: &InterpreterSnapshot) { ... }
}
```

### Why This Is Postulate-Safe

- **P1 (Everything is a tool):** `InterpreterSnapshot` is NOT a Kore value. It
  never appears on the stack. It's a host-level mechanism, like how a debugger
  can inspect CPU registers without violating the CPU's instruction set.

- **P2 (Tools transform stacks):** Snapshot/restore doesn't change what any
  opcode does. It's external observation + rewind. The `Stack → Stack` contract
  of every tool is unchanged.

- **P3 (Composition = concatenation):** If program `P = A · B`, then
  snapshot-after-A + execute-B produces the same result as execute-P.
  Snapshot/restore preserves the compositional semantics exactly.

### Performance Target

- Snapshot: O(s) clone of stack + locals. No heap allocation beyond the clone.
- Restore: O(s) overwrite. Old state dropped.
- For typical MCTS states (stack depth 3-10, simple values): **< 100ns**.

---

## 4. Upgrade 2: Incremental Execution (Step Mode)

### What

Add the ability to execute exactly one "logical step" — a single source-level
operation — and return control to the caller. This avoids re-running the
entire program for each tree node.

### Design

```rust
/// Result of executing a single step.
pub enum StepResult {
    /// Opcode executed successfully, interpreter advanced.
    Continue,
    /// Program completed (hit HALT or end of code).
    Halted,
    /// Error occurred during execution.
    Error(String),
    /// Step limit reached.
    StepLimitReached,
}

impl<'a> Interpreter<'a> {
    /// Execute exactly one opcode and return.
    /// The interpreter state advances by one instruction.
    /// 
    /// This is the same as one iteration of `run()` — no semantic difference.
    /// P3: step(A); step(B) = run(A·B) for single-opcode A, B.
    pub fn step(&mut self) -> StepResult { ... }
    
    /// Execute N opcodes or until halt/error, whichever comes first.
    /// Returns the number of opcodes actually executed and the final status.
    pub fn step_n(&mut self, n: usize) -> (usize, StepResult) { ... }
}
```

### Compile-Then-Step Workflow

The caller compiles source to bytecode once, then feeds opcodes incrementally:

```rust
// Compile once
let module = parser::compile("DUP SORT SWAP LAST")?;

// Create interpreter
let mut interp = Interpreter::from_module(&module);

// Push initial input onto stack
interp.push_value(Value::List(Box::new(vec![
    Value::Int(3), Value::Int(1), Value::Int(2)
])));

// Step through opcodes one at a time
let result = interp.step();  // executes DUP
let snap = interp.snapshot(); // save state after DUP
let result = interp.step();  // executes SORT (compiled as a CALL)
// ... etc.
```

### What "One Step" Means

This is subtle. A source-level `SORT` might compile to `CALL symbol_idx` which
then runs many opcodes inside the function body. There are two modes:

1. **Opcode-level step** (`step()`): Execute exactly one bytecode instruction.
   If that's a `CALL`, the PC moves to the function body. Next `step()` runs
   the first opcode of the function. Caller must track CALL/RET depth if they
   want function-granularity.

2. **Source-level step** (`step_over()`): Execute until we return to the same
   call depth. Treats function calls as atomic. This is what a "one tool
   application" means per Postulate 2.

We implement both. The opcode-level step is trivial (extract one iteration of
`run()`). The source-level step builds on it.

```rust
impl<'a> Interpreter<'a> {
    /// Execute until we return to the current call depth.
    /// Treats function calls as atomic operations.
    /// This corresponds to one "tool application" per P2.
    pub fn step_over(&mut self) -> StepResult { ... }
}
```

### New Public Method: `push_value`

Currently there's no way to push a value onto the interpreter's stack from
outside. The only way to get values in is via bytecode (INT8, LIST, etc.).
We need a host-side push for setting up initial state:

```rust
impl<'a> Interpreter<'a> {
    /// Push a value onto the stack from the host side.
    /// This is the "initial state" setup — P2 says tools transform stacks,
    /// and the host is setting up the stack that tools will transform.
    /// D1: Configuration is initial state.
    pub fn push_value(&mut self, v: Value) { ... }
    
    /// Push multiple values (convenience).
    pub fn push_values(&mut self, values: &[Value]) { ... }
}
```

---

## 5. Upgrade 3: Session-Based Serve Protocol

### What

Extend `korec serve` with a session protocol that supports:
- Creating named sessions with compiled bytecode
- Setting initial stack state
- Stepping through execution
- Snapshotting and restoring
- Forking sessions (cheap clone for tree search)

### Protocol Design

All commands are JSON, one per line. All responses are JSON, one per line.
Session IDs are caller-assigned strings.

#### Command: `compile`
Compile source to bytecode, store as a named program. Does NOT execute.
```json
→ {"cmd":"compile", "id":"sort_prog", "source":"DUP SORT SWAP LAST"}
← {"ok":true, "id":"sort_prog", "bytecode_len":42, "compile_ok":true,
   "typecheck_ok":true}
```

#### Command: `session`
Create a new session from a compiled program.
```json
→ {"cmd":"session", "sid":"s1", "program":"sort_prog", "max_steps":100000}
← {"ok":true, "sid":"s1"}
```

#### Command: `push`
Push values onto the session's stack (initial state setup).
```json
→ {"cmd":"push", "sid":"s1", "values":[[3,1,2]]}
← {"ok":true, "sid":"s1", "stack_depth":1}
```

Value encoding: integers as JSON numbers, booleans as JSON bools, lists as
JSON arrays, strings as JSON strings, nil as `null`, pairs as `{"pair":[a,b]}`.
Detailed in [Upgrade 5](#7-upgrade-5-value-serialization-structured-json).

#### Command: `step`
Execute one source-level operation (step_over).
```json
→ {"cmd":"step", "sid":"s1"}
← {"ok":true, "sid":"s1", "stack":[...], "steps":1, "status":"continue"}
```

#### Command: `step_n`
Execute up to N opcodes.
```json
→ {"cmd":"step_n", "sid":"s1", "n":5}
← {"ok":true, "sid":"s1", "stack":[...], "steps":5, "status":"continue"}
```

#### Command: `run`
Execute until halt or error (existing behavior, within a session).
```json
→ {"cmd":"run", "sid":"s1"}
← {"ok":true, "sid":"s1", "stack":[...], "steps":42, "status":"halted"}
```

#### Command: `snap`
Save current session state under a name.
```json
→ {"cmd":"snap", "sid":"s1", "name":"after_dup"}
← {"ok":true, "sid":"s1", "name":"after_dup"}
```

#### Command: `restore`
Restore session to a named snapshot.
```json
→ {"cmd":"restore", "sid":"s1", "name":"after_dup"}
← {"ok":true, "sid":"s1", "stack":[...], "steps":1}
```

#### Command: `fork`
Create a new session as a clone of an existing one (snapshot + new session).
```json
→ {"cmd":"fork", "sid":"s1", "new_sid":"s2"}
← {"ok":true, "sid":"s2", "stack":[...]}
```

#### Command: `drop_session`
Free a session and all its snapshots.
```json
→ {"cmd":"drop_session", "sid":"s1"}
← {"ok":true}
```

#### Command: `drop_snap`
Free a specific snapshot.
```json
→ {"cmd":"drop_snap", "sid":"s1", "name":"after_dup"}
← {"ok":true}
```

#### Command: `compile_op`
Compile a SINGLE source-level operation and append it to a session's bytecode.
This is the "LLM generates one opcode at a time" mode.
```json
→ {"cmd":"compile_op", "sid":"s1", "op":"SORT"}
← {"ok":true, "sid":"s1", "bytecode_appended":3}
```

Then `step` executes it. This is the key primitive for incremental generation:
the external system can compile one operation, execute it, observe the stack,
decide the next operation, compile that, execute it, etc. — all without
re-compiling or re-running the prefix.

#### Command: `eval` (backward-compatible)
The existing stateless evaluate. Same as today's serve protocol.
```json
→ {"cmd":"eval", "source":"3 5 +", "max_steps":100000}
← {"ok":true, "result":"Int(8)", "stack":["Int(8)"], "steps":5, ...}
```

Also, bare source strings (without JSON wrapper) continue to work for full
backward compatibility.

### Implementation

The serve loop maintains:
```rust
struct ServeState {
    programs: HashMap<String, BytecodeModule>,  // compiled programs
    sessions: HashMap<String, Session>,         // active sessions
}

struct Session {
    interpreter: Interpreter<'static>,  // needs owned bytecode (see below)
    snapshots: HashMap<String, InterpreterSnapshot>,
    program_id: String,
}
```

**Lifetime issue:** `Interpreter<'a>` borrows `&'a [u8]` code. For sessions that
outlive a single request, we need owned bytecode. Solution: store `Vec<u8>` in
the program cache and use `unsafe` to extend the lifetime, OR refactor
`Interpreter` to own its code via `Arc<Vec<u8>>`. We choose the latter — it's
safe, reference-counted (cheap clone), and programs are immutable after compile.

```rust
/// Bytecode storage that can be shared across interpreters.
/// Immutable after creation — safe to share.
pub struct SharedBytecode {
    code: Arc<Vec<u8>>,
    symbol_table: HashMap<u16, usize>,
}
```

---

## 6. Upgrade 4: Batch Evaluation Mode

### What

Execute the same program against many different inputs in a single request.
This is for generating training data (run SORT against 1000 different lists).

### Protocol

```json
→ {"cmd":"batch_eval", "program":"sort_prog",
   "inputs":[[[3,1,2]], [[5,4,3,2,1]], [[1]], ...],
   "max_steps":100000}
← {"results":[
     {"ok":true, "stack":[...], "steps":15},
     {"ok":true, "stack":[...], "steps":42},
     {"ok":true, "stack":[...], "steps":3},
     ...
   ]}
```

Each input is a list of values to push onto a fresh stack before running. The
program is compiled once and reused for all inputs. Zero per-input compilation
overhead.

### Implementation

```rust
fn batch_eval(program: &SharedBytecode, inputs: &[Vec<Value>], max_steps: usize)
    -> Vec<EvalResult>
{
    inputs.iter().map(|input| {
        let mut interp = Interpreter::from_shared(program);
        interp.set_max_steps(max_steps);
        for v in input {
            interp.push_value(v.clone());
        }
        match interp.run() {
            Ok(()) => EvalResult::ok(interp.stack(), interp.steps_executed()),
            Err(e) => EvalResult::err(e, interp.steps_executed()),
        }
    }).collect()
}
```

This is embarrassingly parallel but we start with sequential for correctness.
Threading can be added later (each interpreter is independent — no shared
mutable state).

---

## 7. Upgrade 5: Value Serialization (Structured JSON)

### What

Currently, stack values are serialized as Rust Debug strings: `"Int(8)"`,
`"List([Int(1), Int(2)])"`. This requires the Python side to parse Rust's
Debug format — fragile and lossy.

Replace with structured JSON that round-trips perfectly.

### Encoding

| Kore Value | JSON | Example |
|-----------|------|---------|
| `Int(n)` | number | `42` |
| `Float(f)` | `{"f": n}` | `{"f": 3.14}` |
| `Bool(b)` | boolean | `true` |
| `Str(s)` | string | `"hello"` |
| `Nil` | `null` | `null` |
| `List([...])` | array | `[1, 2, 3]` |
| `Pair(a, b)` | `{"pair": [a, b]}` | `{"pair": [1, "x"]}` |
| `Left(v)` | `{"left": v}` | `{"left": 42}` |
| `Right(v)` | `{"right": v}` | `{"right": "err"}` |
| `Error(s)` | `{"error": s}` | `{"error": "overflow"}` |
| `Quote{..}` | `{"quote": [off, len]}` | `{"quote": [12, 5]}` |
| `Map({...})` | `{"map": {...}}` | `{"map": {"x": 1}}` |
| `Array([...])` | `{"array": [...]}` | `{"array": [1, 2, 3]}` |
| `Linear(v)` | `{"linear": v}` | `{"linear": 42}` |
| `Affine(v)` | `{"affine": v}` | `{"affine": 42}` |
| `Fiber` | `{"fiber": "..."}` | (not round-trippable) |
| `Channel(n)` | `{"channel": n}` | `{"channel": 0}` |

**Disambiguation rule:** Plain JSON numbers are Kore `Int` if they have no
decimal point and fit in i64. Use `{"f": ...}` wrapper for floats. This avoids
the JSON number ambiguity.

### Implementation

Two functions in `interpreter.rs`:

```rust
impl Value {
    /// Serialize to JSON string. No external dependencies (hand-written).
    pub fn to_json(&self) -> String { ... }
    
    /// Deserialize from JSON string. Returns None on invalid input.
    pub fn from_json(s: &str) -> Option<Value> { ... }
}
```

We continue to avoid `serde` as a dependency (matching existing design choice
in `main.rs`). The serializer is simple enough to hand-write.

---

## 8. Upgrade 6: Arena Allocator for Simulation Workloads

### What

During MCTS, thousands of `Value` clones are created and dropped rapidly
(snapshot → try opcode → drop if not promising → restore → try another).
The default allocator (malloc/free) has overhead for this pattern.

Use an arena allocator that can bulk-free all values from a discarded
simulation branch.

### Design

This is a **future optimization** — implement only if profiling shows allocator
overhead is significant. The design is:

```rust
/// Pool for rapid Value allocation during simulation.
/// Values allocated from this pool are freed in bulk when the pool is dropped.
/// 
/// P1-safe: the pool is host-side infrastructure, not a Kore value.
/// Semantics are identical — only allocation strategy changes.
pub struct ValuePool {
    // Bump allocator for Box<T> payloads
    arena: Vec<u8>,
    offset: usize,
}
```

**Risk:** This requires changing `Value` to use pool-allocated Boxes instead of
standard Boxes. Invasive change. Only do this if allocation is measurably the
bottleneck.

**Pragmatic alternative:** Use `jemalloc` or `mimalloc` as the global allocator.
One line in `main.rs`, zero code changes, typically 10-20% speedup for
alloc-heavy workloads.

```rust
// main.rs — just add this
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;
```

### Decision

Start with **mimalloc** (zero-risk). Measure. Only implement arena if needed.

---

## 9. Upgrade 7: Compile-Once Cache

### What

In the session protocol, programs are compiled once and stored. But we can go
further: cache the compilation of individual source-level words.

When `compile_op` receives `"SORT"`, we need to resolve `SORT` — is it a
built-in opcode or a user-defined function? If it's user-defined, we need its
symbol index. This resolution should happen once per word per program.

### Design

The `compile_op` command maintains a parser state that knows the current
dictionary (defined words). When a new op is compiled:

1. Look up the word in the current dictionary
2. If built-in: emit the opcode (1-9 bytes)
3. If user-defined: emit `CALL symbol_idx` (3 bytes)
4. Append to the session's bytecode buffer
5. Update the PC range for stepping

This requires the parser to expose an incremental compilation API:

```rust
impl Parser {
    /// Compile a single token/word in the current context.
    /// Returns the bytecode bytes to append.
    pub fn compile_one(&mut self, word: &str) -> Result<Vec<u8>, String> { ... }
}
```

This is a new entrypoint into the parser. Currently `compile()` takes a full
source string and returns a complete module. We need to factor out the per-word
compilation logic.

---

## 10. Implementation Order

### Phase A: Foundation (Upgrades 1, 2, 5)

These are the core primitives everything else builds on.

| # | Upgrade | Effort | Files Changed |
|---|---------|--------|---------------|
| A1 | InterpreterSnapshot (snap/restore) | 1 day | `interpreter.rs` |
| A2 | `step()` / `step_over()` / `step_n()` | 1 day | `interpreter.rs` |
| A3 | `push_value()` | 30 min | `interpreter.rs` |
| A4 | Value JSON serialization | 1 day | `interpreter.rs` (new section) |
| A5 | SharedBytecode (Arc-based) | 2 hours | `interpreter.rs`, `bytecode.rs` |
| A6 | Comprehensive tests for all above | 1 day | `interpreter.rs` (test module) |

**Deliverable:** After Phase A, the interpreter supports snapshot/restore,
stepping, and structured JSON — all usable from Rust. No serve protocol changes
yet.

### Phase B: Serve Protocol (Upgrades 3, 4)

Build the session-based serve protocol on top of Phase A primitives.

| # | Upgrade | Effort | Files Changed |
|---|---------|--------|---------------|
| B1 | Session state management | 1 day | `main.rs` (new serve2 module) |
| B2 | compile/session/push/step commands | 1 day | `main.rs` |
| B3 | snap/restore/fork/drop commands | 1 day | `main.rs` |
| B4 | batch_eval command | 2 hours | `main.rs` |
| B5 | compile_op incremental compilation | 1 day | `parser.rs`, `main.rs` |
| B6 | Backward compatibility (bare strings) | 2 hours | `main.rs` |
| B7 | Protocol integration tests | 1 day | new test file or `main.rs` tests |

**Deliverable:** After Phase B, Python can drive korec with full session
management over stdin/stdout.

### Phase C: Performance (Upgrades 6, 7)

Optimize based on real profiling data.

| # | Upgrade | Effort | Files Changed |
|---|---------|--------|---------------|
| C1 | mimalloc integration | 30 min | `Cargo.toml`, `main.rs` |
| C2 | Benchmark: snapshot/restore latency | 2 hours | `benchmarks.rs` |
| C3 | Benchmark: step throughput | 2 hours | `benchmarks.rs` |
| C4 | Profile and optimize hot paths | varies | depends on profiling |
| C5 | Compile-once cache | 1 day | `parser.rs` |

**Deliverable:** After Phase C, we have hard numbers on simulation throughput
and have optimized the critical path.

### Total Estimated Effort

- **Phase A:** ~4 days
- **Phase B:** ~5 days
- **Phase C:** ~3 days
- **Total:** ~12 days of focused work

---

## 11. Postulate Compliance Audit

Every upgrade checked against the Three Postulates:

### Upgrade 1: Snapshot/Restore

| Postulate | Compliant? | Reasoning |
|-----------|-----------|-----------|
| P1: Everything is a tool | ✅ | Snapshots are NOT Kore values. They're host-side. They never appear on the stack. |
| P2: Tools transform stacks | ✅ | No tool's behavior changes. Snapshot is pure observation. Restore is rewinding time. |
| P3: Composition = concatenation | ✅ | `snapshot(after A) + run(B) = run(A·B)`. The compositional identity holds. |
| Safety: Capabilities | ✅ | Snapshot preserves `allowed_caps`. Restoring can't escalate caps. |
| Safety: Resources | ✅ | Step counter is part of snapshot. No resource accounting bypass. |
| Safety: Determinism | ✅ | Same snapshot + same opcodes = same result. Always. |

### Upgrade 2: Step Mode

| Postulate | Compliant? | Reasoning |
|-----------|-----------|-----------|
| P1 | ✅ | `step()` is not a Kore tool. It's a host API to the interpreter. |
| P2 | ✅ | Each step applies one tool (opcode) to the stack. Same semantics as `run()`. |
| P3 | ✅ | `step(A); step(B)` = `run(A·B)`. Decomposing execution preserves composition. |
| Determinism | ✅ | Step order is fixed by bytecode sequence. No non-determinism introduced. |

### Upgrade 3: Session Protocol

| Postulate | Compliant? | Reasoning |
|-----------|-----------|-----------|
| P1 | ✅ | Sessions are host-side management. Not Kore values. |
| P2 | ✅ | Each session's interpreter follows normal P2 semantics. |
| P3 | ✅ | The protocol doesn't change how programs compose. |
| D1 (Config = initial state) | ✅ | `push` command is explicitly setting initial state. |

### Upgrade 4: Batch Eval

| Postulate | Compliant? | Reasoning |
|-----------|-----------|-----------|
| All | ✅ | Just runs the same interpreter N times with different initial stacks. Pure convenience. |

### Upgrade 5: Value Serialization

| Postulate | Compliant? | Reasoning |
|-----------|-----------|-----------|
| All | ✅ | Serialization is observation. Round-trip fidelity means no information loss. |

### Upgrades 6, 7: Performance

| Postulate | Compliant? | Reasoning |
|-----------|-----------|-----------|
| All | ✅ | Allocator and caching are implementation details. Zero semantic change. |

---

## 12. Testing Strategy

### Invariants To Test

1. **Snapshot fidelity:** `snapshot → restore → run(rest)` = `run(all)`
   For every program P = A·B:
   ```
   run(P, input) = { run(A, input); snap(); restore(); run(B) }
   ```

2. **Step equivalence:** `step() × N` = `run()` for programs of length N
   ```
   for all programs P:
     step_result = step through P one opcode at a time
     run_result = run P directly
     assert step_result == run_result
   ```

3. **Fork correctness:** `fork(session) → step(A)` on fork doesn't affect original
   ```
   s1 = session(prog)
   push(s1, input)
   step_n(s1, 5)
   fork(s1, s2)
   step(s2)  // should not change s1's state
   assert s1.stack == snapshot_before_fork.stack
   ```

4. **JSON round-trip:** `from_json(to_json(v)) == v` for all Value variants

5. **Backward compatibility:** Bare source strings in serve mode produce
   identical output to current version

### Test Programs

Use the existing test suite plus:
- Fibonacci (recursive, tests call_stack snapshot)
- List sort (tests heap value cloning)
- Nested quotes (tests pc + call_stack interaction)
- Error recovery (tests try/fail across snapshot boundaries)
- Fiber programs (tests fiber state in snapshots)
- Linear value programs (tests linearity enforcement across restore)

---

## 13. What We Are NOT Doing

To be absolutely clear about boundaries:

### ❌ No ML/Neural Net Code in Kore
Kore does not know MCTS exists. It doesn't know about policies or values or
training. It's a programming language interpreter. Period.

### ❌ No "Smart" Opcode Suggestions
Kore will not suggest which opcode to try next. No heuristics, no pruning of
the action space, no "likely valid next opcodes" feature. That's the neural
network's job.

### ❌ No Search Algorithm in Kore
MCTS, beam search, genetic algorithms — all external. Kore is the physics
engine. Search is the brain. Separate concerns.

### ❌ No New Opcodes for AlphaKore
We are not adding opcodes to help the search system. Kore's instruction set
is determined by what makes a good programming language, not by what makes
search easier.

### ❌ No Breaking Changes to Existing API
All current `korec` commands continue to work identically. The serve protocol
is backward-compatible (bare strings still work). No behavioral changes.

### ❌ No serde Dependency
Matching the existing design philosophy: hand-written JSON serialization.
The tiny additional code is worth avoiding the compile-time and binary size
cost of serde.

### ❌ No Multithreading (Phase A/B)
Each session is single-threaded. The MCTS system can launch multiple `korec
serve` processes if it wants parallelism. Adding internal threading is a Phase C
concern, only if profiling shows it's needed.

---

## Appendix A: Performance Estimates

### Snapshot/Restore Cost

Typical MCTS state: stack depth 5, each value is a small list (5-10 ints).

```
Stack: 5 values × 16 bytes = 80 bytes (inline data)
  + 5 heap allocs for List contents: ~200 bytes total
Call stack: ~3 entries × 8 bytes = 24 bytes
Locals: empty = 0 bytes
Total clone cost: ~304 bytes of memcpy

At memory bandwidth of ~40 GB/s: ~7.6 nanoseconds
With allocation overhead: ~50-100 nanoseconds realistic
```

For 800 MCTS simulations × 20 depth: 16,000 snapshots.
At 100ns each: **1.6 milliseconds** total snapshot overhead. Negligible.

### Step Execution Cost

One opcode execution: ~10-50ns (depends on opcode).
With step() overhead (function call, status check): ~20-70ns.

800 simulations × 20 depth × 1 step average: 16,000 steps.
At 50ns each: **0.8 milliseconds**. Negligible.

### JSON Serialization Cost

Typical stack → JSON: 5 values, ~200 bytes of JSON.
At ~1 GB/s serialization throughput: ~200ns.

If we serialize after every step (16,000 times): **3.2 milliseconds**.
If we only serialize at leaf nodes (~800 times): **0.16 milliseconds**.

### Total Per-Move MCTS Overhead (Kore side)

```
Snapshots:      ~1.6 ms
Step execution: ~0.8 ms
JSON serialize: ~0.2 ms (leaf only)
Network I/O:    ~0    (stdin/stdout, same process)
───────────────────────
Total:          ~2.6 ms per MCTS move
```

The neural network forward pass (~5-10ms per batch) will be the bottleneck,
not Kore. This is the correct outcome.

---

## Appendix B: Serve Protocol Quick Reference

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
COMPILE_OP:   {"cmd":"compile_op", "sid":"...", "op":"SORT"}
BATCH_EVAL:   {"cmd":"batch_eval", "program":"...", "inputs":[...], "max_steps":N}
EVAL:         {"cmd":"eval", "source":"...", "max_steps":N}
```
