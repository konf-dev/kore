# Kore Experiment Catalog

> Comprehensive list of experiments to demonstrate Kore's capabilities
> Hardware: RTX 3090 Ti (24GB) + RTX 2070 Super (8GB)

## Gemini's Claims: Reality Check

| Claim | Validity | Notes |
|-------|----------|-------|
| "Constructive search, not theorem proving" | ✅ **Valid** | Correct framing |
| "Soft adjacency matrices for graphs" | ✅ **Valid** | Standard technique |
| "Circuit minimization = Strassen" | ✅ **Valid** | Same problem class |
| "Ramsey R(5,5) improvable" | ⚠️ **Overhyped** | Search space is 2^990, not tractable |
| "Busy Beaver attack" | ⚠️ **Overhyped** | Active research by serious teams |
| "Knot theory via gradients" | 🟡 **Interesting** | Niche, but possible |
| "Counter-example search" | ✅ **Valid** | Good application |

### What Kore CAN Do
- Find **finite mathematical objects** (algorithms, matrices, small graphs)
- **Verify** found objects with Z3
- Run **safe agent code** with capability bounds
- **Optimize** discrete structures via relaxation

### What Kore CANNOT Do
- Prove infinite theorems (Riemann, P=NP)
- Search astronomically large spaces (R(5,5) has 2^990 graphs)
- Replace dedicated solvers for well-studied problems

---

## Master Experiment List

### Tier 1: Foundation (Prove the System Works)

| ID | Name | Category | Difficulty | Hardware | Timeline | Impressiveness |
|----|------|----------|------------|----------|----------|----------------|
| E001 | Agent Runtime | Infrastructure | ⭐⭐ | CPU | 1 week | Medium |
| E002 | Verified Synthesis | Synthesis | ⭐⭐⭐ | CPU | 2 weeks | High |
| E003 | Differentiable Search | Learning | ⭐⭐⭐⭐ | 3090 Ti | 3 weeks | High |

### Tier 2: Demonstrations (Show Practical Value)

| ID | Name | Category | Difficulty | Hardware | Timeline | Impressiveness |
|----|------|----------|------------|----------|----------|----------------|
| E004 | LLM Agent Executor | Hybrid | ⭐⭐ | CPU + API | 1 week | High |
| E005 | Safe Code Sandbox | Security | ⭐⭐ | CPU | 1 week | Medium |
| E006 | Pipeline DSL | Practical | ⭐⭐ | CPU | 1 week | Medium |

### Tier 3: Novel Research (Prove Kore is Special)

| ID | Name | Category | Difficulty | Hardware | Timeline | Impressiveness |
|----|------|----------|------------|----------|----------|----------------|
| E007 | End-to-End LLM+Kore | Hybrid | ⭐⭐⭐⭐ | 3090 Ti | 3 weeks | Very High |
| E008 | Circuit Minimization | Discovery | ⭐⭐⭐ | 3090 Ti | 2 weeks | High |
| E009 | Sorting Network Synthesis | Discovery | ⭐⭐⭐ | 3090 Ti | 2 weeks | High |

### Tier 4: Ambitious (High Risk, High Reward)

| ID | Name | Category | Difficulty | Hardware | Timeline | Impressiveness |
|----|------|----------|------------|----------|----------|----------------|
| E010 | Small Ramsey Bounds | Discovery | ⭐⭐⭐⭐⭐ | Both GPUs | 4 weeks | Very High |
| E011 | Integer Sequence Discovery | Discovery | ⭐⭐⭐⭐ | 3090 Ti | 3 weeks | High |
| E012 | Knot Simplification | Discovery | ⭐⭐⭐⭐ | 3090 Ti | 3 weeks | Medium |

### Tier 5: Moonshots (Probably Won't Work, But Cool If It Does)

| ID | Name | Category | Difficulty | Hardware | Timeline | Impressiveness |
|----|------|----------|------------|----------|----------|----------------|
| E013 | Busy Beaver Candidates | Discovery | ⭐⭐⭐⭐⭐ | Both GPUs | 6 weeks | Extreme |
| E014 | Novel Matrix Mult (3×3) | Discovery | ⭐⭐⭐⭐⭐ | Both GPUs | 8 weeks | Extreme |
| E015 | Collatz Counter-Example | Discovery | ⭐⭐⭐⭐⭐ | Both GPUs | ∞ | Extreme |

---

## Detailed Experiment Specifications

### E001-E003: Already Specified
See existing experiment directories.

---

### E004: LLM Agent Executor

**Goal**: LLM generates Kore code → Kore analyzes → Kore executes if safe

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│    LLM      │────▶│   Kore      │────▶│   Kore      │
│  (Claude)   │     │  Analyzer   │     │  Executor   │
│             │     │  (is safe?) │     │  (run it)   │
└─────────────┘     └─────────────┘     └─────────────┘
       │                   │                   │
       │                   ▼                   ▼
       │            "needs: [fs]"        Result or
       │            "pure: false"        Error
       │                   │
       └───────────────────┘
         Feedback loop: "Code rejected, try again"
```

**Why it matters**: Safe AI agent execution with formal guarantees

**Kore code**:
```kore
; LLM wants to: "Count files in current directory"
; LLM generates:
"." fs-list list-len

; Kore analyzes:
[ "." fs-list list-len ] effect-infer
; => { io: ["fs"], pure: false }

; Check against agent's capabilities:
agent-caps "fs" list-has
[ execute ] [ "Permission denied: needs fs" fail ] if
```

---

### E005: Safe Code Sandbox

**Goal**: Run untrusted user code with hard resource limits

```kore
; User submits code (potentially malicious)
user-code "submitted" def

; Create sandboxed context
{
  caps: []              ; No capabilities
  max-ops: 10000        ; Max operations
  max-stack: 100        ; Max stack depth
  timeout-ms: 1000      ; 1 second timeout
} sandbox-config!

; Analyze first
submitted effect-infer "analysis" def
analysis "pure" map-get not [
  "Rejected: code has IO effects" fail
] when

; Execute in sandbox
submitted sandbox-execute
; => Result or timeout/quota error
```

---

### E006: Pipeline DSL

**Goal**: Declarative data pipelines in Kore

```kore
; Define ETL pipeline
: etl-pipeline ( -- )
  "input.csv" fs-read
  csv-parse
  [ "status" map-get "active" eq ] filter
  [ "value" map-get ] map
  [ 0 gt ] filter
  0 [ add ] fold
  "Total active value: " swap str-concat println
;

; Analyze pipeline effects
[ etl-pipeline ] effect-infer
; => { io: ["fs", "io"], pure: false }

; Generate execution plan
[ etl-pipeline ] optimize
; => Fused, optimized version
```

---

### E007: End-to-End LLM+Kore Training ⭐ KEY EXPERIMENT

**Goal**: Train LLM to write Kore code via gradient descent through the runtime

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  LLM Weights│────▶│ Soft Kore   │────▶│    Loss     │
│     W       │     │  Runtime    │     │  Function   │
└─────────────┘     └─────────────┘     └─────────────┘
       ▲                                       │
       │                                       │
       └──────────── ∇W ◀──────────────────────┘
                  Backprop!
```

**Architecture**:
1. Small LLM (GPT-2 scale, ~100M params) generates token sequence
2. Tokens map to Kore operations
3. Soft Kore runtime executes differentiably
4. Loss = task performance (e.g., compute factorial correctly)
5. Gradients flow back to LLM weights

**Why this is novel**:
- Current LLM+code systems: generate → execute → reward (no gradient through execution)
- Our system: generate → soft execute → gradient → update

**Hardware**: 3090 Ti (LLM) + 2070 Super (Kore runtime batching)

**Kore code**:
```kore
; === LLM-KORE TRAINING ===

; LLM generates soft token distribution
llm-forward "token-probs" def   ; [seq_len, vocab_size]

; Convert to soft Kore operations
token-probs vocab-to-ops        ; [seq_len, num_ops]

; Execute soft program
"soft-program" def
inputs soft-program soft-execute "outputs" def

; Compute loss
outputs targets mse-loss
backward

; Update LLM
llm-weights llm-grad 0.001 mul sub "llm-weights" def
```

---

### E008: Circuit Minimization

**Goal**: Find smallest Boolean circuit for a function

**Example**: XOR of 8 bits using minimal gates

```kore
; Specification: XOR of 8 bits
: xor8-spec ( b0 b1 b2 b3 b4 b5 b6 b7 -- result )
  xor xor xor xor xor xor xor
;

; Available gates (with costs)
{ and: 1, or: 1, xor: 1, not: 1, nand: 1 } "gates" def

; Synthesize minimal circuit
xor8-spec gates 20 synthesize-circuit

; Expected: 7 XOR gates (optimal)
```

---

### E009: Sorting Network Synthesis

**Goal**: Find optimal sorting network for N elements

**Background**: Sorting networks are fixed comparison sequences. Optimal networks are known for small N but unknown for larger N.

| N | Known Optimal Comparisons |
|---|---------------------------|
| 4 | 5 |
| 5 | 9 |
| 6 | 12 |
| 7 | 16 |
| 8 | 19 |
| 9 | 25 |
| 10 | 29 |
| 11 | ? (31-33) |
| 12 | ? (35-39) |

**Kore approach**:
```kore
; A sorting network is a sequence of compare-swap operations
; compare-swap(i,j): if a[i] > a[j], swap them

; Soft sorting network
: soft-compare-swap ( array i j -- array' )
  ; Differentiable min/max
  over i tensor-get
  over j tensor-get
  soft-min soft-max
  ; ... update array
;

; Train to minimize comparisons while sorting correctly
: train-sorting-network ( n -- network )
  n random-permutations 10000 batch   ; training data
  n max-comparisons soft-network-init
  
  1000 [
    forward
    ; Loss = sorted? + λ * num_active_comparisons
    sort-loss comparison-penalty add
    backward
    sgd-step
  ] times
  
  extract-network
;

; Target: Find 11-element network with ≤32 comparisons
11 train-sorting-network
```

---

### E010: Small Ramsey Bounds

**Goal**: Improve bounds on small Ramsey numbers

**Reality check**: R(5,5) is NOT tractable (Gemini was wrong). But smaller cases are:

| Ramsey Number | Known Value | Tractable? |
|---------------|-------------|------------|
| R(3,3) | 6 | ✅ Trivial |
| R(3,4) | 9 | ✅ Easy |
| R(3,5) | 14 | ✅ Medium |
| R(4,4) | 18 | ✅ Medium |
| R(4,5) | 25 | 🟡 Hard |
| R(5,5) | 43-48 | ❌ Intractable |

**Kore approach**: Verify known values, maybe find cleaner proofs

```kore
; Soft adjacency matrix for graph on n vertices
: soft-graph ( n -- adj-matrix )
  n n tensor-zeros
  ; Initialize with learnable edge probabilities
  n n [ random 0.5 sub ] tensor-map
  sigmoid   ; Soft edges in [0,1]
;

; Loss: penalize cliques and independent sets
: ramsey-loss ( adj k l -- loss )
  ; Find soft-clique of size k
  adj k soft-clique-indicator "clique-penalty" def
  
  ; Find soft-independent-set of size l
  adj 1 sub neg l soft-clique-indicator "indep-penalty" def
  
  clique-penalty indep-penalty add
;

; Search for R(4,4) witness (graph on 17 vertices with no K4 or I4)
17 soft-graph
[ 4 4 ramsey-loss backward sgd-step ] 10000 times
extract-graph verify-ramsey
```

---

### E011: Integer Sequence Discovery

**Goal**: Find formulas for integer sequences (like OEIS)

**Example**: Given sequence [1, 1, 2, 3, 5, 8, 13, ...], find the recurrence

```kore
; Input: first 20 terms of unknown sequence
[ 1 1 2 3 5 8 13 21 34 55 89 144 233 377 610 987 1597 2584 4181 6765 ] "seq" def

; Search space: linear recurrences of depth ≤ 3
; a[n] = c1*a[n-1] + c2*a[n-2] + c3*a[n-3]

; Soft coefficients
3 soft-coefficients "c" def

: predict ( seq idx -- val )
  idx 1 sub seq list-get c 0 tensor-get mul
  idx 2 sub seq list-get c 1 tensor-get mul add
  idx 3 sub seq list-get c 2 tensor-get mul add
;

: loss ( -- l )
  seq list-len 3 sub [
    dup 3 add "idx" def
    seq idx predict
    seq idx list-get
    sub abs
  ] map 0 [ add ] fold
;

1000 [ loss backward sgd-step ] times
c extract-coefficients
; => [1, 1, 0] meaning a[n] = a[n-1] + a[n-2] (Fibonacci!)
```

---

### E012: Knot Simplification

**Goal**: Find shortest sequence of Reidemeister moves to unknot a diagram

**Background**: Reidemeister moves are local transformations of knot diagrams. Three types:
- R1: Add/remove a twist
- R2: Add/remove two crossings
- R3: Slide a strand over a crossing

```kore
; Knot represented as Gauss code (sequence of crossings)
; Example: Trefoil = [1 -2 3 -1 2 -3]

; Reidemeister moves as Kore operations
: r1-add ( knot pos -- knot' ) ... ;
: r1-remove ( knot pos -- knot' ) ... ;
: r2-add ( knot pos -- knot' ) ... ;
: r2-remove ( knot pos -- knot' ) ... ;
: r3 ( knot pos -- knot' ) ... ;

; Soft move selection
: soft-move ( knot move-probs -- knot' )
  ; Weighted sum of all possible moves
  ...
;

; Train to minimize crossing number
: simplify-knot ( knot -- moves )
  knot
  100 [  ; Max 100 moves
    soft-move-select soft-move
    dup crossing-count 0 eq [ break ] when
  ] times
  move-history
;
```

---

### E013: Busy Beaver Candidates ⭐ MOONSHOT

**Goal**: Find long-running 5-state Turing machines

**Reality**: This is HARD. The BB(5) challenge has been running for years. But we might find interesting candidates.

**Current records**:
- BB(4) = 107 (proven)
- BB(5) ≥ 47,176,870 (best known machine)
- BB(5) could be much larger

**Kore approach**:
```kore
; Turing machine state: (tape, head-position, state, steps)
; Transition table: state × symbol → (new-symbol, direction, new-state)

; Soft transition table (learnable)
5 2 soft-transition-table "tm" def

: run-tm ( tm max-steps -- steps halted? )
  empty-tape 0 0 0   ; tape head state steps
  max-steps [
    ; ... execute one step
    ; Check for halt
  ] times
;

; Objective: Maximize steps while halting
: bb-loss ( -- loss )
  tm 10000000 run-tm
  dup [ neg ] [ drop -1000000 ] if  ; Penalize non-halting
;

; Search for champion machines
1000000 [
  bb-loss backward sgd-step
  dup 0 mod 10000 eq [ "Steps: " swap println ] when
] times
```

---

### E014: Novel Matrix Multiplication (3×3)

**Goal**: Find new algorithms for 3×3 matrix multiplication

**Background**:
- Standard: 27 multiplications
- Best known: 23 multiplications (various researchers)
- Theoretical minimum: Unknown

This is where AlphaTensor made progress. If Kore can match or beat 23, that's publication-worthy.

---

### E015: Collatz Counter-Example Search

**Goal**: Search for a Collatz counter-example (loop or divergence)

**Reality**: Almost certainly won't find one. But the search itself demonstrates Kore's capabilities.

```kore
; Collatz function
: collatz-step ( n -- n' )
  dup 2 mod 0 eq
  [ 2 div ]           ; Even: n/2
  [ 3 mul 1 add ]     ; Odd: 3n+1
  if
;

; Check if n eventually reaches 1
: collatz-terminates? ( n max-steps -- bool )
  swap
  max-steps [
    dup 1 eq [ true return ] when
    collatz-step
  ] times
  false
;

; Soft search: find numbers that take long to terminate
; (Counter-example would be a number that never terminates)
: collatz-search ( -- )
  ; Start from random large numbers
  ; Use gradients to find numbers with long trajectories
  ; Check promising candidates exhaustively
;
```

---

## Recommended Experiment Order

### Phase 1: Foundation (Weeks 1-3)
1. **E001**: Agent Runtime — prove the system works
2. **E004**: LLM Agent Executor — practical integration

### Phase 2: Core Demonstrations (Weeks 4-7)
3. **E002**: Verified Synthesis — Strassen/Karatsuba with proofs
4. **E003**: Differentiable Search — gradient-based discovery

### Phase 3: Novel Research (Weeks 8-12)
5. **E007**: End-to-End LLM+Kore — train LLM through runtime ⭐
6. **E009**: Sorting Network Synthesis — concrete discovery task
7. **E008**: Circuit Minimization — related to Strassen

### Phase 4: Ambitious Goals (Weeks 13-20)
8. **E011**: Integer Sequence Discovery — accessible math
9. **E010**: Small Ramsey Bounds — graph theory showcase
10. **E014**: 3×3 Matrix Mult — compete with AlphaTensor

### Phase 5: Moonshots (If Time Permits)
11. **E013**: Busy Beaver — probably won't crack it, but interesting
12. **E012**: Knot Simplification — niche but publishable

---

## Hardware Allocation

| GPU | VRAM | Best For |
|-----|------|----------|
| RTX 3090 Ti | 24GB | Large batches, LLM training, main experiments |
| RTX 2070 Super | 8GB | Auxiliary batching, verification, parallel search |

**Multi-GPU Strategy**:
```
3090 Ti: Soft program execution (high memory)
2070 Super: Z3 verification queue (CPU-bound but can offload)

OR

3090 Ti: LLM forward/backward
2070 Super: Kore soft runtime batching
```

---

## Success Metrics by Tier

| Tier | Success = |
|------|-----------|
| Foundation | E001 works, <5ms analysis latency |
| Demonstration | Rediscover Strassen + Karatsuba |
| Novel | End-to-end LLM+Kore trains successfully |
| Ambitious | Beat/match AlphaTensor on 3×3, improve Ramsey bounds |
| Moonshot | Find new BB(5) candidate, novel algorithm |

---

## Publication Targets

| Paper | Experiments | Venue |
|-------|-------------|-------|
| "Safe Execution for LLM Agents" | E001, E004, E005 | AAAI / Industry |
| "Verified Algorithm Synthesis" | E002, E008, E009 | PLDI / POPL |
| "Differentiable Stack Machines" | E003, E007 | NeurIPS / ICML |
| "Gradient-Based Algorithm Discovery" | E003, E010, E014 | Nature Machine Intelligence |

