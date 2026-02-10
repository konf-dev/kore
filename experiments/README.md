# Experiments

Kore programs demonstrating capabilities across domains — from basic algorithms to LLM code generation.

## Subdirectories

| Directory | What it does |
|-----------|-------------|
| [llm-codegen/](llm-codegen/) | Fine-tuning DeepSeek-R1-14B to generate Kore programs (99.6% accuracy on 478 tasks) |
| [algorithms/](algorithms/) | Iteration via while loops + stack ops (Fibonacci, GCD, isqrt, power) |
| [calculus/](calculus/) | Autodiff via dual numbers, gradient descent |
| [combinatorics/](combinatorics/) | Ramsey theory — constructive proof of R(3,3)=6 |
| [linear-algebra/](linear-algebra/) | Matrix multiplication, Strassen's algorithm |
| [neural/](neural/) | XOR neural network from scratch |
| [optimization/](optimization/) | Rosenbrock function, circle packing |
| [proofs/](proofs/) | Verified proofs of stack-effect properties |
| [sorting-networks/](sorting-networks/) | Optimal sorting network search |

## How it fits together

```
Kore bytecode VM
  ├─ algorithms/         ← basic iteration & recursion
  ├─ linear-algebra/     ← arithmetic → matrix ops
  ├─ calculus/           ← dual numbers → autodiff
  ├─ neural/             ← matmul + activation + autodiff
  ├─ optimization/       ← gradient descent, simulated annealing
  ├─ combinatorics/      ← search + arithmetic → Ramsey
  ├─ sorting-networks/   ← exhaustive search over comparator networks
  ├─ proofs/             ← proof checker verifying stack properties
  └─ llm-codegen/        ← LLM learns to generate all of the above
```

## Running

```bash
# Compile and run any experiment
cargo run -- compile experiments/algorithms/iterative.kore -o /tmp/out.korec
cargo run -- run /tmp/out.korec
```
