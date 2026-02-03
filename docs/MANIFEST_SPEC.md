# Kore Tool Manifest Specification

> Version 0.1.0 | Machine-readable tool declarations

---

## Purpose

Every Kore tool must have a manifest that describes:

1. **What it does** (signature)
2. **What it needs** (capabilities, resources)  
3. **What it produces** (effects, outputs)
4. **How it fails** (error conditions)

This allows LLMs to:
- Understand any tool without trial and error
- Compose tools safely
- Predict capability requirements before execution
- Handle errors appropriately

---

## Schema (TOML)

```toml
[tool]
name = "tool-name"           # Required: Unique identifier
version = "0.1.0"            # Required: Semver
tier = "core"                # Required: "core" | "stdlib" | "capability"
deprecated = false           # Optional: Default false
superseded_by = ""           # Optional: Replacement tool name

[signature]
input = ["a:Type", "b:Type"] # Required: Stack inputs (bottom to top)
output = ["c:Type"]          # Required: Stack outputs (bottom to top)
variadic_input = false       # Optional: Takes variable inputs
variadic_output = false      # Optional: Produces variable outputs

[requirements]
capabilities = []            # Optional: List of capability patterns
min_stack_depth = 0          # Optional: Minimum stack depth needed

[resources]
mem = 0                      # Optional: Memory units consumed
rom = 0                      # Optional: Storage units consumed  
compute = 1                  # Optional: Compute units consumed
net = 0                      # Optional: Network units consumed
# Use "dynamic" for variable consumption

[effects]
reads = []                   # Optional: What it reads from
writes = []                  # Optional: What it writes to
nondeterministic = false     # Optional: Same input may give different output
may_block = false            # Optional: May wait for external event
may_fail = false             # Optional: Can raise error
terminates = true            # Optional: Always returns

[errors]
# ErrorName = "description with {interpolation}"

[docs]
brief = ""                   # Required: One-line description
long = ""                    # Optional: Full documentation
example = ""                 # Optional: Usage example
see_also = []                # Optional: Related tools
```

---

## Type System

### Primitive Types

| Type | Description | Examples |
|------|-------------|----------|
| `Null` | Absence of value | `null` |
| `Bool` | Boolean | `true`, `false` |
| `Int` | 64-bit integer | `42`, `-1` |
| `Float` | 64-bit float | `3.14`, `-0.5` |
| `Text` | UTF-8 string | `"hello"` |
| `List` | Ordered sequence | `[1, 2, 3]` |
| `Map` | Key-value store | `{"a": 1}` |
| `Quote` | Quoted code | `[dup mul]` |
| `Handle` | Opaque reference | `<handle:123>` |
| `Error` | Error value | `<error:...>` |

### Type Unions

| Type | Description |
|------|-------------|
| `Num` | `Int \| Float` |
| `Any` | Any value |
| `...` | Variadic (multiple values) |

### Parameterized Types

| Pattern | Meaning |
|---------|---------|
| `List<Int>` | List of integers |
| `Map<Text, Any>` | Map with text keys |
| `Quote<(a -- b)>` | Quote with signature |

---

## Capability Patterns

Capabilities use path-like patterns with interpolation:

```
fs:read:{path}           # Read file at {path}
fs:write:{path}          # Write file at {path}
net:http                  # Make HTTP requests
net:connect:{host}:{port} # Connect to host:port
exec                      # Execute shell commands
env:read                  # Read environment variables
env:write                 # Write environment variables
```

### Interpolation

`{name}` in a capability pattern refers to the input parameter with that name:

```toml
[signature]
input = ["path:Text"]

[requirements]
capabilities = ["fs:read:{path}"]
# If path = "/etc/passwd", requires fs:read:/etc/passwd
```

---

## Effect Categories

### I/O Effects

| Effect | Description |
|--------|-------------|
| `io:stdout` | Writes to standard output |
| `io:stderr` | Writes to standard error |
| `io:stdin` | Reads from standard input |
| `filesystem` | Interacts with filesystem |
| `network` | Makes network requests |
| `process` | Spawns processes |

### State Effects

| Effect | Description |
|--------|-------------|
| `mem:read` | Reads session memory |
| `mem:write` | Writes session memory |
| `rom:read` | Reads persistent storage |
| `rom:write` | Writes persistent storage |
| `introspect` | Reads tool metadata |

### Meta Effects

| Effect | Description |
|--------|-------------|
| `nondeterministic` | Output varies (random, time) |
| `blocking` | May wait for external event |
| `terminating` | May not return (exit, loop) |

---

## Example Manifests

### Core Tool: `add`

```toml
[tool]
name = "add"
version = "0.1.0"
tier = "core"

[signature]
input = ["a:Num", "b:Num"]
output = ["c:Num"]

[resources]
compute = 1

[effects]
may_fail = false
nondeterministic = false

[docs]
brief = "Add two numbers."
example = "3 5 add  # => 8"
see_also = ["sub", "mul", "div"]
```

### Stdlib Tool: `gt`

```toml
[tool]
name = "gt"
version = "0.1.0"
tier = "stdlib"

[signature]
input = ["a:Num", "b:Num"]
output = ["result:Bool"]

[resources]
compute = 2  # Calls swap + lt

[effects]
may_fail = false

[docs]
brief = "Greater than comparison."
long = "Returns true if a > b. Composed as: swap lt"
example = "5 3 gt  # => true"
see_also = ["lt", "ge", "le", "eq"]
```

### Capability Tool: `fs-read`

```toml
[tool]
name = "fs-read"
version = "0.1.0"
tier = "capability"

[signature]
input = ["path:Text"]
output = ["contents:Text"]

[requirements]
capabilities = ["fs:read:{path}"]

[resources]
compute = 1
mem = "dynamic"

[effects]
reads = ["filesystem"]
may_block = true
may_fail = true

[errors]
CapabilityDenied = "Missing fs:read capability for {path}"
NotFound = "File not found: {path}"
IoError = "Could not read {path}: {reason}"
EncodingError = "File is not valid UTF-8: {path}"

[docs]
brief = "Read file contents as UTF-8 text."
long = """
Reads the entire file at `path` and returns its contents as Text.
The file must exist, be readable, and contain valid UTF-8.
"""
example = '''
"/etc/hostname" fs-read println
'''
see_also = ["fs-write", "fs-exists", "fs-list"]
```

### Variadic Tool: `list`

```toml
[tool]
name = "list"
version = "0.1.0"
tier = "core"

[signature]
input = ["n:Int"]
output = ["l:List"]
# Note: Also consumes n items from stack, but that's not in signature

[requirements]
min_stack_depth = 1  # Plus the n items

[resources]
compute = 1
mem = "dynamic"

[effects]
may_fail = true

[errors]
StackUnderflow = "Stack has fewer than {n} items"

[docs]
brief = "Collect n items from stack into a list."
long = """
Pops n from stack, then pops n more items and collects them into a list.
Items are in stack order: bottom of stack = first element of list.
"""
example = '''
1 2 3 3 list  # => [1, 2, 3]
'''
see_also = ["unlist", "collect"]
```

---

## Query Interface

### Introspection Tools

```kore
# Get full manifest as map
"fs-read" manifest
# => { tool: { name: "fs-read", ... }, signature: {...}, ... }

# Check purity (no effects, no capabilities)
"add" is-pure         # => true
"fs-read" is-pure     # => false
"rand-float" is-pure  # => false (nondeterministic)

# Get required capabilities  
"fs-read" required-caps
# => ["fs:read:{path}"]

# Get declared effects
"println" effects
# => ["io:stdout"]

# Get error types
"fs-read" error-types
# => ["CapabilityDenied", "NotFound", "IoError", "EncodingError"]

# Get signature
"add" signature
# => { input: ["a:Num", "b:Num"], output: ["c:Num"] }
```

### Discovery Tools

```kore
# Find by capability requirement
["fs"] find-by-cap
# => ["fs-read", "fs-write", "fs-exists", ...]

# Find by tier
"core" find-by-tier
# => ["add", "dup", "call", ...]

# Find by tag
"math" find-by-tag
# => ["add", "sub", "mul", "div", "abs", "min", "max", ...]

# Find pure tools only
find-pure
# => ["add", "dup", "swap", "eq", ...]
```

---

## Validation Rules

A valid manifest must:

1. **Have required fields**: name, version, tier, signature.input, signature.output, docs.brief
2. **Use valid types**: All types in signature must be valid type names
3. **Declare all effects**: Any capability → corresponding effect
4. **Be consistent**: If `may_fail = false`, no errors defined
5. **Match implementation**: Signature must match actual behavior

### Validation Errors

```
E001: Missing required field: {field}
E002: Unknown type in signature: {type}
E003: Unknown capability pattern: {pattern}
E004: Effect not declared but capability requires it
E005: Error defined but may_fail = false
E006: Signature mismatch: expected {expected}, got {actual}
```

---

## Embedding in Rust

Manifests can be embedded in tool registration:

```rust
Tool::native("add", "(a:Num b:Num -- c:Num)", |stack, ctx| {
    // implementation
})
.with_tier(Tier::Core)
.with_docs("Add two numbers.", "3 5 add  # => 8")
.with_resource(Resource::Compute, 1)
.pure()  // Shorthand for no effects, no capabilities
```

Or generated from TOML:

```rust
Tool::from_manifest(include_str!("manifests/add.toml"))
```

---

## Appendix: Full Type Grammar

```ebnf
type      = primitive | union | parameterized | variadic
primitive = "Null" | "Bool" | "Int" | "Float" | "Text" 
          | "List" | "Map" | "Quote" | "Handle" | "Error"
union     = type ("|" type)+
parameterized = primitive "<" type ("," type)* ">"
variadic  = "..."

signature = "(" inputs "--" outputs ")"
inputs    = (name ":" type)*
outputs   = (name ":" type)*
name      = [a-z][a-z0-9_-]*
```

Examples:
- `(a:Int b:Int -- c:Int)` - Two ints in, one int out
- `(l:List<Int> -- n:Int)` - List of ints in, int out  
- `(f:Quote -- ...)` - Quote in, variable outputs
- `(x:Int|Float -- y:Float)` - Int or float in, float out
