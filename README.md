# Kore

A stack-based programming language with a bytecode compiler, interpreter, Cranelift JIT, and GPU (SPIR-V) backend. Written in Rust. ~23,000 lines, 524 tests.

## Why Kore exists

Kore was built to explore a specific question: **what happens when you design a programming language where the compiler can verify program correctness before execution?**

Most AI code generation today works by generating Python, running it, and hoping it doesn't crash. The LLM has no feedback signal until runtime, and no guarantee that syntactically valid code is semantically correct.

Kore takes a different approach. Its type system (the "proof checker") statically verifies stack effects at compile time — before the program runs. This means:

- **The compiler is a free oracle.** It tells you whether a program is correct without executing it. For AI training, this is a reward function that runs at 50,000 verifications/second.
- **Small action space.** ~140 tokens (vs. ~50,000 for Python). An RL agent or LLM generates programs by choosing from this vocabulary. The search space is orders of magnitude smaller.
- **Deterministic, pure semantics.** Same program → same result, always. No imports, no side effects, no environment dependencies. Evaluation is clean.
- **Capability levels as curriculum.** The vocabulary is organized into levels (literals → arithmetic → stack ops → control flow → data structures → ...). This provides a natural curriculum for progressive training.

We tested this by fine-tuning DeepSeek-R1-14B to generate Kore programs. Result: **99.6% accuracy on 478 tasks** across 16 capability levels, with a **93.1%** success rate on held-out demo tasks phrased differently from training. See [experiments/llm-codegen/](experiments/llm-codegen/) for the full pipeline and results.

## The capability lattice

Kore's type system is organized as a capability lattice — a hierarchy of what a program is allowed to do. This comes from four design principles:

1. **Everything is a stack operation** (P1). Every value, function, and data structure lives on a single stack. There's nothing else.
2. **One composition operator** (P2). Programs compose by concatenation. `f g` means "do f, then do g." No application syntax, no parentheses, no variable binding required.
3. **Composition is associative** (P3). `(f g) h = f (g h)`. Programs are just sequences of tokens.
4. **Capabilities only attenuate** (P4). A subroutine can never gain capabilities its caller doesn't have. If you can't do I/O, nothing you call can do I/O either.

P4 is enforced at compile time by the proof checker. Each operation has a declared stack effect (how many values it consumes and produces), and the checker verifies that every composition is balanced. Linear and affine types prevent resources from being duplicated or silently dropped.

This is narrower than full dependent types (Lean, Coq) but broader than memory safety alone (Rust). It checks semantic properties — stack balance, type consistency, capability monotonicity — at zero runtime cost.

### Capability levels

The vocabulary is organized into 20 levels, each unlocking new operations:

| Level | Category | Tokens |
|-------|----------|--------|
| 0 | Literals | `0`–`10`, `true`, `false`, `nil` |
| 1 | Arithmetic | `+`, `-`, `*`, `div`, `mod`, `neg` |
| 2 | Stack ops | `dup`, `drop`, `swap`, `over`, `rot` |
| 3 | Comparisons | `=`, `<`, `>`, `not`, `and`, `or` |
| 4 | Variables | `->a`, `a`, `->b`, `b`, `->n`, `n` |
| 5 | Control flow | `if`, `else`, `end`, `while`, `do`, `times` |
| 6 | Quotations | `[`, `]`, `apply`, `cond`, `loop` |
| 7 | Bitwise | `band`, `bor`, `bxor`, `shl`, `shr` |
| 8 | Data structures | `pair`, `unpair`, `first`, `second` |
| 9 | Lists | `map`, `fold`, `filter`, `head`, `tail`, `range` |
| 10 | Error handling | `error`, `is-error`, `try`, `fail` |
| 11 | Strings | `str-len`, `str-concat`, `str-slice`, `to-str` |
| 12 | Type conversion | `i2f`, `f2i`, `type-of`, `depth` |
| 13 | Float math | `fadd`, `fsub`, `fmul`, `fdiv`, `fsqrt`, ... |
| 14 | Linear types | `linear`, `affine`, `consume` |
| 15 | Fibers | `fiber-new`, `fiber-step`, `fiber-push` |
| 16 | Concurrency | `spawn`, `chan-new`, `chan-send`, `chan-recv` |
| 17 | Maps | `map-new`, `map-get`, `map-set`, `map-keys` |
| 18 | Reflection | `fetch`, `size`, `ret`, `halt` |
| 19 | Functions | `:`, `;` (define named functions) |

Each level is a superset of the previous. A program at level 5 can only use tokens from levels 0–5. This makes capability restriction trivial — just limit the available vocabulary.

## Quick start

```bash
git clone https://github.com/konf-dev/kore.git
cd kore
cargo build --release
```

```bash
# Interactive REPL
./target/release/korec repl

# Compile and run
./target/release/korec compile examples/square.kore -o /tmp/square.korec
./target/release/korec run /tmp/square.korec

# JIT compile and run (up to 250× faster for numeric code)
./target/release/korec native-run /tmp/square.korec

# JSON eval server for programmatic use (used by the LLM training pipeline)
./target/release/korec serve
```

## Language

Kore is stack-based. Values go on the stack, operations consume and produce values.

```
-- Push and add
3 4 +                  -- 7

-- Define a function
: square dup * ;
5 square               -- 25

-- Composition by concatenation
: double 2 * ;
: quadruple double double ;
3 quadruple            -- 12

-- Local variables
10 ->n  n n *          -- 100

-- Control flow
5 2 > if 1 else 0 end -- 1

-- Quotations (anonymous functions)
7 [ 2 * ] apply       -- 14

-- Lists
( 1 2 3 ) [ dup * ] map   -- (1 4 9)
( 1 2 3 ) 0 [ + ] fold    -- 6

-- Proof annotations
: square ( n -- n*n ) dup * ;
```

See [docs/KORE_GUIDE.md](docs/KORE_GUIDE.md) for the full language reference.

## Performance

The Cranelift JIT achieves significant speedups over the interpreter:

| Benchmark | JIT/Interpreter | JIT vs. Native Rust |
|-----------|:-:|:-:|
| Sum of squares (10K) | **250×** | 5.8× slower |
| Fibonacci iterative (78) | **56×** | 769× slower |
| Bitwise ops (10K) | **44×** | — |
| Float sqrt sum (1K) | **33×** | — |
| Collatz (1..10K) | **12×** | 17× slower |
| Fibonacci recursive (25) | **36×** | 502× slower |
| **Geometric mean** | **30×** | **79× slower** |

The JIT is 30× faster than the interpreter on average, and about 79× slower than hand-written Rust — reasonable for a bytecode VM with no LLVM backend. The `korec serve` JSON eval server sustains ~50,000 program evaluations/second for the LLM training pipeline.

See [docs/BENCHMARK_RESULTS.md](docs/BENCHMARK_RESULTS.md) for full benchmark details, GPU benchmarks, backend capability matrix, and stress test results.

## Backends

| Backend | Status | Use case |
|---------|--------|----------|
| **Interpreter** | Production (134 opcodes) | General purpose, all features |
| **Cranelift JIT** | Production (50 opcodes) | Numeric workloads, benchmarks |
| **SPIR-V / GPU** | Working (20 opcodes) | Element-wise parallel MAP via wgpu |
| **WASM** | WIP (~10 opcodes) | Not production-ready |

The GPU backend compiles Kore functions to SPIR-V compute shaders and dispatches them via wgpu (Vulkan/Metal/DX12). Currently supports element-wise parallel MAP — apply a pure function to every element of an array in parallel.

## CLI

```
korec compile <file> -o <out>   Compile .kore source to bytecode
korec run <file>                Run bytecode in the interpreter
korec native-run <file>         Run bytecode through Cranelift JIT
korec check <file>              Type-check / proof-check without running
korec disasm <file>             Disassemble bytecode
korec repl                      Interactive REPL
korec serve                     JSON eval server (stdin/stdout, ~50K evals/sec)
korec spirv <file> -o <out>     Compile to SPIR-V
korec gpu-run <file>            Run on GPU via wgpu
korec gpu-map <file>            Parallel MAP on GPU
korec gpu-info                  Show GPU device info
korec example <name>            Run a built-in example
korec test                      Run the test suite
```

## Project structure

```
src/
  parser.rs           — lexer + parser (6,031 lines)
  native_backend.rs   — Cranelift JIT (4,147 lines)
  interpreter.rs      — stack VM, fibers, channels (2,976 lines)
  proof_checker.rs    — static stack-effect verification (1,756 lines)
  spirv_backend.rs    — SPIR-V code generation (1,470 lines)
  bytecode.rs         — 142 opcode definitions (1,114 lines)
  optimizer.rs        — peephole + algebraic optimizations (858 lines)
  wasm_backend.rs     — WebAssembly output, WIP (844 lines)
  gpu_runtime.rs      — wgpu compute dispatch (320 lines)
  main.rs             — CLI entry point (1,298 lines)

stdlib/               — 16 modules: math, linear algebra, tensors,
                        autograd, fibers, strings, geometry, etc.
examples/             — example programs (palindrome checker, etc.)
experiments/
  llm-codegen/        — LLM fine-tuning pipeline (SFT+GRPO, 99.6% accuracy)
  algorithms/         — iterative algorithms
  calculus/           — autodiff, gradient descent
  linear-algebra/     — matrix multiplication, Strassen's algorithm
  neural/             — XOR neural network in Kore
  optimization/       — Rosenbrock, circle packing
  proofs/             — verified proofs of stack properties
  sorting-networks/   — optimal sorting network search
docs/
  KORE_GUIDE.md       — full language reference
  BENCHMARK_RESULTS.md — performance data
  PROOF_CHECKER.md    — type system design
  BYTECODE_SPEC.md    — opcode specification
  KORE_COMPARISON.md  — comparison with other languages
```

## Tests

```bash
cargo test           # 524 tests, all passing
```

Covers: parser, interpreter, JIT, bytecode, optimizer, proof checker, GPU runtime, SPIR-V, WASM, cross-backend consistency, industrial benchmarks, stress tests.

## Known limitations

- **WASM backend** is incomplete — generates structurally valid WASM but silently skips most opcodes.
- **Proof checker can't verify recursion** — recursive functions work in the interpreter and JIT, but the stack-effect checker requires known effects at definition time. `factorial.kore` runs fine but fails `korec check`.
- **JIT supports i64 stack values only** — lists, strings, maps are interpreter-only.
- **GPU is element-wise MAP only** — no control flow, no function calls on GPU.
- **No package manager** — stdlib modules are `.kore` files loaded with `include`.
- **LLM experiment curriculum is finite** — 478 tasks is small; the model likely memorizes rather than generalizes. Open-ended generation is future work.

## Feature flags

```toml
[features]
default = ["gpu", "native"]
gpu = ["wgpu", "pollster", "bytemuck"]       # GPU compute via wgpu
native = ["cranelift-codegen", "..."]         # Cranelift JIT
```

Build without GPU or JIT:

```bash
cargo build --release --no-default-features  # interpreter only
```

## What's next

Kore is a research project. Things we think are worth exploring:

- **Scaling the LLM experiment** — larger curriculum, open-ended tasks, held-out test sets, smaller models
- **Self-extending stdlib** — the LLM writes verified Kore functions that persist and compose. Infrastructure exists in `stdlib_grower.py`; not yet trained end-to-end.
- **Kore as a tool language for AI agents** — programs are small, verified, deterministic, and capability-restricted. An agent could write Kore tools instead of Python scripts, with compile-time guarantees that the tool does what it claims.
- **JIT coverage** — extending the JIT to handle lists, strings, and maps
- **WASM completion** — full opcode coverage for browser/edge deployment

## License

MIT
