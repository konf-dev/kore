# Linear Algebra Proofs

Proofs that vector and matrix operations are correct compositions
of scalar arithmetic. No BLAS, no matrix type — just `*`, `+`, and P3.

## Files

| File | Claim | Verified |
|------|-------|----------|
| `dot_product.kore` | dot(a,b) = Σᵢ aᵢ·bᵢ via zip ∘ map ∘ fold | ✅ |
| `matmul.kore` | 2×2 matmul C[i,j] = Σ_k A[i,k]·B[k,j] matches algebraic definition | ✅ |

## Derivation Chain

```
* and + (P1 arithmetic tools)
  └─ dot product = zip(a,b) → map(*) → fold(+)
       └─ matmul[i,j] = dot(row_i, col_j)
```

For A=[[1,2],[3,4]], B=[[5,6],[7,8]]:
- C = [[19,22],[43,50]] — verified on stack
