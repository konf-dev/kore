# Kore Language Reference

This is the complete reference for Kore - everything you need to write programs.

## Table of Contents

1. [Syntax](#syntax)
2. [Types](#types)
3. [Stack Operations](#stack-operations)
4. [Arithmetic](#arithmetic)
5. [Comparison & Logic](#comparison--logic)
6. [Control Flow](#control-flow)
7. [Definitions](#definitions)
8. [Strings](#strings)
9. [Lists](#lists)
10. [Maps](#maps)
11. [Type Checking & Conversion](#type-checking--conversion)
12. [Higher-Order Combinators](#higher-order-combinators)
13. [File System](#file-system)
14. [HTTP](#http)
15. [JSON](#json)
16. [Session Memory](#session-memory)
17. [Persistent Storage (ROM)](#persistent-storage-rom)
18. [System](#system)
19. [Capabilities](#capabilities)
20. [Resources](#resources)
21. [Tracing](#tracing)
22. [Error Handling](#error-handling)
23. [Advanced](#advanced)

---

## Syntax

### Values

```kore
42              # Int (64-bit signed)
3.14            # Float (64-bit)
"hello"         # Text (UTF-8 string)
true false      # Bool
null            # Null

# Lists - use collect
1 2 3 3 collect           # → [1 2 3]
10 20 30 40 4 collect     # → [10 20 30 40]

# Maps - use map-new and map-set
map-new "name" "Alice" map-set "age" 30 map-set
# → {name: "Alice", age: 30}

# Quotes (deferred code)
[1 2 add]       # A quote containing: push 1, push 2, call add
```

### Stack Notation

Tools document their stack effect as `(before -- after)`:
- `dup (a -- a a)` - takes `a`, leaves `a a`
- `add (a b -- c)` - takes `a` and `b`, leaves their sum
- Stack grows to the right: `1 2 3` means 1 is deepest, 3 is on top

### Comments

```kore
# This is a comment (to end of line)
1 2 add  # inline comment
```

---

## Types

| Type    | Description           | Examples                    |
|---------|-----------------------|-----------------------------|
| Null    | Absence of value      | `null`                      |
| Bool    | Truth value           | `true`, `false`             |
| Int     | 64-bit signed integer | `42`, `-17`, `0`            |
| Float   | 64-bit float          | `3.14`, `-0.5`, `1e10`      |
| Text    | UTF-8 string          | `"hello"`, `"line\nbreak"`  |
| List    | Ordered sequence      | `[1 2 3]`, `["a" "b"]`      |
| Map     | Key-value pairs       | `{name: "Alice", age: 30}`  |
| Quote   | Deferred code         | `[dup mul]`                 |
| Handle  | Opaque reference      | (internal use)              |
| Error   | Failure with message  | (created by `fail`)         |

---

## Stack Operations

| Tool   | Stack Effect       | Description                    |
|--------|-------------------|--------------------------------|
| `dup`  | `(a -- a a)`      | Duplicate top                  |
| `drop` | `(a -- )`         | Remove top                     |
| `swap` | `(a b -- b a)`    | Swap top two                   |
| `over` | `(a b -- a b a)`  | Copy second to top             |
| `rot`  | `(a b c -- b c a)`| Rotate third to top            |
| `nip`  | `(a b -- b)`      | Remove second                  |
| `tuck` | `(a b -- b a b)`  | Copy top below second          |
| `pick` | `(n -- v)`        | Copy nth item (0 = top)        |
| `depth`| `(-- n)`          | Count items on stack           |

### Examples

```kore
1 2 dup         # → 1 2 2
1 2 drop        # → 1
1 2 swap        # → 2 1
1 2 over        # → 1 2 1
1 2 3 rot       # → 2 3 1
5 0 pick        # → 5 5 (copy top)
5 6 7 2 pick    # → 5 6 7 5 (copy third from top)
1 2 3 depth     # → 1 2 3 3
```

---

## Arithmetic

| Tool  | Stack Effect    | Description           |
|-------|----------------|-----------------------|
| `add` | `(a b -- a+b)` | Addition              |
| `sub` | `(a b -- a-b)` | Subtraction           |
| `mul` | `(a b -- a*b)` | Multiplication        |
| `div` | `(a b -- a/b)` | Division              |
| `mod` | `(a b -- a%b)` | Modulo                |
| `neg` | `(a -- -a)`    | Negate                |
| `abs` | `(a -- |a|)`   | Absolute value        |
| `min` | `(a b -- min)` | Minimum               |
| `max` | `(a b -- max)` | Maximum               |

### Examples

```kore
10 3 add        # → 13
10 3 sub        # → 7
10 3 mul        # → 30
10 3 div        # → 3
10 3 mod        # → 1
5 neg           # → -5
-5 abs          # → 5
3 7 min         # → 3
3 7 max         # → 7
```

---

## Comparison & Logic

### Comparison

| Tool  | Stack Effect       | Description        |
|-------|-------------------|--------------------|
| `eq`  | `(a b -- a==b)`   | Equal              |
| `neq` | `(a b -- a!=b)`   | Not equal          |
| `lt`  | `(a b -- a<b)`    | Less than          |
| `gt`  | `(a b -- a>b)`    | Greater than       |
| `le`  | `(a b -- a<=b)`   | Less or equal      |
| `ge`  | `(a b -- a>=b)`   | Greater or equal   |

### Logic

| Tool  | Stack Effect       | Description        |
|-------|-------------------|--------------------|
| `and` | `(a b -- a&&b)`   | Logical AND        |
| `or`  | `(a b -- a\|\|b)` | Logical OR         |
| `not` | `(a -- !a)`       | Logical NOT        |

### Examples

```kore
5 5 eq          # → true
5 3 lt          # → false
5 3 gt          # → true
true false and  # → false
true false or   # → true
true not        # → false
```

---

## Control Flow

| Tool   | Stack Effect                    | Description                     |
|--------|--------------------------------|---------------------------------|
| `if`   | `(cond then else -- result)`   | Conditional execution           |
| `call` | `(quote -- ...)`               | Execute a quote                 |
| `loop` | `(body -- ...)`                | Execute until false on stack    |
| `while`| `(cond body -- ...)`           | While condition true, run body  |

### Examples

```kore
# if: condition then-quote else-quote if
true ["yes"] ["no"] if        # → "yes"
false ["yes"] ["no"] if       # → "no"
5 3 gt ["big"] ["small"] if   # → "big"

# call: execute a quote
[1 2 add] call                # → 3

# while: condition-quote body-quote while
0 [dup 5 lt] [1 add] while    # → 5 (counts 0,1,2,3,4,5)

# Nested conditions
5 
dup 10 gt ["large"] [
  dup 5 gt ["medium"] ["small"] if
] if                          # → 5 "medium"
```

---

## Definitions

| Tool        | Stack Effect              | Description                    |
|-------------|--------------------------|--------------------------------|
| `def`       | `(quote name -- )`       | Define a new tool              |
| `words`     | `(-- list)`              | List all tool names            |
| `describe`  | `(name -- signature)`    | Get tool's signature           |
| `meta`      | `(name -- map)`          | Get tool's metadata            |
| `meta!`     | `(map name -- )`         | Set tool's metadata            |
| `register`  | `(quote sig name -- )`   | Register with signature        |
| `unregister`| `(name -- )`             | Remove a tool                  |

### Examples

```kore
# Define a simple tool
[dup mul] "square" def
5 square                      # → 25

# Define with multiple operations
[dup 1 le [drop 1] [dup 1 sub fact mul] if] "fact" def
5 fact                        # → 120

# Check what's available
words                         # → ["add" "sub" ... all tools]

# Get signature
"add" describe                # → "(a:Num b:Num -- c:Num)"

# Conditional definition
[dup 0 lt [neg] [] if] "my-abs" def
-5 my-abs                     # → 5
```

---

## Strings

| Tool          | Stack Effect                    | Description                     |
|---------------|--------------------------------|---------------------------------|
| `str-len`     | `(s -- n)`                     | String length                   |
| `str-get`     | `(s i -- char)`                | Get character at index          |
| `str-slice`   | `(s start end -- sub)`         | Substring                       |
| `str-concat`  | `(a b -- ab)`                  | Concatenate                     |
| `str-split`   | `(s delim -- list)`            | Split by delimiter              |
| `str-join`    | `(list delim -- s)`            | Join with delimiter             |
| `str-trim`    | `(s -- trimmed)`               | Remove whitespace               |
| `str-find`    | `(s needle -- index)`          | Find substring (-1 if not)      |
| `str-starts`  | `(s prefix -- bool)`           | Starts with?                    |
| `str-ends`    | `(s suffix -- bool)`           | Ends with?                      |
| `str-replace` | `(s old new -- result)`        | Replace all occurrences         |
| `char-code`   | `(char -- code)`               | Character to ASCII code         |
| `code-char`   | `(code -- char)`               | ASCII code to character         |

### Examples

```kore
"hello" str-len                     # → 5
"hello" 1 str-get                   # → "e"
"hello" 1 4 str-slice               # → "ell"
"hello" " world" str-concat         # → "hello world"
"a,b,c" "," str-split               # → ["a" "b" "c"]
"a" "b" "c" 3 collect "-" str-join  # → "a-b-c"
"  hello  " str-trim                # → "hello"
"hello" "ll" str-find               # → 2
"hello" "he" str-starts             # → true
"hello" "lo" str-ends               # → true
"hello" "l" "L" str-replace         # → "heLLo"
"A" char-code                       # → 65
65 code-char                        # → "A"
```

---

## Lists

| Tool           | Stack Effect                  | Description                    |
|----------------|------------------------------|--------------------------------|
| `list-len`     | `(list -- n)`                | Length                         |
| `list-get`     | `(list i -- val)`            | Get element at index           |
| `list-set`     | `(list i val -- list')`      | Set element at index           |
| `list-push`    | `(list val -- list')`        | Append to end                  |
| `list-pop`     | `(list -- list' val)`        | Remove and return last         |
| `list-slice`   | `(list start end -- sub)`    | Sublist                        |
| `list-concat`  | `(a b -- ab)`                | Concatenate lists              |
| `list-reverse` | `(list -- reversed)`         | Reverse order                  |
| `list-empty`   | `(-- [])`                    | Empty list                     |
| `collect`      | `(v1..vn n -- list)`         | Collect n items into list      |
| `unlist`       | `(list -- v1..vn n)`         | Spread list onto stack         |

### Examples

```kore
# Create lists
1 2 3 3 collect                     # → [1 2 3]
list-empty                          # → []

# Access
1 2 3 3 collect 0 list-get          # → 1
1 2 3 3 collect 2 list-get          # → 3
1 2 3 3 collect list-len            # → 3

# Modify (returns new list)
1 2 3 3 collect 1 99 list-set       # → [1 99 3]
1 2 3 3 collect 4 list-push         # → [1 2 3 4]
1 2 3 3 collect list-pop            # → [1 2] 3

# Transform
1 2 3 3 collect 0 2 list-slice      # → [1 2]
1 2 2 collect 3 4 2 collect list-concat  # → [1 2 3 4]
1 2 3 3 collect list-reverse        # → [3 2 1]

# Spread
1 2 3 3 collect unlist              # → 1 2 3 3
```

---

## Maps

| Tool        | Stack Effect                  | Description                    |
|-------------|------------------------------|--------------------------------|
| `map-new`   | `(-- {})`                    | Empty map                      |
| `map-get`   | `(map key -- val)`           | Get value (null if missing)    |
| `map-set`   | `(map key val -- map')`      | Set key-value                  |
| `map-has`   | `(map key -- bool)`          | Key exists?                    |
| `map-del`   | `(map key -- map')`          | Remove key                     |
| `map-keys`  | `(map -- list)`              | List of keys                   |
| `map-vals`  | `(map -- list)`              | List of values                 |
| `map-empty` | `(-- {})`                    | Empty map (alias)              |

### Examples

```kore
# Create
map-new                                      # → {}
map-new "name" "Alice" map-set               # → {name: "Alice"}
map-new "x" 1 map-set "y" 2 map-set          # → {x: 1, y: 2}

# Access
map-new "a" 1 map-set "a" map-get            # → 1
map-new "a" 1 map-set "b" map-get            # → null
map-new "a" 1 map-set "a" map-has            # → true

# Modify
map-new "a" 1 map-set "a" 99 map-set         # → {a: 99}
map-new "a" 1 map-set "b" 2 map-set "a" map-del  # → {b: 2}

# Introspect
map-new "a" 1 map-set "b" 2 map-set map-keys # → ["a" "b"]
map-new "a" 1 map-set "b" 2 map-set map-vals # → [1 2]
```

---

## Type Checking & Conversion

### Type Checking

| Tool       | Stack Effect     | Description            |
|------------|-----------------|------------------------|
| `type-of`  | `(val -- type)` | Get type as string     |
| `is-null`  | `(val -- bool)` | Is null?               |
| `is-bool`  | `(val -- bool)` | Is boolean?            |
| `is-int`   | `(val -- bool)` | Is integer?            |
| `is-float` | `(val -- bool)` | Is float?              |
| `is-text`  | `(val -- bool)` | Is string?             |
| `is-list`  | `(val -- bool)` | Is list?               |
| `is-map`   | `(val -- bool)` | Is map?                |
| `is-quote` | `(val -- bool)` | Is quote?              |
| `is-error` | `(val -- bool)` | Is error?              |

### Conversion

| Tool       | Stack Effect       | Description            |
|------------|-------------------|------------------------|
| `to-int`   | `(val -- int)`    | Convert to integer     |
| `to-float` | `(val -- float)`  | Convert to float       |
| `to-text`  | `(val -- text)`   | Convert to string      |
| `to-bool`  | `(val -- bool)`   | Convert to boolean     |
| `to-list`  | `(val -- list)`   | Convert to list        |

### Examples

```kore
42 type-of          # → "Int"
"hi" type-of        # → "Text"
42 is-int           # → true
"42" is-int         # → false

"42" to-int         # → 42
42 to-text          # → "42"
3.14 to-int         # → 3
0 to-bool           # → false
1 to-bool           # → true
```

---

## Higher-Order Combinators

| Tool     | Stack Effect                        | Description                      |
|----------|------------------------------------|---------------------------------|
| `map`    | `(list quote -- list')`            | Apply to each, collect results  |
| `filter` | `(list quote -- list')`            | Keep where quote returns true   |
| `fold`   | `(list init quote -- result)`      | Reduce list to single value     |
| `each`   | `(list quote -- )`                 | Apply to each, discard results  |
| `times`  | `(n quote -- ...)`                 | Execute quote n times           |

### Examples

```kore
# map: transform each element
1 2 3 3 collect [2 mul] map         # → [2 4 6]
1 2 3 3 collect [dup mul] map       # → [1 4 9]

# filter: keep matching elements
1 2 3 4 5 5 collect [2 mod 0 eq] filter  # → [2 4]
1 2 3 4 5 5 collect [3 gt] filter        # → [4 5]

# fold: reduce to single value
1 2 3 3 collect 0 [add] fold        # → 6 (sum)
1 2 3 3 collect 1 [mul] fold        # → 6 (product)

# each: side effects only
1 2 3 3 collect [println] each      # prints 1, 2, 3

# times: repeat
5 ["hello" println] times           # prints "hello" 5 times
0 10 [1 add] times                  # → 10
```

---

## File System

| Tool        | Stack Effect                  | Description                     |
|-------------|------------------------------|---------------------------------|
| `fs-read`   | `(path -- contents)`         | Read file as string             |
| `fs-write`  | `(path contents -- )`        | Write string to file            |
| `fs-append` | `(path contents -- )`        | Append string to file           |
| `fs-exists` | `(path -- bool)`             | File/dir exists?                |
| `fs-list`   | `(path -- list)`             | List directory contents         |
| `fs-mkdir`  | `(path -- )`                 | Create directory                |
| `fs-rm`     | `(path -- )`                 | Remove file/directory           |

### Examples

```kore
# Write and read
"/world/hello.txt" "Hello, World!" fs-write
"/world/hello.txt" fs-read          # → "Hello, World!"

# Append
"/world/log.txt" "Line 1\n" fs-write
"/world/log.txt" "Line 2\n" fs-append

# Check existence
"/world/hello.txt" fs-exists        # → true
"/world/nope.txt" fs-exists         # → false

# Directories
"/world/data" fs-mkdir
"/world" fs-list                    # → ["hello.txt" "data" ...]

# Save structured data
map-new "count" 42 map-set json-encode "/world/state.json" swap fs-write
"/world/state.json" fs-read json-parse "count" map-get  # → 42
```

---

## HTTP

| Tool           | Stack Effect                           | Description                |
|----------------|---------------------------------------|----------------------------|
| `http-get`     | `(url -- response)`                   | Simple GET request         |
| `http-post`    | `(url body -- response)`              | POST with body             |
| `http-request` | `(url options -- response)`           | Full control request       |

### Examples

```kore
# Simple GET
"https://api.example.com/data" http-get

# POST with body
"https://api.example.com/submit" 
"{\"name\": \"test\"}" 
http-post

# Full control
"https://api.example.com/api"
map-new
  "method" "POST" map-set
  "headers" map-new "Content-Type" "application/json" map-set map-set
  "body" "{\"key\": \"value\"}" map-set
http-request

# Parse JSON response
"https://api.example.com/data" http-get json-parse
```

---

## JSON

| Tool          | Stack Effect          | Description                |
|---------------|----------------------|----------------------------|
| `json-parse`  | `(text -- value)`    | Parse JSON string          |
| `json-encode` | `(value -- text)`    | Convert to JSON string     |

### Examples

```kore
# Parse
"{\"name\": \"Alice\", \"age\": 30}" json-parse
# → {name: "Alice", age: 30}

"[1, 2, 3]" json-parse
# → [1 2 3]

# Encode
map-new "x" 1 map-set "y" 2 map-set json-encode
# → "{\"x\":1,\"y\":2}"

1 2 3 3 collect json-encode
# → "[1,2,3]"
```

---

## Session Memory

Volatile key-value storage (lost when program ends).

| Tool       | Stack Effect            | Description                |
|------------|------------------------|----------------------------|
| `mem-set`  | `(key value -- )`      | Store value                |
| `mem-get`  | `(key -- value)`       | Retrieve value             |
| `mem-has`  | `(key -- bool)`        | Key exists?                |
| `mem-del`  | `(key -- )`            | Remove key                 |
| `mem-keys` | `(-- list)`            | List all keys              |

### Examples

```kore
# Store and retrieve
"count" 0 mem-set
"count" mem-get              # → 0

# Increment pattern
"count" "count" mem-get 1 add mem-set
"count" mem-get              # → 1

# Check existence
"count" mem-has              # → true
"other" mem-has              # → false

# Delete
"count" mem-del
"count" mem-has              # → false

# List all
mem-keys                     # → ["...all keys..."]
```

---

## Persistent Storage (ROM)

Persistent key-value storage (survives restarts, if storage is configured).

| Tool       | Stack Effect            | Description                |
|------------|------------------------|----------------------------|
| `rom-set`  | `(key value -- )`      | Store persistently         |
| `rom-get`  | `(key -- value)`       | Retrieve                   |
| `rom-has`  | `(key -- bool)`        | Key exists?                |
| `rom-del`  | `(key -- )`            | Remove key                 |
| `rom-keys` | `(-- list)`            | List all keys              |
| `persist`  | `(-- )`                | Force write to disk        |

### Examples

```kore
# Store persistent data
"settings" map-new "theme" "dark" map-set rom-set
persist  # ensure written

# Later...
"settings" rom-get "theme" map-get  # → "dark"
```

---

## System

| Tool        | Stack Effect            | Description                  |
|-------------|------------------------|------------------------------|
| `exec`      | `(cmd -- output)`      | Run shell command            |
| `print`     | `(val -- )`            | Print without newline        |
| `println`   | `(val -- )`            | Print with newline           |
| `log`       | `(val -- )`            | Log (for debugging)          |
| `read-line` | `(-- text)`            | Read line from stdin         |
| `now`       | `(-- timestamp)`       | Current Unix timestamp       |
| `sleep`     | `(ms -- )`             | Sleep for milliseconds       |
| `uuid`      | `(-- text)`            | Generate UUID v4             |
| `random`    | `(-- float)`           | Random float 0.0-1.0         |
| `pid`       | `(-- int)`             | Process ID                   |
| `cwd`       | `(-- path)`            | Current working directory    |
| `args`      | `(-- list)`            | Command line arguments       |
| `exit`      | `(code -- )`           | Exit with code               |
| `version`   | `(-- text)`            | Kore version                 |
| `env-get`   | `(name -- value)`      | Get environment variable     |
| `env-set`   | `(name value -- )`     | Set environment variable     |

### Examples

```kore
# Shell commands
"ls -la" exec                # → directory listing
"echo hello" exec            # → "hello\n"

# Output
"Hello " print "World" println  # prints "Hello World\n"

# Time
now                          # → 1706918400 (Unix timestamp)
1000 sleep                   # pause 1 second

# Random
random                       # → 0.7234... (0.0 to 1.0)
random 100 mul to-int        # → random 0-99

# UUID
uuid                         # → "550e8400-e29b-41d4-a716-446655440000"

# Environment
"HOME" env-get               # → "/root"
"MY_VAR" "value" env-set
```

---

## Capabilities

Security controls for what operations are allowed.

| Tool             | Stack Effect            | Description                    |
|------------------|------------------------|--------------------------------|
| `cap-has`        | `(name -- bool)`       | Has capability?                |
| `cap-list`       | `(-- list)`            | List all capabilities          |
| `cap-fs`         | `(path op -- bool)`    | Check filesystem permission    |
| `cap-net`        | `(host port -- bool)`  | Check network permission       |
| `cap-leq`        | `(a b -- bool)`        | Capability ordering            |
| `cap-meet`       | `(a b -- c)`           | Capability intersection        |
| `cap-join`       | `(a b -- c)`           | Capability union               |
| `cap-attenuate`  | `(caps -- )`           | Reduce own capabilities        |

### Examples

```kore
# Check what you can do
cap-list                     # → ["fs:read:/world" "net:connect:*:443" ...]
"exec" cap-has               # → true/false

# Check specific permissions
"/world/data" "read" cap-fs  # → true/false
"/etc/passwd" "write" cap-fs # → false (hopefully!)
"api.example.com" 443 cap-net # → true/false
```

---

## Resources

Quotas and usage tracking.

| Tool          | Stack Effect           | Description                   |
|---------------|------------------------|-------------------------------|
| `res-avail`   | `(-- map)`             | Available resources           |
| `res-has`     | `(name -- bool)`       | Has resource?                 |
| `res-mem`     | `(-- bytes)`           | Memory quota                  |
| `res-rom`     | `(-- bytes)`           | Storage quota                 |
| `res-compute` | `(-- units)`           | Compute quota                 |
| `res-net`     | `(-- bytes)`           | Network quota                 |
| `res-all`     | `(-- map)`             | All resource info             |
| `res-split`   | `(name ratio -- )`     | Split resource quota          |
| `res-cons`    | `(name amount -- )`    | Consume resource              |
| `res-add`     | `(name amount -- )`    | Add to resource               |

### Examples

```kore
res-avail                    # → {mem: 1000000, rom: 100000, ...}
"mem" res-has                # → true
res-mem                      # → 1000000
```

---

## Tracing

Debugging and execution observation.

| Tool               | Stack Effect          | Description                   |
|--------------------|-----------------------|-------------------------------|
| `trace-on`         | `(bool -- )`          | Enable/disable tracing        |
| `trace`            | `(msg -- )`           | Add trace message             |
| `trace-step`       | `(-- )`               | Record step                   |
| `trace-fingerprint`| `(-- hash)`           | Get trace fingerprint         |
| `tag`              | `(val tag -- val)`    | Tag a value                   |
| `find-tag`         | `(tag -- val)`        | Find tagged value in trace    |

### Examples

```kore
true trace-on                # enable tracing
"Starting computation" trace
# ... computation ...
"Done" trace
trace-fingerprint            # → unique hash of execution
```

---

## Error Handling

| Tool     | Stack Effect              | Description                    |
|----------|--------------------------|--------------------------------|
| `try`    | `(quote -- result)`      | Execute, catch errors          |
| `fail`   | `(msg -- error)`         | Create an error                |
| `unwrap` | `(val -- val)`           | Unwrap or fail if error        |
| `assert` | `(cond msg -- )`         | Fail if condition false        |
| `panic`  | `(msg -- )`              | Immediately fail               |

### Examples

```kore
# try: catch errors
[1 0 div] try                # → Error("division by zero")
[1 2 add] try                # → 3

# Check if error
[1 0 div] try is-error       # → true
[1 2 add] try is-error       # → false

# Handle errors
[1 0 div] try 
dup is-error 
  ["Error occurred"] 
  [to-text " is the result" str-concat] 
if

# fail: create error
"Something went wrong" fail   # → Error("Something went wrong")

# assert: conditional failure
5 3 gt "should be greater" assert  # passes
5 3 lt "should be less" assert     # fails with "should be less"

# unwrap: extract or fail
[1 2 add] try unwrap          # → 3
[1 0 div] try unwrap          # fails!
```

---

## Advanced

| Tool          | Stack Effect              | Description                    |
|---------------|--------------------------|--------------------------------|
| `spawn`       | `(quote caps -- handle)` | Spawn sub-agent                |
| `load`        | `(path -- )`             | Load and execute .kore file    |
| `load-tools`  | `(path -- )`             | Load tools from file           |
| `calls`       | `(name -- n)`            | Times a tool was called        |
| `stats`       | `(name -- map)`          | Tool statistics                |
| `health`      | `(-- map)`               | System health info             |
| `graph`       | `(-- map)`               | Tool dependency graph          |

### Examples

```kore
# Load library
"/world/lib/utils.kore" load

# Check tool usage
"add" calls                   # → 42
"add" stats                   # → {calls: 42, failures: 0, ...}

# Health check
health                        # → {uptime: 3600, memory: ...}
```

---

## Common Patterns

### Variables with mem-set/mem-get

```kore
# Initialize
"x" 0 mem-set
"y" 10 mem-set

# Update
"x" "x" mem-get 1 add mem-set

# Use
"x" mem-get "y" mem-get add  # x + y
```

### Loops with while

```kore
# Count to 10
"i" 0 mem-set
["i" mem-get 10 lt] [
  "i" mem-get println
  "i" "i" mem-get 1 add mem-set
] while
```

### Recursion

```kore
# Factorial
[dup 1 le [drop 1] [dup 1 sub fact mul] if] "fact" def
5 fact  # → 120

# Fibonacci
[dup 2 lt [] [dup 1 sub fib swap 2 sub fib add] if] "fib" def
10 fib  # → 55
```

### Error handling pattern

```kore
[risky-operation] try
dup is-error [
  "Failed: " swap to-text str-concat println
  "default-value"  # fallback
] [
  # success path
] if
```

### JSON config file

```kore
# Save
map-new 
  "version" 1 map-set
  "name" "my-app" map-set
json-encode "/world/config.json" swap fs-write

# Load
"/world/config.json" fs-read json-parse "config" mem-set
"config" mem-get "name" map-get  # → "my-app"
```

---

## Quick Reference

### Stack: `dup drop swap over rot nip tuck pick depth`
### Math: `add sub mul div mod neg abs min max`
### Compare: `eq neq lt gt le ge`
### Logic: `and or not`
### Control: `if call loop while`
### Define: `def words describe`
### String: `str-len str-get str-slice str-concat str-split str-join str-trim str-find str-starts str-ends str-replace`
### List: `list-len list-get list-set list-push list-pop list-slice list-concat list-reverse list-empty collect unlist`
### Map: `map-new map-get map-set map-has map-del map-keys map-vals`
### Types: `type-of is-null is-bool is-int is-float is-text is-list is-map is-quote is-error is-tensor`
### Convert: `to-int to-float to-text to-bool to-list`
### FP: `map filter fold each times`
### File: `fs-read fs-write fs-append fs-exists fs-list fs-mkdir fs-rm`
### HTTP: `http-get http-post http-request`
### JSON: `json-parse json-encode`
### Memory: `mem-set mem-get mem-has mem-del mem-keys`
### System: `exec print println now sleep uuid random`
### Error: `try fail unwrap assert panic`
### Tensor: `tensor-from-list tensor-zeros tensor-ones tensor-add tensor-mul tensor-sum tensor-matmul tensor-softmax tensor-relu tensor-exp tensor-log tensor-neg tensor-scale`
### Autodiff: `requires-grad backward grad-get zero-grad detach`
### Linear: `linear-new linear-unwrap affine-new affine-unwrap is-linear is-affine linearity`
