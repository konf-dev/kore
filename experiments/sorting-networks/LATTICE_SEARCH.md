# Lattice-Guided Sorting Network Search

## The Problem

Find the minimum number of **comparators** (compare-and-swap operations)
needed to sort $n$ elements. A sorting network is a fixed sequence of
comparators $(i,j)$ where $i < j$: each comparator ensures
$\text{wire}[i] \leq \text{wire}[j]$ after execution.

### Known Optimal Values

| n  | Optimal | Found by          | Year | Status    |
|----|---------|-------------------|------|-----------|
| 1  | 0       | —                 | —    | Trivial   |
| 2  | 1       | —                 | —    | Trivial   |
| 3  | 3       | —                 | —    | Trivial   |
| 4  | 5       | —                 | —    | Known     |
| 5  | 9       | —                 | —    | Known     |
| 6  | 12      | —                 | —    | Known     |
| 7  | 16      | —                 | —    | Known     |
| 8  | 19      | —                 | —    | Known     |
| 9  | 25      | Codish et al.     | 2016 | Known     |
| 10 | 29      | Codish et al.     | 2016 | Known     |
| 11 | 35      | Ehlers & Müller   | 2019 | Known     |
| 12 | 39      | Ehlers & Müller   | 2019 | Known     |
| 13 | ≤ 45    | —                 | —    | **OPEN**  |
| 14 | ≤ 51    | —                 | —    | **OPEN**  |
| 15 | ≤ 56    | —                 | —    | **OPEN**  |
| 16 | ≤ 60    | —                 | —    | **OPEN**  |

Lower bounds (information-theoretic): $\lceil \log_2(n!) \rceil$
comparisons needed.

| n  | $\lceil \log_2(n!) \rceil$ | Best known | Gap |
|----|---------------------------|------------|-----|
| 9  | 19                        | 25         | 6   |
| 13 | 33                        | 45         | 12  |
| 16 | 45                        | 60         | 15  |


## The Kore Insight: Constraint Lattice as Search Topology

### Standard approaches and why they struggle

**SAT/CSP (Codish, Ehlers, Müller)**:
- Encode "there exists a network of $k$ comparators that sorts all
  $2^n$ binary inputs" as a SAT formula
- Feed to a SAT solver (Glucose, Lingeling, CaDiCaL)
- Exponential clause count: the formula for $n=13, k=44$ has ~$10^9$ clauses
- Solver gets lost in the flat, unstructured search space
- No compositionality: can't reuse partial solutions

**Genetic Programming / Evolutionary**:
- Mutate/crossover random networks
- Fitness = number of correctly sorted inputs out of $2^n$
- Problem: crossover is **destructive** — concatenating two partial
  sorters usually doesn't produce a better sorter
- No structural guidance

### The Kore approach: lattice-guided branch-and-bound

Instead of searching over networks directly, we search over **lattice
states** — where each state represents the set of permutations that
are still unsorted after the comparators applied so far.

#### The Lattice

Elements: subsets of $S_n$ (the symmetric group on $n$ elements).

```
⊤ = S_n          (all n! permutations — nothing sorted yet)
⊥ = {identity}   (only the identity remains — fully sorted)
```

Partial order: set inclusion ($A \subseteq B$ means $A \leq B$).

Meet: intersection ($A \wedge B = A \cap B$).
Join: union ($A \vee B = A \cup B$).

This is a **complete distributive lattice** (power set lattice of $S_n$).

#### Comparator as lattice operator

A comparator $(i,j)$ acts on the lattice as:

$$\text{apply}_{(i,j)}(R) = \{ \sigma \circ \tau_{ij} \mid \sigma \in R, \tau_{ij} \text{ fixes } \sigma \text{ if } \sigma(i) \leq \sigma(j) \}$$

More concretely: for each permutation $\sigma$ in $R$:
- If $\sigma(i) \leq \sigma(j)$: keep $\sigma$ unchanged
- If $\sigma(i) > \sigma(j)$: replace with $\sigma'$ where positions $i,j$ are swapped

The result $R' = \text{apply}_{(i,j)}(R)$ satisfies $|R'| \leq |R|$,
and crucially: **once a permutation is "fixed" it stays fixed.**

This gives us **P4 (monotone descent)**: the lattice element can only
shrink or stay the same. It never grows.

#### P3 (composition) gives us free pruning

Since comparators on disjoint wires commute:

$$\text{apply}_{(1,3)} \circ \text{apply}_{(5,7)} = \text{apply}_{(5,7)} \circ \text{apply}_{(1,3)}$$

This means we can **canonicalize** the order: always apply
non-interfering comparators in a fixed order. This eliminates a
massive number of equivalent search branches.

Formally, two comparators $(a,b)$ and $(c,d)$ commute iff
$\{a,b\} \cap \{c,d\} = \emptyset$. For $n=13$, roughly 70% of
comparator pairs commute, giving ~$2^{0.7k}$ symmetry reduction at
depth $k$.

#### P4 (constraints attenuate) gives us information-theoretic pruning

After applying $k$ comparators, we have a set $R_k$ of unsorted
permutations. We can compute:

$$\text{remaining\_lower\_bound} = \lceil \log_2(|R_k|) \rceil$$

because each comparator can at most halve the number of unsorted
permutations (it's a binary decision). If:

$$k + \lceil \log_2(|R_k|) \rceil > \text{best\_known}$$

then this branch **cannot possibly beat the current best**. Prune it.

This is the key advantage: we don't just know "is it valid?", we know
**how far away from solved we are**, giving us a tight bound.

#### P1 (totality) means no dead ends from errors

Every comparator applied to any lattice state produces a valid lattice
state. No "invalid move" exceptions. The search tree has no crashes,
only pruned branches.


## The Zero-One Principle

**Theorem (Knuth)**: A sorting network sorts all inputs correctly if
and only if it sorts all $2^n$ binary (0/1) inputs correctly.

This reduces verification from $n!$ to $2^n$ tests:

| n  | $n!$           | $2^n$  | Speedup    |
|----|----------------|--------|------------|
| 9  | 362,880        | 512    | 709×       |
| 13 | 6,227,020,800  | 8,192  | 760,000×   |
| 16 | 20,922,789,888,000 | 65,536 | 319,000,000× |

For GPU verification: we can test ALL $2^n$ inputs in a single kernel
launch, one thread per input.


## Algorithm Design

### Phase 1: Representation

We DO NOT store $R$ as a set of permutations (too large for $n \geq 10$).

Instead, we use the **binary representation** (zero-one principle):
- State = bitset of $2^n$ entries: 1 = this binary input is NOT YET
  guaranteed sorted, 0 = guaranteed sorted
- Initial state: all $2^n$ bits set to 1 (except the all-zeros and
  all-ones inputs which are trivially sorted) → $2^n - 2$ unsorted
- A comparator $(i,j)$ on a binary input $x$: if $x_i > x_j$, swap
  bits $i$ and $j$ in $x$, producing $x'$. If the resulting $x'$ is
  already sorted, mark the original as resolved.

Actually, more precisely: a binary sequence $b_0 b_1 \ldots b_{n-1}$ is
sorted iff $b_0 \leq b_1 \leq \ldots \leq b_{n-1}$, i.e., all 0s come
before all 1s. There are exactly $n+1$ sorted binary sequences (the
sequence with $k$ ones, for $k = 0, 1, \ldots, n$).

So the state is: for each of the $2^n - (n+1)$ unsorted binary sequences,
does applying the comparator network so far produce a sorted output?

**Representation**: a bitset of $2^n$ bits. Bit $x$ = 1 means "input $x$
is not yet sorted by the current network prefix."

Initial unsorted count: $2^n - (n+1)$.

| n  | Unsorted inputs | Bits needed | Bytes   |
|----|-----------------|-------------|---------|
| 9  | 502             | 512         | 64 B    |
| 13 | 8,178           | 8,192       | 1 KB    |
| 16 | 65,519          | 65,536      | 8 KB    |

This is **tiny**. We can hold millions of lattice states in GPU memory.

### Phase 2: Search Strategy

**Iterative deepening with lattice pruning**:

```
function search(state: Bitset, depth: int, max_depth: int, network: Vec<(i,j)>):
    if state.count_ones() == 0:
        // All inputs sorted! Found a valid network.
        report(network)
        return

    // P4 pruning: information-theoretic lower bound
    remaining = state.count_ones()
    if depth + ceil(log2(remaining)) > max_depth:
        return  // Cannot possibly complete in budget

    // P3 pruning: canonical ordering
    // Only try comparators >= last one in lexicographic order
    // (commutativity means we can impose this without losing solutions)
    for (i, j) in comparators_from(last_comparator(network)):

        new_state = apply_comparator(state, i, j)

        // Skip if comparator had no effect (pruning useless ops)
        if new_state == state:
            continue

        // Recurse
        search(new_state, depth + 1, max_depth, network + [(i,j)])
```

**Symmetry breaking** (additional pruning):

1. **First comparator**: WLOG we can fix the first comparator to
   $(0, 1)$ — any network can be relabeled to start here.

2. **Prefix canonicalization**: after each parallel layer (set of
   non-interfering comparators), canonicalize the wire permutation.

3. **Output symmetry**: if a network sorts $n$ elements, reversing
   all comparators and flipping the output sorts in reverse order.
   Only search for networks where the first comparator is "small."

### Phase 3: GPU Acceleration

The innermost operation — `apply_comparator(state, i, j)` — is
embarrassingly parallel:

```python
@cuda.jit
def apply_comparator_kernel(states, n_states, wire_i, wire_j, n_bits, results):
    """
    For each state (bitset) and each bit position:
    - Read the binary input corresponding to that bit
    - If bit wire_i=1 and bit wire_j=0, swap them
    - Check if result is sorted
    - Update the output bitset
    """
    tid = cuda.grid(1)
    if tid >= n_states * n_bits:
        return

    state_idx = tid // n_bits
    bit_idx = tid % n_bits

    # ... process one (state, input) pair per thread
```

For $n = 13$: each state is 8192 bits = 128 uint64s.
RTX 3090: 10,496 CUDA cores, can process ~10M states/second.

### Phase 4: Guided Search (RL integration)

Once the verifier and pruner work, replace the "for all comparators"
loop with a **learned policy**:

- **State**: current lattice bitset (or features extracted from it)
- **Action**: which comparator $(i,j)$ to try next
- **Reward**: -1 per comparator (minimize total), +large bonus if
  you find a network shorter than best known

The kore-rl infrastructure already has the REINFORCE loop. We just
need to:
1. Replace `StackState` with the sorting network lattice state
2. Replace `KORE_VOCAB` with comparator actions
3. Replace the `check` function with "all bits zero?"

This is Phase 2 — first we validate the search works with
enumeration + pruning.


## Implementation Plan

### Step 1: Core engine (Rust, in kore)

File: `experiments/sorting-networks/search.py`

Actually — we'll do this in **Python + Numba/CuPy** first for rapid
iteration, then port the hot path to CUDA if needed.

```
experiments/sorting-networks/
    LATTICE_SEARCH.md      ← this document
    verify.py              ← GPU verifier (tests 2^n inputs)
    lattice.py             ← lattice state representation + comparator application
    search.py              ← branch-and-bound with lattice pruning
    benchmark.py           ← reproduce known results, measure throughput
```

### Step 2: Validate on small n

- n=5: find 9 comparators (should take < 1 second)
- n=6: find 12 comparators (should take < 10 seconds)
- n=7: find 16 comparators (should take < 1 minute)
- n=8: find 19 comparators (might take minutes to hours)
- n=9: find 25 comparators (this is the real validation)

### Step 3: Attack n=13

Current best: 45 comparators.
Goal: find 44 or prove 45 is optimal.

Strategy:
1. Start with the known 45-comparator network
2. Search for 44-comparator networks using lattice pruning
3. Use the GPU to maintain millions of active search states
4. Apply symmetry breaking aggressively

### Step 4: Write it up

If we improve any bound, that's a publishable result.
If we match known bounds faster than SAT approaches, that's also
publishable (as a new methodology).


## Computational Budget

Hardware: RTX 3090 Ti (24GB, 10,496 CUDA cores)

| n  | States/sec (est.) | Total states | Time (est.) |
|----|-------------------|-------------|-------------|
| 5  | 10M               | ~1000       | < 1ms       |
| 7  | 10M               | ~10⁶        | 0.1s        |
| 9  | 5M                | ~10⁹        | ~3 min      |
| 13 | 1M                | ~10¹²       | ~11 days    |

For n=13, we NEED the lattice pruning and symmetry breaking to cut
the 10¹² down to something feasible. The P4 information-theoretic
bound alone typically prunes 99%+ of branches at depth > 30.

With all pruning combined (P3 commutativity + P4 lower bound +
symmetry breaking + no-effect pruning), the effective search space
for n=13 should be ~10⁸ — hours, not days.


## Success Criteria

1. **Minimum viable**: reproduce optimal for n ≤ 9
2. **Good**: match known upper bounds for n=10,11,12 faster than
   published SAT times
3. **Excellent**: improve any bound for n ≥ 13
4. **Breakthrough**: prove 45 is optimal for n=13 (lower bound proof)


## References

- Knuth, "The Art of Computer Programming, Vol. 3", Section 5.3.4
- Codish, Cruz-Filipe, et al., "Sorting 9 inputs requires 25
  comparisons" (2016)
- Ehlers & Müller, "New bounds on optimal sorting networks" (2019)
- Bundala & Závodný, "Optimal sorting networks" (2014)
- Al-Haj Baddar & Batcher, "Designing Sorting Networks" (2011)
