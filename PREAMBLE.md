# PREAMBLE: The Absolute Truth

This document is the master context for all LLMs operating on or within Kore.
It is the constitution. It is religion. Follow it without exception.

---

## The Three Postulates

These are axioms. They are not negotiable.

### Postulate 1: Everything is a Tool

```
Tool : Stack → Stack
```

There is only one abstraction: the **Tool**.
A tool takes a stack and returns a stack.
Functions are tools. Operators are tools. Control flow is tools.
`if` is a tool. `def` is a tool. Even `push` is a tool.

**Consequence:** There are no special cases. No magic. No exceptions.

### Postulate 2: Tools Transform Stacks

```
execute(tool, stack) → stack'
```

A tool consumes values from the stack.
A tool produces values onto the stack.
The stack is the only interface between tools.

**Consequence:** Tools are pure functions on stacks. Input → Output.

### Postulate 3: Composition is Concatenation

```
(f ; g)(s) = g(f(s))
```

To compose tools, concatenate them.
`f g` means: run f, then run g.
There is no special composition operator.
The syntax IS the composition.

**Consequence:** Programs are just lists of tool names.

---

## The Two Operations

Kore has exactly two operations. Not three. Not one. Two.

### Op::Push(value)
Put a value on the stack.

### Op::Call(name)  
Look up a tool by name and execute it.

That's it. Everything else is built from these.

---

## The Ten Types

Values have exactly ten types:

| Type   | Description              | Example           |
|--------|--------------------------|-------------------|
| Null   | Absence of value         | `null`            |
| Bool   | Truth value              | `true`, `false`   |
| Int    | 64-bit signed integer    | `42`, `-7`        |
| Float  | 64-bit floating point    | `3.14`, `-0.5`    |
| Text   | UTF-8 string             | `"hello"`         |
| List   | Ordered collection       | `[1, 2, 3]`       |
| Map    | Key-value pairs          | `{a: 1, b: 2}`    |
| Quote  | Deferred computation     | `[ dup add ]`     |
| Handle | Opaque resource reference| `<file:3>`        |
| Error  | Failure with message     | `Error("oops")`   |

No type hierarchies. No inheritance. No generics.
A value is exactly one of these ten things.

---

## The Algebraic Foundations

### Capabilities: A Lattice

```
CapSet with operations: ≤, ∧ (meet), ∨ (join), attenuate
```

- **Top (⊤):** All capabilities (`*`)
- **Bottom (⊥):** No capabilities (empty)
- **Partial order:** `a ≤ b` means a is subset of b
- **Meet (∧):** Intersection of capabilities
- **Join (∨):** Union of capabilities
- **Attenuate:** `child = parent ∧ requested` (can only reduce)

**Invariant:** A child context NEVER has more capabilities than its parent.

### Resources: A Monoid

```
Res with operations: +, split, zero
```

- **Identity:** `Res::zero()` (0, 0, 0, 0)
- **Addition:** Component-wise `(a + b)`
- **Split:** `split(r, ratio) = (child, parent)` where `child + parent = r`

**Conservation Law:** Resources cannot be created or destroyed.
`child.mem + parent.mem = original.mem` (and for rom, compute, net)

### Traces: A Monoid

```
Trace with operations: concat, empty
```

- **Identity:** Empty trace
- **Concat:** Append trace steps
- **Fingerprint:** Hash of trace for verification

**Faithfulness:** The trace is a complete record of execution.

---

## The Design Principles

### 1. Minimal

Count things. Fewer is better.
- 2 operations (Push, Call)
- 10 types
- ~50 core primitives
- Everything else is stdlib

### 2. Predictable

No surprises. No magic.
- Same input → same output
- Tools do one thing
- Names say what they do

### 3. Composable

Small pieces that combine.
- Every tool is independent
- Tools don't know about each other
- Composition is just concatenation

### 4. Dumb

Intelligence is in composition, not components.
- Each tool is trivial
- Complexity emerges from combination
- An LLM should understand each tool instantly

### 5. Explicit

Say what you mean. No implicit behavior.
- No automatic conversions
- No hidden state
- No side effects without capability

---

## The Rules for Writing Tools

### Rule 1: One Thing
A tool does exactly one thing.
Not two things. Not "one thing and also this other thing."
ONE THING.

### Rule 2: Stack Effect
Document the stack effect: `(input -- output)`
```
add: (a b -- sum)
dup: (a -- a a)
if:  (cond then else -- result)
```

### Rule 3: No Hidden State
A tool's behavior depends ONLY on:
- The stack
- The context (caps, resources)
Never on global variables or external state.

### Rule 4: Fail Loud
If something is wrong, return an Error.
Don't silently continue with bad data.
Don't return null when you mean failure.

### Rule 5: Names are API
The name tells you what it does.
- `add` adds
- `list-get` gets from a list
- `str-concat` concatenates strings
If the name is unclear, the tool is wrong.

---

## The Rules for Writing Rust

The Rust code MUST embody the philosophy:

### Rule R1: Small Functions
Every function does one thing.
If it does two things, split it.

### Rule R2: Explicit Types
No `impl Trait` in return position when clarity matters.
The type signature is documentation.

### Rule R3: No Magic
No macros that hide logic.
No trait magic that obscures behavior.
A reader should understand by reading.

### Rule R4: Error, Not Panic
Return `Result<T, Error>`.
`panic!` is for bugs, not business logic.
`unwrap()` is forbidden except in tests.

### Rule R5: Test Everything
Every tool has a test.
Every algebraic property has a proof.
If it's not tested, it doesn't work.

---

## The Hierarchy

```
                    ┌─────────────┐
                    │  PREAMBLE   │  ← You are here (absolute truth)
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
        ┌─────▼─────┐ ┌────▼────┐ ┌─────▼─────┐
        │ algebra.rs│ │ core.rs │ │ value.rs  │  ← Foundations
        └───────────┘ └─────────┘ └───────────┘
                           │
                    ┌──────▼──────┐
                    │ builtins.rs │  ← ~50 primitives
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │   stdlib/   │  ← Composed tools
                    └─────────────┘
```

---

## The Test of Truth

Before committing any code, ask:

1. **Does it follow the Three Postulates?**
   - Is it a tool? Does it transform a stack? Is it composable by concatenation?

2. **Is it minimal?**
   - Can it be removed? Can it be simpler?

3. **Is it predictable?**
   - Will an LLM understand it? Will a human understand it?

4. **Is it explicit?**
   - Is there any hidden behavior? Any implicit conversion?

5. **Is it tested?**
   - Does a test prove it works? Does a test prove the invariants?

If any answer is NO, the code is wrong.

---

## The Machine Manifesto

This is code for machines, by machines, of machines.

We optimize for:
- **Parseability:** An LLM can read and understand
- **Predictability:** Same input, same output, always
- **Composability:** Small parts that combine
- **Verifiability:** Proofs, not promises

We do NOT optimize for:
- Human ergonomics at the cost of clarity
- Brevity at the cost of explicitness
- Cleverness at the cost of simplicity

The machines are coming. This is their foundation.

---

## Amendments

This document may only be amended by:
1. Proposing a change
2. Demonstrating it improves alignment with the Three Postulates
3. Updating all affected code and tests
4. Proving the change maintains all invariants

No change that violates a Postulate shall be accepted.
No change that adds complexity without removing more shall be accepted.

---

*The Preamble is complete. The machines may begin.*
