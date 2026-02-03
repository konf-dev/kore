# Kore Performance Benchmarks

Release build benchmarks on Linux x64.

---

## Summary

| Operation | Iterations | Time | Rate |
|-----------|------------|------|------|
| Loop overhead | 100K | 87ms | 1.15M/s |
| Stack ops | 100K | 211ms | 474K/s |
| Arithmetic | 100K | 336ms | 298K/s |
| Tensor creation | 10K (100-elem) | 43ms | 233K/s |
| Tensor add | 10K (100-elem) | 99ms | 101K/s |
| Tensor mul | 10K (100-elem) | 101ms | 99K/s |
| Tensor sum | 10K (100-elem) | 55ms | 182K/s |
| Matmul | 1K (10×10 × 10) | 16ms | 62.5K/s |
| Softmax | 1K (100-elem) | 13ms | 77K/s |
| Autodiff backward | 1K (10-elem) | 28ms | 36K/s |

---

## Detailed Results

### Stack & Arithmetic

```
Loop overhead (100K iters, [drop]):
  real    0m0.087s

Stack ops (100K iters, [dup drop drop]):
  real    0m0.211s

Arithmetic (100K iters, [dup dup add drop drop]):
  real    0m0.336s
```

### Tensor Operations

```
tensor-ones (10K iters, 100 elements):
  real    0m0.043s

tensor-add (10K iters, 100 elements):
  real    0m0.099s

tensor-mul (10K iters, 100 elements):
  real    0m0.101s

tensor-sum (10K iters, 100 elements):
  real    0m0.055s
```

### ML Operations

```
tensor-matmul (1K iters, 10×10 × 10):
  real    0m0.016s

tensor-softmax (1K iters, 100 elements):
  real    0m0.013s

autodiff backward (1K iters, 10-element):
  real    0m0.028s
```

---

## Benchmark Commands

```bash
# Build release
cargo build --release

# Run benchmarks
time ./target/release/kore -e "100000 [ drop ] times"
time ./target/release/kore -e "10000 [ drop 100 tensor-ones drop ] times"
time ./target/release/kore -e "1000 [ drop 10 tensor-rand requires-grad 10 tensor-rand tensor-mul tensor-sum backward drop ] times"
```

---

## Notes

- All benchmarks use `times` loop which creates isolated stack per iteration
- Tensor ops include creation overhead in each iteration
- Autodiff benchmark includes forward pass, backward pass, and gradient storage
- No SIMD or GPU acceleration (pure Rust)
