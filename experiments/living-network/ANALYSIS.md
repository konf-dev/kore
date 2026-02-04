# Living Network Analysis

Gemini's proposal for an "Autopoietic Transformer" in Kore.

---

## Core Insight (Excellent)

The fundamental idea is **differentiable architecture search via soft gating**:

```
output = α * block(x) + (1-α) * x
```

Where `α` is a learnable scalar per layer. When `α → 0`, the layer becomes a skip connection.
When training includes an L1 penalty on α values, the network learns to prune itself.

**This is mathematically sound and implementable today.**

---

## What We Have (Working)

| Feature | Status | Tool |
|---------|--------|------|
| Tensor creation | ✓ | `tensor-ones`, `tensor-rand`, `tensor-randn` |
| Tensor ops | ✓ | `tensor-add`, `tensor-mul`, `tensor-matmul` |
| Autodiff | ✓ | `requires-grad`, `backward`, `grad-get` |
| Memory | ✓ | `mem-set`, `mem-get` |
| Lists | ✓ | `list`, `unlist`, `list-get`, `list-set` |
| Maps | ✓ | `map-new`, `map-set`, `map-get` |
| Loops | ✓ | `times`, `while`, `each` |
| Filtering | ✓ | `filter` |

---

## What's Missing (Gaps)

### 1. `tensor-scale` (scalar × tensor)
Currently have element-wise `tensor-mul`. Need scalar broadcast.

### 2. `tensor-sqr` (element-wise square)
Easy: `dup tensor-mul`

### 3. `pick` / `3 pick` (deep stack access)
Not implemented. Would need `rot rot rot` chains.

### 4. `nip` (remove second)
`swap drop`

### 5. Gradient context management
Current autodiff is global. Need scoped gradient contexts for training loops.

### 6. Optimizer tools
No `apply-gradients` / SGD step.

---

## Implementable Now: Minimal Demo

A simplified version that works with current tools:

```kore
; Create a "soft layer" - just alpha and weight
: make-layer ( dim -- layer )
  map-new
  swap dup tensor-randn requires-grad "weight" swap map-set
  1.0 1 list tensor-from-list requires-grad "alpha" swap map-set
;

; Forward through one layer
: layer-forward ( x layer -- x' )
  dup "weight" map-get    ; x layer weight
  rot                     ; layer weight x
  swap tensor-matmul      ; layer (weight @ x)
  swap "alpha" map-get    ; (weight @ x) alpha
  tensor-mul              ; Scaled output (needs broadcast fix)
;
```

---

## Key Technical Insights

### 1. Soft Gating is Autodiff-Compatible
The formula `α * f(x) + (1-α) * x` has clean gradients:
- ∂L/∂α = f(x) - x (block contribution minus identity)
- If block adds noise, gradient pushes α toward 0

### 2. L1 Regularization on α
Adding `λ * Σ|α|` to loss penalizes complexity.
Network naturally prunes when layers don't help.

### 3. Crystallization = List Filtering
```kore
genome [ "alpha" map-get 0.1 gt ] filter
```
This physically removes dead layers from the program.

### 4. Dynamic Depth via Early Exit
The really novel idea: use confidence to skip layers.
```kore
: maybe-skip ( x -- x' )
  dup confidence 0.9 gt
  [ ] [ expensive-transform ] if
;
```

---

## Three Directions to Pursue

### Direction A: Scalar Broadcasting (Quick Win)
Add `tensor-scale` tool for scalar × tensor operations.
Enables the soft gating formula directly.

### Direction B: Simple Optimizer
Add basic SGD: `tensor-sub-scaled` or similar.
```kore
: sgd-step ( tensor lr -- tensor' )
  over grad-get    ; tensor lr grad
  swap tensor-mul  ; tensor (grad * lr)  -- needs scale
  tensor-sub       ; tensor - (grad * lr)
;
```

### Direction C: The Full Demo
Once A and B work, implement:
1. Create N soft layers
2. Forward pass with gating
3. MSE loss + L1 on alphas
4. Backward + SGD
5. Prune layers where α < threshold
6. Show layer count decreasing

---

## Realistic Scope for RTX 3090

| Experiment | Feasibility |
|------------|-------------|
| Soft-gated MLP (10 layers, 64 dim) | ✓ Today |
| Self-pruning network | ✓ With scale tool |
| Dynamic depth | ✓ Conceptually clean |
| Full transformer | ✗ Need attention impl |
| TinyStories training | ✗ Need tokenizer, embeddings |

---

## Immediate Next Steps

1. **Add `tensor-scale`**: `(tensor scalar -- tensor)`
2. **Test soft gating**: `α * x + (1-α) * identity`
3. **Implement SGD step**: gradient descent update
4. **Build minimal demo**: 5-layer network that prunes to 2
5. **Visualize**: Plot α values over training

---

## The "Category Creation" Angle

The genuinely novel claim:

> "A neural network that physically rewrites its own source code during training"

This is NOT hyperbole in Kore:
- The genome is a list in memory
- Filtering removes elements
- The forward pass iterates the list
- Shorter list = faster inference

PyTorch can't do this naturally because the graph is static.
Kore's dynamic execution makes architecture evolution first-class.
