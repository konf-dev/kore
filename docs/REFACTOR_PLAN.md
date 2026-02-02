# Kore Refactor Plan: Proof-Based Architecture

> **Branch:** `proof-based-refactor`
> **Goal:** Strip Kore to its essential core following the Three Postulates

---

## Analysis: What Violates the Postulates

### Postulate 1: Everything is a Tool

| Item | Violation | Action |
|------|-----------|--------|
| `Op::If` | Special primitive, not a tool | **Remove** - make `if` a tool |
| `Op::Quote` | Really just `Push(Value::Quote(...))` | **Remove** - use Push |
| `memory.rs` | Ambient state, not accessible as tool | **Rethink** - if needed, expose via tools |
| `storage.rs` | Ambient state, not accessible as tool | **Rethink** - if needed, expose via tools |
| `meta.rs` | Stats/health tracked magically | **Simplify** - metadata is just data |
| Capability checks | Hidden in tool implementations | **Make tool** - `cap-check` as explicit tool |
| Resource tracking | Hidden in memory/storage | **Make tool** - `res-consume` as explicit tool |

### Postulate 2: Tools Transform the Stack

| Item | Violation | Action |
|------|-----------|--------|
| `Context` mutation | Tools mutate context, not just stack | **Rethink** - context changes via explicit tools |
| `dict.register()` | Side effect, not stack-based | **OK** - configuration at init time |
| Print/log tools | Side effects without stack representation | **OK** - declared effects |

### Postulate 3: Composition is Concatenation

| Item | Violation | Action |
|------|-----------|--------|
| Everything | ✅ Already follows this | **Keep** |

---

## What Gets Removed

### Docs to Remove (Dead Weight)

| File | Reason |
|------|--------|
| `ARCHITECTURE.md` | Vision doc, outdated, 991 lines of speculation |
| `ARCHITECTURE_V02.md` | Another vision doc |
| `MODULES.md` | Complex module hierarchy we won't build |
| `PLAN.md` | 7-week plan we won't follow |
| `CONTEXT_FLOW.md` | Over-engineered |
| `AGENT_ARCHITECTURE.md` | Speculative |
| `EXPERIMENT_SYSTEM_PLAN.md` | Not needed |
| `SELF_HOSTING_ROADMAP.md` | Future speculation |
| `LITERATURE_SURVEY.md` | Background, not core doc |
| `RESEARCH_SYNTHESIS.md` | Background, not core doc |
| `EMERGENT_COLLECTIVE_INTELLIGENCE.md` | Philosophy, not implementation |

### Docs to Keep/Update

| File | Reason |
|------|--------|
| `PHILOSOPHY.md` | **Keep** - core principles |
| `POSTULATES.md` | **Keep** - inviolable axioms |
| `FORMAL_FOUNDATIONS.md` | **Keep** - mathematical basis |
| `FORMAL_INSIGHTS.md` | **Keep** - derived insights |
| `STATUS.md` | **Rewrite** - reflect new state |
| `PRIMITIVES.md` | **Rewrite** - minimal primitive set |
| `INTROSPECTION.md` | **Keep if useful** |

### Code to Simplify

| File | Current | After |
|------|---------|-------|
| `op.rs` | 4 variants: Push, Call, Quote, If | 2 variants: Push, Call |
| `builtins.rs` | 143 tools | ~40 essential tools |
| `meta.rs` | Complex stats/health tracking | Simple key-value metadata |
| `memory.rs` | 275 lines | Remove or simplify to tools |
| `storage.rs` | 326 lines | Remove or simplify to tools |
| `effect.rs` | Complex type system | Simpler stack effects |

---

## The Minimal Core

### Two Operations (That's It)

```rust
pub enum Op {
    /// Push a value onto the stack
    Push(Value),
    
    /// Call a tool by name
    Call(String),
}
```

Everything else is a tool:
- `if` is a tool: `( bool then:Quote else:Quote -- ... )`
- Quoting is just `Push(Value::Quote(...))`

### Ten Value Types (Unchanged)

```rust
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    List(Vec<Value>),
    Map(IndexMap<String, Value>),
    Quote(Vec<Op>),
    Handle(Handle),
    Error(Box<ErrorValue>),
}
```

### Essential Tools (~40)

#### Execution (4)
```
call      ( quote -- ... )        Execute a quote
try       ( quote -- result )     Execute, capture errors
if        ( bool then else -- )   Conditional
def       ( name quote -- )       Define a tool
```

#### Stack (6)
```
dup       ( a -- a a )
drop      ( a -- )
swap      ( a b -- b a )
over      ( a b -- a b a )
rot       ( a b c -- b c a )
depth     ( -- n )
```

#### Arithmetic (6)
```
add sub mul div mod neg
```

#### Comparison (6)
```
eq neq lt gt le ge
```

#### Logic (3)
```
and or not
```

#### Data: List (6)
```
list-empty list-len list-get list-push list-concat collect
```

#### Data: Map (5)
```
map-empty map-get map-set map-has map-keys
```

#### Data: Text (4)
```
str-len str-concat str-split str-join
```

#### Type (3)
```
type-of is-error unwrap
```

#### Introspection (3)
```
words describe meta
```

### Capability Tools (NEW)

```
cap-has       ( cap:Text -- bool )          Check if we have capability
cap-list      ( -- caps:List )              List all capabilities
cap-leq       ( c1:List c2:List -- bool )   Check c1 ⊆ c2
cap-attenuate ( requested:List -- result )  Attenuate (returns error if invalid)
```

### Resource Tools (NEW)

```
res-status    ( -- map )                    Current resource status
res-check     ( amount:Map -- bool )        Can we afford this?
res-consume   ( amount:Map -- )             Consume resources (or error)
res-split     ( amount:Map -- child:Map )   Split for spawn (or error)
```

### Concurrency Tools (NEW)

```
spawn         ( quote caps res -- handle )  Spawn with attenuated caps/res
await         ( handle -- result )          Wait for completion
channel       ( -- sender receiver )        Create channel pair
send          ( value sender -- )           Send to channel
recv          ( receiver -- value )         Receive from channel
```

### Trace Tools (NEW)

```
trace-start   ( -- )                        Begin recording
trace-stop    ( -- trace )                  Stop, return trace
trace-hash    ( trace -- hash )             Cryptographic fingerprint
```

### I/O Tools (Optional, Capability-Gated)

```
print         ( value -- )                  [io]
fs-read       ( path -- text )              [fs:read:path]
fs-write      ( path text -- )              [fs:write:path]
http-get      ( url -- response )           [net:connect:host]
exec          ( cmd -- output )             [exec]
```

---

## The Executor (Simplified)

```rust
pub async fn execute(ops: &[Op], mut stack: Stack, ctx: Context) -> Result<(Stack, Context)> {
    for op in ops {
        match op {
            Op::Push(value) => {
                stack.push(value.clone())?;
            }
            Op::Call(name) => {
                let tool = ctx.dict.get(name)?;
                (stack, ctx) = tool.execute(stack, ctx).await?;
            }
        }
    }
    Ok((stack, ctx))
}
```

That's the entire execution loop. Everything else is tools.

---

## Context (Simplified)

```rust
pub struct Context {
    /// Tool dictionary (shared)
    pub dict: Arc<RwLock<Dictionary>>,
    
    /// Capabilities (immutable after creation)
    pub caps: CapabilitySet,
    
    /// Resources (mutable, tracked)
    pub resources: ResourceSet,
    
    /// Trace (if recording)
    pub trace: Option<Arc<RwLock<Trace>>>,
}
```

No `memory`, no `storage` as built-in. If you want key-value storage, it's a tool.

---

## Configuration = Initial State

```rust
// Minimal: just the core
let ctx = Context::minimal();

// Standard: core + common tools
let ctx = Context::standard();

// Custom: pick your tools
let mut ctx = Context::new(caps, resources);
ctx.register_core();        // stack, arithmetic, logic
ctx.register_data();        // list, map, text
ctx.register_io();          // print, fs, http (if caps allow)
ctx.register_spawn();       // concurrency
```

Different behaviors = different initial tool sets.

---

## File Structure (After Refactor)

```
kore/
├── Cargo.toml
├── src/
│   ├── lib.rs           # Public API
│   ├── value.rs         # 10 value types
│   ├── op.rs            # 2 operations: Push, Call
│   ├── stack.rs         # Stack operations
│   ├── tool.rs          # Tool trait
│   ├── context.rs       # Execution context
│   ├── executor.rs      # The ~10 line core
│   ├── error.rs         # Error types
│   ├── capability.rs    # Capability lattice
│   ├── resource.rs      # Resource algebra
│   ├── trace.rs         # Execution trace
│   └── tools/           # Tool implementations
│       ├── mod.rs
│       ├── core.rs      # call, if, def
│       ├── stack.rs     # dup, drop, swap, etc
│       ├── math.rs      # add, sub, mul, etc
│       ├── data.rs      # list, map, text
│       ├── caps.rs      # cap-has, cap-attenuate
│       ├── resource.rs  # res-status, res-consume
│       ├── spawn.rs     # spawn, await, channel
│       ├── trace.rs     # trace-start, trace-stop
│       └── io.rs        # print, fs, http, exec
├── docs/
│   ├── PHILOSOPHY.md    # The five principles
│   ├── POSTULATES.md    # The three axioms
│   ├── FORMAL_FOUNDATIONS.md
│   ├── PRIMITIVES.md    # Tool reference
│   └── README.md        # Getting started
└── tests/
    └── ...
```

---

## Implementation Order

### Phase 1: Strip to Core
1. Remove `Op::If` and `Op::Quote` (make them tools)
2. Simplify executor to Push/Call only
3. Remove `memory.rs`, `storage.rs`, `meta.rs` (or gut them)
4. Delete outdated docs

### Phase 2: Add Proof-Based Primitives
1. Implement `CapabilitySet` with lattice operations
2. Implement `ResourceSet` with split/consume
3. Implement `Trace` with semantic events

### Phase 3: Core Tools
1. Implement `if` as a tool (requires executor access)
2. Implement capability tools
3. Implement resource tools
4. Implement trace tools

### Phase 4: Concurrency
1. Implement `spawn` with proper attenuation
2. Implement `await`
3. Implement channels

### Phase 5: Validate
1. Write property tests for monoid/lattice laws
2. Test spawn safety (can't escalate privileges)
3. Test resource conservation (split preserves total)
4. Test trace determinism

---

## Success Criteria

After refactor, these must be true:

1. **Postulate 1:** Every feature is a tool (no special primitives except Push/Call)
2. **Postulate 2:** Every tool transforms Stack → Stack
3. **Postulate 3:** Composition is concatenation

4. **Safety:** `spawn` cannot give more capabilities than parent has
5. **Conservation:** Resources split, not created
6. **Auditability:** Traces are semantic and hashable
7. **Minimal:** Under 2000 lines of core code
8. **Elegant:** A programmer can understand the whole system in an hour

---

## What We Lose (And Why That's OK)

| Feature | Why Remove |
|---------|-----------|
| 143 primitives | Most are conveniences, not essentials |
| Session memory | Use a storage tool if needed |
| Persistent storage | Use a storage tool if needed |
| Complex meta/stats | Introspection tools are enough |
| Capability checking in each tool | Centralize in cap-check |
| Resource tracking in memory/storage | Centralize in res-consume |

**Philosophy:** It's better to have 40 tools that each do one thing perfectly than 143 tools where responsibilities overlap.

---

## Next Steps

1. ✅ Create this plan
2. ⬜ Delete outdated docs
3. ⬜ Simplify `op.rs` to Push/Call
4. ⬜ Implement `if` as a tool
5. ⬜ Implement capability lattice
6. ⬜ Implement resource algebra
7. ⬜ Implement trace
8. ⬜ Implement spawn with safety
9. ⬜ Validate with tests
10. ⬜ Update remaining docs
