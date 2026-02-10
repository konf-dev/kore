# Combinatorics

Constructive proofs in combinatorial mathematics via Kore's stack machine.

## Files

| File | Theorem | Result |
|------|---------|--------|
| `ramsey.kore` | R(3,3) = 6 | Stack: `[0, 5]` (C₅ is valid, C₅+vertex fails) |

## Ramsey R(3,3) = 6

The proof is constructive:

1. **Lower bound**: Exhibit a 2-coloring of K₅ (the cycle C₅) with no monochromatic triangle → R(3,3) > 5
2. **Upper bound**: Show that every 2-coloring of K₆ contains a monochromatic triangle → R(3,3) ≤ 6

Edge checks use stack arithmetic: for each triangle (i,j,k), verify that
not all three edges share a color. Pure composition of comparison ops.
