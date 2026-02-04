# Kore Common Upgrades for Benchmark Experiments

## Current State Audit

### Tensor Operations (✅ = has autodiff, ⬜ = no autodiff)
```
✅ tensor-add         ✅ tensor-sub         ✅ tensor-mul
✅ tensor-scale       ✅ tensor-scale2      ✅ tensor-neg
✅ tensor-exp         ✅ tensor-log         ✅ tensor-relu
✅ tensor-sigmoid     ✅ tensor-softmax     ✅ tensor-matmul
✅ tensor-sum         ⬜ tensor-mean        ⬜ tensor-max
⬜ tensor-dot         ⬜ tensor-outer       ⬜ tensor-clip
⬜ tensor-get         ⬜ tensor-set         ⬜ tensor-copy
⬜ tensor-zeros       ⬜ tensor-ones        ⬜ tensor-rand
⬜ tensor-randn       ⬜ tensor-argmax      ⬜ tensor-shape
```

### Missing Operations (needed for experiments)
```
❌ tensor-pow         (x^n)       - Symbolic regression needs this!
❌ tensor-sqrt        (√x)        - Distance calculations
❌ tensor-div         (x/y)       - Division with gradients
❌ tensor-sin         (sin(x))    - Feynman equations
❌ tensor-cos         (cos(x))    - Feynman equations
❌ tensor-tanh        (tanh(x))   - Neural networks
❌ tensor-abs         (|x|)       - Absolute value
❌ tensor-reshape     (shape->)   - Shape manipulation
❌ tensor-slice       ([i:j])     - Slicing with gradients
❌ tensor-concat      (join)      - Concatenation
❌ tensor-broadcast   (expand)    - Broadcasting
```

---

## Experiment Requirements Matrix

| Operation | Symbolic Reg | Safe RL | ARC | SyGuS |
|-----------|--------------|---------|-----|-------|
| pow       | ✓✓✓          | ✓       |     |       |
| sqrt      | ✓✓           | ✓       |     |       |
| div       | ✓✓✓          | ✓       |     | ✓     |
| sin/cos   | ✓✓✓          |         |     |       |
| tanh      | ✓            | ✓       |     |       |
| abs       | ✓            | ✓       | ✓   | ✓     |
| reshape   |              | ✓       | ✓✓  |       |
| slice     |              | ✓       | ✓✓✓ |       |
| broadcast | ✓✓           | ✓✓      |     |       |

**Priority Legend**: ✓✓✓ = critical, ✓✓ = important, ✓ = nice-to-have

---

## Phase 1: Core Math Operations (Unlocks Symbolic Regression)

These operations need full autodiff support:

### 1.1 `tensor-pow` (x^n where n can be tensor or float)
```
Forward:  y = x^n
Backward: dx = n * x^(n-1) * upstream
          dn = x^n * ln(x) * upstream  (if n is learnable)
```

### 1.2 `tensor-div` (x / y element-wise)
```
Forward:  z = x / y
Backward: dx = upstream / y
          dy = -upstream * x / y^2
```

### 1.3 `tensor-sqrt` (special case of pow)
```
Forward:  y = sqrt(x)
Backward: dx = 0.5 / sqrt(x) * upstream
```

### 1.4 `tensor-sin` and `tensor-cos`
```
Forward:  y = sin(x)
Backward: dx = cos(x) * upstream

Forward:  y = cos(x)
Backward: dx = -sin(x) * upstream
```

### 1.5 `tensor-tanh`
```
Forward:  y = tanh(x)
Backward: dx = (1 - y^2) * upstream
```

### 1.6 `tensor-abs`
```
Forward:  y = |x|
Backward: dx = sign(x) * upstream  (undefined at 0, use 0)
```

---

## Phase 2: Shape Operations (Unlocks ARC & RL)

### 2.1 `tensor-reshape` (keep autodiff tape)
```kore
[1 2 3 4 5 6] tensor-from-list [2 3] tensor-reshape
# Result: 2x3 tensor
```

### 2.2 `tensor-slice` (with gradient support)
```kore
tensor [start end] tensor-slice  # Returns subtensor
# Gradient: place upstream in correct position, zeros elsewhere
```

### 2.3 `tensor-concat` 
```kore
tensor1 tensor2 axis tensor-concat
# Gradient: split upstream back to original shapes
```

### 2.4 Broadcasting Rules
```
Shapes must be compatible:
- Same dimensions, or
- One dimension is 1 (broadcasts)
- Missing dimensions treated as 1

Example: [3] + [3 3] → broadcast [3] to [3 3]
```

---

## Phase 3: DSL & Execution (Unlocks SyGuS & ARC)

### 3.1 Program Enumeration
```kore
# Define grammar as data structure
[
  ["expr" ["num" "var" ["+" "expr" "expr"] ["*" "expr" "expr"]]]
] "grammar" def

# Enumerate programs up to depth
grammar 3 enumerate-programs  # → list of program ASTs
```

### 3.2 Program Execution (Interpreter)
```kore
# Execute AST with bindings
program { "x" 5 } eval-program  # → result
```

### 3.3 Grid Operations (for ARC)
```kore
grid [row col] grid-get    # Get cell
grid value [row col] grid-set  # Set cell
grid grid-rotate  # Rotate 90°
grid grid-flip    # Flip horizontal
grid color grid-fill  # Flood fill
```

---

## Implementation Order

### Week 1: Core Math (Symbolic Regression Foundation)
1. `tensor-pow` with autodiff ← **Most critical**
2. `tensor-div` with autodiff
3. `tensor-sqrt` (can derive from pow)
4. `tensor-sin`, `tensor-cos` with autodiff
5. `tensor-tanh` with autodiff
6. `tensor-abs` with autodiff

### Week 2: Shape Operations
7. Broadcasting in `tensor-add`, `tensor-mul`
8. `tensor-reshape` with grad tracking
9. `tensor-slice` with grad tracking
10. `tensor-concat`

### Week 3+: DSL Infrastructure
11. Program AST representation
12. Program enumeration
13. Symbolic execution
14. Grid DSL for ARC

---

## Postulate Compliance Checklist

All upgrades must follow:

**P1 (Immutability)**: ✓
- Tensor ops return NEW tensors
- Slicing doesn't mutate original
- Reshape creates new view

**P2 (Composition)**: ✓
- Each op is a single tool
- Complex functions = tool composition
- No special syntax needed

**P3 (Shape Verification)**: ✓
- Can statically verify shapes
- Broadcasting rules are deterministic
- Slice bounds are checkable

---

## Quick Win: Symbolic Regression Starter

With just `pow`, `div`, `sin`, `cos` added, we can tackle:

**Feynman-I.6.2a**: `f = exp(-θ²/2) / sqrt(2π)`
```kore
# With new ops:
theta theta tensor-mul -2.0 tensor-scale tensor-pow  # -θ²/2
tensor-exp                                            # exp(-θ²/2)
2 3.14159 tensor-mul tensor-sqrt tensor-div          # / sqrt(2π)
```

**Feynman-I.12.1**: `F = μ * N_n * m / r²`
```kore
mu Nn tensor-mul m tensor-mul  # μ * N_n * m
r r tensor-mul tensor-div      # / r²
```

---

## Estimated Timeline

| Phase | Deliverable | Time | Experiment Unlocked |
|-------|-------------|------|---------------------|
| 1a    | pow, div, sqrt | 2 days | Basic SR |
| 1b    | sin, cos, tanh, abs | 2 days | Feynman SR |
| 2a    | Broadcasting | 2 days | Cleaner ML code |
| 2b    | reshape, slice | 3 days | ARC prep |
| 3     | Program DSL | 1 week | SyGuS, ARC |

**Total before first benchmark attempt: ~2 weeks**

---

## Next Step

Start with Phase 1a: Implement `tensor-pow` with full autodiff support.

This single operation unlocks:
- Polynomial regression
- Power-law discovery
- Half the Feynman equations
