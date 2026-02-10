# Experiments

Demonstrations that everything is derived from the four postulates.
No new axioms. No special-cased primitives. Just tools on tools (P1) composed by concatenation (P3).

## Subdirectories

| Directory | What it proves |
|-----------|---------------|
| [algorithms/](algorithms/) | While loops + stack ops = Turing-complete iteration (Fibonacci, GCD, isqrt, power) |
| [calculus/](calculus/) | Differentiation & optimization derived from Pair (dual numbers) + arithmetic |
| [combinatorics/](combinatorics/) | Ramsey theory — constructive proof of R(3,3)=6 via graph coloring |
| [linear-algebra/](linear-algebra/) | Matrix multiply derived from scalar arithmetic + composition |
| [neural/](neural/) | Neural networks derived from linear algebra + activation functions |

## Derivation Hierarchy

```
P1-P4 (postulates)
  └─ stack ops + arithmetic (bytecode primitives)
       ├─ algorithms/     ← while + stack = iteration
       ├─ linear-algebra/ ← arithmetic = matmul
       ├─ calculus/       ← Pair = dual numbers = autodiff
       ├─ neural/         ← matmul + activation + autodiff
       └─ combinatorics/  ← arithmetic + search = Ramsey
```

## Running

```bash
# Compile and run any experiment
cargo run -- compile experiments/algorithms/iterative.kore -o /tmp/out.korec
cargo run -- run /tmp/out.korec
```
