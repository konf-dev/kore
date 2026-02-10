# Calculus Proofs

Proofs that forward-mode automatic differentiation via dual numbers
is correct — derivatives computed by Kore match the symbolic results.

## Files

| File | Claim | Verified |
|------|-------|----------|
| `autodiff.kore` | d(a,a') ⊕ d(b,b') correctly implements the chain rule for +, ×, and composition | ✅ |

## Key Identity

For dual numbers `d(x, ẋ)`:
- `dadd`: d(a+b, ȧ+ḃ)
- `dmul`: d(a·b, ȧ·b + a·ḃ) (product rule)
- `dneg`: d(−a, −ȧ)

Setting ẋ = 1 gives f'(x) in the derivative slot. No symbolic manipulation needed.
