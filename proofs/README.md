# Proofs

Formal verification that each derived layer is faithful to P1–P4.
Every file compiles, proof-checks, and produces the mathematically expected result.

## Subdirectories

| Directory | What it proves |
|-----------|---------------|
| [calculus/](calculus/) | Dual number arithmetic correctly computes derivatives |
| [data-structures/](data-structures/) | List operations are faithful tools (P1) that compose (P3) |
| [linear-algebra/](linear-algebra/) | Dot product and matmul are correct compositions of arithmetic |

## Methodology

Each proof follows the same structure:

1. **Claim**: state the mathematical identity
2. **Construction**: build it from Kore primitives
3. **Verification**: run it and check the stack matches the expected result

The proof checker (P4) validates type constraints at compile time.
The runtime confirms the numerical result.

```bash
# Verify any proof
cargo run -- compile proofs/linear-algebra/matmul.kore -o /tmp/proof.korec
cargo run -- run /tmp/proof.korec
```
