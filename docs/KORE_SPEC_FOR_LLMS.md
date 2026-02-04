# Kore Language Specification for LLMs

> **Version**: 2.1 | **Date**: 2026-02-04 | **Status**: Production Ready
> 
> This document is the authoritative machine-readable specification for generating Kore code.

## 1. Execution Model

Kore is a **stack-based** language built on three postulates:

```
Postulate 1: Everything is a Tool
  Tool : Stack → Stack
  
Postulate 2: Tools Transform Stacks  
  execute(tool, stack) = stack'
  
Postulate 3: Composition is Concatenation
  (f ; g)(s) = g(f(s))
```

### 1.1 Stack Semantics

- Values are pushed onto a LIFO stack
- Tools consume values from top, push results
- Notation: `( before -- after )` describes stack effect
- Top of stack is rightmost: `1 2 3` means 3 is on top

### 1.2 Two Operations Only

```
Op ::= Push(Value) | Call(ToolName)
```

Every Kore program is a sequence of Push and Call operations.

---

## 2. Type System

### 2.1 Core Types (10)

| Type | Literal Syntax | Example |
|------|---------------|---------|
| Null | `null` | `null` |
| Bool | `true`, `false` | `true` |
| Int | digits, negative with `-` | `42`, `-17` |
| Float | digits with `.` | `3.14`, `-0.5` |
| Text | `"..."` | `"hello"` |
| List | `[...]` | `[1 2 3]` |
| Map | `{key: val ...}` | `{x: 1 y: 2}` |
| Quote | `[...]` (when contains calls) | `[dup mul]` |
| Handle | runtime only | (opaque) |
| Error | runtime only | (opaque) |

### 2.2 Linearity (Kore 2.0)

| Linearity | Can Dup? | Can Drop? | Use |
|-----------|----------|-----------|-----|
| Unrestricted | ✅ | ✅ | Normal values |
| Affine | ❌ | ✅ | At-most-once (Handles) |
| Linear | ❌ | ❌ | Exactly-once |

---

## ⚠️ CRITICAL PITFALLS (Read First!)

> **This section documents common LLM mistakes when generating Kore code.**

### Pitfall 1: `[ 1 2 3 ]` Creates a QUOTE, Not a List!

```kore
; WRONG - this is a Quote, not a List:
[ 1 2 3 ]              ; type-of => "quote"

; CORRECT - create a List:
3 1 2 3 list           ; Creates List [1, 2, 3] - N items followed by N values
0 list 1 list-push 2 list-push 3 list-push  ; Also creates [1, 2, 3]
```

**Key insight**: `[ ... ]` is for code quotation (delayed execution), not data.

### Pitfall 2: Comparison Operators Don't Exist

These tools are **NOT defined**: `gt`, `neq`, `le`, `ge`, `lte`, `gte`

```kore
; WRONG - will fail with "Unknown tool: gt"
5 3 gt

; CORRECT - compose inline:
5 3 swap lt            ; 5 > 3 → true (swap then lt)
5 3 eq not             ; 5 ≠ 3 → true
3 5 swap lt not        ; 3 ≤ 5 → true
```

### Pitfall 3: `abs`, `when`, `unless` Don't Exist

```kore
; WRONG:
-5 abs                 ; Unknown tool: abs
x [ do-something ] when  ; Unknown tool: when

; CORRECT - compose inline:
-5 dup 0 lt [ neg ] [ ] if     ; abs pattern
x [ do-something ] [ ] if      ; when pattern  
x [ ] [ do-something ] if      ; unless pattern
```

### Pitfall 4: `list-push` Appends, Doesn't Prepend

```kore
0 list 1 list-push 2 list-push 3 list-push
; Result: [1, 2, 3] - values appear in push order
```

### Pitfall 5: Can't Print Lists Directly

```kore
; Lists don't have a direct text representation for println
; Either extract elements or use for debugging:
my-list list-len to-text println  ; Print the length
my-list 0 list-get to-text println  ; Print first element
```

---

## 3. Syntax

### 3.1 Lexical Grammar

```
program     ::= item*
item        ::= literal | word | quote | comment
literal     ::= null | bool | int | float | text
word        ::= [a-zA-Z_][a-zA-Z0-9_?!-]*
quote       ::= '[' item* ']'
comment     ::= ';' [^\n]* '\n'
```

### 3.2 Definition Syntax

```kore
; Define a constant
42 "answer" def

; Define a function  
[ dup mul ] "square" def

; Use it
5 square    ; => 25
```

### 3.3 Control Flow

```kore
; Conditional
condition [ then-branch ] [ else-branch ] if

; Loop N times
5 [ "hi" println ] times

; While loop
[ condition ] [ body ] while

; Error handling
[ risky-code ] try    ; => result or error
```

---

## 4. Complete Tool Reference

### 4.1 Stack Operations (7 primitives)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `dup` | `(a -- a a)` | Duplicate top |
| `drop` | `(a -- )` | Remove top |
| `swap` | `(a b -- b a)` | Swap top two |
| `over` | `(a b -- a b a)` | Copy second to top |
| `rot` | `(a b c -- b c a)` | Rotate top three |
| `depth` | `( -- n)` | Push stack depth |
| `dip` | `(a q -- ... a)` | Execute quote under top |

**Composed** (not primitives):
| Tool | Definition | Description |
|------|------------|-------------|
| `nip` | `swap drop` | Remove second |
| `tuck` | `swap over` | Copy top below second |

**Linearity constraints:**
- `dup`: Rejects linear/affine values
- `drop`: Rejects linear values
- `over`: Rejects linear/affine values

### 4.2 Arithmetic (6 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `add` | `(a b -- sum)` | Addition |
| `sub` | `(a b -- diff)` | Subtraction (a - b) |
| `mul` | `(a b -- prod)` | Multiplication |
| `div` | `(a b -- quot)` | Division (a / b) |
| `mod` | `(a b -- rem)` | Modulo (a % b) |
| `neg` | `(a -- -a)` | Negation |

**WARNING**: `abs` is NOT a built-in tool. Compose it as: `dup 0 lt [ neg ] [ ] if`

### 4.3 Comparison (2 primitives ONLY)

**CRITICAL**: Only `eq` and `lt` exist as built-in tools. All other comparisons must be composed inline.

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `eq` | `(a b -- bool)` | Equal |
| `lt` | `(a b -- bool)` | Less than |

**Composition Patterns** (NOT defined tools - use these patterns inline):

| Pattern | How to Write | Example |
|---------|--------------|---------|
| Greater than | `swap lt` | `5 3 swap lt` → true (5 > 3) |
| Not equal | `eq not` | `5 3 eq not` → true |
| Less or equal | `swap lt not` | `3 5 swap lt not` → true (3 ≤ 5) |
| Greater or equal | `lt not` | `5 3 lt not` → true (5 ≥ 3) |

**WARNING**: `gt`, `neq`, `lte`, `gte`, `le`, `ge` are NOT defined tools. You MUST write the composition inline.

### 4.4 Logic (3 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `and` | `(a b -- bool)` | Logical AND |
| `or` | `(a b -- bool)` | Logical OR |
| `not` | `(a -- bool)` | Logical NOT |

### 4.5 Type Operations (14 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `type-of` | `(a -- text)` | Get type name |
| `is-null` | `(a -- bool)` | Check if null |
| `is-bool` | `(a -- bool)` | Check if bool |
| `is-int` | `(a -- bool)` | Check if int |
| `is-float` | `(a -- bool)` | Check if float |
| `is-text` | `(a -- bool)` | Check if text |
| `is-list` | `(a -- bool)` | Check if list |
| `is-map` | `(a -- bool)` | Check if map |
| `is-quote` | `(a -- bool)` | Check if quote |
| `is-error` | `(a -- bool)` | Check if error |
| `to-int` | `(a -- int)` | Convert to int |
| `to-float` | `(a -- float)` | Convert to float |
| `to-text` | `(a -- text)` | Convert to text |
| `to-bool` | `(a -- bool)` | Convert to bool |

### 4.6 List Operations (9 primitives)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `list` | `(n a₁..aₙ -- list)` | Create list from N items |
| `unlist` | `(list -- a₁..aₙ)` | Explode list onto stack |
| `list-len` | `(list -- n)` | Get length |
| `list-get` | `(list n -- item)` | Get item at index (clones, not for linear) |
| `list-set` | `(list n val -- list')` | Set item at index |
| `list-push` | `(list val -- list')` | Append item |
| `list-pop` | `(list -- list' item)` | Remove and return last |
| `list-reverse` | `(list -- list')` | Reverse list |
| `list-concat` | `(list₁ list₂ -- list')` | Concatenate |
| `list-slice` | `(list start end -- list')` | Get slice |
| `list-take` | `(list n -- list' item)` | Move item out (linear-safe) |

**Note**: `list-first` = `0 list-get`, `list-last` = `dup list-len 1 sub list-get`

### 4.7 Map Operations (6 primitives)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `map-new` | `( -- map)` | Create empty map |
| `map-get` | `(map key -- val)` | Get value |
| `map-set` | `(map key val -- map')` | Set key-value |
| `map-del` | `(map key -- map')` | Delete key |
| `map-has` | `(map key -- bool)` | Check key exists |
| `map-keys` | `(map -- list)` | Get all keys |
| `map-vals` | `(map -- list)` | Get all values |

**Note**: `map-take` for linear values planned but use `map-get` + `map-del` for now

### 4.8 String Operations (13 primitives)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `str-len` | `(s -- n)` | String length |
| `str-get` | `(s n -- char)` | Get character at index |
| `str-slice` | `(s start end -- s')` | Get substring |
| `str-concat` | `(s₁ s₂ -- s')` | Concatenate |
| `str-split` | `(s delim -- list)` | Split by delimiter |
| `str-join` | `(list delim -- s)` | Join with delimiter |
| `str-find` | `(s sub -- n)` | Find substring index |
| `str-replace` | `(s old new -- s')` | Replace all occurrences |
| `str-starts` | `(s prefix -- bool)` | Check prefix |
| `str-ends` | `(s suffix -- bool)` | Check suffix |
| `str-trim` | `(s -- s')` | Trim whitespace |
| `char-code` | `(char -- n)` | Character to code point |
| `code-char` | `(n -- char)` | Code point to character |

**Note**: `str-upper`, `str-lower` planned but not yet implemented

### 4.9 Control Flow (8 primitives)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `call` | `(q -- ...)` | Execute quote |
| `if` | `(cond then else -- ...)` | Conditional |
| `times` | `(n q -- ...)` | Repeat N times |
| `while` | `(cond-q body-q -- )` | While loop |
| `loop` | `(q -- )` | Infinite loop (until break) |
| `try` | `(q -- result)` | Try, catch errors |
| `fail` | `(msg -- )` | Raise error |
| `spawn` | `(q caps -- handle)` | Create sandboxed context |

**Composed**: `unwrap` = `dup is-error [ fail ] [ ] if`

**Composition Patterns** (NOT defined tools - write inline):
- `when` pattern: `condition [ body ] [ ] if`
- `unless` pattern: `condition [ ] [ body ] if`

**WARNING**: `when` and `unless` are NOT defined tools. Use the patterns above.

### 4.10 Definition Tools (4 primitives)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `def` | `(val name -- )` | Define word |
| `def-verified` | `(quote name sig -- )` | Define with effect verification |
| `words` | `( -- list)` | List all defined words |
| `meta` | `(name -- map)` | Get tool metadata |

**NEW in 2.1**: `def-verified` verifies the effect matches before defining:
```kore
[ dup mul ] "square" "(n -- n)" def-verified  ; OK - effect matches
[ dup ] "bad" "(a -- a)" def-verified          ; FAILS - effect is (1 -- 2)
```

### 4.11 Combinators (4 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `map` | `(list q -- list')` | Apply to each element |
| `filter` | `(list q -- list')` | Keep elements where q returns true |
| `fold` | `(list init q -- result)` | Reduce list |
| `each` | `(list q -- )` | Execute for each (no result) |

### 4.12 I/O Tools [Requires capabilities]

| Tool | Stack Effect | Capability | Description |
|------|--------------|------------|-------------|
| `print` | `(val -- )` | io | Print without newline |
| `println` | `(val -- )` | io | Print with newline |
| `read-line` | `( -- text)` | io | Read line from stdin |
| `log` | `(val level -- )` | io | Log with level |
| `fs-read` | `(path -- text)` | fs | Read file |
| `fs-write` | `(path text -- )` | fs | Write file |
| `fs-append` | `(path text -- )` | fs | Append to file |
| `fs-exists` | `(path -- bool)` | fs | Check file exists |
| `fs-list` | `(path -- list)` | fs | List directory |
| `fs-rm` | `(path -- )` | fs | Remove file |
| `fs-mkdir` | `(path -- )` | fs | Create directory |
| `http-get` | `(url -- resp)` | net | HTTP GET |
| `http-post` | `(url body -- resp)` | net | HTTP POST |
| `http-request` | `(req -- resp)` | net | Full HTTP request |
| `json-parse` | `(text -- val)` | pure | Parse JSON |
| `json-encode` | `(val -- text)` | pure | Encode to JSON |
| `env-get` | `(name -- val)` | env | Get env variable |
| `env-set` | `(name val -- )` | env | Set env variable |
| `exec` | `(cmd args -- code)` | process | Execute command |
| `pid` | `( -- n)` | process | Get process ID |
| `cwd` | `( -- path)` | process | Current directory |
| `args` | `( -- list)` | process | Command line args |
| `exit` | `(code -- )` | process | Exit with code |

### 4.13 Linear Type Tools (7 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `linear-new` | `(val -- linear)` | Wrap as linear |
| `linear-unwrap` | `(linear -- val)` | Consume linear value |
| `affine-new` | `(val -- affine)` | Wrap as affine |
| `affine-unwrap` | `(affine -- val)` | Consume affine value |
| `is-linear` | `(val -- bool)` | Check if linear |
| `is-affine` | `(val -- bool)` | Check if affine |
| `linearity` | `(val -- sym)` | Get linearity as symbol |

### 4.14 Effect Analysis Tools (12 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `effect-compose` | `(e₁ e₂ -- e₃)` | Compose effects |
| `effect-parse` | `(sig -- effect)` | Parse signature |
| `effect-net` | `(effect -- n)` | Get net stack change |
| `effect-valid?` | `(effect depth -- bool)` | Check validity |
| `effect-new` | `(cons prod -- effect)` | Create effect |
| `effect-infer` | `(quote -- analysis)` | Full static analysis |
| `io-effects` | `(quote -- list)` | Get IO effects |
| `pure?` | `(quote -- bool)` | Check if pure |
| `optimize` | `(quote -- quote')` | Algebraic optimization |
| `simplify` | `(quote -- quote')` | Identities only |
| `axioms` | `( -- list)` | Export algebraic identities |
| `selftest` | `( -- map)` | Verify runtime invariants |

**NEW in 2.1**: `axioms` and `selftest` for formal verification:
```kore
; List all algebraic identities
axioms list-len println  ; "21" - number of axioms
axioms 0 list-get        ; {pattern: "swap swap", reduces_to: "", law: "involution"}

; Verify runtime integrity
selftest "passed" map-get  ; true if all invariants hold
```

### 4.15 Time Tools (2 tools)

| Tool | Stack Effect | IO Effect | Description |
|------|--------------|-----------|-------------|
| `now` | `( -- timestamp)` | time | Current time |
| `sleep` | `(ms -- )` | time | Sleep milliseconds |

### 4.16 Tensor Tools (33 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `tensor-from-list` | `(list -- tensor)` | Create from nested list |
| `tensor-unwrap` | `(tensor -- list)` | Convert to nested list |
| `tensor-zeros` | `(shape -- tensor)` | Zeros tensor |
| `tensor-ones` | `(shape -- tensor)` | Ones tensor |
| `tensor-rand` | `(shape -- tensor)` | Random uniform [0,1) |
| `tensor-randn` | `(shape seed -- tensor)` | Random normal |
| `tensor-shape` | `(tensor -- shape)` | Get shape list |
| `tensor-rank` | `(tensor -- n)` | Get rank (dimensions) |
| `tensor-size` | `(tensor -- n)` | Total element count |
| `tensor-get` | `(tensor indices -- val)` | Get element |
| `tensor-set` | `(tensor indices val -- tensor')` | Set element |
| `tensor-copy` | `(tensor -- tensor')` | Deep copy |
| `tensor-add` | `(t₁ t₂ -- t)` | Element-wise add |
| `tensor-sub` | `(t₁ t₂ -- t)` | Element-wise subtract |
| `tensor-mul` | `(t₁ t₂ -- t)` | Element-wise multiply |
| `tensor-neg` | `(tensor -- tensor')` | Negate all elements |
| `tensor-scale` | `(tensor scalar -- tensor')` | Scalar multiply |
| `tensor-dot` | `(v₁ v₂ -- scalar)` | Dot product |
| `tensor-matmul` | `(A B -- C)` | Matrix multiply |
| `tensor-matmul-t` | `(A B -- C)` | Matmul with B transposed |
| `tensor-outer` | `(v₁ v₂ -- matrix)` | Outer product |
| `tensor-sum` | `(tensor -- scalar)` | Sum all elements |
| `tensor-mean` | `(tensor -- scalar)` | Mean of elements |
| `tensor-max` | `(tensor -- scalar)` | Maximum element |
| `tensor-argmax` | `(tensor -- index)` | Index of maximum |
| `tensor-clip` | `(tensor min max -- tensor')` | Clip to range |
| `tensor-softmax` | `(tensor -- tensor')` | Softmax |
| `tensor-sigmoid` | `(tensor -- tensor')` | Sigmoid activation |
| `tensor-relu` | `(tensor -- tensor')` | ReLU activation |
| `tensor-relu-bwd` | `(tensor grad -- grad')` | ReLU gradient |
| `tensor-log` | `(tensor -- tensor')` | Element-wise log |
| `tensor-exp` | `(tensor -- tensor')` | Element-wise exp |

### 4.17 Capability/Resource Tools (11 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `cap-list` | `( -- list)` | List all capabilities |
| `cap-has` | `(cap -- bool)` | Check if capability held |
| `cap-fs` | `(path -- cap)` | Create filesystem cap |
| `cap-net` | `(host -- cap)` | Create network cap |
| `cap-attenuate` | `(cap subset -- cap')` | Reduce capabilities |
| `cap-leq` | `(cap₁ cap₂ -- bool)` | Check ≤ relation |
| `cap-join` | `(cap₁ cap₂ -- cap')` | Union (∨) |
| `cap-meet` | `(cap₁ cap₂ -- cap')` | Intersection (∧) |
| `res-has` | `(amount resource -- bool)` | Check resource amount |
| `res-add` | `(res₁ res₂ -- res)` | Combine resources |
| `res-split` | `(res amount -- res₁ res₂)` | Split resource |

### 4.18 Memory Tools (10 tools)

| Tool | Stack Effect | IO Effect | Description |
|------|--------------|-----------|-------------|
| `mem-get` | `(key -- val)` | mem | Get from memory |
| `mem-set` | `(val key -- )` | mem | Set in memory (value first, like def) |
| `mem-del` | `(key -- )` | mem | Delete from memory |
| `mem-has` | `(key -- bool)` | mem | Check key exists |
| `mem-keys` | `( -- list)` | mem | List memory keys |
| `rom-get` | `(key -- val)` | mem | Get from read-only |
| `rom-set` | `(val key -- )` | mem | Set in ROM (value first, like def) |
| `rom-del` | `(key -- )` | mem | Delete from ROM |
| `rom-has` | `(key -- bool)` | mem | Check ROM key exists |
| `rom-keys` | `( -- list)` | mem | List ROM keys |

### 4.19 Quote Serialization (2 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `quote-to-text` | `(q -- s)` | Serialize quote to Kore source |
| `text-to-quote` | `(s -- q)` | Parse Kore source to quote |

### 4.20 Trace Tools (3 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `trace-new` | `( -- trace)` | Create empty trace |
| `trace-step` | `(name trace -- trace')` | Add step |
| `trace-fingerprint` | `(trace -- hash)` | Get trace hash |

### 4.20 Autodiff Tools (5 tools)

Reverse-mode automatic differentiation for tensor operations.

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `requires-grad` | `(tensor -- tensor')` | Mark tensor for gradient tracking |
| `backward` | `(loss -- grads)` | Compute gradients (loss must be scalar) |
| `grad-get` | `(tensor grads -- grad)` | Get gradient of tensor |
| `zero-grad` | `(tensor -- tensor')` | Clear gradient info |
| `detach` | `(tensor -- tensor')` | Remove from computation graph |

**Autodiff-aware operations**: tensor-add, tensor-mul, tensor-sum, tensor-relu, tensor-sigmoid, tensor-softmax, tensor-log, tensor-exp, tensor-neg, tensor-matmul

### 4.21 Linear Type Tools (7 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `linear-new` | `(val -- linear)` | Wrap as linear (must use exactly once) |
| `linear-unwrap` | `(linear -- val)` | Consume linear value |
| `affine-new` | `(val -- affine)` | Wrap as affine (use at most once) |
| `affine-unwrap` | `(affine -- val)` | Consume affine value |
| `is-linear` | `(val -- bool)` | Check if linear type |
| `is-affine` | `(val -- bool)` | Check if affine type |
| `linearity` | `(val -- sym)` | Get linearity: `none`, `affine`, or `linear` |

### 4.22 Tensor Type Predicate

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `is-tensor` | `(val -- bool)` | Check if tensor type |

---

## 5. Effect Algebra

### 5.1 Stack Effect Composition

```
Effect = (consumes: Nat, produces: Nat)

compose((a,b), (c,d)) = 
  if b >= c 
  then (a, b - c + d)
  else (a + c - b, d)

net(a, b) = b - a
```

### 5.2 IO Effect Categories

| Effect | Operations |
|--------|------------|
| `fs` | fs-read, fs-write, fs-exists |
| `net` | http-get, http-post, net-* |
| `spawn` | spawn |
| `time` | now, sleep |
| `io` | print, println |
| `env` | env-get, env-set |
| `exec` | exec, shell |
| `mem` | mem-get, mem-set |

IO effects form a semilattice: `effects(A ; B) = effects(A) ∪ effects(B)`

---

## 6. Algebraic Identities

The optimizer automatically applies these simplifications before execution.

### 6.1 Stack Involutions

```
swap swap = ε
not not = ε
neg neg = ε
```

### 6.2 Finite Order

```
rot rot rot = ε  (order 3)
```

### 6.3 Annihilation

```
dup drop = ε
over drop = ε
```

### 6.4 Tensor Algebraic Identities

```
tensor-neg tensor-neg = ε
tensor-exp tensor-log = ε
tensor-log tensor-exp = ε
```

### 6.5 Equivalences

```
over nip = dup
swap nip = drop
```

---

## 7. Code Patterns

### 7.1 Factorial

```kore
: factorial ( n -- n! )
  1 swap                          ; acc n
  [ dup 0 swap lt ] [             ; while 0 < n (n > 0)
    dup rot mul swap              ; acc*n n
    1 sub                         ; acc' n-1
  ] while
  drop                            ; result
;
```

### 7.2 Fibonacci

```kore
: fib ( n -- fib(n) )
  0 1 rot                         ; a b n
  [ dup 0 swap lt ] [             ; while 0 < n (n > 0)
    1 sub                         ; a b n-1
    rot rot                       ; n-1 a b
    over add                      ; n-1 a a+b
    rot                           ; a a+b n-1
  ] while
  drop drop                       ; result
;
```

### 7.3 Map with Linear Values

```kore
; Safe extraction from list containing linear values
: extract-first ( list -- list' item )
  0 list-take                     ; moves, doesn't copy
;
```

### 7.4 Static Analysis

```kore
; Check if code is safe before running
[ fs-read println ] effect-infer
; => { effect: {consumes: 1, produces: 0}, io: ["fs", "io"], pure: false, safe: true }

[ 1 2 add ] pure?  ; => true
```

---

## 8. Error Handling

### 8.1 Try/Catch Pattern

```kore
[ risky-operation ]
try
dup is-error [
  ; handle error
  drop "default"
] [ ] if
```

### 8.2 Unwrap with Default

```kore
: unwrap-or ( result default -- value )
  swap dup is-error [ drop ] [ nip ] if
;
```

### 8.3 Structured Error Info (NEW in 2.1)

```kore
; Get machine-parseable error details
[ unknown-tool ] try
dup is-error [
  error-info              ; Convert to map
  dup "code" map-get println     ; "E_TOOL_NOT_FOUND"
  "message" map-get println      ; "Tool not found: unknown-tool"
] [ ] if
```

The `error-info` tool returns a map with structured fields:
- `code`: Error code (e.g., `E_STACK_UNDERFLOW`, `E_TYPE`)
- `message`: Human-readable message
- Additional fields vary by error type
```

---

## 9. Complete Grammar (EBNF)

```ebnf
program     = { item } ;
item        = value | word | definition ;
value       = null | bool | number | text | list | quote ;
null        = "null" ;
bool        = "true" | "false" ;
number      = integer | float ;
integer     = [ "-" ], digit, { digit } ;
float       = [ "-" ], digit, { digit }, ".", digit, { digit } ;
text        = '"', { char }, '"' ;
list        = "[", { item }, "]" ;
quote       = "[", { item }, "]" ;  (* context-dependent *)
word        = identifier ;
identifier  = (letter | "_"), { letter | digit | "_" | "?" | "!" | "-" } ;
definition  = ":", identifier, { item }, ";" ;
comment     = ";", { any except newline }, newline ;
```

---

## 10. Semantic Constraints

1. **Stack Safety**: Effect (consumes, produces) must match runtime behavior
2. **Linearity**: Linear values cannot be duplicated or discarded
3. **Affinity**: Affine values cannot be duplicated (can be dropped)
4. **Capabilities**: IO operations require matching capabilities
5. **Resources**: spawn consumes allocated resource budget

---

## 11. Meta-Programming & Introspection

### 11.1 Discovering Available Tools

```kore
; List all defined words
words  ; => ["factorial" "fib" "square" ...]

; Get metadata about any tool
"map" meta
; => { effect: "(2 -- 1)", doc: "Apply quote to each element", io: [] }

; Check if a word exists (use defined? instead)
"http-get" defined?  ; => true if http-get available
```

### 11.2 Verifying Code Before Execution

```kore
; Parse and validate a signature
"(a b -- c)" effect-parse
; => { consumes: 2, produces: 1 }

; Infer effect of arbitrary code
[ dup mul ] effect-infer
; => { effect: {consumes: 1, produces: 1}, io: [], pure: true, safe: true }

; Check if code is pure (no IO)
[ 1 2 add ] pure?  ; => true
[ "hi" println ] pure?  ; => false

; Validate effect matches expected
[ swap drop ] effect-infer "effect" map-get
dup "consumes" map-get 2 eq
swap "produces" map-get 1 eq
and  ; => true
```

### 11.3 Serializing Programs with Annotations

```kore
; Define a capability-aware function with metadata
[ 
  ; Read file, parse JSON, return specific field
  fs-read json-parse "data" map-get
] "read-json-field" def

; Attach metadata (convention: use ROM for function metadata)
{
  effect: "(path field -- value)"
  capabilities: ["fs"]
  doc: "Read JSON file and extract field"
  version: "1.0"
} "read-json-field:meta" rom-set

; Later retrieve
"read-json-field:meta" rom-get
```

### 11.4 Self-Verification Pattern

```kore
; Before defining, verify the code does what we expect
: verified-square ( n -- n² )
  ; First, test our implementation
  [ dup mul ] 
  dup effect-infer "effect" map-get
  dup "consumes" map-get 1 eq
  swap "produces" map-get 1 eq
  and
  [ call ] [ drop "effect mismatch" fail ] if
;

; Or inline test
5 verified-square 25 eq [ "test passed" ] [ "test failed" fail ] if
```

### 11.5 Accessing External Resources

```kore
; When uncertain about implementation, fetch documentation
"https://docs.kore.dev/api/tensor" http-get 
json-parse "examples" map-get

; Search for usage patterns
"https://api.kore.dev/search" 
{ query: "tensor matmul example" } json-encode
http-post json-parse

; Cache results for reuse
dup "tensor-matmul-examples" mem-set
```

### 11.6 Capability Serialization for Sandboxed Execution

```kore
; Create a restricted capability set
"fs" cap-fs "/tmp/sandbox" cap-attenuate
"net" cap-net "api.example.com" cap-attenuate
cap-join  ; combined caps

; Spawn sandboxed execution with serialized capabilities
[ 
  ; This code runs with only /tmp/sandbox fs and api.example.com net
  "/tmp/sandbox/input.json" fs-read
  json-parse process-data json-encode
  "https://api.example.com/submit" swap http-post
] swap spawn

; The capability set can be serialized for later use
{ 
  fs: ["/tmp/sandbox"]
  net: ["api.example.com"]
  spawn: false
  exec: false
} "sandbox-profile" rom-set
```

---

## 12. Training Curriculum Notes

> *This section describes capabilities progressively unlocked during RL training.*

### Phase 1: Core Stack & Arithmetic
- Stack manipulation: `dup`, `drop`, `swap`, `rot`, `over`
- Arithmetic: `add`, `sub`, `mul`, `div`, `mod`
- Basic types and literals

### Phase 2: Control Flow & Definitions
- Conditionals: `if` (compose `when`/`unless` patterns from `if`)
- Loops: `times`, `while`
- Definitions: `def`, named functions

### Phase 3: Data Structures
- Lists: creation, access, manipulation
- Maps: creation, get/set, iteration
- Combinators: `map`, `filter`, `fold`

### Phase 4: Introspection & Verification
- `words`, `meta` - discover available tools
- `effect-infer`, `pure?` - verify code properties
- Self-testing patterns

### Phase 5: I/O & Capabilities
- File system: `fs-read`, `fs-write`
- Network: `http-get`, `http-post`
- Capability awareness: `cap-has`, `cap-attenuate`

### Phase 6: Advanced Patterns
- Fetch documentation when uncertain
- Serialize capability profiles
- Spawn sandboxed workers
- Linear type handling

---

*This specification is designed for LLM consumption. All tools are verified through 456+ automated tests.*
