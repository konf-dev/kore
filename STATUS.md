# Kore Status

## Current State: Solid Foundation ✓

**Version**: 0.1.0  
**Branch**: `stable-v0.1-agent-working`  
**Tests**: 232 passing  
**Clippy**: Clean  

## What's Working

### Core (7 files, ~5,500 lines)

| Module | Lines | Purpose |
|--------|-------|---------|
| `builtins.rs` | 2,856 | 143 primitives |
| `meta.rs` | 357 | Unified metadata for tools |
| `executor.rs` | 314 | Core execution loop with stats tracking |
| `context.rs` | 240 | Execution environment |
| `stack.rs` | 150 | Stack with depth limit |
| `tool.rs` | 180 | Tool definition (3 fields) |
| `value.rs` | 400 | Value types |

### 143 Primitives by Category

| Category | Count | Examples |
|----------|-------|----------|
| Stack | 6 | `dup`, `drop`, `swap`, `over`, `rot`, `depth` |
| Arithmetic | 6 | `add`, `sub`, `mul`, `div`, `mod`, `neg` |
| Comparison | 6 | `eq`, `lt`, `gt`, `le`, `ge`, `neq` |
| Logic | 3 | `and`, `or`, `not` |
| Control | 5 | `if`, `call`, `times`, `each`, `while` |
| Combinators | 4 | `map`, `filter`, `fold`, `collect` |
| Lists | 12 | `list-get`, `list-set`, `list-push`, `list-pop`, etc. |
| Maps | 7 | `map-get`, `map-set`, `map-del`, `map-has`, `map-keys`, etc. |
| Strings | 15 | `str-concat`, `str-split`, `str-find`, `str-slice`, etc. |
| Types | 9 | `type`, `to-int`, `to-float`, `to-text`, `is-*` predicates |
| Error | 7 | `try`, `unwrap`, `is-error`, `error`, `throw`, `catch`, `assert` |
| Dictionary | 5 | `def`, `undef`, `tools`, `describe`, `source` |
| I/O | 4 | `print`, `println`, `debug`, `input` |
| Memory | 6 | `mem-set`, `mem-get`, `mem-del`, `mem-has`, `mem-keys`, `mem-clear` |
| Resources | 10 | quota queries and reservation |
| Capabilities | 4 | `can?`, `caps`, `require-cap` |
| File System | 6 | `fs-read`, `fs-write`, `fs-list`, `fs-exists`, etc. |
| HTTP | 3 | `http-get`, `http-post`, `http-request` |
| Process | 2 | `exec`, `exit` |
| Environment | 2 | `env-get`, `env-set` |
| Time | 4 | `now`, `sleep`, `time-fmt`, `time-parse` |
| JSON | 2 | `json-parse`, `json-encode` |
| Introspection | 8 | `meta`, `meta!`, `calls`, `graph`, `tag`, `find-tag`, `health`, `stats` |
| Persistence | 5 | `persist`, `register`, `load-tools`, `unregister`, `list-persisted` |

## Recent Fixes (This Session)

### 1. Stats Not Updating - FIXED
- **Issue**: `meta.record_call()` was never called in executor
- **Fix**: Added timing and stats update in `executor.rs` after each tool call
- **Test**: `test_stats_are_tracked` verifies stats increment

### 2. Persistence Key Mismatch - FIXED
- **Issue**: `persist` used `tools/` prefix but `load-tools` looked for `tools_`
- **Fix**: Changed `load-tools` and `list-persisted` to use `tools/`

### 3. Capability Errors Inconsistent - FIXED
- **Issue**: Capability denials used generic `Runtime` error instead of `CapabilityDenied`
- **Fix**: All 10 capability checks now use `Error::CapabilityDenied { capability, tool }`

### 4. Dictionary Missing get_mut - FIXED
- **Issue**: No way to update tool metadata in place
- **Fix**: Added `Dictionary::get_mut()` and `Dictionary::update_stats()`

## Design Decisions

### Streaming
HTTP operations are synchronous (await full response). This is intentional for stack-based semantics - values on the stack are complete.

### Stats Tracking
Stats are updated after every tool call via write lock on dictionary. This adds some overhead but provides accurate telemetry for introspection.

### Memory Semantics
`try` blocks share memory with parent context. Memory changes persist even if the block fails. This is documented behavior, not a bug.

## Known Limitations

1. **Dictionary Unbounded**: No limit on number of tools (could be fixed with tool count quota)
2. **No True Streaming**: Large HTTP responses are fully buffered
3. **Stats Lock Contention**: High-frequency calls may contend on write lock

## Test Coverage

```
src/lib.rs (kore)     68 tests
integration_tests.rs  62 tests
language_semantics.rs 34 tests
primitives.rs         68 tests
----------------------------------------
TOTAL                 232 tests
```

## Usage Example

```
# Define a tool
"square" [dup mul] def

# Add metadata
"square" "doc" "Square a number" meta!
"square" "math" tag

# Use it
5 square  # -> 25

# Introspect it
"square" meta    # -> {name: "square", doc: "Square a number", ...}
"square" stats   # -> {calls: 1, failures: 0, time_ms: 0}
"math" find-tag  # -> ["square"]
```
