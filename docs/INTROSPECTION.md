# Introspection & Persistence - Quick Reference

Everything is a **Thing** with value + metadata. Same interface for all.
No special treatment. Compose for power.

## The Meta Structure

Every tool carries its own metadata:

```
meta: {
  name: "square"          -- tool name
  sig: "(n -- n)"         -- type signature  
  doc: "Squares a number" -- documentation
  status: "ok"            -- "ok", "warn", "error"
  warnings: []            -- warning messages
  errors: []              -- error messages
  tags: ["math"]          -- discovery tags
  stats: {
    calls: 100            -- successful calls
    failures: 2           -- failed calls
    time_ms: 50           -- total execution time
    last_call: 1706...    -- last call timestamp
  }
  ...                     -- custom fields
}
```

## Introspection Primitives (8)

| Primitive | Signature | Description |
|-----------|-----------|-------------|
| `meta` | `(name -- map)` | Get all metadata |
| `meta!` | `(name key val -- )` | Set metadata field |
| `calls` | `(name -- list)` | Get immediate dependencies |
| `graph` | `(name -- tree)` | Get full call tree |
| `tag` | `(name tag -- )` | Add tag to tool |
| `find-tag` | `(tag -- list)` | Find tools with tag |
| `health` | `( -- list)` | Get unhealthy tools |
| `stats` | `(name -- map)` | Get call statistics |

## Persistence Primitives (5)

| Primitive | Signature | Description |
|-----------|-----------|-------------|
| `persist` | `(name quote -- )` | Save to ROM only |
| `register` | `(name quote -- )` | Define AND persist |
| `load-tools` | `( -- n)` | Load from ROM into dict |
| `unregister` | `(name -- )` | Remove from dict and ROM |
| `list-persisted` | `( -- list)` | List persisted tool names |

## Examples

### Self-Documenting Tool

```kore
# Define with documentation
"factorial" [
  dup 1 le 
  [drop 1]
  [dup 1 sub factorial mul]
  if
] register

# Add documentation
"factorial" "doc" "Compute n! recursively" meta!
"factorial" "math" tag
"factorial" "recursive" tag

# Query it
"factorial" meta    # -> full metadata map
"factorial" calls   # -> ["dup", "le", "drop", "sub", "factorial", "mul"]
"factorial" graph   # -> full call tree
```

### Discovery

```kore
# Find all math tools
"math" find-tag     # -> ["factorial", "square", "abs", ...]

# Check system health
health              # -> [{name: "broken", status: "error", ...}]

# Get performance stats
"factorial" stats   # -> {calls: 42, failures: 0, time_ms: 15}
```

### Persistence

```kore
# Tools survive restarts
"helpers/double" [2 mul] register

# On next session
load-tools          # -> 1 (one tool loaded)
5 helpers/double    # -> 10

# Remove completely
"helpers/double" unregister
```

## Design Philosophy

1. **Same interface for all** - Tools, workflows, data all use `meta`
2. **Self-documenting** - Metadata lives WITH the thing
3. **Discoverable** - Tags enable exploration  
4. **Observable** - Stats track automatically
5. **Composable** - Query metadata, transform it, act on it

## Status Tracking

Status auto-updates based on:
- `"ok"` - no warnings, no errors, failure rate < 10%
- `"warn"` - has warnings OR failure rate > 10%
- `"error"` - has errors

```kore
# Check health of the system
health [ 
  "name" map-get println
] each
```

## Total Primitives: 143

- 130 core primitives
- 8 introspection primitives
- 5 persistence primitives
