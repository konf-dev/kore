# Formal Verification in Kore: A Literature Survey and Design

> "A proof is a program, and the formula it proves is the type for the program."  
> — Curry-Howard Correspondence

This document explores how Kore can provide **mathematical guarantees** about program correctness through its unique design, and what this means for AI-generated code.

---

## Table of Contents

1. [The Core Question](#the-core-question)
2. [What is "Correctness"?](#what-is-correctness)
3. [Why Kore is Uniquely Suited for Verification](#why-kore-is-uniquely-suited)
4. [Literature Survey](#literature-survey)
5. [Stack Effects as Types](#stack-effects-as-types)
6. [The Proof System](#the-proof-system)
7. [What Proofs Can Verify](#what-proofs-can-verify)
8. [What Proofs Cannot Verify](#what-proofs-cannot-verify)
9. [Pros and Cons](#pros-and-cons)
10. [Implementation Roadmap](#implementation-roadmap)
11. [Use Cases Beyond AI](#use-cases-beyond-ai)
12. [References](#references)

---

## The Core Question

> *"AI will be writing this code by composing existing code. Is there a way to mathematically verify if a composition is correct?"*

**Yes.** And Kore's design makes this not just possible, but elegant.

The key insight is that Kore's three postulates create a system where:
1. Every tool has a **fixed, known effect** (stack transformation)
2. Composition is **concatenation** (effects compose algebraically)
3. Verification becomes **type checking** (decidable at compile time)

---

## What is "Correctness"?

"Correctness" is not a single thing—it's a spectrum of properties we might want to verify:

### Level 1: Stack Safety (We have this now)
> "This program won't crash due to stack underflow."

```
Property: At every point, stack depth ≥ values consumed
Proof: Static analysis of stack effects
Decidable: Yes, in O(n) time
```

### Level 2: Type Safety
> "This program won't apply operations to wrong types."

```
Property: If a tool expects (Int, Int), it receives (Int, Int)
Proof: Type inference + checking
Decidable: Yes, with type annotations
```

### Level 3: Functional Correctness
> "This program computes what it claims to compute."

```
Property: output = specification(input)
Proof: Dependent types or theorem proving
Decidable: Partially (undecidable in general)
```

### Level 4: Resource Safety
> "This program respects capability and resource constraints."

```
Property: Never uses capabilities it doesn't have
Proof: Capability calculus (already in Kore's design)
Decidable: Yes
```

### Level 5: Termination
> "This program always halts."

```
Property: No infinite loops
Proof: Structural recursion / well-founded ordering
Decidable: No (Halting Problem), but decidable for restricted cases
```

---

## Why Kore is Uniquely Suited

### The Concatenative Advantage

Most languages have complex composition:
```python
# Python: What's the "type" of this composition?
result = f(g(x, y), h(z))
# Answer: Depends on f, g, h, x, y, z in complex ways
```

Kore has simple composition:
```kore
# Kore: Composition IS concatenation
x y g z h f
# Effect: sum of individual effects, in order
```

This is the **algebraic structure of a monoid**:
- Identity: empty program `[]`  
- Operation: concatenation `;`
- Associativity: `(P ; Q) ; R = P ; (Q ; R)`

### The Mathematical Framework

Kore programs form a **category**:
- **Objects**: Stack types (like `[Int Int]`)
- **Morphisms**: Tools (stack transformations)
- **Composition**: Concatenation
- **Identity**: No-op (empty quote)

This maps directly to **Curry-Howard-Lambek correspondence**:
| Logic | Types | Category |
|-------|-------|----------|
| Proposition | Type | Object |
| Proof | Term | Morphism |
| Implication | Function | Hom-set |
| Conjunction | Product | × |
| Disjunction | Sum | + |

---

## Literature Survey

### 1. Curry-Howard Correspondence (1934-1969)

**Key Papers:**
- Curry (1934): Types as axiom schemes
- Howard (1969): Proofs as programs, formulas as types
- de Bruijn (1968): Automath proof checker

**Core Insight:** 
> A type-correct program IS a proof of its type signature.

If we can verify `add : (Int Int -- Int)`, we have proved that `add` transforms two integers into one integer.

**Relevance to Kore:**
Kore's effect signatures `(consumed -- produced)` are propositions. A well-typed Kore program is a proof that the stack transformation is valid.

### 2. Linear Type Systems (1987-present)

**Key Papers:**
- Girard (1987): Linear Logic
- Wadler (1990): Linear Types Can Change the World
- Walker (2002): Substructural Type Systems

**Core Insight:**
> Resources are used exactly once.

| Type System | Exchange | Weakening | Contraction | Use |
|-------------|----------|-----------|-------------|-----|
| **Ordered** | No | No | No | Exactly once, in order |
| **Linear** | Yes | No | No | Exactly once |
| **Affine** | Yes | Yes | No | At most once |
| **Relevant** | Yes | No | Yes | At least once |
| **Normal** | Yes | Yes | Yes | Arbitrarily |

**Relevance to Kore:**
Kore's stack is **ordered linear**: values on the stack are used exactly once, in order. This is the strictest (and most verifiable) discipline.

```kore
# Linear: x is consumed exactly once
x dup add    # Creates x x, then consumes both
```

### 3. Concatenative Language Theory (1977-present)

**Key Papers:**
- Moore (1970): Forth language design
- von Thun (2001): Joy, mathematical foundations
- Diggins (2008): Cat type system

**Core Insight:**
> Composition = Concatenation. No variables needed.

```
Theorem: The syntax and semantics of concatenative languages 
         form the algebraic structure of a monoid.
```

**Relevance to Kore:**
Kore is concatenative by design. The effect algebra is:

```
Effect(P ; Q) = compose(Effect(P), Effect(Q))

where compose((a, b), (c, d)) = 
  if b >= c then (a, b - c + d)
  else (a + c - b, d)
```

### 4. Effect Systems (1988-present)

**Key Papers:**
- Lucassen & Gifford (1988): Polymorphic Effect Systems
- Wadler (1995): Monads for functional programming
- Leijen (2017): Koka language

**Core Insight:**
> Track not just what a program returns, but what it DOES.

Effects include:
- Memory (read, write, allocate)
- I/O (file, network)
- Control (exceptions, continuations)
- Non-determinism

**Relevance to Kore:**
Every Kore tool has declared effects via capabilities:

```kore
# Tool signature with effects
fs-write : (Text Text -- ) [cap:fs-write]
```

### 5. Refinement Types (1991-present)

**Key Papers:**
- Freeman & Pfenning (1991): Refinement Types for ML
- Rondon, Kawaguchi, Jhala (2008): Liquid Types

**Core Insight:**
> Types can include predicates: `{x : Int | x > 0}`

```haskell
-- Liquid Haskell example
divide :: Int -> {v:Int | v /= 0} -> Int
```

**Relevance to Kore:**
We could add refinement types to tool signatures:

```kore
# Refined effect
sqrt : ({x:Float | x >= 0} -- {y:Float | y >= 0})
```

### 6. Dependent Types (1970-present)

**Key Papers:**
- Martin-Löf (1971): Intuitionistic Type Theory
- Coquand & Huet (1988): Calculus of Constructions
- Brady (2013): Idris language

**Core Insight:**
> Types can depend on values.

```idris
-- Idris: Vector with length in the type
append : Vect n a -> Vect m a -> Vect (n + m) a
```

**Relevance to Kore:**
Dependent effects could express invariants:

```kore
# Dependent effect: tensor shape preserved
tensor-add : (Tensor[n,m] Tensor[n,m] -- Tensor[n,m])
```

---

## Stack Effects as Types

### The Effect Algebra

Every Kore tool has an effect signature:

```
Tool : (inputs -- outputs)
     = (consumes, produces)
     = (n, m) where n, m ∈ ℕ
```

Effects compose:

```
compose : Effect × Effect → Effect
compose((a, b), (c, d)) = 
  let delta = b - c in
  if delta >= 0 
    then (a, delta + d)      # enough outputs for next inputs
    else (a - delta, d)      # need more inputs
```

### The Verification Theorem

```
Theorem (Stack Safety):
  Given a sequence of operations P = [op₁, op₂, ..., opₙ]
  with effects E = [e₁, e₂, ..., eₙ]
  
  P is stack-safe iff:
    ∀i ∈ [1,n]: running_depth(i) ≥ 0
    
  where running_depth(i) = Σⱼ₌₁ⁱ net_change(eⱼ)
        net_change((c, p)) = p - c
        
  This is decidable in O(n) time.
```

### Implementation

We already have this in `src/analyzer.rs`:

```rust
fn analyze_ops(&mut self, ops: &[Op]) -> StackAnalysis {
    let mut current = 0i32;  // running depth
    
    for op in ops {
        let (consumes, produces) = self.get_tool_effect(op);
        
        if current < consumes {
            // UNDERFLOW: proved incorrect
            return error("stack underflow");
        }
        
        current = current - consumes + produces;
    }
    
    // Reached end: proved correct
    StackAnalysis::ok(min_depth, current)
}
```

---

## The Proof System

### What We Can Prove Now

**1. Stack Safety** (implemented)
```kore
# ✓ Provably safe: effect (0 -- 1)
[1 2 add]

# ✗ Provably unsafe: underflow at 'add'
[add 1 2]
```

**2. Effect Composition** (implemented)
```kore
# Effect of 'double': (1 -- 1)
[ dup add ] "double" def

# Effect of 'quadruple': compose((1,1), (1,1)) = (1,1) ✓
[ double double ] "quadruple" def
```

### What We Could Prove (Future Work)

**3. Type Safety**
```kore
# Refined signature
add : (Int Int -- Int)

# This should fail type check
["hello" 42 add]  # Error: Text is not Int
```

**4. Capability Safety** (designed, not fully implemented)
```kore
# This tool requires fs-write capability
"data.txt" "hello" fs-write

# In a context without fs-write → compile error
```

**5. Resource Bounds** (designed, not fully implemented)
```kore
# Declare resource usage
expensive-op : (1 -- 1) [uses: compute 1000]

# Static analysis can sum up resource usage
```

---

## What Proofs Can Verify

### ✓ Properties We CAN Prove

| Property | Decidable? | Complexity | Status |
|----------|------------|------------|--------|
| Stack safety | Yes | O(n) | Implemented |
| Effect composition | Yes | O(n) | Implemented |
| Type safety | Yes* | O(n) with inference | Design phase |
| Capability safety | Yes | O(n) | Designed |
| Resource bounds | Yes | O(n) | Designed |
| Termination (restricted) | Yes | O(n²) | Not started |

*With some type annotations

### ✗ Properties We CANNOT Prove (Undecidable)

| Property | Why Undecidable | Workaround |
|----------|-----------------|------------|
| General termination | Halting problem | Structural recursion |
| Full functional correctness | Rice's theorem | Testing + spec |
| Arbitrary refinements | SMT solving limits | Liquid types |
| Semantic equivalence | Undecidable | Bisimulation |

### The Fundamental Limit

> **Rice's Theorem:** Any non-trivial semantic property of programs is undecidable.

We can prove:
- "This program doesn't crash" (syntactic)
- "This program uses these capabilities" (syntactic)

We cannot prove:
- "This program computes factorial" (semantic)
- "This program is equivalent to that program" (semantic)

---

## Pros and Cons

### Pros of Formal Verification in Kore

| Advantage | Explanation |
|-----------|-------------|
| **Compositional** | Verify components, get system guarantees for free |
| **Decidable** | No "maybe" answers for stack/type safety |
| **Fast** | O(n) verification, runs at compile time |
| **AI-friendly** | AI can generate verified code by following rules |
| **Self-documenting** | Effect signatures ARE the specification |
| **No runtime overhead** | Proofs are compile-time, zero cost at runtime |
| **Incremental** | Add verification gradually, not all-or-nothing |

### Cons of Formal Verification

| Disadvantage | Explanation |
|--------------|-------------|
| **Limited scope** | Can't prove everything (fundamental limits) |
| **Annotation burden** | May need type/effect annotations |
| **Expressiveness tradeoff** | Some valid programs may not type-check |
| **Learning curve** | Developers must understand effects |
| **False positives** | Conservative analysis may reject good code |
| **Complexity ceiling** | Dependent types are hard to infer |

### The Tradeoff Spectrum

```
More Expressive ←————————————————————→ More Verifiable

Python     JavaScript    Rust      Haskell    Idris     Coq
  ↑                                              ↑
  │                                              │
"Anything goes"                    "Everything proven"

              Kore aims here: ──────────┤
              Maximum verification with practical expressiveness
```

---

## Implementation Roadmap

### Phase 1: Stack Effects (DONE ✓)

```rust
// Current analyzer.rs
pub fn analyze(ops: &[Op]) -> StackAnalysis
```

**Proves:** No stack underflows

### Phase 2: Type Effects (Next)

```rust
// Add type tracking to effects
type Effect = (Vec<Type>, Vec<Type>);

fn type_check(ops: &[Op]) -> TypeAnalysis {
    // Track types, not just counts
}
```

**Proves:** Type correctness

### Phase 3: Capability Effects

```rust
// Track capabilities required
type CapEffect = HashSet<Capability>;

fn cap_check(ops: &[Op], available: &CapEffect) -> CapAnalysis {
    // Verify capabilities are available
}
```

**Proves:** Capability safety

### Phase 4: Refinement Types

```rust
// Add predicates to types
type RefinedType = (Type, Predicate);

fn refine_check(ops: &[Op]) -> RefinedAnalysis {
    // Use SMT solver for predicates
}
```

**Proves:** Value-level invariants

### Phase 5: Dependent Effects (Research)

```rust
// Effects that depend on values
type DepEffect = (Vec<DepType>, Vec<DepType>);
// Where DepType can reference term variables
```

**Proves:** Shape preservation, index safety, etc.

---

## Use Cases Beyond AI

### 1. Smart Contracts

```kore
# Financial transaction: must be provably correct
: transfer ( from to amount -- )
  balance-check        # Proves: from has >= amount
  debit                # Proves: from -= amount  
  credit               # Proves: to += amount
  log-transfer         # Proves: audit trail created
;
```

Verification ensures: no double-spending, no negative balances, complete audit trail.

### 2. Medical Devices

```kore
# Drug dosage calculation: lives depend on correctness
: calculate-dose ( weight age drug -- dose )
  lookup-base-dose     # ( weight age base -- )
  weight-adjust        # Refinement: dose in safe range
  age-adjust           # Refinement: dose in safe range
  verify-bounds        # Proves: min <= dose <= max
;
```

Verification ensures: dose always in safe therapeutic range.

### 3. Aerospace Systems

```kore
# Flight control: must handle all cases
: autopilot-adjust ( sensors -- controls )
  validate-sensors     # Proves: all readings present
  compute-trajectory   # Proves: physics constraints
  bound-outputs        # Proves: actuator limits
  apply-controls       # Capability: requires flight-control
;
```

Verification ensures: physical constraints always respected.

### 4. Cryptographic Protocols

```kore
# Secure communication: security properties
: encrypt-message ( msg key -- ciphertext )
  validate-key-size    # Proves: key is correct size
  pad-message          # Proves: padding is correct
  aes-encrypt          # Capability: crypto
  authenticate         # Proves: MAC attached
;
```

Verification ensures: crypto primitives used correctly.

### 5. Compiler Backends

```kore
# Code generation: correctness-preserving
: compile-expr ( ast -- bytecode )
  validate-ast         # Proves: well-formed input
  type-check           # Proves: type-correct
  optimize             # Proves: semantics preserved
  emit-code            # Proves: valid bytecode
;
```

Verification ensures: compilation doesn't introduce bugs.

### 6. Robotics

```kore
# Robot movement: safety constraints
: move-arm ( target -- )
  collision-check      # Proves: path is clear
  velocity-limit       # Proves: speed in bounds
  joint-limit          # Proves: angles valid
  execute-motion       # Capability: motor-control
;
```

Verification ensures: physical safety constraints.

---

## What This Means for Kore

### The Design Philosophy Holds

Kore's postulates naturally support verification:

| Postulate | Verification Implication |
|-----------|-------------------------|
| P1: Everything is a Tool | All effects are declared |
| P2: Tools Transform Stack | All effects are composable |
| P3: Composition is Concatenation | All proofs are modular |

### The Mathematical Foundation

Kore programs live in a well-understood mathematical structure:

```
Kore ≅ Free Monoid on Tools
     ≅ Category of Stack Transformations
     ≅ Linear Logic (multiplicative fragment)
```

This means 50+ years of research applies directly.

### The Verification is a Tool

Per Postulate 1, verification itself should be a tool:

```kore
# Load and verify a program
"my-program.kore" fs-read parse effect-check

# Returns: { stack_safe: true, min_depth: 0, net_change: 0 }
```

---

## Conclusion

### Summary

1. **Correctness is provable** in Kore for many important properties
2. **Stack safety is decidable** and already implemented
3. **Type safety is decidable** and next to implement
4. **The algebra is clean** because concatenative = compositional
5. **AI can generate verified code** by following the type/effect rules
6. **Some properties remain undecidable** (that's fundamental)

### The Vision

```
AI writes code → Kore verifies effects → Only correct code runs
                        ↓
              "Composition is correct by construction"
```

This is not a distant dream—it's the natural consequence of Kore's design.

---

## References

### Foundational Papers

1. Curry, H. (1934). "Functionality in Combinatory Logic". PNAS.
2. Howard, W. (1969/1980). "The formulae-as-types notion of construction".
3. Martin-Löf, P. (1971). "Intuitionistic Type Theory".
4. Girard, J-Y. (1987). "Linear Logic". Theoretical Computer Science.

### Concatenative Languages

5. von Thun, M. (2001). "Mathematical Foundations of Joy".
6. Diggins, C. (2008). "What is a Concatenative Language". Dr. Dobb's.
7. Purdy, J. (2012). "Why Concatenative Programming Matters".

### Effect Systems

8. Lucassen & Gifford (1988). "Polymorphic Effect Systems". POPL.
9. Wadler, P. (1995). "Monads for Functional Programming".
10. Leijen, D. (2017). "Type Directed Compilation of Row-typed Algebraic Effects". POPL.

### Refinement & Dependent Types

11. Freeman & Pfenning (1991). "Refinement Types for ML". PLDI.
12. Rondon, Kawaguchi, Jhala (2008). "Liquid Types". PLDI.
13. Brady, E. (2013). "Idris: A Language with Dependent Types".

### Linear Types

14. Walker, D. (2002). "Substructural Type Systems". In Pierce (ed.) TAPL.
15. Bernardy et al. (2017). "Linear Haskell". POPL.
16. Baker, H. (1993). "Linear Logic and Permutation Stacks".

### Practical Systems

17. Coq Development Team. "The Coq Proof Assistant".
18. Agda Development Team. "The Agda Programming Language".
19. Vazou, N. (2018). "Liquid Haskell". POPL Tutorial.
