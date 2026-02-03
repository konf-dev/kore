# Kore

**A stack-based language for verifiable computation.**

---

## Why Kore Exists

Modern programming languages are built for humans. They have complex syntax, hidden state, implicit behavior, and dozens of ways to do the same thing. This works fine when humans write and read code.

But what if machines write the code? What if an AI agent needs to:
1. **Generate** programs that are correct by construction
2. **Reason** about what a program does before running it  
3. **Verify** that code cannot access unauthorized resources
4. **Compose** small pieces into complex behavior safely

Existing languages fail here. Python has global state. JavaScript has implicit coercions. Even Rust, despite its safety guarantees, has syntax complexity that makes machine generation unreliable.

**Kore is different.** It is a language designed from first principles for machine-authored, mathematically-verifiable code.

---

## What Kore Is

Kore is a **stack-based concatenative language** built on three postulates:

### The Three Postulates

**Postulate 1: Everything is a Tool**

$$\text{Tool} : \text{Stack} \rightarrow \text{Stack}$$

There is exactly one kind of thing: a Tool. A Tool takes a stack and returns a stack. There are no functions, methods, classes, modules, or special forms. Just Tools.

**Postulate 2: Tools Transform Stacks**

$$\text{execute}(t, s) = s'$$

A Tool consumes values from the top of the stack and produces values onto the stack. All input comes from the stack. All output goes to the stack. There are no arguments, no return values, no side channels.

**Postulate 3: Composition is Concatenation**

$$(f \circ g)(s) = g(f(s))$$

To compose two programs, write them next to each other. The output stack of the first becomes the input stack of the second. That's it. No function calls, no parentheses, no nesting.

---

## What Makes Kore Different

### 1. Two Operations Only

Every Kore program is a sequence of exactly two operations:

| Operation | What it does |
|-----------|--------------|
| **Push** | Put a value on the stack |
| **Call** | Look up a tool by name and run it |

That's the entire execution model. Conditionals? The `if` tool. Loops? The `loop` tool. Definitions? The `def` tool. Everything is a tool.

### 2. Ten Value Types

| Type | Example | What it means |
|------|---------|---------------|
| `Null` | `null` | Nothing |
| `Bool` | `true`, `false` | Truth values |
| `Int` | `42`, `-7` | Whole numbers |
| `Float` | `3.14` | Decimal numbers |
| `Text` | `"hello"` | Strings |
| `List` | `[1, 2, 3]` | Ordered sequences |
| `Map` | `{x: 1, y: 2}` | Key-value pairs |
| `Quote` | `[dup mul]` | Deferred code (a program as data) |
| `Handle` | `@file:42` | Reference to external resource |
| `Error` | `Error(msg)` | Captured failure |

### 3. Stack-Based Execution

Programs operate on a stack. Values are pushed. Tools pop their inputs and push their outputs.

```kore
3 4 add      ; Push 3, push 4, call add → stack: [7]
dup mul      ; Duplicate 7, multiply → stack: [49]
```

Reading: left to right. Data flows through the stack.

### 4. Quotations = Higher-Order Programming

A **Quote** is a program wrapped in brackets. It doesn't execute immediately—it becomes a value you can pass around.

```kore
[dup mul]         ; Push a quote onto the stack
"square" def      ; Name it "square"
5 square          ; Now: 5 dup mul → 25
```

This gives Kore the power of lambda calculus, but simpler.

### 5. Algebraic Safety Guarantees

Kore is built on three mathematical structures that **guarantee safety**:

#### Capabilities: A Bounded Lattice

$$(\text{CapSet}, \leq, \land, \lor, \bot, \top)$$

- Every operation that touches the outside world requires a **capability**
- Capabilities form a lattice: you can only **attenuate** (reduce), never escalate
- If you spawn a child process, it gets ≤ your capabilities

```
cap("fs:read:/home") ≤ cap("fs:read:/")  ; More specific is weaker
```

**Theorem (Capability Safety):** For any execution with capabilities $C$:
$$\forall e \in \text{trace}: \text{caps-used}(e) \subseteq C$$

You cannot use capabilities you weren't granted.

#### Resources: A Commutative Monoid

$$(R, +, \mathbf{0})$$

- Resources (memory, compute, network) are **finite and conserved**
- You can split resources but never create them
- Every operation costs resources; when you run out, execution stops

$$R_{\text{child}} + R_{\text{parent}} = R_{\text{original}}$$

**Theorem (Termination):** Under bounded resources with positive costs, execution always terminates.

#### Traces: A Monoid

$$(\text{Trace}, \cdot, \epsilon)$$

- Every operation appends to an execution trace
- Traces are append-only, unforgeable
- You can replay, verify, and audit any execution

**Theorem (Determinism):** Same program + same initial state = same trace.

---

## Tensor Operations

Kore includes first-class tensor operations for machine learning and numerical computing:

```kore
; Create tensors
[1 2 3 4] tensor-from-list           ; Vector
[2 2] tensor-zeros                   ; 2×2 zero matrix
[3 3] 42 tensor-randn                ; Random 3×3 (seeded)

; Arithmetic (element-wise)
t1 t2 tensor-add                     ; Add
t1 t2 tensor-mul                     ; Multiply

; Matrix operations
A B 3 4 tensor-matmul                ; Matrix multiply (3 rows, 4 cols)
v1 v2 tensor-outer                   ; Outer product

; Activations
t tensor-relu                        ; ReLU: max(0, x)
t tensor-softmax                     ; Softmax: exp(x)/sum(exp(x))
```

**What this means:** You can build neural networks and numerical pipelines in Kore. The tensor operations are **shape-checked** at runtime, preventing dimension mismatches that cause cryptic errors in NumPy/PyTorch.

---

## Fibers: Pausable Computation

Kore supports **fibers** (lightweight coroutines) for complex control flow:

```kore
; Create a paused computation
[ 1 fiber-yield 2 fiber-yield 3 ] fiber-new

; Resume to get values one at a time
0 swap fiber-resume    ; → 1, fiber'
0 swap fiber-resume    ; → 2, fiber''
0 swap fiber-resume    ; → 3, fiber'''
```

**What this means:** Fibers let you write iterators, generators, and cooperative concurrency. The computation is **reified as a value**—you can pause it, resume it, or even fork it.

---

## Safety Guarantees Compared

Different languages provide different safety guarantees. Here's how Kore compares:

| Safety Type | Kore | Rust | Python | JavaScript |
|-------------|------|------|--------|------------|
| **Memory Safety** | ✅ GC + Handles | ✅ Borrow checker | ✅ GC | ✅ GC |
| **Type Safety** | ✅ Runtime checked | ✅ Static | ❌ Dynamic | ❌ Dynamic |
| **Null Safety** | ✅ Explicit `Null` type | ✅ `Option<T>` | ❌ `None` exceptions | ❌ `null`/`undefined` |
| **Resource Safety** | ✅ Capability lattice | ⚠️ Convention | ❌ None | ❌ None |
| **Effect Safety** | ✅ Static effect analysis | ❌ None | ❌ None | ❌ None |
| **Termination** | ✅ Resource bounds | ❌ None | ❌ None | ❌ None |
| **Linearity** | ✅ Affine/Linear types | ⚠️ Move semantics | ❌ None | ❌ None |
| **Auditability** | ✅ Immutable traces | ❌ Logging | ❌ Logging | ❌ Logging |

### What Each Safety Means

**Memory Safety:** Can the program access invalid memory?
- *Kore:* Garbage collected. Handles are opaque references.

**Type Safety:** Are type errors caught before they cause problems?
- *Kore:* Every value carries its type. Operations check types at runtime.

**Null Safety:** Can null/undefined values cause crashes?
- *Kore:* `Null` is an explicit type. You can't accidentally dereference it.

**Resource Safety:** Can the program access files/network/system it shouldn't?
- *Kore:* Capabilities form a **lattice**. You can only pass capabilities you have. Child processes get ≤ your capabilities. This is **mathematically enforced**, not just convention.

**Effect Safety:** Can you know what side effects a function has?
- *Kore:* Every tool has a stack effect signature `(inputs -- outputs)` plus IO effects. You can statically determine if code is pure.

**Termination:** Can you guarantee the program eventually stops?
- *Kore:* Resources are **finite and conserved**. Every operation costs resources. When resources run out, execution halts. No infinite loops.

**Linearity:** Can you ensure values are used exactly once?
- *Kore:* Affine types (use at most once) for handles. Linear types (use exactly once) for critical resources. `dup` on a linear value is a runtime error.

**Auditability:** Can you prove what happened during execution?
- *Kore:* Every execution produces an **immutable trace**. The trace is append-only and unforgeable. You can replay and verify any execution.

---

## How Context Flows Through the System

In traditional languages, data flows through function arguments and return values. In Kore, **all context flows through the stack**. This is the key insight.

### The Stack is the Only State

```
┌─────────────────────────────────────────────────────────────────┐
│                         STACK                                   │
│  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐                       │
│  │  v₁ │ │  v₂ │ │  v₃ │ │  v₄ │ │ ... │  ← values flow here   │
│  └─────┘ └─────┘ └─────┘ └─────┘ └─────┘                       │
└─────────────────────────────────────────────────────────────────┘
       ↑                                     ↑
       │         Tools transform this        │
       └─────────────────────────────────────┘
```

Every tool reads from the stack, writes to the stack. No hidden channels.

### Data Flow is Left-to-Right

```kore
3 4 add 2 mul
```

Reading this program:
1. `3` → pushes 3 onto stack: `[3]`
2. `4` → pushes 4 onto stack: `[3, 4]`
3. `add` → pops 3 and 4, pushes 7: `[7]`
4. `2` → pushes 2: `[7, 2]`
5. `mul` → pops 7 and 2, pushes 14: `[14]`

**Data flows left to right.** The output of each tool becomes input for the next.

### Compare with Traditional Languages

```python
# Python: Data flows through nested function calls
result = mul(add(3, 4), 2)
#        ↑   ↑   ↑  ↑   ↑
#        5   3   1  2   4   (evaluation order is non-obvious)
```

```kore
; Kore: Data flows linearly
3 4 add 2 mul
; 1 2  3  4  5   (evaluation order = reading order)
```

### The `dip` Combinator: Reaching Under

Sometimes you need to operate "under" the top value. The `dip` combinator does this:

```kore
1 2 3 [ add ] dip
; Stack: [1, 2, 3]
; dip saves 3, runs [add] on [1, 2], gets [3], restores 3
; Result: [3, 3]
```

This is how you manipulate context deeper in the stack without losing the top.

### Quotes Capture Context

A **quote** is a frozen piece of code that can be passed around:

```kore
5 [ dup mul ] call    ; Execute immediately: 25
5 [ dup mul ]         ; Don't execute—just push the quote
                      ; Stack: [5, [dup mul]]
```

Quotes are **closures without variable capture**. They operate on whatever stack they're given.

### Context Flow Through Control Structures

Control flow is just tools that take quotes:

```kore
condition [ then-code ] [ else-code ] if
```

The `if` tool:
1. Pops the condition
2. Pops both quotes
3. Calls the appropriate quote on the remaining stack
4. That quote's output becomes the new stack

**No special syntax.** Just data flowing through tools.

### Effect Flow: IO is Tracked

When you compose tools, their IO effects compose too:

$$\text{effects}(A \; B) = \text{effects}(A) \cup \text{effects}(B)$$

```kore
[ "file.txt" fs-read json-parse ] io-effects   ; → ["fs"]
[ 1 2 add ] io-effects                          ; → []
```

This means you can **statically determine** what IO a program needs before running it.

### Capability Flow: Permissions Attenuate

When you spawn a child process, capabilities flow **downward only**:

```kore
; Parent has: {fs: "/", net: "*"}
[ child-code ] {fs: "/tmp"} spawn
; Child gets: {fs: "/tmp"}  ← strictly less than parent
```

$$C_{\text{child}} \leq C_{\text{parent}}$$

You can never grant capabilities you don't have. This is **mathematically enforced** by the lattice structure.

### Trace Flow: History is Append-Only

Every execution appends to an immutable trace:

```kore
trace-new                      ; Create empty trace
"step1" swap trace-step        ; Add step
"step2" swap trace-step        ; Add step
trace-fingerprint              ; Get cryptographic hash
```

$$\text{trace}_{n+1} = \text{trace}_n \cdot \text{step}_{n+1}$$

Traces form a **monoid**: you can concatenate them but never modify past entries.

---

## Core Primitives

Kore has ~80 primitives organized by category. Here are the essentials:

### Stack Manipulation
```kore
dup     ; (a -- a a)        Duplicate top
drop    ; (a -- )           Remove top
swap    ; (a b -- b a)      Swap top two
over    ; (a b -- a b a)    Copy second to top
rot     ; (a b c -- b c a)  Rotate top three
```

### Arithmetic
```kore
add sub mul div mod neg
```

### Comparison & Logic
```kore
eq lt gt le ge neq         ; Comparison → Bool
and or not                  ; Logic
```

### Control Flow
```kore
call    ; (quote -- ...)     Execute a quote
if      ; (bool then else -- ...) Conditional
loop    ; (body -- ...)      Loop while true on stack
times   ; (n body -- ...)    Repeat n times
```

### Error Handling
```kore
try       ; (quote -- result)    Execute, capture errors
fail      ; (msg -- )            Raise error
is-error  ; (val -- bool)        Check if error
unwrap    ; (val -- inner)       Extract or propagate error
```

### Data Structures
```kore
list-len list-get list-set list-push list-pop
map-get map-set map-has map-keys map-vals
str-len str-concat str-split str-find
```

### Definition & Introspection
```kore
def      ; (val name -- )   Define a tool
words    ; ( -- list)       List all tools
meta     ; (name -- map)    Get tool metadata
```

---

## Example: Factorial

```kore
; Define factorial recursively
[ 
  dup 1 le                    ; Is n ≤ 1?
  [ drop 1 ]                  ; If yes: return 1
  [ dup 1 sub factorial mul ] ; If no: n * factorial(n-1)
  if 
] "factorial" def

; Compute 5!
5 factorial    ; → 120
```

## Example: Map Over List

```kore
; Square each element
[1 2 3 4 5] [dup mul] map    ; → [1 4 9 16 25]
```

## Example: Error Handling

```kore
[
  "config.json" fs-read json-parse
] try

dup is-error
[ drop {default: true} ]     ; Use default on error
[ ]                          ; Use parsed result
if
```

---

## Why Stack-Based?

Stack languages have properties that matter for machine-generated code:

1. **No Naming Required**: Values flow through the stack; no variable names to invent
2. **Composition is Trivial**: `a b` means "do a, then do b"
3. **Effect Analysis**: Stack effect `(a b -- c)` tells you exactly what a tool does
4. **Minimal Syntax**: No parentheses, no commas, no semicolons
5. **Streaming-Friendly**: An LLM can emit valid Kore token by token

Compare:
```python
# Python: Needs names, parentheses, order matters differently
result = multiply(add(3, 4), 2)
```

```kore
; Kore: Data flows left to right through the stack
3 4 add 2 mul
```

---

## The Algebra of Effects

Every Kore tool has a **stack effect signature**:

```
(inputs -- outputs)
```

For example:
- `dup` : `(a -- a a)` — consumes 1, produces 2
- `add` : `(a b -- c)` — consumes 2, produces 1
- `drop` : `(a -- )` — consumes 1, produces 0

Effects compose mathematically:

$$\text{compose}((a, b), (c, d)) = 
\begin{cases}
(a, b - c + d) & \text{if } b \geq c \\
(a + c - b, d) & \text{otherwise}
\end{cases}$$

**Net stack change**: $\text{net}(a, b) = b - a$

This means we can **statically verify** that a program won't underflow the stack before running it.

---

## Capability-Based Security

Dangerous operations require explicit capabilities:

```kore
; This fails without fs:read capability
"secret.txt" fs-read

; Spawn with reduced capabilities
[some-code] {caps: ["fs:read:/tmp"]} spawn
```

The capability algebra guarantees:
- You can only grant capabilities you have
- Child processes get ≤ parent capabilities
- No capability escalation is possible

$$\text{spawn}(q, C', R') \text{ requires } C' \leq C \text{ and } R' \leq R$$

---

## File Structure

```
src/
├── core/         # Primitive operations (Rust)
├── algebra.rs    # CapSet, Res, Trace (mathematical structures)
├── executor.rs   # Main execution loop
├── value.rs      # The 10 types
├── stack.rs      # Stack operations
└── cap/          # Capability tools (fs, net, exec)

stdlib/
├── prelude.kore  # Standard library in Kore
└── math.kore     # Math utilities
```

---

## Quick Start

```bash
# Build
cargo build --release

# Run a file
./target/release/kore run program.kore

# REPL
./target/release/kore repl

# Run tests
cargo test
```

---

## Summary

| Aspect | Kore | Rust | Python | JavaScript |
|--------|------|------|--------|------------|
| **Operations** | 2 (Push, Call) | Dozens | Dozens | Dozens |
| **Composition** | Concatenation | Function calls | Function calls | Function calls |
| **State** | Explicit (stack) | Variables + borrow | Variables + globals | Variables + closures |
| **Types** | 10 runtime | Static generics | Dynamic | Dynamic |
| **Capabilities** | Lattice (∧,∨,⊥,⊤) | None | None | None |
| **Resources** | Monoid (tracked) | RAII (compile-time) | None | None |
| **Effects** | Static analysis | None | None | None |
| **Traces** | Immutable log | None | None | None |
| **Termination** | Bounded | Not guaranteed | Not guaranteed | Not guaranteed |
| **Tensors** | Built-in (21 ops) | External crate | NumPy/PyTorch | tf.js |
| **Linearity** | Affine/Linear | Move semantics | None | None |
| **Target** | Machine-generated | Human-written | Human-written | Human-written |

Kore exists because we need a language that machines can **write**, **verify**, and **reason about**. It's not for humans to write large programs in—it's for building systems where correctness is provable.

---

## Further Reading

- [Formal Foundations](docs/FORMAL_FOUNDATIONS.md) — Full mathematical treatment
- [Postulates](docs/POSTULATES.md) — The three postulates in depth
- [Philosophy](docs/PHILOSOPHY.md) — Design principles
- [Primitives](docs/PRIMITIVES.md) — Complete tool reference
- [LLM Spec](docs/KORE_SPEC_FOR_LLMS.md) — Machine-readable specification

---

## License

MIT
