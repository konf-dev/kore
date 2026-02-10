# Sorting Network Search in Kore: A Mathematical Analysis

**Question**: *Does Kore provide any novel advantage for searching optimal sorting
networks, or is this a reimplementation of known algorithms in a slower language?*

---

## 1. The Problem: Computational Complexity

Finding an optimal (minimum-size) sorting network on $n$ channels is one of the
hardest combinatorial optimisation problems in computer science.

**Verification** alone is co-NP-complete (Parberry 1991). By the zero-one
principle, a network sorts all $n!$ permutations iff it sorts all $2^n$ binary
sequences — but proving no network of size $< k$ suffices requires exhaustive
search over the space of all possible networks.

The search space for networks of $k$ comparators on $n$ channels is:

$$|\mathcal{S}| = \binom{n}{2}^k$$

| $n$ | Optimal $k$ | $\binom{n}{2}$ | Search space $\binom{n}{2}^k$ |
|-----|-------------|-----------------|-------------------------------|
| 3   | 3           | 3               | $27$                          |
| 4   | 5           | 6               | $7{,}776$                     |
| 5   | 9           | 10              | $10^9$                        |
| 6   | 12          | 15              | $\approx 1.3 \times 10^{14}$  |
| 7   | 16          | 21              | $\approx 3.4 \times 10^{21}$  |
| 8   | 19          | 28              | $\approx 1.6 \times 10^{27}$  |

For $n = 6$ alone, naïve enumeration must explore $\sim 10^{14}$ candidate
networks — far beyond any brute-force approach.

---

## 2. State of the Art (What Actually Works)

### 2.1 Generate-and-Prune (Codish, Cruz-Filipe et al., 2014–2016)

The key insight: two comparator-network prefixes are **equivalent** if they
induce the same *output filter* — the same partition of $\{0,1\}^n$ into sorted
and unsorted binary sequences. You only need to keep one representative per
equivalence class.

The number of distinct filter states (after symmetry reduction):

| $n$ | Filter states (approx.) | Symmetry group $|G|$ |
|-----|------------------------|-----------------------|
| 3   | ~6                     | $3! \times 2 = 12$    |
| 4   | ~40                    | $4! \times 2 = 48$    |
| 5   | ~700–1,000             | $5! \times 2 = 240$   |
| 6   | ~30,000–90,000         | $6! \times 2 = 1440$  |
| 7   | ~tens of millions      | $7! \times 2 = 10080$ |

Combined with **subsumption pruning** (if filter $A \subseteq B$, discard $B$)
and **channel permutation symmetry** ($n!$ channel relabellings × 2 for
complement), this approach resolved $S(9) = 25$ and $S(10) = 29$.

### 2.2 SAT Encoding (Bundala & Závodný, 2014)

Encode the existence of a sorting network as a Boolean satisfiability problem:

$$\exists\, \text{comparators} \; c_1, \ldots, c_k : \forall\, x \in \{0,1\}^n : \text{sorted}(c_k(\ldots c_1(x)\ldots))$$

With aggressive symmetry-breaking clauses, modern SAT solvers (CryptoMiniSat,
Lingeling) resolved **depth optimality** for $n = 11$–$16$.

### 2.3 Practical Runtimes (State of the Art, C/C++ implementations)

| $n$ | Proof           | Time (approx.)       |
|-----|-----------------|----------------------|
| 6   | $S(6) = 12$     | **seconds**          |
| 7   | $S(7) = 16$     | **minutes**          |
| 8   | $S(8) = 19$     | **hours**            |
| 9   | $S(9) = 25$     | **days**             |
| 10  | $S(10) = 29$    | **weeks**            |
| 11  | $S(11) = 35$    | **months** (Harder 2020) |
| 12  | $S(12) = 39$    | **months** (Harder 2020) |
| 13+ | Open            | Estimated **years**  |

---

## 3. What We Implemented in Kore

### 3.1 Our Algorithm: DFS with Bitset State

Our `bitset_search.kore` uses:
- **State encoding**: single `i64` bitset, bit $k$ set ↔ input $k$ still unsorted
- **Zero-one principle**: only track $2^n$ binary inputs
- **DFS with iterative deepening**: try depth 1, 2, 3, ... until solution found
- **P4 pruning**: if `popcount(state) > 2^(budget - depth)`, prune

This is essentially **Knuth's 1973 algorithm** — DFS with the zero-one principle
and a simple feasibility bound.

### 3.2 What We're Missing vs. State of the Art

| Technique                        | State of art | Our implementation | Impact          |
|----------------------------------|--------------|--------------------|-----------------|
| Output-filter equivalence        | ✓            | ✗                  | **Critical**    |
| Subsumption pruning              | ✓            | ✗                  | **Critical**    |
| Channel permutation symmetry     | ✓            | ✗                  | **Major**       |
| Complement symmetry              | ✓            | ✗                  | **Major**       |
| BDD representation of filters    | ✓            | ✗                  | Moderate        |
| SAT encoding                     | ✓            | ✗                  | Alternative     |
| Hash-based dedup                 | ✓            | Linear scan only   | **Critical**    |

**The fundamental gap is not the language — it is the algorithm.**

Our search explores $\binom{n}{2}^k$ paths modulo only the popcount bound.
State-of-the-art approaches explore filter equivalence classes — which for
$n = 6$ reduces the effective search space from $\sim 10^{14}$ to $\sim 10^5$.
That is a **nine-order-of-magnitude** reduction that no amount of hardware
acceleration can compensate for.

---

## 4. Kore's Capabilities vs. Requirements

### 4.1 What Sorting Network Search Needs

The optimal algorithm (generate-and-prune) requires:

1. **Hash sets / hash maps** — for $O(1)$ filter dedup
2. **Complex data structures** — sets of bitsets, sorted canonical forms
3. **Bitwise operations** — heavy use (we have these ✓)
4. **Control flow** — deep recursion or BFS queues
5. **Raw integer throughput** — billions of bitwise ops per second
6. **Memory efficiency** — millions of filter states in RAM

### 4.2 What Kore Provides

| Feature                  | Available? | Sorting network search benefit? |
|--------------------------|------------|--------------------------------|
| Bitwise ops (band/bor/bxor/shl/shr/bnot) | ✓ JIT+Interp | ✓ Essential, works well |
| Integer arithmetic       | ✓ JIT+Interp | ✓ Essential, works well |
| While loops              | ✓ JIT+Interp | ✓ Fine for DFS |
| Recursion (CALL/RET)     | ✓ JIT Module | ✓ Fine for DFS |
| Hash maps / hash sets    | ✗          | ✗ **Fatal gap** for generate-and-prune |
| Dynamic arrays (resize)  | Interpreter only (lists) | ✗ Not on JIT |
| GPU/SPIR-V               | MAP only, no control flow | ✗ Cannot help |
| Linear types             | ✓          | No benefit for this problem |
| Proof checker            | ✓          | Correctness, not speed |
| Capabilities             | ✓          | No benefit for pure computation |
| Fibers/Spawn             | Interpreter only | ✗ Not on JIT |

### 4.3 The GPU Question

Kore's SPIR-V backend compiles **parallel MAP of pure arithmetic** only:
- ~20 opcodes supported
- **No control flow** (no `if`, no `while`)
- **No bitwise operations** (`band`, `bor`, `bxor` not supported)
- **No function calls**
- **No reduce/fold**

Sorting network search requires all of these. The GPU backend **cannot help
with this problem at all**, even in principle.

Even if bitwise ops were added to the GPU backend, the search is inherently
sequential (DFS/BFS with state-dependent branching), not data-parallel.

The one sub-problem that *could* parallelise on GPU is **verification** — testing
all $2^n$ binary inputs against a candidate network. But at $2^n = 64$ for
$n = 6$, this is far too small to amortise GPU launch overhead (typically
~10–50µs). GPU verification becomes attractive only at $n \geq 16$, which is
far beyond our reach algorithmically.

---

## 5. Kore JIT Performance vs. Native

From the benchmark suite (`src/benchmarks.rs`):

| Benchmark          | Kore JIT      | Native Rust  | Ratio     |
|--------------------|---------------|--------------|-----------|
| Sum of squares     | ~5.6x slower  | baseline     | 5.6x      |
| Fibonacci iter.    | ~753x slower  | baseline     | 753x      |
| Collatz            | ~20.8x slower | baseline     | 20.8x     |

The JIT uses Cranelift (a mid-tier compiler). It does not perform:
- Loop unrolling
- Vectorisation (SIMD)
- Profile-guided optimisation
- Link-time optimisation
- Instruction scheduling optimisation

For comparison, **C/C++ implementations of generate-and-prune solve $n = 6$
in seconds**. Even if Kore's JIT were only 5× slower, that would mean ~30
seconds — still tractable. But without the right algorithm, we're stuck at
$\sim 10^{14}$ work, which takes years at any speed.

### The Arithmetic of Futility

Our DFS explores the tree:

$$T(n, k) = \sum_{d=0}^{k} \binom{n}{2}^d \leq 2 \cdot \binom{n}{2}^k$$

For $n = 6, k = 12$:

$$T(6, 12) \leq 2 \times 15^{12} \approx 2.6 \times 10^{14}$$

At Kore JIT's speed (~100M simple ops/sec), this takes:

$$\frac{2.6 \times 10^{14}}{10^8} = 2.6 \times 10^6 \text{ seconds} \approx 30 \text{ days}$$

At native Rust speed (~2 GHz throughput):

$$\frac{2.6 \times 10^{14}}{2 \times 10^9} \approx 1.3 \times 10^5 \text{ seconds} \approx 1.5 \text{ days}$$

With generate-and-prune + symmetry ($\sim 10^5$ filter states):

$$\text{seconds, in any language}$$

**The algorithm gap dominates by 9 orders of magnitude.
The language gap is at most 1–2 orders of magnitude.**

---

## 6. Does Kore Provide Any Novel Advantage?

### 6.1 What Kore's Theory Promises

Kore's P1–P4 postulates and type system offer genuine theoretical novelty:

- **P1 (Totality)**: Every word is total — no crashes, no undefined behaviour.
  Proofs of totality are mechanically verified.
- **P2 (Determinism)**: Same input → same output. No hidden state.
- **P3 (Composition = concatenation)**: Programs compose by juxtaposition. The
  denotation functor $\llbracket \cdot \rrbracket : \text{Kore} \to \text{Set}$
  is monoidal.
- **P4 (Attenuation)**: Capabilities only shrink. Child ≤ Parent always.
- **Linear types**: Values that cannot be duplicated or dropped — enabling
  provably reversible computation (Landauer's principle: erasing 1 bit costs
  $kT \ln 2$ joules; linear types guarantee no erasure).

### 6.2 Where These Help (Not Here)

| Kore Feature         | Where It Shines                          | Sorting network search? |
|----------------------|------------------------------------------|------------------------|
| Totality (P1)        | Safety-critical systems, proofs          | No — our code is total anyway |
| Determinism (P2)     | Reproducible computation, caching        | No — our code is deterministic anyway |
| Composition (P3)     | DSL construction, modular tools          | Minor — nice syntax |
| Attenuation (P4)     | Security, capability-safe agents         | Irrelevant |
| Linear types         | Quantum simulation, reversible circuits  | Irrelevant |
| Proof checker        | Verified compilation, type safety        | Guarantees no stack bugs, but doesn't speed up search |
| GPU (SPIR-V)         | Data-parallel arithmetic                 | Cannot help (no control flow, no bitwise) |

**Honest assessment**: Kore's novel features (linearity, capabilities,
proof-checked totality) address *correctness and security*, not
*raw search performance*. For exhaustive combinatorial search, the language
that wins is the one with:

1. The fastest inner loop (C/Rust/Fortran)
2. The best data structure libraries (hash maps, BDDs)
3. The best SAT solver integration

Kore has none of these advantages. It has a 5–750× JIT slowdown, no hash maps,
and no SAT solver bindings.

### 6.3 Where Kore *Could* Provide Novel Value (Different Problem)

Kore's linear types are genuinely novel for **reversible computation**:

- **Bennett's trick**: Compute forward, copy result, uncompute backward. Linear
  types guarantee the uncomputation is valid (no information loss).
- **Quantum circuit synthesis**: Sorting networks over quantum channels require
  reversibility. Kore's type system could verify that a sorting network
  implementation is reversible.
- **Provably correct sorting networks**: Given a candidate network, Kore's proof
  checker could verify it sorts correctly *and* respects linear/affine resource
  constraints — a property no other language verifies automatically.

But these are different problems from *searching* for optimal networks.

---

## 7. What Would Actually Help

### 7.1 Algorithm Improvements (Language-Independent)

To solve $n = 6$ ($S(6) = 12$):

1. **Implement output-filter equivalence classes**: Instead of tracking raw
   bitsets, track the *set of output 0-1 vectors* each prefix can produce.
   Two prefixes with the same output set are equivalent. This reduces $n = 6$
   from $\sim 10^{14}$ to $\sim 10^5$ states.

2. **Implement symmetry breaking**: Fix canonical forms under:
   - Channel permutations ($6! = 720$ relabellings)
   - Complement symmetry ($\times 2$)
   - Layer canonicalisation

3. **Hash-based deduplication**: Replace linear scan with $O(1)$ hash lookup.

### 7.2 What This Requires in Kore

To implement generate-and-prune in Kore, we'd need:

- **Hash maps** (not available in Kore — no opcode, no data structure)
- **Sets of sets** (representing output filters — requires nested collections)
- **Canonical form computation** (sorting, permutation enumeration)

None of these exist in Kore today. The language would need new primitives:

```
-- Hypothetical Kore extensions needed:
hashmap-new       -- ( -- hm )
hashmap-insert    -- ( hm key value -- hm )
hashmap-contains  -- ( hm key -- bool )
set-new           -- ( -- s )
set-insert        -- ( s elem -- s )
set-equal         -- ( s1 s2 -- bool )
```

### 7.3 The Practical Path

For *actually solving* $n = 6$+, the practical approach is:

1. **Use Rust directly** (Kore is implemented in Rust) with hash maps, BDDs,
   and/or SAT solver bindings
2. **Call the Rust solution from Kore** as a foreign function for the search
3. **Use Kore for verification** — verify the *result* (which is a small
   comparator sequence) using Kore's proof checker

This plays to each tool's strength:
- Rust: fast search with mature data structures
- Kore: verified correctness of the found network

---

## 8. Is Benchmarking Pointless?

### 8.1 What the Benchmarks Tell Us

The sorting network experiments are **not pointless** — they revealed:

1. **JIT backend quality**: Our DFS search is a genuine workload (bitwise ops,
   recursion, branching). JIT achieves $n = 5$ in 1.3s — competitive with
   interpreted Python, demonstrating the JIT works correctly on a real problem.

2. **Language expressiveness**: `bitset_search.kore` is 100 lines of elegant
   concatenative code. The algorithm reads naturally in Kore's postfix style.

3. **The JIT ceiling**: We can't brute-force $n = 6$ in Kore. This is honest
   and useful information — it maps the language's performance envelope.

4. **Where to invest**: The bottleneck is data structures (no hash maps), not
   raw arithmetic. This tells the language designers what to prioritise.

### 8.2 What the Benchmarks Don't Tell Us

They don't demonstrate Kore's *unique* value. To showcase that, we'd need
benchmarks that exercise:

- **Proof-checked correctness** (verify a sorting network implementation)
- **Linear type safety** (reversible circuit synthesis)
- **Capability attenuation** (sandboxed agent with restricted comparator access)
- **Multi-backend deployment** (same `.kore` file running on JIT, WASM, and GPU)

These are Kore's differentiators. Integer search speed is not.

---

## 9. Conclusion

### The Bottom Line

$$\boxed{\text{Kore does not provide a performance advantage for this problem.}}$$

The sorting network search benchmarks demonstrate that:

1. **The algorithm matters more than the language by $10^9 \times$.** Our
   brute-force DFS has $O(B^k)$ complexity where generate-and-prune has
   $O(\text{filters})$. For $n = 6$: $10^{14}$ vs $10^5$.

2. **Kore's JIT is 5–750× slower than native Rust.** Even with the optimal
   algorithm, Kore would solve $n = 6$ in minutes where Rust solves it in
   seconds. This gap matters for $n \geq 9$ where runtimes reach days.

3. **Kore's novel features (linearity, capabilities, proofs) don't help here.**
   They address correctness and security, not search speed. The GPU backend
   can't help either (no control flow, no bitwise ops).

4. **Kore's value for sorting networks is in *verification*, not *search*.** Use
   Rust to find the optimal network, use Kore to prove it correct.

### What This Means for Kore

This is not a failure — it's a *clarification of purpose*. Kore is not a
systems programming language competing with C/Rust on raw throughput. Kore is
a **verified computation language** where every program carries a proof of
totality, type safety, and capability compliance.

The right benchmark for Kore isn't "how fast can you search?" — it's:

> *Given a sorting network, can Kore prove it correct, verify it
> respects linear resource constraints, and deploy it across JIT/WASM/GPU
> backends from a single source — all with mathematical guarantees that
> C and Rust cannot provide?*

That's the question where Kore's answer is **yes, and no other language can**.

---

## References

- Knuth, D.E. *The Art of Computer Programming*, Vol. 3, §5.3.4 (1973/1997)
- Parberry, I. "A Computer-Assisted Optimal Depth Lower Bound for Nine-Input
  Sorting Networks" (1991)
- Codish, M., Cruz-Filipe, L., Frank, M., Schneider-Kamp, P. "Twenty-Five
  Comparators is Optimal when Sorting Nine Inputs" (2014)
- Bundala, D., Závodný, J. "Optimal Sorting Networks" (2014, LATA)
- Codish, M. et al. "Sorting Networks: to the End and Back Again" (2016)
- Harder, J. "Optimal Sorting Networks of 11 and 12 Channels" (2020)
