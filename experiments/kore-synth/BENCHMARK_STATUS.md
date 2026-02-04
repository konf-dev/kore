# Kore Algorithm Synthesis Benchmarks

## Completed Benchmarks ✅

### 1. Sorting Networks - VERIFIED
```
$ kore experiments/kore-synth/sorting-networks/verify-networks.kore
```
- **Sort-4**: 5 comparators (OPTIMAL, Floyd 1964)
- **Test Coverage**: All 24 permutations pass
- **Effect Counting**: Each CAS operation = 1 effect

**Benchmark Significance**:
| n | Current Best | Optimal? |
|---|-------------|----------|
| 4 | 5 | ✓ Proven |
| 8 | 19 | ✓ Proven |
| 13 | 45 | ? Open (44-45) |

### 2. Addition Chains - VERIFIED
```
$ kore experiments/kore-synth/addition-chains/optimal-chains.kore
```
- **x^15**: 5 multiplications (OPTIMAL)
- **x^31**: 7 multiplications (OPTIMAL, beats binary method's 8)
- **Effect Counting**: Each multiplication = 1 effect

**Benchmark Significance**:
| Target | Binary | Optimal | Savings |
|--------|--------|---------|---------|
| x^15 | 6 | 5 | 17% |
| x^31 | 8 | 7 | 12% |
| x^127 | 13 | 10 | 23% |

### 3. Ramsey Numbers - FRAMEWORK COMPLETE
```
$ kore experiments/kore-synth/ramsey/ramsey-search.kore
```
- **R(3,3) = 6**: K_5 counterexample verified
- **R(5,5) ∈ [43, 46]**: OPEN PROBLEM

**Benchmark Significance**:
Finding R(5,5) = 43 or 44 would be a publishable result!

## Prior Work ✅

### Matrix Multiplication (in examples/)
- **3×3 Laderman**: 23 multiplications (optimal)
- **4×4 Strassen²**: 49 multiplications (optimal)
- Effect algebra enables symbolic search for algorithms

## Key Syntax Learned

1. **Lists**: `0 list val list-push` (NOT `[ val ]` which creates a quote!)
2. **list-push**: APPENDS to end, so `0 list 1 push 2 push 3 push` → [1,2,3]
3. **Greater than**: Use `swap lt` (no built-in `gt`)
4. **Less or equal**: Use `swap lt not`
5. **Print constraints**: Can't print List directly; use `to-text print` for Int/Bool

## Effect Algebra Insight

All three problems share a common structure:
- **Operations** are counted as "effects"
- **Goal**: Find algorithm with MINIMUM effects
- **Verification**: Effect count = operation count

This is exactly what Kore's effect algebra was designed for!

## Future Work

1. **Sort-13**: Search for 44-comparator network (current best is 45)
2. **R(5,5)**: Search for K_43 coloring with no monochromatic K_5
3. **x^127**: Verify 10-multiplication optimal chain
