# Kore

A stack-based programming language with a bytecode compiler, interpreter, Cranelift JIT, and GPU (SPIR-V) backend. Written in Rust. ~23,000 lines, 524 tests.

## Why Kore exists

Kore was built to explore a specific question: **what happens when you design a programming language where the compiler can act as a reward function while training a llm on it?**

Most AI code generation today works by generating Python, running it, and hoping it doesn't crash. The LLM has no feedback signal until runtime, and no guarantee that syntactically valid code is semantically correct.

Kore takes a different approach. Its type system (the "proof checker") statically verifies stack effects at compile time — before the program runs. This means:

- **The compiler is a free oracle.** It tells you whether a program is correct without executing it. For AI training, this is a reward function that runs at 50,000 verifications/second.
- **Small action space.** ~140 tokens (vs. ~50,000 for Python). An RL agent or LLM generates programs by choosing from this vocabulary. The search space is orders of magnitude smaller.
- **Deterministic, pure semantics.** Same program → same result, always. No imports, no side effects, no environment dependencies. Evaluation is clean.
- **Capability-gated security.** Programs declare what they need (I/O, filesystem, exec). The proof checker verifies these at compile time and the runtime enforces them. Pure programs — no capabilities — are safe by construction.

We tested this by fine-tuning DeepSeek-R1-14B to generate Kore programs on held-out demo tasks phrased differently from training. See [experiments/llm-codegen/](experiments/llm-codegen/) for the full pipeline and results.

## The capability lattice

Kore's security model is built on four postulates. The fourth — **capabilities only attenuate** — is the core design constraint:

1. **Everything is a stack operation** (P1). Every value, function, and data structure lives on a single stack. There's nothing else.
2. **One composition operator** (P2). Programs compose by concatenation. `f g` means "do f, then do g." No application syntax, no parentheses, no variable binding required.
3. **Composition is associative** (P3). `(f g) h = f (g h)`. Programs are just sequences of tokens.
4. **Capabilities only attenuate** (P4). A subroutine can never gain capabilities its caller doesn't have. If you can't do I/O, nothing you call can do I/O either. Constraints form a lattice — they can only narrow, never widen.

P4 means that when you `spawn` a sandboxed computation, the child's capabilities are `parent_caps & requested_caps` — it can never escalate. This is enforced twice: the proof checker verifies capability usage at compile time, and the runtime re-checks at execution time. A compiled `.korec` binary stores its required capabilities in the file header; you must explicitly grant them at runtime (`--allow io`, `--allow fs`, etc.) or the program won't run.

### What the proof checker verifies

The proof checker statically verifies:
- **Stack effects** — every operation declares how many values it consumes and produces. The checker walks the entire program and verifies that every composition is balanced.
- **Capability requirements** — `print` requires `io`, `file-read` requires `fs`, `exec` requires `exec`. If a program uses `print`, it's tagged `[caps: io]` at compile time and rejected at runtime without `--allow io`.
- **Linear/affine types** — values marked `linear` must be consumed exactly once (no `dup`, no `drop`). Values marked `affine` can be dropped but not duplicated. This prevents resource leaks and double-use.
- **Capability monotonicity** — a `spawn`'ed child computation inherits a subset of the parent's capabilities. The lattice goes one direction.

```bash
$ korec compile pure.kore         # → "compiled 42 bytes [pure]"
$ korec compile hello.kore        # → "compiled 58 bytes [caps: io]"
$ korec run hello.korec           # → Error: Capability denied: io
$ korec run hello.korec --allow io  # → hello world
```

This is narrower than full dependent types (Lean, Coq) but broader than memory safety alone (Rust). It checks semantic properties — stack balance, type consistency, capability monotonicity — at zero runtime cost.

### Capabilities

| Capability | Flag | Gated operations | Purpose |
|------------|------|-------------------|---------|
| `io` | 0x01 | `print`, `println`, `rand`, `time-now`, `env-get` | Console I/O, non-determinism |
| `fs` | 0x02 | `file-read`, `file-write`, `file-exists` | Filesystem access |
| `net` | 0x04 | *(future)* | Network access |
| `exec` | 0x08 | `exec` | Shell command execution |

Programs that use none of these are **pure** — they run anywhere (interpreter, JIT, GPU, WASM) with no permission flags, and their output is fully deterministic.

### Why this matters for AI

A pure Kore program is a closed-world computation: same input → same output, no side effects, no environmental dependencies. The compiler can verify correctness without executing it. For LLM code generation, this means:

- **The compiler is a free reward function.** It accepts or rejects programs at ~50K/sec. No test harness needed.
- **Small action space.** ~140 tokens vs. ~50K for Python. The LLM's search space is orders of magnitude smaller.
- **Capability restriction as safety.** An AI agent writing Kore tools can be given a restricted capability set. The proof checker guarantees the tool can't do I/O, touch the filesystem, or exec shell commands — by construction, not by sandboxing.

The LLM training experiment uses a 20-level curriculum that progressively introduces vocabulary (literals → arithmetic → control flow → data structures → ...), but this curriculum is a training artifact, not the capability lattice itself. See [experiments/llm-codegen/](experiments/llm-codegen/) for details.

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

The Cranelift JIT compiles Kore bytecode to native machine code. Measured times (lower is better):

| Benchmark | Interpreter | JIT | Native Rust | JIT speedup |
|-----------|-------------|-----|-------------|-------------|
| Sum of squares (10K) | 2,659 µs | 55 µs | 9.5 µs | 48× faster |
| Fibonacci iterative (78) | 24 µs | 0.6 µs | 0.8 ns | 39× faster |
| Collatz (1..10K) | 245 ms | 19 ms | 1.1 ms | 13× faster |
| Fibonacci recursive (25) | 49 ms | 1.4 ms | 2.7 µs | 36× faster |
| **Geometric mean** | | | | **31× faster** |

The JIT is ~31× faster than the interpreter on average, and ~79× slower than hand-written Rust — reasonable for a bytecode VM using Cranelift (no LLVM). The `korec serve` JSON eval server sustains ~50,000 program evaluations/second for the LLM training pipeline.

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
