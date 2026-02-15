# Kore Upgrade Plan v3: Phase 0 — Language Uniformity

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
