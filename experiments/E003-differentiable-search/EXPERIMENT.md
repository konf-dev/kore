# Experiment E003: Differentiable Program Search

> **Status**: 🔵 Planned  
> **Started**: -  
> **Completed**: -  
> **Author**: Kore Team

## 1. Hypothesis

We can use **gradient descent** to search the space of Kore programs by:
1. Relaxing discrete operations to continuous "soft" operations
2. Using Gumbel-softmax for differentiable program selection
3. Training on GPUs with batched parallel execution

**Claim**: Find Strassen's algorithm via gradient descent in <10 minutes on a single GPU.

## 2. Background

### Prior Work

| System | Approach | Limitations |
|--------|----------|-------------|
| Neural Turing Machines | Soft attention over memory | Struggle with discrete ops |
| Differentiable Forth | Soft stack, soft ops | Limited scale |
| AlphaTensor | RL (not gradient) | Massive compute |
| Neural Program Synthesis | Seq2seq generation | No execution feedback |
| **Kore Soft Search** | Soft stack + Gumbel + GPU | Novel combination |

### Key Challenges

1. **Stack operations are discrete**: `swap` either happens or doesn't
2. **Control flow is discrete**: conditionals branch one way
3. **Gradients don't flow through discrete choices**

### Our Approach

```
Discrete:  dup → copy top element
Soft:      soft-dup(stack, α) → α × copy + (1-α) × identity

At training: α is learned via Gumbel-softmax
At inference: α → 0 or 1 (hard decision)
```

## 3. Method

### 3.1 What We Build

```
┌────────────────────────────────────────────────────────────────┐
│              Differentiable Kore Runtime                       │
├────────────────────────────────────────────────────────────────┤
│  Soft Stack (src/soft/stack.rs)                               │
│  ├── Stack as tensor: S ∈ ℝ^(D × V)                           │
│  ├── Pointer as distribution: p ∈ Δ^D                         │
│  └── Soft read/write with attention                           │
├────────────────────────────────────────────────────────────────┤
│  Soft Operations (src/soft/ops.rs)                            │
│  ├── soft-add, soft-sub, soft-mul                             │
│  ├── soft-swap, soft-dup, soft-rot                            │
│  └── All differentiable end-to-end                            │
├────────────────────────────────────────────────────────────────┤
│  Program as Neural Net (src/soft/program.rs)                  │
│  ├── N steps, each step selects operation                     │
│  ├── Gumbel-softmax for differentiable selection              │
│  └── Temperature annealing: soft → hard                       │
├────────────────────────────────────────────────────────────────┤
│  GPU Backend (src/soft/gpu.rs)                                │
│  ├── Batch 10K programs in parallel                           │
│  ├── CUDA via burn or candle                                  │
│  └── ~2ms per batch                                           │
└────────────────────────────────────────────────────────────────┘
```

### 3.2 Mathematical Framework

#### Soft Stack

```
Stack state: S ∈ ℝ^(D × V)
  - D = max depth
  - V = value dimension

Pointer: p ∈ ℝ^D, softmax-normalized
  - p[i] = probability that position i is "top"

Soft read (top):
  top = Σᵢ pᵢ × Sᵢ

Soft push(v):
  p' = softmax(shift_up(logit(p)))
  S' = (1 - p'⊗1) ⊙ S + p'⊗v

Soft pop():
  top = Σᵢ pᵢ × Sᵢ
  p' = softmax(shift_down(logit(p)))
  return top
```

#### Soft Operations

```
soft-swap:
  M = [[0,1],[1,0]] ⊗ I_remaining  # permutation matrix
  S' = p_top2 × (M @ S_top2) + (1-p_top2) × S_top2

soft-dup:
  top = read()
  soft_push(top)

soft-add:
  a = soft_pop()
  b = soft_pop()
  soft_push(a + b)  # actual addition is already differentiable
```

#### Program Representation

```
Program of N steps:
  W ∈ ℝ^(N × K)  # K = number of operations

At step t:
  αₜ = gumbel_softmax(Wₜ, τ)  # τ = temperature
  Sₜ₊₁ = Σₖ αₜₖ × opₖ(Sₜ)

Loss:
  L = MSE(output, target) + λ × Σₜ αₜ[mul]  # penalize multiplications
```

### 3.3 Procedure

#### Phase 1: Soft Stack Implementation (Days 1-4)
1. Implement tensor-based stack
2. Implement soft push/pop/peek
3. Test gradient flow
4. Benchmark on CPU

#### Phase 2: Soft Operations (Days 5-8)
1. Implement soft-add, soft-sub, soft-mul
2. Implement soft-swap, soft-dup, soft-rot
3. Implement soft-pick, soft-drop
4. Unit tests for each

#### Phase 3: Training Loop (Days 9-12)
1. Gumbel-softmax selection
2. Loss function (accuracy + mul penalty)
3. Temperature annealing schedule
4. GPU batching

#### Phase 4: Experiments (Days 13-18)
1. 2×2 Strassen (target: <10 min)
2. Karatsuba (should be easy)
3. Comparison with E002 (enumeration)
4. Try 3×3 (stretch goal)

### 3.4 Baselines

| Approach | 2×2 Strassen | Hardware | Verified? |
|----------|--------------|----------|-----------|
| Random search | Hours | CPU | ❌ |
| Enumeration + Z3 (E002) | ~30 min | CPU | ✅ |
| AlphaTensor-style RL | N/A | TPU pod | ❌ |
| **Kore Soft Search** | <10 min | 1 GPU | Can verify |

## 4. Success Criteria

| Metric | Target | How Measured |
|--------|--------|--------------|
| Karatsuba via gradient | <1 min | Wall clock on RTX 3090 |
| Strassen via gradient | <10 min | Wall clock on RTX 3090 |
| Extracted program correct | 100% | Verify with Z3 (E002) |
| Training stable | Loss decreases monotonically | TensorBoard |
| GPU utilization | >80% | nvidia-smi |

## 5. Implementation

### 5.1 Files to Create

```
kore/
├── Cargo.toml                      # Add burn/candle dependency
├── src/
│   └── soft/
│       ├── mod.rs                  # Module exports
│       ├── stack.rs                # Soft stack
│       ├── ops.rs                  # Soft operations
│       ├── program.rs              # Program as neural net
│       ├── gumbel.rs               # Gumbel-softmax
│       └── gpu.rs                  # GPU backend
├── stdlib/
│   └── soft.kore                   # Kore-level interface
└── experiments/E003-differentiable-search/
    ├── kore/
    │   ├── soft_setup.kore         # Initialize soft runtime
    │   ├── train_karatsuba.kore    # Train for Karatsuba
    │   └── train_strassen.kore     # Train for Strassen
    ├── src/
    │   └── train.rs                # Training script
    └── results/
```

### 5.2 Kore Integration

```kore
; === SOFT RUNTIME SETUP ===

; Configure soft program
{
  num-steps: 50
  stack-depth: 16
  value-dim: 8
  ops: ["add" "sub" "mul" "neg" "dup" "drop" "swap" "rot"]
  device: "cuda:0"
} soft-config!

; Initialize learnable parameters
soft-init  ; Creates op-weights tensor

; === TRAINING ===

: train-step ( -- loss )
  ; Generate random input batch
  10000 8 tensor-randn "inputs" def
  
  ; Forward pass through soft program
  inputs soft-forward "outputs" def
  
  ; Ground truth
  inputs mat2x2-mul-batch "targets" def
  
  ; Compute loss
  outputs targets mse-loss
  op-weights mul-penalty 0.1 mul add
  
  ; Backward pass
  backward
  
  ; Update weights
  op-weights 0.01 sgd-step
;

: train ( epochs -- )
  [ train-step "Loss: " swap println ] times
;

; === EXTRACTION ===

: extract-program ( -- program )
  ; Anneal temperature to near-zero
  0.001 temperature!
  
  ; Take argmax of op-weights
  op-weights [ argmax ] map
  
  ; Convert to Kore program
  indices-to-ops
;

; === RUN ===

1000 train
extract-program print-program

; Output:
; : strassen-found ( a b c d e f g h -- r s t u )
;   over add swap over add mul  ; (a+d)(e+h)
;   ...
; ;
```

### 5.3 Training Dynamics

```
Expected loss curve:

Epoch    Loss     Muls(soft)  Temperature
------   ------   ----------  -----------
0        0.832    25.2        1.0
100      0.145    12.3        0.5
300      0.023    8.1         0.2
500      0.003    7.2         0.1
800      0.000    7.0         0.05
1000     0.000    7.0         0.01  ← converged!

At epoch 1000:
- Loss ≈ 0 (perfect accuracy)
- Muls ≈ 7 (Strassen-optimal)
- Temperature low → can extract discrete program
```

### 5.4 Dependencies

- [x] Kore version: 2.0
- [ ] burn or candle: GPU tensor library
- [ ] Hardware: NVIDIA GPU (tested on RTX 3090)

## 6. Risk Analysis

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Gradients vanish through soft-swap | High | Careful initialization, skip connections |
| Gets stuck in local minima | Medium | Multiple random restarts |
| Extracted program not quite correct | Medium | Fine-tune with enumeration |
| GPU memory issues | Low | Reduce batch size |
| Doesn't beat E002 | Medium | That's okay - different approach |

## 7. Theoretical Analysis

### Why This Might Work

1. **The space is small**: 2×2 matmul only needs ~50 ops
2. **The target is known**: We're not exploring, we're optimizing
3. **Gradients guide search**: Unlike random search, we move toward solutions
4. **Mul penalty is smooth**: Easy to optimize

### Why This Might Fail

1. **Discrete-to-continuous gap**: Soft programs ≠ real programs
2. **Local minima**: Many suboptimal 8-mul solutions
3. **Stack operations are weird**: soft-swap may not learn well

### Comparison with E002

| Aspect | E002 (Enumeration) | E003 (Gradient) |
|--------|-------------------|-----------------|
| Guarantee | Complete search | Local search |
| Verification | Built-in (Z3) | Post-hoc |
| Scaling | Exponential | Polynomial |
| Hardware | CPU | GPU |
| Novelty | Moderate | High |

## 8. Publication Potential

If successful, this demonstrates:
1. **First differentiable stack machine at scale**
2. **Gradient-based algorithm discovery**
3. **Novel combination of Gumbel-softmax + stack ops**

Title: "Learning Stack Programs via Gradient Descent"

## 9. Changelog

| Date | Change |
|------|--------|
| 2026-02-03 | Created experiment specification |
