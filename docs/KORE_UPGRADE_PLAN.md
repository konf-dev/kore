# Kore Upgrade Plan v3

> **Goal:** Make Kore a uniform execution substrate that ANY search/training
> algorithm (MCTS, RL, LLM, genetic, beam search) can drive to teach AI to
> solve ANY task — starting with ARC-AGI, extensible to anything.

> **Constraint:** Every change must honor the Three Postulates, preserve all
> safety/math guarantees, and pass all existing 68 tests. No ML, no heuristics,
> no decision-making inside Kore. Pure mechanism.

---

## Architecture Reality

This plan targets the **release/kore** codebase — a clean AST interpreter:

| Component | File(s) | What it is |
|-----------|---------|------------|
| Op | `src/op.rs` (278 lines) | `enum Op { Push(Value), Call(String) }` — only 2 operations |
| Executor | `src/executor.rs` (328 lines) | Async execute loop: Push → stack, Call → dict lookup |
| Stack | `src/stack.rs` (218 lines) | LIFO `Vec<Value>` with max depth 10K |
| Value | `src/value.rs` (621 lines) | 11 variants: Null, Bool, Int, Float, Text, List, Map, Quote, Handle, Error, Ext |
| Tool | `src/tool.rs` (187 lines) | `ToolBody::Native(closure)` or `ToolBody::Ops(Vec<Op>)` |
| Dictionary | `src/context.rs` (235 lines) | `HashMap<String, Tool>` — name → tool |
| Core | `src/core/` (15 modules) | 80 core primitives |
| Cap | `src/cap/` (13 modules) | 55 capability tools |
| Analyzer | `src/analyzer.rs` (641 lines) | Stack-effect analysis for each tool |

**Key insight:** Everything is a named tool. Adding new operations = registering
new tools. No bytecodes, no parser changes, no opcode slots.

---

## Phase 0 Changes

### Implementation Checklist

#### 1. compose — Quote Concatenation
- [x] Create `src/core/compose.rs` with `compose` tool
- [x] Tests: basic, empty quotes, nested quotes, non-quote args, chaining (8 tests)
- [x] Wire into `src/core/mod.rs` (mod + register + CORE_PRIMITIVES)
- [x] Add stack effect to `src/analyzer.rs`: `"compose" => (2, 1)`

#### 2. emptylist — Bracket-Free Empty List  
- [x] Add `emptylist` tool to `src/core/data.rs`
- [x] Tests: basic push, emptylist-then-unlist (2 tests)
- [x] Update `src/core/mod.rs` CORE_PRIMITIVES + count
- [x] Add stack effect to `src/analyzer.rs`: `"emptylist" => (0, 1)`

#### 3. describe — Tool Signature Lookup
- [x] Add `describe` tool to `src/core/definition.rs`
- [x] Tests: known tool, unknown tool errors, user-defined tool (3 tests)
- [x] Update `src/core/mod.rs` CORE_PRIMITIVES + count
- [x] Add stack effect to `src/analyzer.rs`: `"describe" => (1, 1)`

#### 4. store0–store7 / load0–load7 — Local Variable Slots
- [x] Add `locals: [Option<Value>; 8]` to `Stack` struct in `src/stack.rs`
- [x] Add `store_local()` and `load_local()` methods to `Stack`
- [x] Update `Stack::restore()` to also restore locals
- [x] Create `src/core/locals.rs` with 16 tools (store0–7, load0–7)
- [x] Tests: store/load roundtrip, slot isolation, unset = Null, all 8 slots (10 tests)
- [x] Wire into `src/core/mod.rs`
- [x] Add stack effects to `src/analyzer.rs`: storeN → (1,0), loadN → (0,1)

#### 5. Integration
- [x] All 478 tests pass (264 lib + 18 analyzer + 34 integration + 27 formal + 23 advanced + 44 comprehensive + 68 e2e)
- [x] `cargo build` clean (no new warnings)
- [x] `cargo clippy` clean (only 2 pre-existing warnings in optimizer.rs)
- [ ] Commit on `phase0-language-uniformity` branch

---

## 1. compose — Quote Concatenation

**The single most important addition.** Makes Postulate 3 (composition =
concatenation) a runtime tool the model can use.

```
compose  ( q1:Quote q2:Quote -- q3:Quote )
```

Since quotes are `Value::Quote(Vec<Op>)`, composition is literally
`Vec::extend` — concatenate the ops of q1 and q2.

**Why it matters for training:** Instead of generating multi-token `[ dup 0 gt ]`,
the model generates flat atomic actions:
```
[dup] [0] compose [gt] compose    # 5 tokens, zero bracket matching
```

### File: `src/core/compose.rs` (NEW)

```rust
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "compose",
        "(q1:Quote q2:Quote -- q3:Quote)",
        |mut stack, ctx| Box::pin(async move {
            let q2 = stack.pop()?.into_quote()?;
            let q1 = stack.pop()?.into_quote()?;
            let mut combined = q1;
            combined.extend(q2);
            stack.push(Value::Quote(combined))?;
            Ok((stack, ctx))
        }),
    ));
}
```

### Edge cases
- `[] [] compose → []` (two empties)
- `[a] [] compose → [a]` (identity right)
- `[] [a] compose → [a]` (identity left)
- `[a [b]] [c] compose → [a [b] c]` (nested quotes preserved)
- `1 [a] compose → TypeError` (non-quote first arg)

---

## 2. emptylist — Bracket-Free Empty List

```
emptylist  ( -- l:List )   push empty list
```

**Why:** `( )` requires brackets. Model needs bracket-free list construction:
```
emptylist 1 list-push 2 list-push 3 list-push  →  [1, 2, 3]
```

### File: `src/core/data.rs` (add after map-new, ~line 62)

```rust
dict.register(Tool::native(
    "emptylist",
    "( -- l:List)",
    |mut stack, ctx| Box::pin(async move {
        stack.push(Value::List(vec![]))?;
        Ok((stack, ctx))
    }),
));
```

One-liner. Fits naturally beside `map-new` which does the same for maps.

---

## 3. describe — Tool Signature Lookup

```
describe  ( name:Text -- sig:Text )   return tool's type signature
```

**Why:** The model needs to query what a tool does. `meta` (in cap/introspect)
returns a full Map — too heavy. `describe` returns just the signature string.

### File: `src/core/definition.rs` (add after words, ~line 47)

```rust
dict.register(Tool::native(
    "describe",
    "(name:Text -- sig:Text)",
    |mut stack, ctx| Box::pin(async move {
        let name = stack.pop()?.into_text()?;
        let dict = ctx.dict.read().await;
        let tool = dict.get(&name)?;  // ToolNotFound error if missing
        drop(dict);
        let sig = tool.meta.sig.unwrap_or_else(|| "( -- )".to_string());
        stack.push(Value::Text(sig))?;
        Ok((stack, ctx))
    }),
));
```

### Design decision: what to return for tools without a signature?
- **Choice:** Return `"( -- )"` (unknown effect) rather than error
- **Rationale:** Failing on valid tools with missing sigs would be surprising

---

## 4. store0–store7 / load0–load7 — Local Variable Slots

**Problem:** This codebase is a pure stack machine with NO local variables.
The plan's `-> name` syntax doesn't exist here. The model needs scratch storage
without complex stack juggling.

**Solution:** Add 8 local variable slots to the `Stack` struct itself.

```
store0  ( a -- )    pop TOS into slot 0
load0   ( -- a )    push slot 0 value (Null if unset)
store1  ( a -- )    pop TOS into slot 1
load1   ( -- a )    push slot 1 value
...
store7 / load7
```

### File: `src/stack.rs` — Structural change

Add to `Stack` struct:
```rust
pub struct Stack {
    values: Vec<Value>,
    max_depth: usize,
    locals: [Option<Value>; 8],  // NEW: 8 local variable slots
}
```

All constructors initialize `locals: Default::default()` (all `None`).

Add methods:
```rust
pub fn store_local(&mut self, slot: usize, value: Value) -> Result<()> {
    if slot >= 8 { return Err(Error::Runtime("local slot must be 0-7".into())); }
    self.locals[slot] = Some(value);
    Ok(())
}

pub fn load_local(&self, slot: usize) -> Result<Value> {
    if slot >= 8 { return Err(Error::Runtime("local slot must be 0-7".into())); }
    Ok(self.locals[slot].clone().unwrap_or(Value::Null))
}
```

Update `restore()`:
```rust
pub fn restore(&mut self, checkpoint: Stack) {
    self.values = checkpoint.values;
    self.locals = checkpoint.locals;  // NEW: restore locals too
}
```

### Scoping behavior (automatic, no extra code needed)

| Operation | Locals behavior | Why |
|-----------|----------------|-----|
| `call` (run a quote) | **Shared** | Same stack passed through |
| `loop` / `while` | **Shared** | Same stack across iterations |
| `if` | **Shared** | Same stack into branch |
| `dip` | **Shared** | Same stack to quote |
| `spawn` | **Fresh** | `Stack::new()` → all None |
| `times` | **Fresh per iter** | `Stack::new()` each iteration |
| `map`/`filter`/`each` | **Fresh per item** | `Stack::new()` each element |
| `try`/`catch` | **Checkpoint** | `checkpoint()` + `restore()` saves/restores locals |

### File: `src/core/locals.rs` (NEW)

16 tools registered via a loop to avoid repetition:
```rust
for slot in 0..8u8 {
    // Register storeN
    dict.register(Tool::native(
        format!("store{}", slot),
        "(a -- )",
        move |mut stack, ctx| Box::pin(async move {
            let val = stack.pop()?;
            stack.store_local(slot as usize, val)?;
            Ok((stack, ctx))
        }),
    ));
    // Register loadN
    dict.register(Tool::native(
        format!("load{}", slot),
        "( -- a)",
        move |mut stack, ctx| Box::pin(async move {
            let val = stack.load_local(slot as usize)?;
            stack.push(val)?;
            Ok((stack, ctx))
        }),
    ));
}
```

---

## 5. words — Already Implemented ✅

`words` exists in `src/core/definition.rs` lines 35–46. Returns a sorted
`List` of all tool names as `Text` values. No changes needed.

---

## Files Changed Summary

| File | Change | Risk |
|------|--------|------|
| `src/stack.rs` | Add `locals` field + 2 methods + update `restore()` | **Medium** — structural |
| `src/core/compose.rs` | **New** — compose tool + tests | **Low** — additive |
| `src/core/locals.rs` | **New** — 16 store/load tools + tests | **Low** — additive |
| `src/core/data.rs` | Add emptylist tool (12 lines) | **Low** — additive |
| `src/core/definition.rs` | Add describe tool (15 lines) | **Low** — additive |
| `src/core/mod.rs` | Wire 3 new modules + update CORE_PRIMITIVES | **Low** — additive |
| `src/analyzer.rs` | Add 19 stack-effect entries | **Low** — additive |

**Files NOT changed:** executor.rs, op.rs, value.rs, context.rs, tool.rs,
builtins.rs, lib.rs, effects.rs, lookahead.rs, error.rs, bin/kore.rs,
all cap/* files, all existing test files.

---

## Postulate Compliance

| Change | P1 (Everything is a Tool) | P2 (Stack → Stack) | P3 (Compose = Concat) |
|--------|--------------------------|--------------------|-----------------------|
| compose | ✅ Registered as Tool | ✅ (q1 q2 -- q3) | ✅ IS P3 itself |
| emptylist | ✅ Registered as Tool | ✅ ( -- list) | ✅ Composes freely |
| describe | ✅ Registered as Tool | ✅ (text -- text) | ✅ Composes freely |
| store/load | ✅ 16 separate Tools | ✅ storeN: (a --), loadN: (-- a) | ✅ Compose freely |

All changes are pure tool registrations. No new Op variants, no new Value
variants, no executor changes, no parser changes.

---

## Testing Strategy

Each new tool gets:
1. **Happy path** — basic functionality
2. **Edge cases** — empty inputs, boundary values
3. **Error paths** — wrong types, underflow
4. **Composition** — works with existing tools
5. **Integration** — existing 68 tests still pass

---
---

# Phase 1: Simulation Infrastructure

> **Goal:** Make Kore drivable by external search/training algorithms (MCTS, RL,
> beam search, LLM token generation). Add snapshot/restore, step execution,
> JSON serialization, and a session protocol over stdin/stdout.

> **Constraint:** All new code is host-side infrastructure. No changes to Kore
> semantics, no new tools, no new Value variants. The executor and tool system
> remain untouched.

---

## Architecture Analysis

The current executor is **functional**: `execute(&[Op], Stack, Context) -> (Stack, Context)`.
There is no mutable `Interpreter` struct, no program counter. Each call iterates
`for op in ops`. This is clean but doesn't support:

1. **Step execution** — can't pause mid-program  
2. **Snapshot/restore** — no single state object to capture  
3. **Incremental action append** — ops list is borrowed `&[Op]`  

**Solution:** Create a `Session` struct that owns `Vec<Op>`, tracks a program
counter, and provides step/snapshot/restore/fork methods. The session calls
`execute_op()` one op at a time instead of looping through all ops.

Key insight: `execute_op()` already exists and handles a single op. We just need
to manage the iteration externally instead of internally.

---

## Phase 1 Changes

### Implementation Checklist

#### 1. Session — Stateful Execution Engine
- [x] Create `src/session.rs` with `Session` struct
- [x] Fields: `ops: Vec<Op>`, `pc: usize`, `stack: Stack`, `ctx: Context`, `status: SessionStatus`
- [x] `Session::new(ops, stack, ctx)` constructor
- [x] `Session::step()` — execute one op, advance pc
- [x] `Session::step_n(n)` — execute up to n ops  
- [x] `Session::run()` — execute until halt or error
- [x] `Session::append_ops(ops)` — append ops (for incremental generation)
- [x] `Session::stack()` — observe current stack
- [x] `Session::status()` — check if running/halted/error
- [x] Tests: step, step_n, run, append, status transitions (7 tests)
- [x] Wire into `src/lib.rs` (pub mod + re-export)

#### 2. Snapshot/Restore/Fork
- [x] `Session::snapshot()` -> `Snapshot` (owned clone of pc + ops_len + stack + locals)
- [x] `Session::restore(snapshot)` — rewind to snapshot (truncates ops)
- [x] `Session::fork()` -> new `Session` (clone entire session)
- [x] `Snapshot` struct: `pc: usize`, `ops_len: usize`, `stack: Stack`, `steps: usize`
- [x] Tests: snapshot fidelity, restore rewinds, fork isolation, MCTS pattern (5 tests)

#### 3. Value JSON Serialization
- [x] `value_to_json(v: &Value) -> serde_json::Value` in `src/session.rs`
- [x] `stack_to_json(s: &Stack) -> serde_json::Value` — array of values
- [x] Round-trip correctness for: Null, Bool, Int, Float, Text, List, Map, Error
- [x] Quote → `{"__quote": "<ops_debug>"}` (not round-trippable, observation only)
- [x] Handle → `{"__handle": "kind:id"}` (observation only)
- [x] Tests: round-trip all types (18 tests)

#### 4. Serve Mode — Session Protocol
- [x] Add `--serve` flag to `src/bin/kore.rs`
- [x] JSON protocol over stdin/stdout (one JSON object per line)
- [x] Commands: `eval`, `session`, `push`, `step`, `step_n`, `run`
- [x] Commands: `compile_op`, `snap`, `restore`, `fork`, `drop`, `drop_snap`, `list`
- [x] Integration tested: eval, MCTS pattern, push+fork

#### 5. Integration
- [x] All existing tests still pass (508 total: 294 lib + 214 integration)
- [x] `cargo build` clean
- [x] `cargo clippy` clean (no new warnings)
- [ ] Commit on `phase1-simulation` branch

---

## 1. Session — Stateful Execution Engine

The core new struct. Wraps the existing functional executor into a stateful
object that external search algorithms can drive.

### File: `src/session.rs` (NEW)

```rust
pub enum SessionStatus {
    /// Ready to execute more ops
    Running,
    /// Reached end of ops (pc == ops.len())
    Halted,
    /// Hit a runtime error
    Error(String),
}

pub struct Session {
    /// The program (owned, appendable) 
    ops: Vec<Op>,
    /// Program counter
    pc: usize,
    /// Current stack state
    stack: Stack,
    /// Execution context (dict + caps + resources)
    ctx: Context,
    /// Current status
    status: SessionStatus,
    /// Step counter (total ops executed)
    steps: usize,
    /// Max steps before forced halt (0 = unlimited)
    max_steps: usize,
}
```

### Key methods

```rust
impl Session {
    /// Execute exactly one op. Returns the op that was executed.
    pub async fn step(&mut self) -> SessionStatus { ... }
    
    /// Execute up to n ops. Returns number actually executed.
    pub async fn step_n(&mut self, n: usize) -> (usize, SessionStatus) { ... }

    /// Execute until halt or error.
    pub async fn run(&mut self) -> SessionStatus { ... }

    /// Append ops (for incremental program building).
    /// If status was Halted, goes back to Running.
    pub fn append_ops(&mut self, ops: Vec<Op>) { ... }

    /// Snapshot current state.
    pub fn snapshot(&self) -> Snapshot { ... }
    
    /// Restore to a snapshot.
    pub fn restore(&mut self, snap: &Snapshot) { ... }
    
    /// Fork: create an independent clone.
    pub fn fork(&self) -> Session { ... }
    
    /// Observe the stack.
    pub fn stack(&self) -> &Stack { ... }
}
```

### Why this works with the existing executor

`execute_op()` in `executor.rs` already handles single ops:
```rust
async fn execute_op(op: &Op, stack: Stack, ctx: Context) -> Result<(Stack, Context)>
```

But it's currently private and takes owned `Stack` + `Context`. For `Session::step()`,
we call it directly. We need to make `execute_op` pub(crate) or expose it.

**Change to `src/executor.rs`:** Make `execute_op` `pub` (1 line change).

### Snapshot struct

```rust
pub struct Snapshot {
    pc: usize,
    stack: Stack,      // Clone of stack (includes locals)
    steps: usize,
}
```

Note: We do NOT snapshot the Context (dict/caps/resources). The dictionary is
shared via `Arc<RwLock>` and tools defined during execution persist across
snapshots. This matches MCTS semantics: the "game rules" (tools) don't change
between moves, only the "board state" (stack + pc) does.

Memory is also NOT snapshotted — it's session-level state behind `Arc<RwLock>`.
If the training use case needs memory isolation, use `fork()` instead.

---

## 2. Value JSON Serialization

For the Python controller to observe stack state and push initial values.

### Mapping

| Kore Value | JSON | Round-trip? |
|-----------|------|-------------|
| `Null` | `null` | ✅ |
| `Bool(b)` | `true`/`false` | ✅ |
| `Int(n)` | `n` | ✅ (if fits i64) |
| `Float(f)` | `f` | ✅ |
| `Text(s)` | `"s"` | ✅ |
| `List([...])` | `[...]` | ✅ |
| `Map({...})` | `{"__map": {...}}` | ✅ |
| `Error(e)` | `{"__error": {"code": "...", "message": "..."}}` | ✅ |
| `Quote(ops)` | `{"__quote": "[ op1 op2 ... ]"}` | ❌ (observation) |
| `Handle(h)` | `{"__handle": "kind:id"}` | ❌ (observation) |
| `Ext(e)` | `{"__ext": {"kind": N, "data": ...}}` | ❌ (observation) |

**Why `__map` prefix?** A plain JSON object `{}` is ambiguous — it could be a
Map value or a tagged type like Error. The `__` prefix signals "this is a typed
Kore value." Lists and primitives need no prefix since they're unambiguous.

### Implementation

Uses `serde_json::Value` (already in Cargo.toml). Hand-written conversion for
full control over the format. ~100 lines for `to_json` + `from_json`.

---

## 3. Serve Mode — Session Protocol

Extend the CLI binary with `--serve` mode. JSON over stdin/stdout, one
message per line (JSON Lines / NDJSON).

### Commands

```
→ {"cmd":"eval", "source":"3 5 add"}
← {"ok":true, "stack":[8], "steps":3}

→ {"cmd":"session", "id":"s1", "source":"", "max_steps":100000}
← {"ok":true, "id":"s1"}

→ {"cmd":"push", "id":"s1", "values":[[3,1,2]]}
← {"ok":true, "id":"s1", "stack":[[3,1,2]], "depth":1}

→ {"cmd":"compile_op", "id":"s1", "source":"dup"}
← {"ok":true, "id":"s1", "ops_added":1}

→ {"cmd":"step", "id":"s1"}
← {"ok":true, "id":"s1", "stack":[[3,1,2],[3,1,2]], "steps":1, "status":"running"}

→ {"cmd":"step_n", "id":"s1", "n":10}
← {"ok":true, "id":"s1", "stack":[...], "steps":10, "status":"running"}

→ {"cmd":"run", "id":"s1"}
← {"ok":true, "id":"s1", "stack":[...], "steps":42, "status":"halted"}

→ {"cmd":"snap", "id":"s1", "name":"checkpoint1"}
← {"ok":true, "id":"s1", "name":"checkpoint1"}

→ {"cmd":"restore", "id":"s1", "name":"checkpoint1"}
← {"ok":true, "id":"s1", "stack":[...], "status":"running"}

→ {"cmd":"fork", "id":"s1", "new_id":"s2"}
← {"ok":true, "id":"s2"}

→ {"cmd":"drop", "id":"s1"}
← {"ok":true}
```

### Serve State

```rust
struct ServeState {
    sessions: HashMap<String, Session>,
    snapshots: HashMap<(String, String), Snapshot>,  // (session_id, snap_name)
    ctx_template: Context,  // shared dict with builtins pre-loaded
}
```

### Backward Compatibility

Bare source strings (not JSON) still work as shorthand for `eval`:
```
→ 3 5 add
← {"ok":true, "stack":[8], "steps":3}
```

---

## Files Changed Summary

| File | Change | Risk |
|------|--------|------|
| `src/session.rs` | **New** — Session, Snapshot, SessionStatus, JSON helpers | **Low** — additive |
| `src/executor.rs` | Make `execute_op` pub | **Trivial** |
| `src/lib.rs` | Add `pub mod session` + re-exports | **Trivial** |
| `src/bin/kore.rs` | Add `--serve` mode | **Medium** — new binary mode |
| `Cargo.toml` | No changes (serde_json already present) | **None** |

**Files NOT changed:** stack.rs, value.rs, op.rs, context.rs, tool.rs,
core/*, cap/*, analyzer.rs, effects.rs. Zero semantic changes.

---

## Postulate Compliance

| Change | P1 (Tool) | P2 (Stack → Stack) | P3 (Compose = Concat) |
|--------|-----------|--------------------|-----------------------|
| Session | Host-side, not a Tool | Tools still S→S | step(A);step(B) = run(A·B) |
| Snapshot | Host-side observation | No tool changes | snap(A)+run(B) = run(A·B) |
| JSON | Serialization only | No tool changes | No semantic change |
| Serve | Protocol wrapper | Delegates to executor | compile_op = append |

All changes are infrastructure. Kore's execution semantics are untouched.
