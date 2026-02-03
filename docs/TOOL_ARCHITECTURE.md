# Kore Tool Architecture: Machine-Readable Design

> **Principle**: Everything is dumb. Just a pipeline. No magic.  
> Each tool does exactly 1 thing. Everything is composable.  
> Written for machines (LLMs), not humans.

---

## Executive Summary

This document specifies the complete refactoring of Kore's tool system into three tiers:

1. **CORE** (~27 primitives): Hardcoded in Rust, irreducible
2. **STDLIB** (~80 tools): Composed from core, written in .kore files
3. **CAPABILITY** (~40 tools): Require permissions, declared effects

Every tool is self-documenting with machine-readable manifests.

---

## Current State: 143 Builtins

| Category | Count | Status |
|----------|-------|--------|
| Execution | 7 | Keep 4 core |
| Error | 5 | Keep 3 core |
| Stack | 9 | Keep 5 core, 4 → stdlib |
| Arithmetic | 9 | Keep 6 core, 3 → stdlib |
| Comparison | 6 | Keep 2 core, 4 → stdlib |
| Logic | 3 | Keep all 3 core |
| String | 13 | → native stdlib |
| List | 12 | Keep 3 core, rest → stdlib |
| Map | 8 | Keep 1 core, rest → stdlib |
| Type | 9 | → stdlib |
| Conversion | 5 | → stdlib |
| Combinators | 6 | → stdlib |
| OS/FS | 7 | → capability |
| OS/Process | 1 | → capability |
| OS/IO | 4 | → capability |
| OS/Time | 2 | → capability |
| OS/Misc | 2 | → capability (rename) |
| OS/System | 5 | → capability (rename) |
| OS/Env | 2 | → capability |
| OS/Module | 1 | → capability |
| HTTP | 3 | → capability |
| JSON | 2 | → stdlib |
| Trace | 4 | → capability |
| Resources | 9 | → capability |
| Capabilities | 8 | → capability |
| Memory | 5 | → capability |
| Storage | 5 | → capability |
| Introspection | 8 | → capability |
| Persistence | 5 | → capability |

---

## Tier 1: CORE (27 Primitives)

These are **irreducible**. They cannot be composed from anything else.

### 1.1 Execution (4)

```
call   : (Quote -- ...)
         Execute quoted code on current stack.
         
spawn  : (Quote CapList ResList -- Handle)
         Execute in sandbox with attenuated caps/resources.
         Formal basis: capability lattice + resource monoid.
         
if     : (Bool ThenQ ElseQ -- ...)
         Execute ThenQ if true, ElseQ if false.
         
loop   : (BodyQ ExitQ -- ...)
         Execute BodyQ. If ExitQ leaves true on top, exit.
         Otherwise repeat.
```

### 1.2 Definition (2)

```
def    : (Name:Text Body:Quote -- )
         Register new tool in dictionary.
         
words  : ( -- Names:List)
         List all defined tool names.
```

### 1.3 Error (3)

```
try    : (Quote -- ...Values | Error)
         Execute quote. If error, push Error value instead.
         
fail   : (Message:Text -- !)
         Raise error with message. Never returns.
         
is-error : (Any -- Bool)
         Test if value is an Error.
```

### 1.4 Stack (5)

```
dup    : (a -- a a)
         Duplicate top element.
         
drop   : (a -- )
         Discard top element.
         
swap   : (a b -- b a)
         Swap top two elements.
         
rot    : (a b c -- b c a)
         Rotate top three: third to top.
         
depth  : ( -- n:Int)
         Push current stack depth.
```

### 1.5 Arithmetic (6)

```
add    : (Num Num -- Num)
         Addition. Works on Int or Float.
         
sub    : (Num Num -- Num)
         Subtraction: a b sub = a - b
         
mul    : (Num Num -- Num)
         Multiplication.
         
div    : (Num Num -- Num)
         Division. Error if divisor is 0.
         
mod    : (Int Int -- Int)
         Modulo: a b mod = a % b
         
neg    : (Num -- Num)
         Negate: -a
```

### 1.6 Comparison (2)

```
eq     : (Any Any -- Bool)
         Structural equality.
         
lt     : (Num Num -- Bool)
         Less than: a b lt = a < b
```

**Why only 2?** The rest compose:
- `gt  = swap lt`
- `le  = gt not`  
- `ge  = lt not`
- `neq = eq not`

### 1.7 Logic (3)

```
and    : (Bool Bool -- Bool)
         Logical AND.
         
or     : (Bool Bool -- Bool)
         Logical OR.
         
not    : (Bool -- Bool)
         Logical NOT.
```

### 1.8 Data (3)

```
list   : (n:Int -- List)
         Collect n items from stack into list.
         Items are in stack order (bottom = first).
         
unlist : (List -- ...items)
         Spread list items onto stack.
         
map-new : ( -- Map)
         Create empty map.
```

### 1.9 Total: 27 Core Primitives

```
EXECUTION:  call, spawn, if, loop           (4)
DEFINITION: def, words                       (2)
ERROR:      try, fail, is-error              (3)
STACK:      dup, drop, swap, rot, depth      (5)
ARITHMETIC: add, sub, mul, div, mod, neg     (6)
COMPARISON: eq, lt                           (2)
LOGIC:      and, or, not                     (3)
DATA:       list, unlist, map-new            (3)
────────────────────────────────────────────────
TOTAL                                        27
```

---

## Tier 2: STDLIB (Composed Tools)

### 2.1 File Structure

```
stdlib/
├── prelude.kore     # Auto-loaded: basic extensions
├── stack.kore       # over, nip, tuck, pick
├── math.kore        # abs, min, max, pow, sqrt
├── compare.kore     # gt, le, ge, neq
├── string.kore      # str-len, str-*, char-code, code-char
├── list.kore        # list-len, list-get, list-set, ...
├── map.kore         # map-get, map-set, map-has, map-del, ...
├── type.kore        # type-of, is-null, is-bool, ...
├── convert.kore     # to-int, to-float, to-text, to-bool
├── combinators.kore # map, filter, fold, each, times
├── algebra.kore     # cap-meet, cap-join, res-split (pure)
└── json.kore        # json-parse, json-encode
```

### 2.2 Native Stdlib

Some tools need native implementation for performance:

```
NATIVE STDLIB (implemented in Rust, categorized as stdlib):
- str-len, str-get, str-slice, str-split, str-join
- str-concat, str-trim, str-find, str-replace
- str-starts, str-ends, char-code, code-char
- json-parse, json-encode
```

These are pure functions (no effects) but run native code.

### 2.3 Example Compositions

**stdlib/stack.kore**
```kore
# over: (a b -- a b a)
"over" [ swap dup rot swap ] def

# nip: (a b -- b)
"nip" [ swap drop ] def

# tuck: (a b -- b a b)
"tuck" [ swap over ] def

# pick: (i -- a) Copy ith element (0 = top)
"pick" [
  1 add       # Adjust for 0-indexing
  depth swap  # Get stack depth
  # TODO: implement using rot/swap loop
] def
```

**stdlib/compare.kore**
```kore
# gt: (a b -- Bool) Greater than
"gt" [ swap lt ] def

# le: (a b -- Bool) Less or equal
"le" [ gt not ] def

# ge: (a b -- Bool) Greater or equal
"ge" [ lt not ] def

# neq: (a b -- Bool) Not equal
"neq" [ eq not ] def
```

**stdlib/math.kore**
```kore
# abs: (n -- |n|)
"abs" [ dup 0 lt [ neg ] [ ] if ] def

# min: (a b -- min)
"min" [ over over lt [ drop ] [ swap drop ] if ] def

# max: (a b -- max)
"max" [ over over gt [ drop ] [ swap drop ] if ] def
```

**stdlib/combinators.kore**
```kore
# each: (list quote -- ) Execute quote for each item
"each" [
  # list quote
  swap                    # quote list
  [ dup list-empty not ]  # condition: list not empty
  [
    dup list-first        # quote list item
    rot dup               # list item quote quote
    rot swap              # list quote item quote
    call                  # list quote (after executing)
    swap list-rest swap   # (rest) quote
  ]
  loop
  drop drop               # clean up
] def

# map: (list quote -- list')
"map" [
  swap list-empty swap    # result list quote
  rot                     # quote result list
  [ dup list-empty not ]
  [
    dup list-first        # quote result list item
    rot rot rot           # list item quote result
    rot dup rot           # list quote item quote result
    swap call             # list quote result item'
    swap list-push        # list quote result'
    rot                   # quote result' list
    list-rest             # quote result' rest
    rot rot               # rest quote result'
  ]
  loop
  rot drop swap drop      # result'
] def
```

---

## Tier 3: CAPABILITY TOOLS

### 3.1 Capability Categories

| Category | Capability | Tools |
|----------|------------|-------|
| Filesystem | `fs:read:{path}`, `fs:write:{path}` | fs-read, fs-write, fs-list, ... |
| Network | `net:http`, `net:connect:{host}:{port}` | http-get, http-post, ... |
| Process | `exec` | exec |
| Environment | `env:read`, `env:write` | env-get, env-set |

### 3.2 Effect Categories

| Effect | Description | Tools |
|--------|-------------|-------|
| `io:stdout` | Writes to stdout | print, println |
| `io:stderr` | Writes to stderr | log |
| `io:stdin` | Reads from stdin | read-line |
| `time:read` | Reads system time | time-now |
| `time:wait` | Blocks execution | time-sleep |
| `rand` | Nondeterministic | rand-float, rand-uuid |
| `system:read` | Reads system info | sys-pid, sys-cwd, sys-args |
| `system:exit` | Terminates process | sys-exit |
| `mem:write` | Modifies session memory | mem-set, mem-del |
| `mem:read` | Reads session memory | mem-get, mem-has, mem-keys |
| `rom:write` | Modifies persistent storage | rom-set, rom-del |
| `rom:read` | Reads persistent storage | rom-get, rom-has, rom-keys |

### 3.3 Naming Convention

```
OLD NAME      NEW NAME       REASON
─────────────────────────────────────
random        rand-float     Category prefix
uuid          rand-uuid      Category prefix
now           time-now       Category prefix
sleep         time-sleep     Category prefix
pid           sys-pid        Category prefix
cwd           sys-cwd        Category prefix
args          sys-args       Category prefix
exit          sys-exit       Category prefix
version       sys-version    Category prefix
```

### 3.4 Tool Directory Structure

```
src/cap/
├── mod.rs              # register_capability_tools()
│
├── fs/
│   ├── mod.rs
│   ├── read.rs         # fs-read
│   ├── write.rs        # fs-write
│   ├── append.rs       # fs-append
│   ├── exists.rs       # fs-exists
│   ├── list.rs         # fs-list
│   ├── rm.rs           # fs-rm
│   ├── mkdir.rs        # fs-mkdir
│   └── load.rs         # load (execute .kore file)
│
├── net/
│   ├── mod.rs
│   ├── http_get.rs     # http-get
│   ├── http_post.rs    # http-post
│   └── http_request.rs # http-request
│
├── process/
│   ├── mod.rs
│   └── exec.rs         # exec
│
├── io/
│   ├── mod.rs
│   ├── print.rs        # print
│   ├── println.rs      # println
│   ├── read_line.rs    # read-line
│   └── log.rs          # log
│
├── env/
│   ├── mod.rs
│   ├── get.rs          # env-get
│   └── set.rs          # env-set
│
├── time/
│   ├── mod.rs
│   ├── now.rs          # time-now
│   └── sleep.rs        # time-sleep
│
├── rand/
│   ├── mod.rs
│   ├── float.rs        # rand-float
│   └── uuid.rs         # rand-uuid
│
├── system/
│   ├── mod.rs
│   ├── pid.rs          # sys-pid
│   ├── cwd.rs          # sys-cwd
│   ├── args.rs         # sys-args
│   ├── version.rs      # sys-version
│   └── exit.rs         # sys-exit
│
├── mem/
│   ├── mod.rs
│   ├── set.rs          # mem-set
│   ├── get.rs          # mem-get
│   ├── del.rs          # mem-del
│   ├── has.rs          # mem-has
│   └── keys.rs         # mem-keys
│
├── rom/
│   ├── mod.rs
│   ├── set.rs          # rom-set
│   ├── get.rs          # rom-get
│   ├── del.rs          # rom-del
│   ├── has.rs          # rom-has
│   └── keys.rs         # rom-keys
│
├── resource/
│   ├── mod.rs
│   ├── avail.rs        # res-avail
│   ├── consume.rs      # res-cons
│   └── info.rs         # res-mem, res-rom, res-compute, res-net, res-all
│
├── capability/
│   ├── mod.rs
│   ├── has.rs          # cap-has
│   ├── list.rs         # cap-list
│   ├── fs.rs           # cap-fs
│   └── net.rs          # cap-net
│
├── trace/
│   ├── mod.rs
│   ├── on.rs           # trace-on
│   ├── get.rs          # trace
│   ├── step.rs         # trace-step
│   └── fingerprint.rs  # trace-fingerprint
│
├── introspection/
│   ├── mod.rs
│   ├── meta.rs         # meta
│   ├── meta_set.rs     # meta!
│   ├── calls.rs        # calls
│   ├── graph.rs        # graph
│   ├── tag.rs          # tag
│   ├── find_tag.rs     # find-tag
│   ├── health.rs       # health
│   └── stats.rs        # stats
│
└── persistence/
    ├── mod.rs
    ├── persist.rs      # persist
    ├── register.rs     # register
    ├── load_tools.rs   # load-tools
    ├── unregister.rs   # unregister
    └── list_persisted.rs # list-persisted
```

---

## Machine-Readable Manifest Format

### 4.1 TOML Schema

Every tool has a manifest (embedded or file):

```toml
[tool]
name = "fs-read"
version = "0.1.0"
tier = "capability"      # "core" | "stdlib" | "capability"

[signature]
input = ["path:Text"]
output = ["contents:Text"]
# Stack effect: (path:Text -- contents:Text)

[requirements]
capabilities = ["fs:read:{path}"]
# {path} is interpolated from input

[resources]
compute = 1
mem = "dynamic"          # Depends on output size
# rom = 0 (default)
# net = 0 (default)

[effects]
reads = ["filesystem"]
writes = []
nondeterministic = false
may_block = true
may_fail = true

[errors]
CapabilityDenied = "Missing fs:read capability for {path}"
IoError = "Could not read {path}: {reason}"
NotFound = "File not found: {path}"

[docs]
brief = "Read entire file contents as UTF-8 text."
long = """
Reads the file at `path` and returns its contents as Text.
Fails if file doesn't exist, isn't readable, or isn't valid UTF-8.
"""
example = '''
"/etc/hostname" fs-read println
'''
see_also = ["fs-write", "fs-exists"]
```

### 4.2 Rust Embedding

```rust
pub fn fs_read() -> Tool {
    Tool::native("fs-read", "(path:Text -- contents:Text)", |mut stack, ctx| {
        Box::pin(async move {
            let path = stack.pop()?.into_text()?;
            
            // Capability check
            if !ctx.caps.can_read_path(Path::new(&path)) {
                return Err(Error::CapabilityDenied {
                    capability: format!("fs:read:{}", path),
                    tool: "fs-read".into(),
                });
            }
            
            // Resource tracking
            ctx.consume_compute(1)?;
            
            // Actual operation
            let contents = tokio::fs::read_to_string(&path).await?;
            
            // Track memory usage
            ctx.consume_mem(contents.len())?;
            
            stack.push(Value::Text(contents))?;
            Ok((stack, ctx))
        })
    })
    .with_tier(Tier::Capability)
    .with_capability("fs:read:{path}")
    .with_effect(Effect::Read("filesystem"))
    .with_error("CapabilityDenied", "Missing fs:read capability")
    .with_error("IoError", "Could not read file")
}
```

### 4.3 Query API

LLMs can introspect any tool:

```kore
# Get full manifest
"fs-read" manifest
# => { name: "fs-read", tier: "capability", signature: {...}, ... }

# Quick checks
"fs-read" is-pure       # => false
"add" is-pure           # => true

"fs-read" required-caps # => ["fs:read:{path}"]
"add" required-caps     # => []

"exec" effects          # => ["process:spawn"]
"dup" effects           # => []

# Find tools by property
[ "fs" ] find-by-cap    # => ["fs-read", "fs-write", ...]
[ "pure" ] find-by-tag  # => ["add", "mul", "dup", ...]
```

---

## Removal List

### 5.1 Duplicates to Remove

| Remove | Keep | Reason |
|--------|------|--------|
| `map-empty` | `map-new` | Same function |
| `collect` | `list` | Same function |
| `panic` | `fail` | Same function |
| `assert` | *compose* | `[ fail ] [ drop ] if` |
| `unwrap` | *compose* | `dup is-error [ ... fail ] [ ] if` |

### 5.2 Moves to Stdlib

| Tool | Compose From |
|------|--------------|
| `over` | `swap dup rot swap` |
| `nip` | `swap drop` |
| `tuck` | `swap over` |
| `pick` | loop with rot |
| `gt` | `swap lt` |
| `le` | `gt not` |
| `ge` | `lt not` |
| `neq` | `eq not` |
| `abs` | `dup 0 lt [ neg ] [ ] if` |
| `min` | compare + drop |
| `max` | compare + drop |

### 5.3 Renames

| Old | New | Reason |
|-----|-----|--------|
| `random` | `rand-float` | Category prefix |
| `uuid` | `rand-uuid` | Category prefix |
| `now` | `time-now` | Category prefix |
| `sleep` | `time-sleep` | Category prefix |
| `pid` | `sys-pid` | Category prefix |
| `cwd` | `sys-cwd` | Category prefix |
| `args` | `sys-args` | Category prefix |
| `exit` | `sys-exit` | Category prefix |
| `version` | `sys-version` | Category prefix |

---

## Implementation Phases

### Phase 1: Core Extraction (3 days)

1. Create `src/core/` module
2. Move 27 primitives from builtins.rs
3. Each in own file with manifest
4. Test: existing programs still work

### Phase 2: Stdlib Files (3 days)

1. Create `stdlib/` directory
2. Write composed tools in Kore
3. Create stdlib loader
4. Test: stdlib tools work

### Phase 3: Native Stdlib (2 days)

1. Move string ops to `src/stdlib/string.rs`
2. Move JSON ops to `src/stdlib/json.rs`
3. Mark as tier=stdlib but native
4. Test: performance maintained

### Phase 4: Capability Separation (3 days)

1. Create `src/cap/` structure
2. Move all capability tools
3. Add formal declarations
4. Test: capabilities enforced

### Phase 5: Manifest System (2 days)

1. Create manifest parser
2. Generate manifests for all tools
3. Add introspection tools
4. Test: LLM can query any tool

### Phase 6: Cleanup (1 day)

1. Delete old builtins.rs
2. Update documentation
3. Remove duplicates
4. Final testing

---

## Success Metrics

| Metric | Target |
|--------|--------|
| Core primitives | ≤ 30 |
| Lines in executor.rs | ≤ 50 |
| Duplicated tools | 0 |
| Tools without manifest | 0 |
| Undeclared capabilities | 0 |
| Hidden effects | 0 |

---

## Appendix: Complete Tool Inventory

### A.1 Core (27)

```
call spawn if loop
def words
try fail is-error
dup drop swap rot depth
add sub mul div mod neg
eq lt
and or not
list unlist map-new
```

### A.2 Stdlib: Composed (~40)

```
# Stack
over nip tuck pick

# Math
abs min max

# Compare
gt le ge neq

# Type
type-of is-null is-bool is-int is-float is-text is-list is-map is-quote

# Convert
to-int to-float to-text to-bool to-list

# Combinators
map filter fold each times while
```

### A.3 Stdlib: Native (~20)

```
# String
str-len str-get str-slice str-split str-join str-concat
str-trim str-find str-starts str-ends str-replace
char-code code-char

# List
list-len list-get list-set list-push list-pop
list-slice list-concat list-reverse list-first list-rest

# Map  
map-get map-set map-has map-del map-keys map-vals

# JSON
json-parse json-encode

# Algebra (pure)
cap-leq cap-meet cap-join cap-attenuate
res-split res-add res-has
```

### A.4 Capability (~45)

```
# Filesystem (cap: fs)
fs-read fs-write fs-append fs-exists fs-list fs-rm fs-mkdir load

# Network (cap: net)
http-get http-post http-request

# Process (cap: exec)
exec

# I/O (effect: io)
print println read-line log

# Environment (cap: env)
env-get env-set

# Time (effect: time)
time-now time-sleep

# Random (effect: rand)
rand-float rand-uuid

# System (effect: sys)
sys-pid sys-cwd sys-args sys-version sys-exit

# Memory (effect: mem)
mem-set mem-get mem-del mem-has mem-keys

# Storage (effect: rom)
rom-set rom-get rom-del rom-has rom-keys

# Resources
res-avail res-cons res-mem res-rom res-compute res-net res-all

# Capabilities
cap-has cap-list cap-fs cap-net

# Trace
trace-on trace trace-step trace-fingerprint

# Introspection (effect: introspect)
meta meta! calls graph tag find-tag health stats words describe

# Persistence (effect: persist)
persist register load-tools unregister list-persisted
```

---

## Appendix: Design Decisions

### Why 2 comparison ops instead of 6?

The others compose trivially:
- `a > b` = `b < a` = `swap lt`
- `a ≥ b` = `¬(a < b)` = `lt not`
- `a ≤ b` = `¬(a > b)` = `swap lt not`
- `a ≠ b` = `¬(a = b)` = `eq not`

An LLM can discover these compositions. Keeping all 6 adds no expressiveness.

### Why `or` if De Morgan's law works?

`or = not and not` is confusing:
- `a or b = not(not(a) and not(b))`
- This requires: `swap not swap not and not`

Too complex. Keep `or` for clarity.

### Why native stdlib for strings?

String operations implemented in Kore would be:
1. Very slow (character-by-character loops)
2. Very verbose (many lines for str-split)
3. Error-prone (unicode handling)

Native Rust gives us correctness and performance with the same pure semantics.

### Why separate capability and effect?

**Capability**: Permission needed before execution
- Static: Can be checked at spawn time
- Example: `fs:read:/home/user`

**Effect**: Observable behavior during execution  
- Dynamic: Happens when tool runs
- Example: `io:stdout`

A tool might need no capability but have effects (`print`).
A tool might need capability but be "pure" in effect terms (`fs-read` returns a value).
