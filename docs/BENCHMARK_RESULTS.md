# Kore Benchmark Results

> **Living document** — re-run `./benchmark.sh` after every update.
> Last updated: 2026-02-06 | Commit: `f0178ed` (dev) | Tests: 490 passing

---

## Environment

| Property       | Value |
|----------------|-------|
| **CPU**        | Intel Xeon E5-2690 v4 @ 2.60GHz (14C/28T) |
| **RAM**        | 62 GB DDR4 |
| **GPU**        | NVIDIA RTX 3090 Ti + RTX 2070 SUPER (Vulkan) |
| **OS**         | Linux 6.18.3-arch1-1 (Arch) |
| **Rust**       | 1.93.0 |
| **Cranelift**  | 0.111 |
| **Profile**    | `--release` (optimized) |

---

## How to Reproduce

```bash
# Full benchmark suite (23 tests)
cargo test --release 'benchmarks::tests' -- --nocapture

# Summary report only
cargo test --release benchmark_summary_report -- --nocapture

# Capability matrix
cargo test --release benchmark_backend_capability_matrix -- --nocapture

# Stress tests only
cargo test --release 'benchmarks::tests::stress_' -- --nocapture
```

---

## Performance Summary

### Industrial Benchmarks (measured 2026-02-06)

| Benchmark             | Rust (ns)     | JIT (ns)      | Interp (ns)     | JIT/Rust  | JIT/Interp |
|-----------------------|---------------|---------------|-----------------|-----------|------------|
| Fib iter (78)         | 0.8           | 620.7         | 24,141          | 769.5×    | 38.9×      |
| Sum i² (10K)          | 9,509         | 55,331        | 2,658,965       | 5.8×      | 48.1×      |
| Collatz (10K)         | 1,134,641     | 19,464,061    | 244,550,886     | 17.2×     | 12.6×      |
| Fib rec (25)          | 2,705         | 1,358,960     | 49,479,641      | 502.4×    | 36.4×      |
| **GEO MEAN**          |               |               |                 | **78.8×** | **30.5×**  |

### Micro Benchmarks (from benchmark test suite)

| Benchmark             | JIT (µs)    | Interpreter (µs) | JIT/Interp |
|-----------------------|-------------|-------------------|------------|
| Sum i² (1..10K)       | 7.70        | 1,927.40          | **250×**   |
| Fib iterative (78)    | 0.44        | 24.93             | **56×**    |
| Fib recursive (20)    | 1,414.14    | 4,475.70          | **3×**     |
| Collatz (1..10K)      | 20,237      | 247,728           | **12×**    |
| Float sqrt sum (1K)   | 6.04        | 200.47            | **33×**    |
| Bitwise ops (10K)     | 66.86       | 2,945.77          | **44×**    |
| Nested calls (27K)    | 2,478.00    | 2,703.00          | **1×**     |
| **Geometric mean**    |             |                   | **19×**    |

### Key Observations

- **Sum of squares** achieves **250× speedup** — JIT sweet spot (tight loop, pure stack arithmetic, SSA path).
- **Fibonacci iterative** at **56×** shows efficient STORE/LOAD handling.
- **Fibonacci recursive** improved from **3×** to **36×** after Fix 1 (used-locals-only CALL/RET). The micro benchmark above was measured before this optimization; the industrial benchmark reflects the current state.
- **Float pipeline** at **33×** — hardware float ops via Cranelift vs. interpreted dispatch.
- **Bitwise ops** at **44×** — direct CPU instructions vs. dispatch loop.
- **Nested calls** at **1×** — with 27,000 function calls per run of trivial bodies, call overhead dominates. Future work: per-function compilation (Fix 3) would use native `call`/`ret` instead of `br_table` dispatch.

---

## Backend Capability Matrix

Tested opcodes: 33 across all 4 backends.

| Opcode | Interpreter | JIT (Cranelift) | WASM | SPIR-V | Cross-match |
|--------|:-----------:|:---------------:|:----:|:------:|:-----------:|
| NOP    | ✓ | ✓ | ✓ | ✓ | ✓ |
| DROP   | ✓ | ✓ | ✓ | ✓ | ✓ |
| DUP    | ✓ | ✓ | ✓ | ✓ | ✓ |
| SWAP   | ✓ | ✓ | ✓ | ✓ | ✓ |
| ROT    | ✓ | ✓ | ✓* | ✓* | ✓ |
| OVER   | ✓ | ✓ | ✓* | ✓* | ✓ |
| INT8   | ✓ | ✓ | ✓ | ✓ | ✓ |
| ADD    | ✓ | ✓ | ✓ | ✓ | ✓ |
| SUB    | ✓ | ✓ | ✓ | ✓ | ✓ |
| MUL    | ✓ | ✓ | ✓ | ✓ | ✓ |
| DIV    | ✓ | ✓ | ✓* | ✓ | ✓ |
| MOD    | ✓ | ✓ | ✓* | ✓* | ✓ |
| NEG    | ✓ | ✓ | ✓* | ✓ | ✓ |
| I2F    | ✓ | ✓ | ✓* | ✓ | ✓ |
| FADD   | ✓ | ✓ | ✓* | ✓ | ✓ |
| FSUB   | ✓ | ✓ | ✓* | ✓ | ✓ |
| FMUL   | ✓ | ✓ | ✓* | ✓ | ✓ |
| FDIV   | ✓ | ✓ | ✓* | ✓ | ✓ |
| FNEG   | ✓ | ✓ | ✓* | ✓ | ✓ |
| EQ     | ✓ | ✓ | ✓* | ✓* | ✓ |
| LT     | ✓ | ✓ | ✓* | ✓* | ✓ |
| GT     | ✓ | ✓ | ✓* | ✓* | ✓ |
| LE     | ✓ | ✓ | ✓ | ✓ | ✓ |
| GE     | ✓ | ✓ | ✓* | ✓* | ✓ |
| NE     | ✓ | ✓ | ✓* | ✓* | ✓ |
| BAND   | ✓ | ✓ | ✓* | ✓* | ✓ |
| BOR    | ✓ | ✓ | ✓* | ✓* | ✓ |
| BXOR   | ✓ | ✓ | ✓* | ✓* | ✓ |
| BNOT   | ✓ | ✓ | ✓* | ✓* | ✓ |
| SHL    | ✓ | ✓ | ✓* | ✓* | ✓ |
| SHR    | ✓ | ✓ | ✓* | ✓* | ✓ |
| STORE  | ✓ | ✓ | ✓* | ✓* | ✓ |
| HALT   | ✓ | ✓ | ✓ | ✓ | ✓ |

**✓** = Fully implemented and tested  
**✓\*** = Compiles without error but the opcode is **silently skipped** — the binary is structurally valid WASM/SPIR-V but the operation is a no-op

### True Opcode Support (ops that actually execute)

| Backend     | Fully Implemented | Silently Skipped | Total Opcodes | Status |
|-------------|:-----------------:|:----------------:|:-------------:|--------|
| Interpreter | **134**           | 0                | 134           | ✅ Production |
| JIT SSA     | **50**            | 0                | 50            | ✅ Production |
| JIT Module  | **50**            | 0                | 50            | ✅ Production |
| SPIR-V      | **~20**           | ~10+             | 20            | ⚠️ GPU MAP only |
| WASM        | **~10**           | ~20+             | 10            | ❌ Not supported |

#### SPIR-V — GPU Parallel MAP (working)

The SPIR-V backend is designed for **element-wise parallel MAP** via `korec gpu-map`.
It compiles a per-element function body (e.g. `dup mul`) to a SPIR-V compute shader.
Each GPU thread applies the function to one array element in parallel.

Implemented opcodes (sufficient for element-wise compute):
`NOP`, `DROP`, `DUP`, `SWAP`, `INT8`, `ADD`, `SUB`, `MUL`, `DIV`, `NEG`,
`FADD`, `FSUB`, `FMUL`, `FDIV`, `FNEG`, `I2F`, `F2I`, `F64`, `LE`, `HALT`

Not implemented (not needed for element-wise MAP):
`JMP`/`JZ`/`JNZ` (control flow), `CALL`/`RET` (functions), `STORE`/`LOAD` (locals),
all comparison ops except `LE`, all bitwise ops, `ROT`/`OVER`.

This is intentional: GPU parallel MAP applies a pure arithmetic kernel to each element.
Control flow within GPU threads adds warp divergence and complexity for minimal benefit.

#### WASM — Not Yet Supported

> ⚠️ **The WASM backend is not production-ready.** It compiles structurally valid
> WASM binaries but only 10 opcodes actually execute. All other opcodes are silently
> skipped, producing wrong results with no error. Do not use WASM output for anything
> other than testing the compilation pipeline itself.

Implemented (truly functional): `NOP`, `DROP`, `DUP`, `SWAP`, `INT8`, `ADD`, `SUB`, `MUL`, `LE`, `HALT`

WASM support is planned for a future release. The priority is browser/edge deployment.

### Known Issue: Silent Skip in WASM and SPIR-V

Both backends have `_ => {}` catch-all in their opcode dispatch — unknown opcodes are
silently ignored. The binary compiles successfully but produces incorrect results.
This is the same class of bug that was fixed in the JIT backend.

**TODO**: Replace `_ => {}` with `_ => return Err(...)` in both backends.

---

## GPU Cross-Platform Support

Kore's GPU execution uses [wgpu](https://wgpu.rs/) — a cross-platform GPU abstraction
that maps to the native API on each platform:

| Platform         | GPU API      | Status |
|------------------|--------------|--------|
| Linux            | Vulkan       | ✅ Tested (RTX 3090 Ti) |
| Windows          | Vulkan / DX12| ✅ Expected (wgpu) |
| macOS            | Metal        | ✅ Expected (wgpu translates SPIR-V → MSL) |
| iOS              | Metal        | ✅ Expected (wgpu) |
| Android          | Vulkan       | ✅ Expected (device-dependent) |
| Browser          | WebGPU       | ⏳ Requires WGSL translation (not SPIR-V passthrough) |

### How It Works

1. Kore source → bytecode (`parser::compile`)
2. Bytecode → SPIR-V compute shader (`spirv_backend::compile_to_spirv_parallel_map`)
3. SPIR-V shader + input array → wgpu dispatch (`gpu_runtime::run_compute`)
4. wgpu selects Vulkan/Metal/DX12 automatically via `Backends::all()`

### What Works Today

- **`korec gpu-map`**: Apply a pure arithmetic function to each element of an array in parallel.
  Example: `korec gpu-map "dup mul" --input "1 2 3 4 5"` → `[1, 4, 9, 16, 25]`
- Supports: integer and float arithmetic, basic stack ops.
- Each GPU thread runs the function body on one element (GlobalInvocationID indexing).

### What Does NOT Work on GPU

- **Control flow**: No `if`/`else`/loops on GPU (no JMP/JZ/JNZ in SPIR-V backend).
- **Function calls**: No CALL/RET on GPU.
- **Tensor/matrix ops**: No matmul, conv, batched operations.
- **Training**: No autograd, no backward pass on GPU.

### Competitive Context

| Feature | Kore GPU | PyTorch | ONNX Runtime |
|---------|:--------:|:-------:|:------------:|
| Element-wise parallel MAP | ✅ | ✅ | ✅ |
| Matrix multiply / Conv | ❌ | ✅ | ✅ |
| Autograd (backward) | ❌ | ✅ | ❌ |
| Custom kernel from source | ✅ | Via CUDA/Triton | ❌ |
| Cross-platform (no CUDA) | ✅ (wgpu) | ❌ (CUDA) | Partial |
| Formal proof checking | ✅ | ❌ | ❌ |
| Lines of code | ~1,800 | Millions | Millions |

Kore's GPU story is **not competing with PyTorch/ONNX for training or inference**.
It is a general-purpose language with GPU compute as one capability. The value proposition:
- Compiles user-defined functions to GPU shaders automatically (no CUDA, no vendor lock-in).
- Works on any GPU through wgpu (Vulkan, Metal, DX12).
- The SPIR-V compilation pipeline, proof checker, and type system provide a foundation
  that could grow toward tensor ops and training in the future.

---

## Stress Tests (23 total)

| Test Category                  | Tests | Status |
|--------------------------------|:-----:|:------:|
| Performance benchmarks (01-10) | 10    | ✓      |
| Summary report                 | 1     | ✓      |
| Capability matrix              | 1     | ✓      |
| Every opcode category (36 ops) | 1     | ✓      |
| Deep recursion (100 levels)    | 1     | ✓      |
| Mutual recursion               | 1     | ✓      |
| Max locals (30 vars)           | 1     | ✓      |
| Large loop (100K iters)        | 1     | ✓      |
| Integer boundaries             | 1     | ✓      |
| Float edge cases               | 1     | ✓      |
| WASM compilation (9 programs)  | 1     | ✓      |
| SPIR-V compilation (10 programs) | 1  | ✓      |
| Determinism (100 runs)         | 1     | ✓      |
| Interpreter-only features      | 1     | ✓      |
| **Total**                      | **23**| **✓**  |

### Cross-Backend Consistency

Every test that runs on both interpreter and JIT verifies **identical results**:
- 36 opcode categories: all match
- 10 benchmark programs: all match  
- 100-iteration determinism test: all match
- Integer boundary values (i64::MAX, i64::MIN, 0, -1): all match
- Float operations (sqrt, fabs, fneg, i2f/f2i, fadd/fsub/fmul/fdiv): all match

### Known Limitations

1. **JIT recursive depth**: ~100 levels before stack overflow (SIGSEGV). The JIT uses native call stack for recursion — no guard page/detection yet.
2. **CALL/RET overhead**: JIT function calls use `br_table` dispatch + frame save/restore. Fix 1 (used-locals-only) reduced this from ~330 to ~30 instructions per CALL, but future per-function compilation (Fix 3) would use native CPU `call`/`ret`.
3. **WASM not supported**: The WASM backend compiles structurally valid binaries but silently skips most opcodes. Do not use for production.
4. **SPIR-V: GPU MAP only**: The SPIR-V backend works for element-wise parallel MAP (`korec gpu-map`). It does not support control flow, functions, or locals — those are not needed for element-wise kernels.
5. **No array/list support in JIT**: Lists, maps, strings are interpreter-only. JIT operates on `i64` stack values only.

---

## Benchmark Descriptions

### B01: Sum of Squares (sum i² for i=1..10000)
Tight arithmetic loop, pure stack operations. No locals, no calls. Tests the JIT's SSA path at peak throughput. **250× speedup.**

### B02: Fibonacci Iterative (fib(78))
Uses 4 local variables with STORE/LOAD in a loop. Tests variable access patterns. **56× speedup.**

### B03: Fibonacci Recursive (fib(20), 21891 calls)
Deep recursive function calls. Tests CALL/RET overhead. **3× speedup** — bottlenecked by memory-backed calling convention.

### B04: Collatz Conjecture (n=1..10000)
Nested while loops with branching (if/else). Tests branch prediction and mixed arithmetic. **12× speedup.**

### B05: Float Pipeline (sum sqrt(i) for i=1..1000)
Float arithmetic: I2F, FSQRT, FADD, F2I. Tests floating-point throughput. **33× speedup.**

### B06: Bitwise Operations (10K iterations)
XOR, AND, OR chains. Tests bitwise instruction generation. **44× speedup.**

### B07: Nested Calls (add1→add3→add9→add27, 1000 iterations)
27,000 function calls per run with trivial bodies. Tests pure call overhead. **1× speedup** — call overhead dominates.

### B08: Locals Throughput (5 vars, 10K iterations)
Heavy STORE/LOAD with 5 variables per iteration. Tests variable access patterns under load. **39× speedup.**

### B09: Comparison Heavy (mod 3/5/7, nested if/else, 10K)
Branch-heavy with triple-nested conditionals. Tests comparison + branch generation. **14× speedup.**

### B10: SSA vs Module Path (sum 1..1000)
Same program compiled via both JIT paths. Tests compilation path overhead. SSA path is typically 2-3× faster than module path.

---

## Test Coverage

```
Total tests:     490
  Parser:        305
  JIT backend:    96 (incl. 9 cross-backend + 4 P1-P4 verification)
  Benchmarks:     23 (stress + performance)
  Industrial:     24 (full report + feature coverage)
  SPIR-V:         13
  Optimizer:      12
  Proof checker:   5 (type lattice, linearity, capabilities)
  Interpreter:     5
  gpu_runtime:     2
  wasm_backend:    2
  bytecode:        2
  metacircular:    1
  All passing:   ✓
```

---

## Benchmark History

| Date       | Commit    | Total Tests | JIT/Interp | JIT/Rust | Notes |
|------------|-----------|:-----------:|:----------:|:--------:|-------|
| 2026-02-06 | `f0178ed` | 490         | **30×**    | **78.8×** | Fix 1-5 applied, used-locals CALL/RET, SSA in module path |
| 2026-02-06 | `7aa04c7` | 466         | **19×**    | **117×**  | Full benchmark suite, all backends tested, 14 JIT bugs fixed |

---

## Updating This Document

Run the comprehensive benchmark script after every change:

```bash
./benchmark.sh
# Generates: benchmark-results/YYYY-MM-DD_HHMMSS.txt
```

The script runs all tests, experiments, proof checks, cross-backend verification,
and benchmarks. Compare the generated file with previous runs to detect regressions.
