# Kore Refactor Summary

> Quick reference for the machine-readable tool architecture refactor

---

## The Vision

**Before**: 143 builtins in one 3440-line file, mixed concerns, no formal contracts.

**After**: 
- 27 core primitives (hardcoded)
- ~60 stdlib tools (composed from core or native-pure)
- ~45 capability tools (with formal declarations)
- Every tool has machine-readable manifest
- LLMs can introspect any tool programmatically

---

## Key Documents

| Document | Purpose |
|----------|---------|
| [TOOL_ARCHITECTURE.md](TOOL_ARCHITECTURE.md) | Complete refactor specification |
| [MANIFEST_SPEC.md](MANIFEST_SPEC.md) | Machine-readable manifest format |
| [EXTENDED_FOUNDATIONS.md](EXTENDED_FOUNDATIONS.md) | Mathematical proofs for extensions |
| [REFACTOR_PLAN.md](REFACTOR_PLAN.md) | Earlier refactor notes (superseded) |

---

## Quick Reference

### Core Primitives (27)

```
EXECUTION   call spawn if loop              4
DEFINITION  def words                       2
ERROR       try fail is-error               3
STACK       dup drop swap rot depth         5
ARITHMETIC  add sub mul div mod neg         6
COMPARISON  eq lt                           2
LOGIC       and or not                      3
DATA        list unlist map-new             3
─────────────────────────────────────────────
TOTAL                                      27
```

### Stdlib (Composed)

```
Stack:    over nip tuck pick
Math:     abs min max
Compare:  gt le ge neq
Type:     type-of is-null is-bool is-int is-float is-text is-list is-map is-quote
Convert:  to-int to-float to-text to-bool to-list
Combos:   map filter fold each times while
```

### Stdlib (Native)

```
String:   str-len str-get str-slice str-split str-join str-concat 
          str-trim str-find str-starts str-ends str-replace char-code code-char
List:     list-len list-get list-set list-push list-pop list-slice 
          list-concat list-reverse list-first list-rest
Map:      map-get map-set map-has map-del map-keys map-vals
JSON:     json-parse json-encode
Algebra:  cap-leq cap-meet cap-join cap-attenuate res-split res-add res-has
```

### Capability Tools

```
Filesystem: fs-read fs-write fs-append fs-exists fs-list fs-rm fs-mkdir load
Network:    http-get http-post http-request
Process:    exec
I/O:        print println read-line log
Environment: env-get env-set
Time:       time-now time-sleep
Random:     rand-float rand-uuid
System:     sys-pid sys-cwd sys-args sys-version sys-exit
Memory:     mem-set mem-get mem-del mem-has mem-keys
Storage:    rom-set rom-get rom-del rom-has rom-keys
Resources:  res-avail res-cons res-mem res-rom res-compute res-net res-all
Caps:       cap-has cap-list cap-fs cap-net
Trace:      trace-on trace trace-step trace-fingerprint
Intro:      meta meta! calls graph tag find-tag health stats describe
Persist:    persist register load-tools unregister list-persisted
```

---

## Removals

| Remove | Reason |
|--------|--------|
| `map-empty` | Duplicate of `map-new` |
| `collect` | Duplicate of `list` |
| `panic` | Duplicate of `fail` |
| `assert` | Compose: `not [ msg fail ] [ ] if` |
| `unwrap` | Compose: `is-error [ "unwrap error" fail ] [ ] if` |

## Renames

| Old | New |
|-----|-----|
| `random` | `rand-float` |
| `uuid` | `rand-uuid` |
| `now` | `time-now` |
| `sleep` | `time-sleep` |
| `pid` | `sys-pid` |
| `cwd` | `sys-cwd` |
| `args` | `sys-args` |
| `exit` | `sys-exit` |
| `version` | `sys-version` |

---

## File Structure After Refactor

```
kore/
├── src/
│   ├── lib.rs
│   ├── value.rs          # 10 value types
│   ├── op.rs             # Push, Call
│   ├── stack.rs          # Stack
│   ├── executor.rs       # Core loop
│   ├── context.rs        # Execution context
│   ├── error.rs
│   ├── core/             # 27 primitives
│   │   ├── mod.rs
│   │   ├── execution.rs  # call, spawn, if, loop
│   │   ├── definition.rs # def, words
│   │   ├── error.rs      # try, fail, is-error
│   │   ├── stack.rs      # dup, drop, swap, rot, depth
│   │   ├── arithmetic.rs # add, sub, mul, div, mod, neg
│   │   ├── comparison.rs # eq, lt
│   │   ├── logic.rs      # and, or, not
│   │   └── data.rs       # list, unlist, map-new
│   ├── stdlib/           # Native stdlib
│   │   ├── mod.rs
│   │   ├── string.rs
│   │   ├── list.rs
│   │   ├── map.rs
│   │   └── json.rs
│   ├── cap/              # Capability tools
│   │   ├── mod.rs
│   │   ├── fs/
│   │   ├── net/
│   │   ├── io/
│   │   └── ...
│   ├── tool.rs           # Tool struct with manifest
│   ├── manifest.rs       # Manifest parsing
│   └── algebra.rs        # CapSet, Res, Trace
├── stdlib/               # Composed tools (.kore)
│   ├── prelude.kore
│   ├── stack.kore
│   ├── math.kore
│   ├── compare.kore
│   ├── type.kore
│   ├── convert.kore
│   └── combinators.kore
└── manifests/            # TOML manifests
    ├── core/
    ├── stdlib/
    └── cap/
```

---

## Implementation Order

### Week 1: Core Extraction

- [ ] Create `src/core/` module
- [ ] Move 27 primitives from builtins.rs
- [ ] Add manifest metadata to each
- [ ] Test everything still works

### Week 2: Stdlib

- [ ] Create `stdlib/` directory
- [ ] Write composed tools in .kore
- [ ] Move native stdlib to `src/stdlib/`
- [ ] Create stdlib loader

### Week 3: Capability Tools

- [ ] Create `src/cap/` structure
- [ ] Move filesystem tools
- [ ] Move network tools
- [ ] Move all other capability tools
- [ ] Add formal capability declarations

### Week 4: Manifest System

- [ ] Define TOML manifest schema
- [ ] Generate manifests for all tools
- [ ] Add introspection tools
- [ ] Delete old builtins.rs

---

## Design Principles

1. **Minimal Core**: Only what cannot be composed
2. **Everything Documented**: Machine-readable manifests
3. **Pure by Default**: Effects are exceptional, declared
4. **Explicit Capabilities**: Check before execute
5. **Consistent Naming**: `category-action` pattern
6. **No Duplicates**: One canonical tool per operation
7. **Composable**: Small tools that combine

---

## How LLMs Use This

**Before** (guessing):
```
LLM: "I'll try fs-read... error. Maybe I need a capability? Let me try again..."
```

**After** (informed):
```kore
"fs-read" manifest
# => { 
#   capabilities: ["fs:read:{path}"],
#   effects: { reads: ["filesystem"], may_fail: true },
#   errors: { CapabilityDenied: "...", NotFound: "...", IoError: "..." }
# }

# LLM now knows:
# 1. What capability to request
# 2. That it might fail  
# 3. What errors to handle
# 4. It has no hidden side effects
```

This enables **predictable, reliable** agent behavior.
