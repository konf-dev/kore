# Kore Quickstart

Get productive in 5 minutes.

## 0. Key Insight: Transparent Optimization

Kore optimizes I/O automatically. Write sequential code:

```kore
"url1" http-get
"url2" http-get
"url3" http-get
```

The runtime detects independent I/O operations and executes them **in parallel**, 
but commits results in **program order**. You get 3x speedup without changing your code.

This preserves Kore's postulates:
- Tool : Stack → Stack ✓
- Deterministic: same input → same output ✓
- Traces are linear ✓

## 1. Basic Syntax

```kore
# Values go on stack
42                      # Push integer
"hello"                 # Push string
true false null         # Booleans and null

# Tools operate on stack
1 2 add                 # → 3
"hello" " world" str-concat  # → "hello world"

# Quotes are deferred code (use [ ] brackets!)
[1 2 add] call          # → 3

# Define new tools
[dup mul] "square" def
5 square                # → 25
```

## 2. Variables (use mem-set/mem-get)

```kore
# Store: value "key" mem-set (value first, like def)
10 "x" mem-set
20 "y" mem-set

# Retrieve: "key" mem-get
"x" mem-get "y" mem-get add    # → 30

# Update: get, modify, set
"x" mem-get 1 add "x" mem-set  # x = x + 1
```

## 3. Control Flow

```kore
# if: condition [then] [else] if
5 3 gt ["big"] ["small"] if    # → "big"

# while: [condition] [body] while
0 "i" mem-set
["i" mem-get 5 lt] [
  "i" mem-get println
  "i" mem-get 1 add "i" mem-set
] while
# prints 0, 1, 2, 3, 4
```

## 4. Lists

```kore
# Create: values count collect
1 2 3 3 collect         # → [1 2 3]

# Access
[1 2 3] 0 list-get      # → 1
[1 2 3] list-len        # → 3

# Transform
[1 2 3] [2 mul] map     # → [2 4 6]
[1 2 3] 0 [add] fold    # → 6 (sum)
```

## 5. Maps

```kore
# Create
map-new "name" "Alice" map-set "age" 30 map-set
# → {name: "Alice", age: 30}

# Access
map "name" map-get      # → "Alice"
map "name" map-has      # → true
```

## 6. Files

```kore
# Write
"/world/test.txt" "Hello!" fs-write

# Read
"/world/test.txt" fs-read    # → "Hello!"

# Check/List
"/world/test.txt" fs-exists  # → true
"/world" fs-list             # → ["test.txt" ...]
```

## 7. HTTP

```kore
# GET
"https://httpbin.org/get" http-get json-parse

# POST
"https://httpbin.org/post" "{\"data\": 123}" http-post
```

## 8. Error Handling

```kore
# try catches errors
[1 0 div] try           # → Error("division by zero")
[1 2 add] try           # → 3

# Check if error
[risky-op] try dup is-error
  ["Error: " swap to-text str-concat println]
  ["Success: " swap to-text str-concat println]
if
```

## 9. Common Patterns

### Factorial
```kore
[dup 1 le [drop 1] [dup 1 sub fact mul] if] "fact" def
5 fact  # → 120
```

### Loop with counter
```kore
"i" 0 mem-set
["i" mem-get 10 lt] [
  # do work here
  "i" "i" mem-get 1 add mem-set
] while
```

### Save/load state
```kore
# Save
map-new "count" 42 map-set json-encode 
"/world/state.json" swap fs-write

# Load
"/world/state.json" fs-read json-parse
"count" map-get  # → 42
```

## 10. Spawn Sub-Tasks

```kore
# Spawn runs a quote with attenuated capabilities
[
  # This runs in a child context
  1 2 add
] cap-list spawn  # → 3

# Child cannot have MORE capabilities than parent
# Resources are split between parent and child
```

## Quick Reference

| Category | Key Tools |
|----------|-----------|
| Stack | `dup drop swap over rot depth` |
| Math | `add sub mul div mod` |
| Compare | `eq neq lt gt le ge` |
| Logic | `and or not` |
| Control | `if call while` |
| Define | `def words describe` |
| String | `str-concat str-split str-len` |
| List | `collect list-get list-len map filter fold` |
| Map | `map-new map-get map-set map-keys` |
| Memory | `mem-set mem-get mem-keys` |
| File | `fs-read fs-write fs-exists fs-list` |
| HTTP | `http-get http-post` |
| JSON | `json-parse json-encode` |
| Error | `try is-error fail` |
| Spawn | `spawn cap-list cap-attenuate` |
| Tensor | `tensor-from-list tensor-add tensor-mul tensor-sum` |
| Autodiff | `requires-grad backward grad-get` |

## Your Workspace

- `/world/` - Your persistent workspace (read/write)
- `/opt/kore/docs/REFERENCE.md` - Full reference (186 tools)
- `/opt/kore/examples/` - Working examples

## Next Steps

1. Try `words` to see all available tools
2. Try `"tool-name" describe` for any tool's signature
3. Read examples: `"/opt/kore/examples/algorithms.kore" fs-read`
4. Build something and save it to `/world/`
