# Neural Networks

Neural network inference derived from linear algebra + activation functions.
Architecture is pure composition: layer = matmul → bias → activation.

## Files

| File | Architecture | Result |
|------|-------------|--------|
| `neural_xor.kore` | 2 → 2 → 1 (hand-tuned weights) | `[0, 1, 1, 0]` (XOR truth table) |

## Derivation

```
scalar arithmetic (P1)
  └─ dot product (composition of *, +)
       └─ linear layer: y = W·x + b
            └─ activation: ReLU(y) = max(0, y)
                 └─ network: layer ∘ layer ∘ ... (P3)
```

The XOR network uses hand-picked weights that solve XOR.
All four input pairs `(0,0), (0,1), (1,0), (1,1)` are evaluated
and produce the correct outputs `0, 1, 1, 0`.
