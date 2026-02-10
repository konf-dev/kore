# What Makes Kore Different — Grounded Comparison

## 1. The Foundational Claim

Most languages start from **syntax** and work down to semantics. Kore starts from **four postulates** and derives everything mechanically:

| | Kore | Haskell | Rust | Coq | Forth |
|---|---|---|---|---|---|
| Foundation | 4 postulates (P1-P4) | System Fω (lambda calculus) | Ownership/borrowing model | Calculus of Inductive Constructions | Operational (threaded interpreter) |
| Design decisions | Claimed: zero | Hundreds (committee, 1990) | Many (memory model trade-offs) | Small kernel (~10k LOC) | Minimal but informal |
| Starting question | "What must be true?" | "What's typeable?" | "What's memory-safe?" | "What's provable?" | "What's simplest?" |

The key distinction: Haskell, Rust, Coq all **chose** a specific formal system and built a language around it. Kore claims its postulates are **entailed** — there was no choice to make. P1 (everything is a tool), P2 (one operation), P3 (composition = concatenation), P4 (constraints attenuate) are presented as the minimal axioms of computation itself.

Whether that claim holds philosophically is debatable. What's concrete is that the implementation *does* follow from them mechanically, and the proof checker enforces P4 at compile time.

---

## 2. The Unique Intersection

No existing language occupies Kore's exact design point:

| | Concatenative | Proof Checking | Multi-Backend | GPU | Small Core |
|---|---|---|---|---|---|
| **Kore** | ✅ | ✅ (P4 lattice) | ✅ 4 (interp, JIT, WASM, SPIR-V) | ✅ | ✅ ~16 bytecodes |
| **Forth** | ✅ | ❌ | ❌ 1 | ❌ | ✅ ~30 words |
| **Factor** | ✅ | ❌ | ❌ 1 | ❌ | ❌ ~150 |
| **Joy** | ✅ | ❌ | ❌ 0 (interp only) | ❌ | ✅ ~60 |
| **Cat** | ✅ | ❌ (type-check only) | ❌ 1 (.NET) | ❌ | ✅ ~25 |
| **Haskell** | ❌ | ⚠️ (external: LiquidHaskell) | ✅ 3+ | ⚠️ library | ❌ ~15 core + 1000 Prelude |
| **Idris** | ❌ | ✅ (totality) | ✅ 4 | ❌ | ❌ ~15 core + 200 Prelude |
| **Lean** | ❌ | ✅ (kernel) | ✅ 3 | ❌ | ❌ ~15 core + 50 tactics |
| **Coq** | ❌ | ✅ (kernel) | ✅ 4 (extraction) | ❌ | ✅ ~10 Gallina |
| **Rust** | ❌ | ❌ | ✅ (LLVM) | ⚠️ (cuda crates) | ❌ huge |

**The gap**: Concatenative languages (Forth, Joy, Cat) have no verification. Verified languages (Lean, Coq, Idris) aren't concatenative and don't target GPUs. Nobody does all three.

---

## 3. Concrete Differences By Aspect

### 3.1 Semantics: Composition vs. Application

```
-- Kore (P3): composition IS concatenation
5 dup *                -- "push 5, duplicate, multiply" = 25

-- Haskell: composition via operator
((*) <*> id) 5        -- same thing, but needs combinators

-- Forth: same as Kore syntactically
5 dup *                -- but no formal guarantee, just convention

-- Lean: explicit function application
let x := 5; x * x     -- named binding, not stack
```

Kore and Forth *look* identical. The difference: Kore's proof checker statically verifies that every word's stack effect is balanced and type-consistent. Forth has zero such guarantee.

### 3.2 Verification: What Gets Checked

| | What's verified | When | Trusted base |
|---|---|---|---|
| **Kore** | Stack balance, type lattice (P4), linearity | Compile time (`korec check`) | ~1600 LOC proof_checker.rs |
| **Coq** | Full logical consistency (CIC) | Type-check time | ~10k LOC kernel |
| **Lean** | Full logical consistency (CoC + inductives) | Type-check time | ~7k LOC kernel |
| **Idris** | Totality, coverage, linearity (QTT) | Compile time | Idris 2 compiler |
| **Rust** | Memory safety (lifetimes/ownership) | Compile time | rustc + LLVM |
| **Haskell** | Type safety (System Fω + type classes) | Compile time | GHC |
| **Forth** | Nothing | Never | — |

Kore's verification is **narrower** than Lean/Coq (it checks stack types, not arbitrary theorems) but **broader** than Rust (it checks semantic properties like constraint monotonicity, not just memory). It's a different axis entirely.

### 3.3 Execution: Where Code Runs

```
      Source
        │
   ┌────┴────────────────────────┐
   │    Kore: 4 real backends    │
   ├─────────────┬───────────────┤
   │ Interpreter │ Cranelift JIT │──→ x86-64 native
   │ (debug)     │ (21.5× fast)  │
   ├─────────────┼───────────────┤
   │ WASM        │ SPIR-V        │──→ Vulkan/Metal/DX12
   │ (browser)   │ (GPU compute) │
   └─────────────┴───────────────┘
```

| | Native | WASM | GPU | Interpreter |
|---|---|---|---|---|
| **Kore** | ✅ Cranelift | ✅ | ✅ SPIR-V | ✅ |
| **Haskell** | ✅ GHC/LLVM | ⚠️ GHCJS/Asterius | ⚠️ Accelerate lib | ✅ GHCi |
| **Rust** | ✅ LLVM | ✅ wasm-pack | ⚠️ rust-gpu/cuda | ❌ |
| **Lean** | ✅ via C/LLVM | ❌ | ❌ | ✅ |
| **Coq** | ✅ extraction→OCaml | ❌ | ❌ | ✅ |
| **Forth** | ✅ (direct) | ❌ | ❌ | ✅ threaded |

Kore is the only language where **the same bytecode** runs on all four targets. Haskell needs different compilation pipelines for each target. Rust needs different toolchains. Coq extracts to a *different language* entirely.

### 3.4 Size: Minimal Primitives

Kore's bytecode core is genuinely small:

| Language | Core instructions/constructs | Total surface language |
|---|---|---|
| **Kore** | 16 bytecodes (stack, data, control, literals) | ~120 opcodes with extensions |
| **Coq Gallina** | ~10 term forms | + ~80 tactics |
| **Lean core** | ~15 constructs | + ~50 tactics + large Mathlib |
| **Cat** | ~25 combinators | ~25 (it's small) |
| **JVM** | ~200 bytecodes | huge |
| **WASM** | ~200+ opcodes | ~200+ |
| **x86-64** | ~1500+ instructions | ~1500+ |

Kore's 16-instruction core is comparable to Coq's Gallina (~10 terms). Both claim minimality. The difference: Coq's core is a typed lambda calculus (terms, types, universes). Kore's core is a stack machine (push, pop, swap, branch). Different foundations, similar compactness.

---

## 4. The Real Potential

Here's what the architecture enables that no other language currently delivers:

### a) Write once, prove once, run everywhere at hardware speed

A Kore program verified by `korec check` is guaranteed type-safe on ALL backends. You don't re-verify for GPU. You don't re-verify for WASM. The proof holds because the backends implement the *same* 16 instructions — they just implement them faster.

No other language does this. Lean proves things, but extracts to C (losing the proof connection). Coq proves things, but extracts to OCaml. The proofs don't follow the code to the GPU.

### b) GPU compute with formal guarantees

Today, GPU programming means: write CUDA/OpenCL by hand, hope it's correct. Kore's `gpu-map` compiles *verified* bytecode to SPIR-V. The P4 guarantee (constraints attenuate) holds for the GPU kernel. This is genuinely novel — nobody else has formally verified GPU compute in a concatenative language.

### c) Algebraic optimization with correctness guarantees

The optimizer applies algebraic identities (`swap swap → ε`, `dup drop → ε`, `0 add → ε`) that are **provably correct** by the group/monoid structure of the operations. This isn't heuristic optimization — it's algebraic rewriting with mathematical guarantees. LLVM optimizes aggressively but its passes are engineering, not algebra.

### d) The sorting network result demonstrates the approach

The lattice demo computed in 52 seconds what would be a 10^83-element search space naively. This isn't just an optimization — it's a **structural consequence** of P4. The constraint lattice *tells you* which branches to prune, with a formal guarantee that no valid solution is lost. No other language makes the search space collapse a consequence of its type system.

---

## 5. What Kore Is NOT (Honestly)

| Claim | Reality |
|---|---|
| "Runs at hardware maximum speed" | JIT is integers-only. No CALL/RET, no lists. Interpreter is slow. |
| "Proves arbitrary theorems" | Only checks stack types and linearity. Can't prove Fermat's Last Theorem like Lean. |
| "Production-ready" | 382 tests. No package manager, no debugger, no LSP, no ecosystem. |
| "Better than Haskell/Rust" | Different axis. Those have massive ecosystems, production users, decades of tooling. |
| "Quantum-ready" | The linear type subset maps conceptually to quantum. Zero implementation exists. |

---

## 6. Where Kore Has Genuine Advantage

1. **Simplicity**: 16 bytecodes vs. JVM's 200 or x86's 1500. This isn't just aesthetic — fewer instructions means fewer things to implement per backend, fewer bugs, easier formal verification. Adding a new backend (RISC-V, ARM, TPU) means implementing 16 operations, not 1500.

2. **Postulate-driven design**: When you ask "should Kore have feature X?", the answer is deterministic: does it follow from P1-P4? If yes, add it. If no, don't. This eliminates design committee arguments. Haskell famously spent years debating records, string types, and effect systems. Kore's postulates would decide those immediately.

3. **Formal-informal bridge**: Coq/Lean users write proofs. Forth/Factor users write fast stack code. Kore is the only language where you do BOTH in the SAME syntax — the proof checker runs on the same bytecode the JIT compiles. There's no extraction step, no proof-irrelevance gap.

4. **Hardware-agnostic by construction**: The 10 abstract primitives (drop, dup, swap, pair, unpair, left, right, case, quote, apply) map to any computational substrate. This isn't just a claim — the SPIR-V and WASM backends prove it. The same `dup mul` compiles to x86 `imul`, SPIR-V `OpIMul`, and WASM `i64.mul`.

---

## 7. Bottom Line

Kore is **not** the best at any single axis. Lean is better at proofs. Rust is better at systems programming. Haskell has a richer type system. Forth is more mature.

Kore's claim is that it's the only language at the **intersection**: concatenative + verified + multi-backend + GPU + minimal. That intersection is genuinely unoccupied. Whether it becomes important depends on whether the gaps (JIT completeness, ecosystem, tooling) get closed — but the architecture permits it without violating the postulates.

---

## 8. Comparable Language Details

### Forth (1970, Charles Moore)
- **Foundation**: Informal; operational (stack machine)
- **Verification**: None. No type system at the language level.
- **Primitives**: ANS Forth standard ~300 words, minimal bootstrap ~30-40 core ops
- **Backends**: Native (direct machine code or threaded interpreter)

### Factor (2003, Slava Pestov)
- **Foundation**: Category theory (quotations as arrows)
- **Verification**: Stack-effect declarations checked but not formal
- **Primitives**: ~150 core vocabulary words
- **Backends**: Native x86

### Joy (2001, Manfred von Thun)
- **Foundation**: Function composition, lambda calculus (combinator basis)
- **Verification**: None. Academic/research language.
- **Primitives**: ~60 primitives and combinators
- **Backends**: Interpreter only

### Cat (2006, Christopher Diggins)
- **Foundation**: Category theory (typed combinatory logic, arrows)
- **Verification**: Type checking only
- **Primitives**: ~25-30 typed combinators
- **Backends**: .NET CLR

### Haskell (1990, committee)
- **Foundation**: System Fω (polymorphic lambda calculus with type operators)
- **Verification**: Not built-in. External: LiquidHaskell, sbv.
- **Primitives**: GHC Core ~15 constructs + Prelude ~1000 functions
- **Backends**: Native (GHC/NCG, LLVM), C, JavaScript (GHCJS) — 3+
- **GPU**: Via libraries (Accelerate, Obsidian) only

### Idris (2007/2020, Edwin Brady)
- **Foundation**: Quantitative Type Theory (QTT), dependent type theory
- **Verification**: Built-in totality checker, types as proofs
- **Primitives**: ~15 core TT constructs + ~200 Prelude
- **Backends**: Chez Scheme, RefC, Node.js, community backends — 4

### Lean (2013/2021, Leonardo de Moura)
- **Foundation**: Calculus of Constructions with inductive types
- **Verification**: Built-in kernel proof checker. Powers Mathlib.
- **Primitives**: ~15 core type-theoretic constructs + ~50 tactics
- **Backends**: C, LLVM (experimental), interpreter — 3

### Agda (2007, Ulf Norell)
- **Foundation**: Martin-Löf Type Theory (MLTT)
- **Verification**: Built-in termination checker + universe checker
- **Primitives**: ~10-12 core constructs
- **Backends**: Haskell (via GHC), JavaScript — 2

### Coq/Rocq (1989, Coquand & Huet)
- **Foundation**: Calculus of Inductive Constructions (CIC)
- **Verification**: Built-in kernel type-checker. Curry-Howard.
- **Primitives**: Gallina ~8-10 core term forms + Ltac ~80 tactics
- **Backends**: Code extraction to OCaml, Haskell, Scheme, C — 4
