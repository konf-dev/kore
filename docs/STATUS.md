# Kore OS Status

> **Current Version**: v0.2.0
> **Last Updated**: Session 5

## Summary

Kore is a feature-complete OS for LLM agents with **143 primitives**.

### Architecture (Cleaned Up)

**Single Source of Truth**: Every Tool has exactly 3 fields:
- `name: String` - the tool's identifier
- `body: ToolBody` - Native(fn) or Ops(vec)
- `meta: Meta` - all metadata (sig, doc, stats, health, tags)

No duplication. No legacy. Everything is a Thing with value + metadata.

## Primitives by Category (143 total)

### Execution (7)
- `call`, `try`, `if`, `loop`, `def`, `words`, `describe`

### Error Handling (4)
- `is-error`, `unwrap`, `assert`, `panic`

### Stack Manipulation (6)
- `dup`, `drop`, `swap`, `over`, `rot`, `depth`

### Arithmetic (6)
- `add`, `sub`, `mul`, `div`, `mod`, `neg`

### Comparison (6)
- `eq`, `neq`, `lt`, `gt`, `le`, `ge`

### Logic (3)
- `and`, `or`, `not`

### String (13)
- `str-len`, `str-get`, `str-slice`, `str-split`, `str-join`
- `str-concat`, `str-trim`, `str-find`, `str-starts`, `str-ends`
- `str-replace`, `char-code`, `code-char`

### List (10)
- `list-len`, `list-get`, `list-set`, `list-push`, `list-pop`
- `list-slice`, `list-concat`, `list-reverse`, `list-empty`, `collect`

### Map (7)
- `map-get`, `map-set`, `map-has`, `map-del`, `map-keys`, `map-vals`, `map-empty`

### Type Checking (9)
- `type-of`, `is-null`, `is-bool`, `is-int`, `is-float`
- `is-text`, `is-list`, `is-map`, `is-quote`

### Conversion (5)
- `to-int`, `to-float`, `to-text`, `to-bool`, `to-list`

### Combinators (6)
- `map`, `filter`, `fold`, `each`, `times`, `while`

### OS: File System (7)
- `fs-read`, `fs-write`, `fs-append`, `fs-exists`, `fs-list`, `fs-rm`, `fs-mkdir`

### OS: Process (1)
- `exec`

### OS: I/O (4)
- `print`, `println`, `read-line`, `log`

### OS: Time (2)
- `now`, `sleep`

### OS: Misc (2)
- `uuid`, `random`

### OS: System Info (5)
- `pid`, `cwd`, `args`, `exit`, `version`

### OS: Module Loading (1)
- `load`

### OS: Environment (2)
- `env-get`, `env-set`

### OS: HTTP (3)
- `http-get`, `http-post`, `http-request`

### Data: JSON (2)
- `json-parse`, `json-encode`

### Resources (5)
- `res-mem`, `res-rom`, `res-compute`, `res-net`, `res-all`

### Capabilities (4)
- `cap-has`, `cap-list`, `cap-fs`, `cap-net`

### Session Memory (5)
- `mem-set`, `mem-get`, `mem-del`, `mem-has`, `mem-keys`

### Persistent Storage (5)
- `rom-set`, `rom-get`, `rom-del`, `rom-has`, `rom-keys`

### Introspection (8) - NEW
- `meta` - get all metadata for a tool
- `meta!` - set metadata field
- `calls` - get immediate dependencies
- `graph` - get full call tree
- `tag` - add tag to tool
- `find-tag` - find tools by tag
- `health` - get unhealthy tools
- `stats` - get call statistics

### Persistence (5) - NEW
- `persist` - save tool to ROM only
- `register` - define AND persist tool
- `load-tools` - load from ROM into dictionary
- `unregister` - remove from dictionary and ROM
- `list-persisted` - list persisted tool names

## Source Files

| File | Lines | Purpose |
|------|-------|---------|
| builtins.rs | 2805 | All 143 primitives |
| value.rs | 389 | 10 value types |
| meta.rs | 357 | Metadata structure |
| capabilities.rs | 369 | Permission system |
| effect.rs | 331 | Type signatures (optional) |
| storage.rs | 325 | Persistent ROM |
| executor.rs | 299 | Execution loop |
| memory.rs | 274 | Session storage |
| op.rs | 245 | 4 operation types |
| resources.rs | 243 | Quota tracking |
| stack.rs | 212 | LIFO data structure |
| context.rs | 220 | Execution environment |
| tool.rs | 180 | Tool definition |
| error.rs | 120 | Error types |
| lib.rs | 89 | Public API |

**Total**: ~5,458 lines of Rust

## Tests

- **67** unit tests (lib)
- **62** integration tests
- **34** language semantics tests
- **68** primitives tests
- **231 total** - all passing

## What Was Cleaned Up (Session 5)

1. **Removed duplication in Tool**:
   - `doc: Option<String>` → use `meta.doc`
   - `effect: Option<Effect>` → use `meta.sig`

2. **Removed legacy APIs**:
   - `Context::has_capability()` → use `ctx.caps.has()`
   - `Context::with_capability()` → use `ctx.with_caps()`
   - `Tool::native_opt()` → use `Tool::native()`

3. **Simplified Tool API**:
   - `Tool::composed(name, sig, ops)` - sig is now `Option<&str>`
   - `tool.sig()` / `tool.doc()` - accessor methods
   - Single source of truth: `tool.meta`

## What's Next

### Phase 5: Network Layer (from ARCHITECTURE_V02.md)
- External communication (HTTP client already done)
- Agent-to-agent messaging (spawn, join, select, cancel)

### Phase 6: Self-Hosting
- Parser in Kore
- Executor in Kore
- Standard library in Kore

### Long-term: Emergent Collective Intelligence
- See `docs/EMERGENT_COLLECTIVE_INTELLIGENCE.md`
- Multiple agents sharing RAM/ROM
- Workflow evolution and learning
