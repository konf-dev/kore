# Linear Algebra

Matrix multiplication derived from scalar arithmetic + composition (P3).
No matrix primitive. No BLAS. Just `*` and `+`.

## Files

| File | Muls | Operation | Result |
|------|------|-----------|--------|
| `matmul_2x2.kore` | 8 | [[1,2],[3,4]] × [[5,6],[7,8]] (naive) | `[[19,22],[43,50]]` |
| `matmul4x4.kore` | 64 | A² where A = [[1..16]] (naive, inline) | 16-element result |
| `strassen_2x2.kore` | **7** | Same 2×2 product via Strassen | `[[19,22],[43,50]]` |
| `strassen_4x4.kore` | **49** | Same A² via recursive block Strassen | 16-element result |
| `strassen_verify.kore` | — | Proves `naive(A,A) == strassen(A,A)` | `true` |

## Strassen Algorithm

Strassen (1969) decomposes 2×2 matrix multiplication into 7 scalar
multiplications instead of 8, using clever additions/subtractions:

```
M1 = (a+d)*(e+h)    M2 = (c+d)*e      M3 = a*(f-h)
M4 = d*(g-e)         M5 = (a+b)*h      M6 = (c-a)*(e+f)
M7 = (b-d)*(g+h)

C11 = M1+M4-M5+M7   C12 = M3+M5
C21 = M2+M4          C22 = M1-M2+M3+M6
```

For 4×4: partition into four 2×2 blocks, apply Strassen at the block
level (7 block-multiplications), where each block-multiply is itself
Strassen (7 scalar muls). Total: 7 × 7 = **49 multiplications**.

| Size | Naive | Strassen | Reduction | Asymptotic |
|------|-------|----------|-----------|------------|
| 2×2 | 8 | 7 | 12.5% | — |
| 4×4 | 64 | 49 | 23.4% | O(n^2.807) vs O(n^3) |
| n×n | n³ | n^2.807 | grows with n | Strassen wins |

All implemented purely in Kore. Zero new opcodes beyond `*`, `+`, `-`.
