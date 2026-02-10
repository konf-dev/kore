# Kore

A stack-based programming language with a bytecode compiler, interpreter, Cranelift JIT, and GPU (SPIR-V/WGSL) backends. Written in Rust.

## What it does

Kore compiles a Forth-like language to bytecode and runs it through multiple backends:

- **Interpreter** — stack-based VM with 142 opcodes, fibers, channels, local variables
- **JIT compiler** — Cranelift-based native code generation (~80× faster than interpreter for numeric workloads)
- **GPU backend** — SPIR-V shader generation + wgpu runtime for parallel computation
- **WASM backend** — WebAssembly output (work in progress, not fully functional)
- **Proof checker** — static stack-effect verification at compile time

## Quick start

```bash
git clone https://github.com/konf-dev/kore.git
cd kore
cargo build --release
```

Try it:

```bash
# REPL
./target/release/korec repl

# Compile and run
./target/release/korec compile examples/square.kore -o /tmp/square.korec
./target/release/korec run /tmp/square.korec

# JIT compile and run (much faster for numeric code)
./target/release/korec native-run /tmp/square.korec

# Run on GPU
./target/release/korec gpu-run /tmp/math_test.korec
```

## Language

Kore is a stack-based language. Values go on the stack, operations consume and produce values.

```
-- Define a function
: square dup * ;

-- Use it
5 square    -- leaves 25 on the stack
```

```
-- Composition
: double 2 * ;
: quadruple double double ;
: inc 1 + ;

3 quadruple inc    -- 13
```

Functions can have proof annotations for compile-time stack-effect checking:

```
: square ( n -- n*n ) dup * ;
```

The language supports strings, booleans, local variables (`->name` to bind, `name` to recall), conditionals (`if`/`else`/`end`), loops (`loop`/`break`), fibers, channels, error handling, and capability-gated I/O.

See [docs/KORE_GUIDE.md](docs/KORE_GUIDE.md) for the full language reference.

## CLI commands

```
korec compile <file> -o <output>   Compile .kore source to bytecode
korec run <file>                   Run bytecode in the interpreter
korec native-run <file>            Run bytecode through Cranelift JIT
korec check <file>                 Type-check without running
korec disasm <file>                Disassemble bytecode
korec repl                         Interactive REPL
korec serve                        JSON eval server (stdin/stdout)
korec spirv <file> -o <output>     Compile to SPIR-V
korec gpu-run <file>               Run on GPU via wgpu
korec gpu-map <file>               Map a function over GPU data
korec gpu-info                     Show GPU device info
korec example <name>               Run a built-in example
korec test                         Run the test suite
```

## Project structure

```
src/
  parser.rs          — lexer + parser (~6,000 lines)
  bytecode.rs        — opcode definitions, 142 opcodes
  interpreter.rs     — stack VM with fibers, channels, locals (~3,000 lines)
  native_backend.rs  — Cranelift JIT (~4,100 lines)
  proof_checker.rs   — static stack-effect verification (~1,750 lines)
  spirv_backend.rs   — SPIR-V code generation (~1,450 lines)
  wasm_backend.rs    — WASM output (WIP, ~850 lines)
  gpu_runtime.rs     — wgpu compute pipeline (~320 lines)
  optimizer.rs       — peephole + algebraic optimizations (~860 lines)
  main.rs            — CLI entry point

stdlib/              — 16 standard library modules (math, linear algebra,
                       tensors, autograd, fibers, strings, etc.)
examples/            — example programs
experiments/         — algorithms, calculus, proofs, sorting networks, neural nets
docs/                — specifications and guides
```

## Tests

```bash
cargo test
```

524 tests pass. The test suite covers the parser, interpreter, JIT backend, bytecode, optimizer, proof checker, GPU runtime, and industrial benchmarks.

## Known limitations

- **WASM backend** is incomplete. It generates valid WASM for basic programs but doesn't cover the full opcode set.
- **Proof checker vs. recursion**: The stack-effect checker cannot verify recursive functions because it requires known stack effects at definition time. Recursive code runs fine in the interpreter and JIT — it just can't be statically verified. Example: `factorial.kore` works in the REPL but fails `korec check`.
- **GPU backend** requires a GPU. The `gpu` feature is enabled by default; disable with `--no-default-features` if you don't have one.
- **No package manager**. The stdlib is included as `.kore` files, loaded with `include`.

## Feature flags

```toml
[features]
default = ["gpu", "native"]
gpu = ["wgpu", "pollster", "bytemuck"]       # GPU compute via wgpu
native = ["cranelift-codegen", "..."]         # Cranelift JIT
```

Build without GPU or JIT:

```bash
cargo build --release --no-default-features
```

## License

MIT
