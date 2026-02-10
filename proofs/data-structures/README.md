# Data Structure Proofs

Proofs that Kore's compound data structures are faithful P1 tools
that compose by concatenation (P3).

## Files

| File | Claim | Verified |
|------|-------|----------|
| `list_ops.kore` | List literal, len, get, map, fold, zip are compositional P1 tools | ✅ |

## What's Proved

- `( 10 20 30 )` creates a list via push + LIST — pure composition
- `len` peeks length without mutation — tool is read-only
- `0 get` / `1 get` extracts elements — tool is indexable
- `[ 2 * ] map` transforms each element — higher-order via Quote (P2)
- `[ + ] 0 fold` reduces to a scalar — composition of apply
- `zip` pairs corresponding elements — structural correspondence
