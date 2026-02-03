# Experiment E002: Verified Algorithm Synthesis

> **Status**: 🔵 Planned  
> **Started**: -  
> **Completed**: -  
> **Author**: Kore Team

## 1. Hypothesis

Kore can **automatically discover optimal algorithms** by:
1. Treating expensive operations (mul) as linear resources
2. Using Z3 to verify correctness
3. Searching program space with verification-guided pruning

**Claim**: We can rediscover Strassen's algorithm (7 muls for 2×2 matrices) and Karatsuba multiplication with formal proofs of correctness.

## 2. Background

### Prior Work

| System | What it does | Limitations |
|--------|--------------|-------------|
| AlphaTensor (DeepMind) | RL to find matrix mult algorithms | Massive compute, no proofs |
| Superoptimizers (STOKE) | Search for optimal x86 sequences | No semantic verification |
| Program synthesis (Sketch) | SAT-based synthesis | Limited to small programs |
| **Kore Synthesis** | Linear-resource bounded + Z3 verified | Novel combination |

### Key Insight

Most synthesizers search, then verify. We **verify during search**:
- Each multiplication is a linear token (can't be copied)
- Z3 prunes invalid candidates instantly
- Search space shrinks exponentially

## 3. Method

### 3.1 What We Build

```
┌────────────────────────────────────────────────────────────────┐
│              Verified Algorithm Synthesizer                    │
├────────────────────────────────────────────────────────────────┤
│  Z3 Backend (src/synthesis/z3_backend.rs)                     │
│  ├── Translate Kore ops → Z3 constraints                       │
│  ├── Symbolic execution                                        │
│  └── Equivalence checking                                      │
├────────────────────────────────────────────────────────────────┤
│  Cost Model (src/synthesis/cost.rs)                           │
│  ├── mul: 10 units (expensive)                                 │
│  ├── add/sub: 1 unit (cheap)                                   │
│  └── stack ops: 0 units (free)                                 │
├────────────────────────────────────────────────────────────────┤
│  Search Engine (src/synthesis/search.rs)                      │
│  ├── Enumerative: try programs up to length N                  │
│  ├── Pruning: reject invalid types early                       │
│  └── Linear tracking: count muls as resources                  │
├────────────────────────────────────────────────────────────────┤
│  Kore Integration (stdlib/synthesis.kore)                     │
│  └── synthesize, verify-equiv?, count-ops tools               │
└────────────────────────────────────────────────────────────────┘
```

### 3.2 Search Space Analysis

#### 2×2 Matrix Multiplication

```
Input:  A = [a b; c d], B = [e f; g h]  (8 values)
Output: C = [ae+bg af+bh; ce+dg cf+dh]  (4 values)

Standard: 8 multiplications
Strassen: 7 multiplications (1969)

Search space for 30-step programs:
- Naive: 8^30 ≈ 10^27 (impossible)
- With type checking: ~10^15 (still huge)
- With verification pruning: ~10^6 (tractable)
```

#### Karatsuba Multiplication

```
Input:  (a₁,a₀), (b₁,b₀) representing a₁×10+a₀, b₁×10+b₀
Output: (r₂,r₁,r₀) representing r₂×100+r₁×10+r₀

Standard: 4 multiplications
Karatsuba: 3 multiplications

Much smaller search space - good warm-up.
```

### 3.3 Procedure

#### Phase 1: Z3 Integration (Days 1-3)
1. Add z3 crate dependency
2. Implement Kore → Z3 constraint translation
3. Implement `verify-equiv?` tool
4. Test on simple programs

#### Phase 2: Search Engine (Days 4-6)
1. Implement enumerative search
2. Add type-checking pruning
3. Add linear resource tracking
4. Test on small problems

#### Phase 3: Karatsuba (Days 7-8)
1. Define specification in Kore
2. Run synthesis with 3-mul budget
3. Verify discovered algorithm
4. Document

#### Phase 4: Strassen (Days 9-12)
1. Define 2×2 matmul specification
2. Run synthesis with 7-mul budget
3. Verify discovered algorithm
4. Compare to known Strassen

#### Phase 5: Extensions (Days 13-14)
1. Try 3×3 (much harder, may not complete)
2. Genetic search variant
3. Write-up and analysis

### 3.4 Baselines

| Approach | 2×2 Matmul | Karatsuba | Verified? |
|----------|------------|-----------|-----------|
| Brute force (no pruning) | Hours | Minutes | ❌ |
| Type-pruned enumeration | Minutes | Seconds | ❌ |
| **Kore + Z3** | Seconds | Seconds | ✅ |

## 4. Success Criteria

| Metric | Target | How Measured |
|--------|--------|--------------|
| Karatsuba synthesis | <60 seconds | Wall clock |
| Karatsuba verified | Z3 returns UNSAT | verify-equiv? |
| Strassen synthesis | <1 hour | Wall clock |
| Strassen verified | Z3 returns UNSAT | verify-equiv? |
| Discovered == Known | Algorithms match | Manual inspection |

## 5. Implementation

### 5.1 Files to Create

```
kore/
├── Cargo.toml                      # Add z3 dependency
├── src/
│   └── synthesis/
│       ├── mod.rs                  # Module exports
│       ├── z3_backend.rs           # Z3 integration
│       ├── cost.rs                 # Operation costs
│       ├── search.rs               # Search algorithms
│       └── linear.rs               # Linear resource tracking
├── stdlib/
│   └── synthesis.kore              # Kore-level tools
└── experiments/E002-verified-synthesis/
    ├── kore/
    │   ├── karatsuba_spec.kore     # Karatsuba specification
    │   ├── karatsuba_synth.kore    # Synthesis runner
    │   ├── strassen_spec.kore      # 2×2 matmul specification
    │   └── strassen_synth.kore     # Synthesis runner
    └── results/
```

### 5.2 Kore Specification Language

```kore
; === KARATSUBA SPECIFICATION ===

: karatsuba-spec ( a1 a0 b1 b0 -- r2 r1 r0 )
  ; Ground truth: standard algorithm (4 muls)
  ; (a1*10 + a0) * (b1*10 + b0)
  ; = a1*b1*100 + (a1*b0 + a0*b1)*10 + a0*b0
  
  3 pick 1 pick mul    ; a1*b1 → r2
  2 pick 0 pick mul    ; a0*b0 → r0
  3 pick 0 pick mul    ; a1*b0
  2 pick 1 pick mul    ; a0*b1
  add                  ; a1*b0 + a0*b1 → r1
  
  ; Clean up stack, arrange outputs
  4 roll 4 roll drop drop drop drop
  rot                  ; r2 r1 r0
;

; === SYNTHESIS SETUP ===

{ ops: ["add" "sub" "mul" "dup" "swap" "rot" "over" "pick"]
  costs: { mul: 10, add: 1, sub: 1, default: 0 }
  max-muls: 3
  max-length: 25
} "synth-config" def

; === RUN SYNTHESIS ===

karatsuba-spec synth-config synthesize
; => Candidate program (if found)

dup print-program
; => : karatsuba-found ( a1 a0 b1 b0 -- r2 r1 r0 ) ... ;

karatsuba-spec verify-equiv?
; => true (Z3 proved it!)
```

### 5.3 Z3 Constraint Generation

```kore
; How verify-equiv? works internally:

: verify-equiv? ( program spec -- bool )
  ; 1. Declare symbolic inputs
  spec arity "inputs" def
  inputs [ declare-symbolic ] times
  
  ; 2. Symbolically execute both
  inputs program symbolic-eval "prog-out" def
  inputs spec symbolic-eval "spec-out" def
  
  ; 3. Assert inequality (looking for counterexample)
  prog-out spec-out zip
  [ neq ] map [ or ] fold   ; Any output differs?
  
  ; 4. Check satisfiability
  z3-check
  
  ; 5. If UNSAT, no counterexample → programs equivalent
  "unsat" eq
;
```

### 5.4 Dependencies

- [x] Kore version: 2.0
- [ ] z3 crate: SMT solver bindings
- [ ] Hardware: Any (CPU-bound)

## 6. Expected Results

### Karatsuba (Expected Output)

```kore
; Synthesized in ~30 seconds

: karatsuba-found ( a1 a0 b1 b0 -- r2 r1 r0 )
  ; z0 = a0 * b0
  1 pick 0 pick mul         ; mul 1 of 3
  
  ; z2 = a1 * b1
  3 pick 2 pick mul         ; mul 2 of 3
  
  ; z1 = (a1 + a0)(b1 + b0) - z0 - z2
  3 pick 2 pick add         ; a1 + a0
  1 pick 0 pick add         ; b1 + b0
  mul                       ; mul 3 of 3
  2 pick sub                ; - z2
  1 pick sub                ; - z0
  
  ; Arrange: r2=z2, r1=z1, r0=z0
  rot rot                   ; z2 z1 z0
;

; Verification:
karatsuba-found karatsuba-spec verify-equiv?  ; => true
karatsuba-found count-muls                    ; => 3
```

### Strassen (Expected Output)

```kore
; Synthesized in ~30 minutes

: strassen-found ( a b c d e f g h -- r s t u )
  ; M1 = (a + d)(e + h)
  7 pick 4 pick add
  3 pick 0 pick add
  mul                       ; mul 1 of 7
  
  ; M2 = (c + d) * e
  6 pick 4 pick add
  3 pick mul                ; mul 2 of 7
  
  ; M3 = a * (f - h)
  7 pick
  2 pick 0 pick sub
  mul                       ; mul 3 of 7
  
  ; M4 = d * (g - e)
  4 pick
  1 pick 3 pick sub
  mul                       ; mul 4 of 7
  
  ; M5 = (a + b) * h
  7 pick 6 pick add
  0 pick mul                ; mul 5 of 7
  
  ; M6 = (c - a)(e + f)
  6 pick 7 pick sub
  3 pick 2 pick add
  mul                       ; mul 6 of 7
  
  ; M7 = (b - d)(g + h)
  6 pick 4 pick sub
  1 pick 0 pick add
  mul                       ; mul 7 of 7
  
  ; r = M1 + M4 - M5 + M7
  ; s = M3 + M5
  ; t = M2 + M4
  ; u = M1 - M2 + M3 + M6
  ; ... (assembly)
;

; Verification:
strassen-found strassen-spec verify-equiv?  ; => true
strassen-found count-muls                   ; => 7
```

## 7. Risks & Mitigations

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Z3 too slow | Medium | Add caching, incremental solving |
| Search space too large | Medium | Better pruning heuristics |
| Strassen not found in 1hr | Low | Guided search, more time |
| Z3 binding issues | Low | Well-maintained crate |

## 8. Changelog

| Date | Change |
|------|--------|
| 2026-02-03 | Created experiment specification |
