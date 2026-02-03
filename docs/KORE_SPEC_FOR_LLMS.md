# Kore Language Specification for LLMs

> **Version**: 2.0 | **Date**: 2026-02-03 | **Status**: Production Ready
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

### 4.1 Stack Operations (9 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `dup` | `(a -- a a)` | Duplicate top |
| `drop` | `(a -- )` | Remove top |
| `swap` | `(a b -- b a)` | Swap top two |
| `over` | `(a b -- a b a)` | Copy second to top |
| `rot` | `(a b c -- b c a)` | Rotate top three |
| `nip` | `(a b -- b)` | Remove second |
| `tuck` | `(a b -- b a b)` | Copy top below second |
| `depth` | `( -- n)` | Push stack depth |
| `dip` | `(a q -- a)` | Execute quote under top |

**Linearity constraints:**
- `dup`: Rejects linear/affine values
- `drop`: Rejects linear values
- `over`: Rejects linear/affine values

### 4.2 Arithmetic (7 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `add` | `(a b -- sum)` | Addition |
| `sub` | `(a b -- diff)` | Subtraction (a - b) |
| `mul` | `(a b -- prod)` | Multiplication |
| `div` | `(a b -- quot)` | Division (a / b) |
| `mod` | `(a b -- rem)` | Modulo (a % b) |
| `neg` | `(a -- -a)` | Negation |
| `abs` | `(a -- |a|)` | Absolute value |

### 4.3 Comparison (6 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `eq` | `(a b -- bool)` | Equal |
| `neq` | `(a b -- bool)` | Not equal |
| `lt` | `(a b -- bool)` | Less than |
| `lte` | `(a b -- bool)` | Less or equal |
| `gt` | `(a b -- bool)` | Greater than |
| `gte` | `(a b -- bool)` | Greater or equal |

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

### 4.6 List Operations (13 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `list` | `(n a₁..aₙ -- list)` | Create list from N items |
| `unlist` | `(list -- a₁..aₙ)` | Explode list onto stack |
| `list-len` | `(list -- n)` | Get length |
| `list-get` | `(list n -- item)` | Get item at index |
| `list-set` | `(list n val -- list')` | Set item at index |
| `list-push` | `(list val -- list')` | Append item |
| `list-pop` | `(list -- list' item)` | Remove and return last |
| `list-first` | `(list -- item)` | Get first item |
| `list-last` | `(list -- item)` | Get last item |
| `list-reverse` | `(list -- list')` | Reverse list |
| `list-concat` | `(list₁ list₂ -- list')` | Concatenate |
| `list-slice` | `(list start end -- list')` | Get slice |
| `list-take` | `(list n -- list' item)` | Move item out (linear-safe) |

### 4.7 Map Operations (8 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `map-new` | `( -- map)` | Create empty map |
| `map-get` | `(map key -- val)` | Get value |
| `map-set` | `(map key val -- map')` | Set key-value |
| `map-del` | `(map key -- map')` | Delete key |
| `map-has` | `(map key -- bool)` | Check key exists |
| `map-keys` | `(map -- list)` | Get all keys |
| `map-vals` | `(map -- list)` | Get all values |
| `map-take` | `(map key -- map' val)` | Move value out (linear-safe) |

### 4.8 String Operations (16 tools)

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
| `str-upper` | `(s -- s')` | To uppercase |
| `str-lower` | `(s -- s')` | To lowercase |
| `str-trim` | `(s -- s')` | Trim whitespace |
| `char-code` | `(char -- n)` | Character to code point |
| `code-char` | `(n -- char)` | Code point to character |

### 4.9 Control Flow (10 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `call` | `(q -- ...)` | Execute quote |
| `if` | `(cond then else -- ...)` | Conditional |
| `when` | `(cond q -- )` | Execute if true |
| `unless` | `(cond q -- )` | Execute if false |
| `times` | `(n q -- ...)` | Repeat N times |
| `while` | `(cond-q body-q -- )` | While loop |
| `loop` | `(q -- )` | Infinite loop (until break) |
| `try` | `(q -- result)` | Try, catch errors |
| `fail` | `(msg -- )` | Raise error |
| `unwrap` | `(result -- val)` | Unwrap or propagate error |

### 4.10 Definition Tools (5 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `def` | `(val name -- )` | Define word |
| `words` | `( -- list)` | List all defined words |
| `meta` | `(name -- map)` | Get tool metadata |
| `meta!` | `(name key val -- )` | Set metadata field |
| `defined?` | `(name -- bool)` | Check if word exists |

### 4.11 Combinators (4 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `map` | `(list q -- list')` | Apply to each element |
| `filter` | `(list q -- list')` | Keep elements where q returns true |
| `fold` | `(list init q -- result)` | Reduce list |
| `each` | `(list q -- )` | Execute for each (no result) |

### 4.12 I/O Tools (6 tools) [Requires capabilities]

| Tool | Stack Effect | IO Effect | Description |
|------|--------------|-----------|-------------|
| `print` | `(val -- )` | io | Print without newline |
| `println` | `(val -- )` | io | Print with newline |
| `fs-read` | `(path -- text)` | fs | Read file |
| `fs-write` | `(path text -- )` | fs | Write file |
| `fs-exists` | `(path -- bool)` | fs | Check file exists |
| `json-parse` | `(text -- val)` | pure | Parse JSON |
| `json-encode` | `(val -- text)` | pure | Encode to JSON |

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

### 4.14 Effect Analysis Tools (7 tools)

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

### 4.15 Time Tools (2 tools)

| Tool | Stack Effect | IO Effect | Description |
|------|--------------|-----------|-------------|
| `now` | `( -- timestamp)` | time | Current time |
| `sleep` | `(ms -- )` | time | Sleep milliseconds |

### 4.16 Tensor Tools (21 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `tensor-from-list` | `(list -- tensor)` | Create from list |
| `tensor-to-list` | `(tensor -- list)` | Convert to list |
| `tensor-zeros` | `(shape -- tensor)` | Zeros tensor |
| `tensor-ones` | `(shape -- tensor)` | Ones tensor |
| `tensor-randn` | `(shape seed -- tensor)` | Random normal |
| `tensor-shape` | `(tensor -- shape)` | Get shape |
| `tensor-add` | `(t₁ t₂ -- t)` | Element-wise add |
| `tensor-sub` | `(t₁ t₂ -- t)` | Element-wise subtract |
| `tensor-mul` | `(t₁ t₂ -- t)` | Element-wise multiply |
| `tensor-div` | `(t₁ t₂ -- t)` | Element-wise divide |
| `tensor-matmul` | `(A B rows cols -- C)` | Matrix multiply |
| `tensor-matmul-t` | `(A B rows cols -- C)` | Matmul with transpose |
| `tensor-outer` | `(v₁ v₂ -- matrix)` | Outer product |
| `tensor-scale` | `(tensor scalar -- tensor')` | Scalar multiply |
| `tensor-sum` | `(tensor -- scalar)` | Sum all elements |
| `tensor-mean` | `(tensor -- scalar)` | Mean of elements |
| `tensor-max` | `(tensor -- scalar)` | Maximum element |
| `tensor-min` | `(tensor -- scalar)` | Minimum element |
| `tensor-argmax` | `(tensor -- index)` | Index of maximum |
| `tensor-softmax` | `(tensor -- tensor')` | Softmax |
| `tensor-relu` | `(tensor -- tensor')` | ReLU activation |
| `tensor-relu-bwd` | `(tensor grad -- grad')` | ReLU gradient |
| `tensor-log` | `(tensor -- tensor')` | Element-wise log |
| `tensor-exp` | `(tensor -- tensor')` | Element-wise exp |

### 4.17 Capability/Resource Tools (8 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `cap-has` | `(cap -- bool)` | Check capability |
| `cap-get` | `( -- capset)` | Get current caps |
| `cap-attenuate` | `(cap subset -- cap')` | Reduce capabilities |
| `cap-leq` | `(cap₁ cap₂ -- bool)` | Check ≤ relation |
| `cap-join` | `(cap₁ cap₂ -- cap')` | Union (∨) |
| `cap-meet` | `(cap₁ cap₂ -- cap')` | Intersection (∧) |
| `res-has` | `(amount resource -- bool)` | Check resource |
| `res-split` | `(res amount -- res₁ res₂)` | Split resource |

### 4.18 Memory Tools (8 tools)

| Tool | Stack Effect | IO Effect | Description |
|------|--------------|-----------|-------------|
| `mem-get` | `(key -- val)` | mem | Get from memory |
| `mem-set` | `(key val -- )` | mem | Set in memory |
| `mem-del` | `(key -- )` | mem | Delete from memory |
| `mem-keys` | `( -- list)` | mem | List memory keys |
| `rom-get` | `(key -- val)` | mem | Get from read-only |
| `rom-set` | `(key val -- )` | mem | Set in ROM (init only) |
| `rom-del` | `(key -- )` | mem | Delete from ROM |
| `rom-keys` | `( -- list)` | mem | List ROM keys |

### 4.19 Trace Tools (3 tools)

| Tool | Stack Effect | Description |
|------|--------------|-------------|
| `trace-new` | `( -- trace)` | Create empty trace |
| `trace-step` | `(name trace -- trace')` | Add step |
| `trace-fingerprint` | `(trace -- hash)` | Get trace hash |

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

### 6.4 Equivalences

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
  [ dup 0 gt ] [                  ; while n > 0
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
  [ dup 0 gt ] [
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
] when
```

### 8.2 Unwrap with Default

```kore
: unwrap-or ( result default -- value )
  swap dup is-error [ drop ] [ nip ] if
;
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

*This specification is designed for LLM consumption. All tools are verified through 456+ automated tests.*
