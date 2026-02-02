# Kore Standard Library

This directory contains composed tools built from the core primitives.

## Philosophy

> "Because of our postulates, there's no difference between a native primitive and a composed tool."

The stdlib tools are first-class citizens. They're just defined as compositions of primitives rather than native Rust functions. This means:

1. **Same semantics**: A stdlib tool behaves exactly like a primitive
2. **Same capabilities**: stdlib tools have the same power as native tools
3. **Replaceable**: You can override any stdlib tool with your own definition

## Modules

| Module | Description | Dependencies |
|--------|-------------|--------------|
| [prelude.kore](prelude.kore) | Essential tools loaded by default | core only |
| [list.kore](list.kore) | List manipulation | prelude |
| [str.kore](str.kore) | String manipulation | prelude |
| [math.kore](math.kore) | Advanced math | prelude |
| [io.kore](io.kore) | Input/output (requires caps) | prelude |
| [fs.kore](fs.kore) | File system (requires caps) | io |
| [net.kore](net.kore) | Networking (requires caps) | io |
| [json.kore](json.kore) | JSON handling | str |

## Loading

```kore
# Load a single module
"list" import

# Load with prefix
"str" "s/" import-as

# Load specific tools
[ "map" "filter" "fold" ] "list" import-from
```

## Creating New Modules

A module is just a `.kore` file that defines tools:

```kore
# mylib.kore

# Define a tool
[ dup * ] "square" def

# Define with documentation
[ dup * ] "square" "(n -- n²) Square a number" def-doc

# Export only specific tools
[ "square" ] export
```

## Capability Requirements

Some modules require capabilities to function:

| Module | Required Capabilities |
|--------|----------------------|
| fs.kore | `fs:read`, `fs:write` |
| net.kore | `net:connect`, `net:listen` |
| io.kore | `io:stdin`, `io:stdout` |

If a module requires capabilities you don't have, the import succeeds but the tools will fail at runtime with a capability error.
