# Calculus

Forward-mode automatic differentiation and gradient-based optimization,
derived entirely from `Pair` (dual numbers) and float arithmetic.

## Files

| File | What it demonstrates | Key result |
|------|---------------------|------------|
| `autodiff.kore` | Dual number arithmetic: d/dx(x²) = 2x, d/dx(x³) = 3x², chain rule | Exact symbolic derivatives |
| `gradient_descent.kore` | 50-step unrolled GD minimizing f(x)=x² | x: 10.0 → 0.000143 |
| `gradient_descent_loop.kore` | Same via `while` loop (44% smaller) | x: 10.0 → 0.000143, f(x) ≈ 0 |

## Derivation

```
Pair (P1 tool)
  └─ dual(x, ẋ) = Pair(value, derivative)
       └─ dadd, dmul, dneg, dsub (arithmetic on duals)
            └─ Forward-mode AD: f(dual(x,1)) → dual(f(x), f'(x))
                 └─ Gradient descent: x ← x - lr·f'(x)
```

Zero new opcodes. Differentiation is a **consequence** of Pair + arithmetic.
