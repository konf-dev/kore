# Kore Primitives Reference

**Total: ~200 primitives**

All primitives do exactly one thing. No magic.

## Execution (8)
| Name | Signature | Description |
|------|-----------|-------------|
| `call` | `(q:Quote -- ...)` | Run a quote |
| `try` | `(q:Quote -- result:Any)` | Run a quote, capture errors as Error values |
| `if` | `(cond:Bool then:Quote else:Quote -- ...)` | Conditional execution |
| `loop` | `(body:Quote -- ...)` | Repeat until false on stack |
| `def` | `(name:Text body:Quote -- )` | Define a new tool from a quote |
| `def-verified` | `(body:Quote name:Text sig:Text -- )` | Define with effect verification |
| `words` | `( -- names:List)` | List all tool names |
| `describe` | `(name:Text -- sig:Text)` | Get tool signature |

## Error Inspection (5)
| Name | Signature | Description |
|------|-----------|-------------|
| `is-error` | `(v:Any -- result:Bool)` | Check if value is an Error |
| `error-info` | `(err:Error -- info:Map)` | Get structured error details |
| `unwrap` | `(v:Any -- result:Any)` | Extract value, or stop if Error |
| `assert` | `(cond:Bool msg:Text -- )` | Fail with message if condition is false |
| `panic` | `(msg:Text -- )` | Intentionally fail with message |

## Stack Manipulation (6)
| Name | Signature | Description |
|------|-----------|-------------|
| `dup` | `(a:Any -- a:Any a:Any)` | Duplicate top value |
| `drop` | `(a:Any -- )` | Remove top value |
| `swap` | `(a:Any b:Any -- b:Any a:Any)` | Swap top two values |
| `over` | `(a:Any b:Any -- a:Any b:Any a:Any)` | Copy second value to top |
| `rot` | `(a:Any b:Any c:Any -- b:Any c:Any a:Any)` | Rotate top three values |
| `depth` | `( -- n:Int)` | Get stack depth |

## Arithmetic (6)
| Name | Signature | Description |
|------|-----------|-------------|
| `add` | `(a:Num b:Num -- c:Num)` | Add two numbers |
| `sub` | `(a:Num b:Num -- c:Num)` | Subtract two numbers |
| `mul` | `(a:Num b:Num -- c:Num)` | Multiply two numbers |
| `div` | `(a:Num b:Num -- c:Num)` | Divide two numbers |
| `mod` | `(a:Int b:Int -- c:Int)` | Integer modulo |
| `neg` | `(a:Num -- b:Num)` | Negate a number |

## Comparison (2 primitives + patterns)
| Name | Signature | Description |
|------|-----------|-------------|
| `eq` | `(a:Any b:Any -- result:Bool)` | Equal |
| `lt` | `(a:Num b:Num -- result:Bool)` | Less than |

**NOTE**: `gt`, `neq`, `le`, `ge` are NOT primitives. Use these patterns:
- Greater than: `swap lt`
- Not equal: `eq not`
- Less or equal: `swap lt not`  
- Greater or equal: `lt not`

## Logic (3)
| Name | Signature | Description |
|------|-----------|-------------|
| `and` | `(a:Bool b:Bool -- result:Bool)` | Logical AND |
| `or` | `(a:Bool b:Bool -- result:Bool)` | Logical OR |
| `not` | `(a:Bool -- result:Bool)` | Logical NOT |

## String (13)
| Name | Signature | Description |
|------|-----------|-------------|
| `str-len` | `(s:Text -- n:Int)` | String length |
| `str-get` | `(s:Text i:Int -- c:Text)` | Get character at index |
| `str-slice` | `(s:Text start:Int end:Int -- sub:Text)` | Get substring |
| `str-split` | `(s:Text sep:Text -- parts:List)` | Split string |
| `str-join` | `(parts:List sep:Text -- s:Text)` | Join strings |
| `str-concat` | `(a:Text b:Text -- c:Text)` | Concatenate strings |
| `str-trim` | `(s:Text -- t:Text)` | Trim whitespace |
| `str-find` | `(s:Text pat:Text -- i:Int)` | Find substring (-1 if not found) |
| `str-starts` | `(s:Text prefix:Text -- result:Bool)` | Check prefix |
| `str-ends` | `(s:Text suffix:Text -- result:Bool)` | Check suffix |
| `str-replace` | `(s:Text old:Text new:Text -- t:Text)` | Replace all occurrences |
| `char-code` | `(c:Text -- n:Int)` | Character to Unicode codepoint |
| `code-char` | `(n:Int -- c:Text)` | Unicode codepoint to character |

## List (10)
| Name | Signature | Description |
|------|-----------|-------------|
| `list-len` | `(l:List -- n:Int)` | List length |
| `list-get` | `(l:List i:Int -- v:Any)` | Get element at index |
| `list-set` | `(l:List i:Int v:Any -- l:List)` | Set element at index |
| `list-push` | `(l:List v:Any -- l:List)` | Append element |
| `list-pop` | `(l:List -- l:List v:Any)` | Remove and return last element |
| `list-slice` | `(l:List start:Int end:Int -- sub:List)` | Get sublist |
| `list-concat` | `(a:List b:List -- c:List)` | Concatenate lists |
| `list-reverse` | `(l:List -- r:List)` | Reverse list |
| `list-empty` | `( -- l:List)` | Create empty list |
| `collect` | `(n:Int -- l:List)` | Collect top n stack items into list |

## Map (7)
| Name | Signature | Description |
|------|-----------|-------------|
| `map-get` | `(m:Map k:Text -- v:Any)` | Get value by key |
| `map-set` | `(m:Map k:Text v:Any -- m:Map)` | Set key-value |
| `map-has` | `(m:Map k:Text -- result:Bool)` | Check if key exists |
| `map-del` | `(m:Map k:Text -- m:Map)` | Delete key |
| `map-keys` | `(m:Map -- keys:List)` | Get all keys |
| `map-vals` | `(m:Map -- vals:List)` | Get all values |
| `map-empty` | `( -- m:Map)` | Create empty map |

## Type (9)
| Name | Signature | Description |
|------|-----------|-------------|
| `type-of` | `(v:Any -- t:Text)` | Get type name |
| `is-null` | `(v:Any -- result:Bool)` | Check if Null |
| `is-bool` | `(v:Any -- result:Bool)` | Check if Bool |
| `is-int` | `(v:Any -- result:Bool)` | Check if Int |
| `is-float` | `(v:Any -- result:Bool)` | Check if Float |
| `is-text` | `(v:Any -- result:Bool)` | Check if Text |
| `is-list` | `(v:Any -- result:Bool)` | Check if List |
| `is-map` | `(v:Any -- result:Bool)` | Check if Map |
| `is-quote` | `(v:Any -- result:Bool)` | Check if Quote |

## Conversion (5)
| Name | Signature | Description |
|------|-----------|-------------|
| `to-int` | `(v:Any -- n:Int)` | Convert to integer |
| `to-float` | `(v:Any -- f:Float)` | Convert to float |
| `to-text` | `(v:Any -- s:Text)` | Convert to text |
| `to-bool` | `(v:Any -- b:Bool)` | Convert to boolean |
| `to-list` | `(v:Any -- l:List)` | Wrap in list |

## Quote Serialization (2)
| Name | Signature | Description |
|------|-----------|-------------|
| `quote-to-text` | `(q:Quote -- s:Text)` | Serialize quote to Kore source |
| `text-to-quote` | `(s:Text -- q:Quote)` | Parse Kore source to quote |

**Use case:** Enables metaprogramming, persistence of learned tools, and code generation.
```kore
[dup mul] quote-to-text    # → "[ dup mul ]"
"[ 1 2 add ]" text-to-quote call  # → 3
```

## Combinators (6)
| Name | Signature | Description |
|------|-----------|-------------|
| `map` | `(l:List f:Quote -- l:List)` | Apply function to each element |
| `filter` | `(l:List f:Quote -- l:List)` | Keep elements where function returns true |
| `fold` | `(l:List init:Any f:Quote -- result:Any)` | Reduce list to single value |
| `each` | `(l:List f:Quote -- )` | Execute function for each element |
| `times` | `(n:Int f:Quote -- )` | Execute function n times |
| `while` | `(body:Quote -- )` | Execute while top of stack is true |

## OS: File System (7)
Requires capabilities: `fs:read:/path` or `fs:write:/path`

| Name | Signature | Description |
|------|-----------|-------------|
| `fs-read` | `(path:Text -- contents:Text)` | Read file contents |
| `fs-write` | `(path:Text contents:Text -- )` | Write text to file |
| `fs-append` | `(path:Text contents:Text -- )` | Append text to file |
| `fs-exists` | `(path:Text -- exists:Bool)` | Check if path exists |
| `fs-list` | `(path:Text -- entries:List)` | List directory contents |
| `fs-rm` | `(path:Text -- )` | Remove file or directory |
| `fs-mkdir` | `(path:Text -- )` | Create directory |

## OS: Process (1)
Requires capability: `exec`

| Name | Signature | Description |
|------|-----------|-------------|
| `exec` | `(cmd:Text -- output:Text)` | Run shell command |

## OS: I/O (4)
| Name | Signature | Description |
|------|-----------|-------------|
| `print` | `(text:Text -- )` | Print to stdout |
| `println` | `(text:Text -- )` | Print with newline |
| `read-line` | `( -- line:Text)` | Read line from stdin |
| `log` | `(level:Text message:Text -- )` | Log to stderr |

## OS: Time (2)
| Name | Signature | Description |
|------|-----------|-------------|
| `now` | `( -- ms:Int)` | Current unix timestamp (ms) |
| `sleep` | `(ms:Int -- )` | Sleep for milliseconds |

## OS: Misc (2)
| Name | Signature | Description |
|------|-----------|-------------|
| `uuid` | `( -- id:Text)` | Generate UUID v4 |
| `random` | `( -- f:Float)` | Random float 0.0-1.0 |

## OS: System Info (5)
| Name | Signature | Description |
|------|-----------|-------------|
| `pid` | `( -- n:Int)` | Get process ID |
| `cwd` | `( -- path:Text)` | Get current directory |
| `args` | `( -- args:List)` | Get command line arguments |
| `exit` | `(code:Int -- )` | Exit process |
| `version` | `( -- v:Text)` | Get kore version |

## OS: Module Loading (1)
| Name | Signature | Description |
|------|-----------|-------------|
| `load` | `(path:Text -- ...)` | Execute a .kore file |

## OS: Environment (2)
Requires capabilities: `env:read` or `env:write`

| Name | Signature | Description |
|------|-----------|-------------|
| `env-get` | `(name:Text -- value:Text\|Null)` | Get environment variable |
| `env-set` | `(name:Text value:Text -- )` | Set environment variable |

## OS: HTTP (3)
| Name | Signature | Description |
|------|-----------|-------------|
| `http-get` | `(url:Text -- body:Text)` | HTTP GET request |
| `http-post` | `(url:Text body:Text -- response:Text)` | HTTP POST request |
| `http-request` | `(method:Text url:Text body:Text headers:Map -- response:Map)` | Generic HTTP request |

## Data: JSON (2)
| Name | Signature | Description |
|------|-----------|-------------|
| `json-parse` | `(s:Text -- v:Any)` | Parse JSON string |
| `json-encode` | `(v:Any -- s:Text)` | Encode as JSON |

## Resources (5)
Query resource quotas and usage.

| Name | Signature | Description |
|------|-----------|-------------|
| `res-mem` | `( -- info:Map)` | Get memory quota info |
| `res-rom` | `( -- info:Map)` | Get storage quota info |
| `res-compute` | `( -- info:Map)` | Get compute quota info |
| `res-net` | `( -- info:Map)` | Get network quota info |
| `res-all` | `( -- info:Map)` | Get all resource quotas |

Each returns: `{total, used, reserved, free}`

## Capabilities (4)
Query granted capabilities.

| Name | Signature | Description |
|------|-----------|-------------|
| `cap-has` | `(cap:Text -- result:Bool)` | Check if capability granted |
| `cap-list` | `( -- caps:List)` | List all capabilities |
| `cap-fs` | `(path:Text mode:Text -- result:Bool)` | Check fs capability (read/write) |
| `cap-net` | `(host:Text port:Int mode:Text -- result:Bool)` | Check net capability |

## Session Memory (5)
Volatile key-value storage (cleared on restart). Uses `mem` resource quota.

| Name | Signature | Description |
|------|-----------|-------------|
| `mem-set` | `(value:Any key:Text -- )` | Store value under key |
| `mem-get` | `(key:Text -- value:Any)` | Get value (Null if not found) |
| `mem-del` | `(key:Text -- )` | Remove key |
| `mem-has` | `(key:Text -- exists:Bool)` | Check if key exists |
| `mem-keys` | `( -- keys:List)` | List all keys |

**Note:** `mem-set` uses `(value key --)` to match `def`'s `(body name --)` pattern.
Example: `42 "x" mem-set` mirrors `[dup mul] "square" def`

## Persistent Storage (5)
ROM storage that survives restarts. Uses `rom` resource quota.

| Name | Signature | Description |
|------|-----------|-------------|
| `rom-set` | `(value:Any key:Text -- )` | Store value under key |
| `rom-get` | `(key:Text -- value:Any)` | Get value (Null if not found) |
| `rom-del` | `(key:Text -- )` | Remove key |
| `rom-has` | `(key:Text -- exists:Bool)` | Check if key exists |
| `rom-keys` | `( -- keys:List)` | List all keys |

**Note:** `rom-set` uses `(value key --)` to match `def` and `mem-set` patterns.

---

## Capability Format

Capabilities are strings:

```
fs:read:/path     - Read files under /path
fs:write:/path    - Write files under /path
net:connect:*:80  - Connect to any host on port 80
net:listen:8080   - Listen on port 8080
exec              - Run shell commands
spawn             - Spawn sub-agents
env:read          - Read environment variables
env:write         - Write environment variables
all               - All capabilities (trusted mode)
```

Set via `KORE_CAPS` environment variable:
```bash
KORE_CAPS="fs:read:/tmp,fs:write:/tmp,exec" kore script.kore
```

## Resource Limits

Resources are abstract units (not bytes). Set via environment:

```bash
KORE_MEM_LIMIT=1000    # Session memory units
KORE_ROM_LIMIT=100     # Persistent storage units
KORE_COMPUTE_LIMIT=0   # 0 = unlimited
KORE_NET_LIMIT=0       # 0 = unlimited
```

---

## Tensor Operations (33)

Multi-dimensional arrays with automatic differentiation support.

### Creation
| Name | Signature | Description |
|------|-----------|-------------|
| `tensor-zeros` | `(shape:List -- tensor:Tensor)` | Create zero tensor |
| `tensor-ones` | `(shape:List -- tensor:Tensor)` | Create ones tensor |
| `tensor-rand` | `(shape:List -- tensor:Tensor)` | Random uniform [0,1] |
| `tensor-randn` | `(shape:List seed:Int -- tensor:Tensor)` | Random normal (seeded) |
| `tensor-from-list` | `(values:List -- tensor:Tensor)` | Create from flat list |

### Element-wise Operations
| Name | Signature | Description |
|------|-----------|-------------|
| `tensor-add` | `(a:Tensor b:Tensor -- c:Tensor)` | Element-wise add |
| `tensor-sub` | `(a:Tensor b:Tensor -- c:Tensor)` | Element-wise subtract |
| `tensor-mul` | `(a:Tensor b:Tensor -- c:Tensor)` | Element-wise multiply |
| `tensor-neg` | `(a:Tensor -- b:Tensor)` | Negate all elements |
| `tensor-scale` | `(a:Tensor s:Float -- b:Tensor)` | Multiply by scalar |

### Math Functions
| Name | Signature | Description |
|------|-----------|-------------|
| `tensor-exp` | `(a:Tensor -- b:Tensor)` | Element-wise exp |
| `tensor-log` | `(a:Tensor -- b:Tensor)` | Element-wise natural log |
| `tensor-relu` | `(a:Tensor -- b:Tensor)` | ReLU activation |
| `tensor-sigmoid` | `(a:Tensor -- b:Tensor)` | Sigmoid activation |
| `tensor-softmax` | `(a:Tensor -- b:Tensor)` | Softmax (last axis) |

### Reductions
| Name | Signature | Description |
|------|-----------|-------------|
| `tensor-sum` | `(a:Tensor -- b:Tensor)` | Sum all elements |
| `tensor-mean` | `(a:Tensor -- b:Tensor)` | Mean of all elements |
| `tensor-max` | `(a:Tensor -- b:Tensor)` | Max element |
| `tensor-argmax` | `(a:Tensor -- idx:Int)` | Index of max element |

### Matrix Operations
| Name | Signature | Description |
|------|-----------|-------------|
| `tensor-matmul` | `(a:Tensor b:Tensor -- c:Tensor)` | Matrix multiplication |
| `tensor-matmul-t` | `(a:Tensor b:Tensor -- c:Tensor)` | Matmul with B transposed |
| `tensor-outer` | `(a:Tensor b:Tensor -- c:Tensor)` | Outer product |
| `tensor-dot` | `(a:Tensor b:Tensor -- c:Tensor)` | Dot product |

### Shape & Access
| Name | Signature | Description |
|------|-----------|-------------|
| `tensor-shape` | `(a:Tensor -- shape:List)` | Get tensor shape |
| `tensor-rank` | `(a:Tensor -- n:Int)` | Number of dimensions |
| `tensor-size` | `(a:Tensor -- n:Int)` | Total elements |
| `tensor-get` | `(a:Tensor idx:Int -- v:Float)` | Get element |
| `tensor-set` | `(a:Tensor idx:Int v:Float -- b:Tensor)` | Set element |
| `tensor-unwrap` | `(a:Tensor -- list:List)` | Convert to list |
| `tensor-copy` | `(a:Tensor -- b:Tensor)` | Deep copy |
| `tensor-clip` | `(a:Tensor min:Float max:Float -- b:Tensor)` | Clamp values |
| `is-tensor` | `(v:Any -- result:Bool)` | Type check |

---

## Automatic Differentiation (5)

Reverse-mode automatic differentiation for tensor operations.

| Name | Signature | Description |
|------|-----------|-------------|
| `requires-grad` | `(t:Tensor -- t:Tensor)` | Mark for gradient tracking |
| `backward` | `(loss:Tensor -- grads:Map)` | Compute all gradients |
| `grad-get` | `(t:Tensor grads:Map -- grad:Tensor)` | Get gradient of tensor |
| `zero-grad` | `(t:Tensor -- t:Tensor)` | Reset gradient |
| `detach` | `(t:Tensor -- t:Tensor)` | Create copy without tracking |

### Example
```kore
# d/dx(x²) at x=2 should be 4
2.0 1 list tensor-from-list requires-grad
dup tensor-mul tensor-sum   # x² = 4
backward                     # compute gradients
# gradient is 4 (2 * x = 2 * 2)
```

---

## Linear Types (7)

Affine (use at most once) and linear (use exactly once) types for resource management.

| Name | Signature | Description |
|------|-----------|-------------|
| `linear-new` | `(v:Any -- linear:Linear)` | Create linear value |
| `linear-unwrap` | `(linear:Linear -- v:Any)` | Consume linear value |
| `affine-new` | `(v:Any -- affine:Affine)` | Create affine value |
| `affine-unwrap` | `(affine:Affine -- v:Any)` | Consume affine value |
| `is-linear` | `(v:Any -- result:Bool)` | Check if linear |
| `is-affine` | `(v:Any -- result:Bool)` | Check if affine |
| `linearity` | `(v:Any -- type:Text)` | Get linearity type |

---

## Effect Analysis (12)

Static analysis and formal verification tools.

| Name | Signature | Description |
|------|-----------|-------------|
| `effect-infer` | `(q:Quote -- analysis:Map)` | Full static analysis |
| `effect-compose` | `(e1:Map e2:Map -- e3:Map)` | Compose two effects |
| `effect-parse` | `(sig:Text -- effect:Map)` | Parse signature string |
| `effect-net` | `(e:Map -- n:Int)` | Get net stack change |
| `effect-valid?` | `(e:Map depth:Int -- result:Bool)` | Check if valid at depth |
| `effect-new` | `(consumes:Int produces:Int -- e:Map)` | Create effect |
| `io-effects` | `(q:Quote -- effects:List)` | Get IO effects |
| `pure?` | `(q:Quote -- result:Bool)` | Check if pure |
| `optimize` | `(q:Quote -- q:Quote)` | Algebraic optimization |
| `simplify` | `(q:Quote -- q:Quote)` | Apply identities only |
| `axioms` | `( -- axioms:List)` | Export algebraic identities |
| `selftest` | `( -- result:Map)` | Verify runtime invariants |

### Usage
```kore
# Infer effect of a quote
[dup mul] effect-infer           # => {effect: {consumes: 1, produces: 1}, pure: true, ...}

# Check if pure
[println] pure?                   # => false
[dup mul] pure?                   # => true

# List algebraic axioms
axioms 0 list-get                 # => {pattern: "swap swap", reduces_to: "", law: "involution"}

# Verify runtime integrity
selftest "passed" map-get         # => true
```
