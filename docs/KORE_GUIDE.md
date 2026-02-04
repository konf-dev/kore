# The Kore Programming Language

## A Complete Guide for Humans

> *"Kore doesn't trust the Author. It trusts the Algebra."*

---

## Table of Contents

1. [Why Kore Exists](#1-why-kore-exists)
2. [The Three Postulates](#2-the-three-postulates)
3. [Getting Started](#3-getting-started)
4. [The Stack Machine](#4-the-stack-machine)
5. [Types and Values](#5-types-and-values)
6. [Defining New Tools](#6-defining-new-tools)
7. [Control Flow](#7-control-flow)
8. [Working with Collections](#8-working-with-collections)
9. [Linear Types: Memory Safety](#9-linear-types-memory-safety)
10. [Effect System: IO Safety](#10-effect-system-io-safety)
11. [The Algebraic Optimizer](#11-the-algebraic-optimizer)
12. [Capabilities: Security Model](#12-capabilities-security-model)
13. [Complete Reference](#13-complete-reference)
14. [Philosophy and Design](#14-philosophy-and-design)

---

## 1. Why Kore Exists

### The Problem

Modern AI systems generate code, but we can't trust that code:

- **Security**: LLM-generated code might access files, networks, or execute commands
- **Correctness**: Generated code might crash, loop forever, or corrupt data
- **Predictability**: Side effects make behavior hard to reason about

### The Solution

Kore is designed so that **machines can safely execute code from other machines**:

1. **Capability-based security**: No ambient authority. Every permission is explicit.
2. **Linear types**: Resources can't be leaked or double-freed.
3. **Static effect inference**: We know what IO a program needs *before* running it.
4. **Algebraic verification**: The type system is mathematically grounded.

### Who Should Use Kore?

- **AI agents** that need to execute code safely
- **Orchestration systems** that run untrusted workflows
- **Sandboxed computation** where isolation matters
- **Anyone** who wants a simple, formally verified language

---

## 2. The Three Postulates

Kore is built on exactly three axioms. Everything else follows from these.

### Postulate 1: Everything is a Tool

```
Tool : Stack → Stack
```

In Kore, there's only one kind of thing: **tools**. A tool takes a stack and returns a new stack. That's it.

- Numbers? They're tools that push themselves onto the stack.
- Functions? Tools that transform the stack.
- Operators? Tools.
- Control flow? Tools.

This uniformity is powerful. You can pass tools around, store them, compose them.

### Postulate 2: Tools Transform Stacks

```
execute(tool, stack) = stack'
```

Every tool is a **pure function** from stack to stack. Given the same input stack, you always get the same output stack.

(Side effects like IO are handled through the capability system, which we'll cover later.)

### Postulate 3: Composition is Concatenation

```
(f ; g)(s) = g(f(s))
```

To compose two tools, just write them next to each other. Running `a b c` means:
1. Run `a` on the stack
2. Run `b` on the result
3. Run `c` on that result

This is the essence of stack-based programming: **programs are sentences**.

---

## 3. Getting Started

### Your First Program

```kore
3 4 add
```

This program:
1. Pushes `3` onto the stack
2. Pushes `4` onto the stack  
3. Calls `add`, which pops two values and pushes their sum

Result: Stack contains `7`.

### Arithmetic

```kore
10 3 sub     ; 7  (10 - 3)
6 7 mul      ; 42 (6 * 7)
20 4 div     ; 5  (20 / 4)
17 5 mod     ; 2  (17 % 5)
```

Note: Comments start with `;` and go to end of line.

### Stack Manipulation

The stack is your workspace. Learn these tools:

```kore
5 dup        ; 5 5    (duplicate top)
1 2 drop     ; 1      (remove top)
1 2 swap     ; 2 1    (swap top two)
1 2 over     ; 1 2 1  (copy second to top)
1 2 3 rot    ; 2 3 1  (rotate top three)
```

### Visualizing Stack Effects

We write stack effects as `( before -- after )`:

```
dup  : ( a -- a a )
drop : ( a -- )
swap : ( a b -- b a )
over : ( a b -- a b a )
rot  : ( a b c -- b c a )
```

The stack grows to the right. `( a b -- )` means `b` is on top, `a` is below.

---

## 4. The Stack Machine

### How the Stack Works

Think of the stack as a vertical pile of values:

```
Initial:  []
Push 1:   [1]
Push 2:   [1, 2]      ← 2 is on top
Push 3:   [1, 2, 3]   ← 3 is on top
add:      [1, 5]      ← popped 2 and 3, pushed 5
mul:      [5]         ← popped 1 and 5, pushed 5
```

### Stack Depth

You can query the stack depth:

```kore
1 2 3 depth   ; 1 2 3 3  (depth was 3)
```

### The `dip` Combinator

Sometimes you want to execute code "under" the top value:

```kore
1 2 3 [ add ] dip
; Stack: 1 2 3
; Save 3, execute [add] on remaining stack: 1 + 2 = 3
; Restore 3
; Result: 3 3
```

`dip` is incredibly useful for complex stack manipulation.

---

## 5. Types and Values

### The Ten Types

Kore has exactly 10 types:

| Type | Examples | Description |
|------|----------|-------------|
| **Null** | `null` | The absence of value |
| **Bool** | `true`, `false` | Boolean values |
| **Int** | `42`, `-17`, `0` | 64-bit integers |
| **Float** | `3.14`, `-0.5` | 64-bit floating point |
| **Text** | `"hello"`, `""` | UTF-8 strings |
| **List** | `[1 2 3]` | Ordered collection |
| **Map** | `{x: 1 y: 2}` | Key-value pairs |
| **Quote** | `[dup mul]` | Deferred code |
| **Handle** | (runtime) | External resources |
| **Error** | (runtime) | Error values |

### Type Checking

Check types at runtime:

```kore
42 is-int       ; true
"hi" is-text    ; true
[1 2] is-list   ; true
3.14 type-of    ; "Float"
```

### Type Conversion

Convert between types:

```kore
"42" to-int     ; 42
42 to-text      ; "42"
42 to-float     ; 42.0
1 to-bool       ; true
0 to-bool       ; false
```

### Truthiness

For `if` and other conditionals:
- **Falsy**: `false`, `null`, `0`, `0.0`, `""`, `[]`, `{}`
- **Truthy**: Everything else

---

## 6. Defining New Tools

### Constants

```kore
42 "answer" def
answer answer mul   ; 1764
```

### Functions

A function is just a quote that you name:

```kore
[ dup mul ] "square" def
5 square   ; 25

[ dup square swap cube ] "test" def  ; works if cube is defined
```

### Verified Definitions

For safety-critical code, use `def-verified` to ensure the effect matches:

```kore
[ dup mul ] "square" "(n -- n)" def-verified  ; OK - effect is (1 -- 1)
[ dup ] "bad" "(a -- a)" def-verified          ; FAILS - actual effect is (1 -- 2)
```

This catches bugs at definition time, not runtime.

### The Traditional Syntax

Kore supports a cleaner syntax for definitions:

```kore
: square ( n -- n² )
  dup mul
;

: cube ( n -- n³ )
  dup dup mul mul
;

5 square   ; 25
3 cube     ; 27
```

The `( n -- n² )` is a **stack effect comment** - it documents what the function does to the stack.

### Recursive Functions

Kore supports recursion naturally:

```kore
: factorial ( n -- n! )
  dup 0 eq
  [ drop 1 ]           ; base case: 0! = 1
  [ dup 1 sub factorial mul ]  ; n * (n-1)!
  if
;

5 factorial   ; 120
```

---

## 7. Control Flow

### Conditional: `if`

```kore
condition [ then-branch ] [ else-branch ] if
```

Example:

```kore
5 3 gt [ "yes" ] [ "no" ] if   ; "yes"
```

### Short-circuit: `when` and `unless`

```kore
true [ "executed" println ] when     ; prints "executed"
false [ "executed" println ] when    ; does nothing

false [ "executed" println ] unless  ; prints "executed"
```

### Loop N Times: `times`

```kore
5 [ "hello" println ] times   ; prints "hello" 5 times

; Accumulator pattern
0                             ; initial sum
5 [ 1 add ] times             ; add 1 five times
; Result: 5
```

### While Loop: `while`

```kore
[ condition-quote ] [ body-quote ] while
```

Example - countdown:

```kore
5                             ; start at 5
[ dup 0 gt ]                  ; while n > 0
[ dup println 1 sub ] while   ; print and decrement
drop                          ; remove final 0
```

Output:
```
5
4
3
2
1
```

### Infinite Loop: `loop`

```kore
[ 
  ; do something
  condition [ break ] when
] loop
```

---

## 8. Working with Collections

### Lists

```kore
; Create a list
[ 1 2 3 ]              ; literal syntax
3 1 2 3 list           ; from stack: count then items

; Access
[1 2 3] 0 list-get     ; 1 (first element)
[1 2 3] list-len       ; 3
[1 2 3] list-first     ; 1
[1 2 3] list-last      ; 3

; Modify (creates new list)
[1 2 3] 4 list-push    ; [1 2 3 4]
[1 2 3 4] list-pop     ; [1 2 3] 4  (returns list and popped item)
[1 2 3] 1 10 list-set  ; [1 10 3]

; Transform
[1 2 3] list-reverse   ; [3 2 1]
[1 2 3] [4 5] list-concat  ; [1 2 3 4 5]
[1 2 3 4 5] 1 4 list-slice ; [2 3 4]
```

### Higher-Order List Operations

```kore
; Map: apply function to each element
[1 2 3] [ dup mul ] map    ; [1 4 9]

; Filter: keep elements that satisfy predicate
[1 2 3 4 5] [ 2 mod 0 eq ] filter   ; [2 4]

; Fold: reduce list to single value
[1 2 3 4] 0 [ add ] fold   ; 10

; Each: execute for side effects
[1 2 3] [ println ] each   ; prints 1, 2, 3
```

### Maps

```kore
; Create
map-new                     ; empty map
{ name: "Alice" age: 30 }   ; literal syntax

; Access
{ x: 1 y: 2 } "x" map-get   ; 1
{ x: 1 } "x" map-has        ; true
{ x: 1 } "z" map-has        ; false

; Modify
{ x: 1 } "y" 2 map-set      ; { x: 1 y: 2 }
{ x: 1 y: 2 } "x" map-del   ; { y: 2 }

; Iterate
{ x: 1 y: 2 } map-keys      ; ["x" "y"]
{ x: 1 y: 2 } map-vals      ; [1 2]
```

---

## 9. Linear Types: Memory Safety

### The Problem with Copying

Normally, you can copy any value:

```kore
5 dup drop   ; fine: 5 5 → 5
```

But what about file handles? Network connections? Database transactions?

```kore
file-handle dup   ; DANGEROUS: now two things think they own the file
```

### Linear Types to the Rescue

Kore 2.0 introduces **linear types** - values that can't be copied or discarded.

```kore
; Create a linear value
42 linear-new     ; wraps 42 as linear

; This will ERROR:
42 linear-new dup   ; ERROR: cannot duplicate linear value

; This will also ERROR:
42 linear-new drop  ; ERROR: cannot drop linear value

; You MUST consume it exactly once:
42 linear-new linear-unwrap   ; 42
```

### Affine Types (Use At Most Once)

**Affine** values can be dropped but not copied. Handles are automatically affine:

```kore
; Handles are affine by default
some-file-handle dup   ; ERROR: cannot duplicate handle
some-file-handle drop  ; OK: handle is cleaned up
```

### Checking Linearity

```kore
42 linear-new is-linear   ; true
42 linear-new is-affine   ; true (linear implies affine)
42 is-linear              ; false
42 is-affine              ; false

42 linearity              ; :unrestricted
42 linear-new linearity   ; :linear
42 affine-new linearity   ; :affine
```

### Safe Collection Access

Regular `list-get` copies the element. For linear values, use `list-take`:

```kore
; list-get copies (fails for linear values)
[ 1 2 3 ] 0 list-get   ; 1, list unchanged

; list-take MOVES the element out
[ 1 2 3 ] 0 list-take  ; [2 3] 1
```

Similarly for maps: use `map-take` instead of `map-get`.

---

## 10. Effect System: IO Safety

### The Problem with IO

How do you know if code will:
- Read your files?
- Make network requests?
- Execute shell commands?

In most languages, you can't know without reading every line of code.

### Static Effect Inference

Kore analyzes code **before running it** to determine what IO it needs:

```kore
[ 1 2 add ] pure?           ; true - no IO
[ "hello" println ] pure?   ; false - has IO effect

[ "file.txt" fs-read ] io-effects   ; ["fs"]
[ now println ] io-effects          ; ["time" "io"]
```

### Full Analysis

```kore
[ "data.txt" fs-read json-parse ] effect-infer
; Returns:
; {
;   effect: { consumes: 1, produces: 1 },
;   io: ["fs"],
;   pure: false,
;   safe: true,
;   errors: [],
;   warnings: []
; }
```

### IO Effect Categories

| Effect | Description | Operations |
|--------|-------------|------------|
| `fs` | File system | fs-read, fs-write |
| `net` | Network | http-get, http-post |
| `spawn` | Sub-processes | spawn |
| `time` | Clock | now, sleep |
| `io` | Console | print, println |
| `env` | Environment | env-get, env-set |
| `exec` | Shell | exec, shell |
| `mem` | Mutable state | mem-get, mem-set |

### Using Effects for Sandboxing

```kore
; Before running untrusted code, check what it needs:
untrusted-code io-effects   ; ["fs" "net"]

; Only grant necessary capabilities
untrusted-code { fs: read-only net: none } with-caps call
```

---

## 11. The Algebraic Optimizer

### Stack Operations Form a Group

Mathematical insight: stack operations have algebraic structure!

```
swap ∘ swap = identity   (swap is self-inverse)
rot ∘ rot ∘ rot = identity   (rot has order 3)
dup ∘ drop = identity   (create then destroy)
```

### Tensor Algebraic Identities

Tensor operations also have algebraic properties that the optimizer exploits:

```
tensor-neg ∘ tensor-neg = identity   (negate twice = identity)
tensor-exp ∘ tensor-log = identity   (exp and log are inverses)
tensor-log ∘ tensor-exp = identity
```

### Automatic Simplification

Kore uses these identities to optimize code:

```kore
[ 1 2 swap swap add ] simplify   ; [ 1 2 add ]
[ 5 dup drop ] simplify          ; [ 5 ]
[ rot rot rot ] simplify         ; [ ]
[ tensor-neg tensor-neg ] simplify  ; [ ]
```

### Constant Folding

With `optimize`, constants are computed at compile time:

```kore
[ 3 4 add ] optimize     ; [ 7 ]
[ 6 7 mul ] optimize     ; [ 42 ]
[ true not ] optimize    ; [ false ]
[ "a" "b" str-concat ] optimize   ; [ "ab" ]
```

### Why This Matters for AI

When an LLM generates code, it might produce:

```kore
x swap swap dup drop neg neg
```

The optimizer automatically simplifies this to:

```kore
x
```

Better performance, same behavior, mathematically guaranteed.

---

## 12. Capabilities: Security Model

### No Ambient Authority

In Kore, you can't just read files or make network requests. You need **capabilities**.

```kore
; This will FAIL without fs capability:
"/etc/passwd" fs-read   ; ERROR: capability denied
```

### Granting Capabilities

Capabilities are granted when spawning agents:

```kore
[ "/data/config.json" fs-read ] { fs: "/data/" } spawn
```

This grants read access only to `/data/` - nothing else.

### Capability Attenuation

You can **reduce** capabilities, never increase:

```kore
current-caps { fs: read-only } cap-attenuate
; Now you can read files but not write
```

### The Capability Lattice

Capabilities form a mathematical lattice:

```
cap-leq  : check if cap1 ≤ cap2
cap-join : union (∨)
cap-meet : intersection (∧)
```

This means the security model is formally verifiable.

---

## 13. Complete Reference

### Stack Tools

| Tool | Effect | Description |
|------|--------|-------------|
| `dup` | `(a -- a a)` | Duplicate |
| `drop` | `(a -- )` | Remove |
| `swap` | `(a b -- b a)` | Swap |
| `over` | `(a b -- a b a)` | Copy second |
| `rot` | `(a b c -- b c a)` | Rotate |
| `nip` | `(a b -- b)` | Remove second |
| `tuck` | `(a b -- b a b)` | Copy top under second |
| `depth` | `( -- n)` | Stack depth |
| `dip` | `(a q -- a)` | Execute under top |

### Arithmetic

| Tool | Effect | Description |
|------|--------|-------------|
| `add` | `(a b -- a+b)` | Add |
| `sub` | `(a b -- a-b)` | Subtract |
| `mul` | `(a b -- a*b)` | Multiply |
| `div` | `(a b -- a/b)` | Divide |
| `mod` | `(a b -- a%b)` | Modulo |
| `neg` | `(a -- -a)` | Negate |
| `abs` | `(a -- |a|)` | Absolute value |

### Comparison

| Tool | Effect | Description |
|------|--------|-------------|
| `eq` | `(a b -- bool)` | Equal |
| `neq` | `(a b -- bool)` | Not equal |
| `lt` | `(a b -- bool)` | Less than |
| `lte` | `(a b -- bool)` | Less or equal |
| `gt` | `(a b -- bool)` | Greater than |
| `gte` | `(a b -- bool)` | Greater or equal |

### Logic

| Tool | Effect | Description |
|------|--------|-------------|
| `and` | `(a b -- bool)` | AND |
| `or` | `(a b -- bool)` | OR |
| `not` | `(a -- bool)` | NOT |

### Control Flow

| Tool | Effect | Description |
|------|--------|-------------|
| `call` | `(q -- ...)` | Execute quote |
| `if` | `(c t e -- ...)` | Conditional |
| `when` | `(c q -- )` | If true |
| `unless` | `(c q -- )` | If false |
| `times` | `(n q -- ...)` | Loop N times |
| `while` | `(c b -- )` | While loop |
| `loop` | `(q -- )` | Infinite loop |
| `try` | `(q -- r)` | Try/catch |
| `fail` | `(msg -- )` | Raise error |
| `unwrap` | `(r -- v)` | Unwrap result |

### Analysis Tools

| Tool | Effect | Description |
|------|--------|-------------|
| `effect-infer` | `(q -- map)` | Full analysis |
| `io-effects` | `(q -- list)` | IO effects |
| `pure?` | `(q -- bool)` | Is pure? |
| `optimize` | `(q -- q')` | Full optimize |
| `simplify` | `(q -- q')` | Algebraic only |

---

## 14. Philosophy and Design

### Why Stack-Based?

1. **Simplicity**: Only two operations (Push, Call)
2. **Composability**: Programs are sentences
3. **Verifiability**: Stack effects can be statically analyzed
4. **Efficiency**: No variable lookup, just stack operations

### Why These Axioms?

The three postulates are **minimal**:
- Remove Postulate 1 → you need multiple concepts
- Remove Postulate 2 → execution is undefined
- Remove Postulate 3 → composition is undefined

They're also **complete** - every program can be expressed.

### The Trust Model

```
┌─────────────────────────────────────────┐
│           Untrusted Code                │
│   (generated by AI, downloaded, etc.)   │
└─────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────┐
│          Static Analysis                │
│  - Stack effect verification            │
│  - IO effect inference                  │
│  - Linearity checking                   │
└─────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────┐
│        Capability Restriction           │
│  - Only grant required permissions      │
│  - Attenuate to minimum necessary       │
└─────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────┐
│          Safe Execution                 │
│  - Runtime capability enforcement       │
│  - Resource limits                      │
│  - Trace logging                        │
└─────────────────────────────────────────┘
```

### Mathematical Foundations

| Concept | Structure | Operations |
|---------|-----------|------------|
| Stack Effects | Monoid | compose, identity |
| Capabilities | Lattice | join, meet, ≤ |
| IO Effects | Semilattice | union |
| Resources | Monoid | add, split |
| Traces | Monoid | concat, empty |

These aren't just abstractions - they're the **formal semantics** of Kore.

### Design Principles

1. **Minimal**: 2 operations, 10 types, 186 tools
2. **Formal**: Every tool has a verified stack effect
3. **Secure**: Capability-based, no ambient authority
4. **Composable**: Tools are the only abstraction
5. **Analyzable**: Static verification is possible

---

## Appendix: Quick Reference Card

### Stack
```
dup drop swap over rot nip tuck depth dip
```

### Math
```
add sub mul div mod neg abs
eq neq lt lte gt gte
and or not
```

### Types
```
is-null is-bool is-int is-float is-text is-list is-map is-quote is-error
type-of to-int to-float to-text to-bool
```

### Lists
```
list unlist list-len list-get list-set list-push list-pop
list-first list-last list-reverse list-concat list-slice list-take
map filter fold each
```

### Maps
```
map-new map-get map-set map-del map-has map-keys map-vals map-take
```

### Strings
```
str-len str-get str-slice str-concat str-split str-join
str-find str-replace str-starts str-ends str-upper str-lower str-trim
char-code code-char
```

### Control
```
call if when unless times while loop try fail unwrap
```

### Linear
```
linear-new linear-unwrap affine-new affine-unwrap
is-linear is-affine linearity
```

### Tensor
```
tensor-from-list tensor-zeros tensor-ones tensor-rand
tensor-add tensor-mul tensor-sub tensor-neg tensor-scale
tensor-sum tensor-mean tensor-max tensor-argmax
tensor-matmul tensor-outer tensor-dot
tensor-relu tensor-sigmoid tensor-softmax tensor-exp tensor-log
tensor-shape tensor-size tensor-get tensor-set is-tensor
```

### Autodiff
```
requires-grad backward grad-get zero-grad detach
```

### Analysis
```
effect-infer effect-compose effect-parse effect-net effect-valid? effect-new
io-effects pure? optimize simplify axioms selftest
```

### Definition
```
def def-verified words meta meta! defined?
```

### Error Handling
```
try fail is-error error-info
```

---

*Kore: The Fundamental Runtime for Safe Machine-to-Machine Code Execution*
