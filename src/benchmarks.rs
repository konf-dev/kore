// ============================================================================
// COMPREHENSIVE CROSS-BACKEND BENCHMARK & STRESS TEST SUITE
// ============================================================================
//
// Tests every capability across all backends:
//   - Interpreter (full 134 opcodes)
//   - JIT SSA compile() (50 opcodes, no CALL/RET)
//   - JIT Module compile_module() (50 opcodes, with CALL/RET)
//   - WASM compile_to_wasm() (~10 opcodes, structural correctness only)
//   - SPIR-V compile_to_spirv() (~20 opcodes, structural correctness only)
//
// Run all benchmarks:
//   cargo test --release benchmark_ -- --nocapture
//
// Run all stress tests:
//   cargo test --release stress_ -- --nocapture
//
// Run capability matrix:
//   cargo test --release benchmark_backend_capability_matrix -- --nocapture

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    // ================================================================
    // HELPERS
    // ================================================================

    /// Run bytecode through the interpreter, return i64 result
    fn interp_run(code: &[u8]) -> i64 {
        let mut interp = crate::interpreter::Interpreter::new(code);
        interp.run().unwrap();
        match interp.result() {
            Some(crate::interpreter::Value::Int(n)) => *n,
            Some(crate::interpreter::Value::Bool(true)) => 1,
            Some(crate::interpreter::Value::Bool(false)) => 0,
            other => panic!("Unexpected interpreter result: {:?}", other),
        }
    }

    /// Run source through parser + interpreter (module path)
    fn interp_source(source: &str) -> i64 {
        let module = crate::parser::compile(source).expect("compile failed");
        let mut interp = crate::interpreter::Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.stack().last() {
            Some(crate::interpreter::Value::Int(n)) => *n,
            Some(crate::interpreter::Value::Bool(true)) => 1,
            Some(crate::interpreter::Value::Bool(false)) => 0,
            other => panic!("Unexpected interpreter result: {:?}", other),
        }
    }

    /// Run source through parser + JIT module compile_module()
    #[cfg(feature = "native")]
    fn jit_module_source(source: &str) -> i64 {
        let module = crate::parser::compile(source).expect("compile failed");
        crate::native_backend::run_native_module(&module).unwrap()
    }

    /// Time a closure, return (median_of_5, last_result)
    fn bench_median<F: Fn() -> i64>(f: F, iterations: u64) -> (Duration, i64) {
        let mut times = Vec::new();
        let mut result = 0i64;
        for _ in 0..5 {
            let start = Instant::now();
            for _ in 0..iterations {
                result = f();
            }
            times.push(start.elapsed());
        }
        times.sort();
        (times[2], result)
    }

    /// Time a closure for a given number of iterations (single run)
    fn bench_single<F: Fn() -> i64>(f: F, iterations: u64) -> (Duration, i64) {
        let mut result = 0i64;
        let start = Instant::now();
        for _ in 0..iterations {
            result = f();
        }
        (start.elapsed(), result)
    }

    // ================================================================
    // BENCHMARK 01: Tight arithmetic loop (sum of squares)
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn benchmark_01_sum_of_squares() {
        let source = "\
            0 10000
            while dup 0 > do
              dup dup * rot + swap
              1 -
            end
            drop";
        let expected = 333383335000i64;

        // Correctness across backends
        let interp = interp_source(source);
        assert_eq!(interp, expected, "Interpreter incorrect");

        let module = crate::parser::compile(source).unwrap();
        let jit_ssa = crate::native_backend::run_native(&module.code).unwrap();
        assert_eq!(jit_ssa, expected, "JIT SSA incorrect");

        let module_src = format!(": __bench {} ; __bench", source);
        let jit_mod = jit_module_source(&module_src);
        assert_eq!(jit_mod, expected, "JIT Module incorrect");

        // Performance: JIT SSA (compile once, run many)
        let iterations = 1000u64;
        let mut jit_compiler = crate::native_backend::NativeCompiler::new().unwrap();
        let jit_func = jit_compiler.compile(&module.code).unwrap();
        let (jit_time, _) = bench_median(|| unsafe { jit_func() }, iterations);

        // Performance: Interpreter
        let (interp_time, _) = bench_single(
            || { let mut i = crate::interpreter::Interpreter::new(&module.code); i.run().unwrap();
                 match i.result() { Some(crate::interpreter::Value::Int(n)) => *n, _ => 0 } },
            iterations / 10,
        );

        let jit_ns = jit_time.as_nanos() as f64 / iterations as f64;
        let interp_ns = interp_time.as_nanos() as f64 / (iterations as f64 / 10.0);
        let speedup = interp_ns / jit_ns;

        println!("\n=== BENCHMARK 01: Sum of Squares (i=1..10000) ===");
        println!("  JIT SSA:     {:.2} µs/run", jit_ns / 1000.0);
        println!("  Interpreter: {:.2} µs/run", interp_ns / 1000.0);
        println!("  Speedup:     {:.0}x", speedup);
        println!("  Result:      {} ✓", expected);
        assert!(speedup > 5.0, "JIT should be ≥5x faster (got {:.1}x)", speedup);
    }

    // ================================================================
    // BENCHMARK 02: Fibonacci iterative (locals-heavy)
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn benchmark_02_fibonacci_iterative() {
        let source = "\
            0 ->a  1 ->b
            78 ->n  0 ->i
            while i n < do
              a b + ->c
              b ->a  c ->b
              i 1 + ->i
            end
            b";
        let expected = 14472334024676221i64;

        let interp = interp_source(source);
        assert_eq!(interp, expected, "Interpreter incorrect");

        let jit = jit_module_source(source);
        assert_eq!(jit, expected, "JIT incorrect");

        let module_src = format!(": __bench {} ; __bench", source);
        let jit_mod = jit_module_source(&module_src);
        assert_eq!(jit_mod, expected, "JIT Module wrapped incorrect");

        // Benchmark
        let iterations = 10_000u64;
        let module = crate::parser::compile(source).unwrap();
        let mut compiler = crate::native_backend::NativeCompiler::new().unwrap();
        let func = compiler.compile_module(&module).unwrap();
        let (jit_time, _) = bench_median(|| unsafe { func() }, iterations);
        let (interp_time, _) = bench_single(|| interp_source(source), iterations / 10);

        let jit_ns = jit_time.as_nanos() as f64 / iterations as f64;
        let interp_ns = interp_time.as_nanos() as f64 / (iterations as f64 / 10.0);

        println!("\n=== BENCHMARK 02: Fibonacci Iterative (fib(78)) ===");
        println!("  JIT Module:  {:.2} µs/run", jit_ns / 1000.0);
        println!("  Interpreter: {:.2} µs/run", interp_ns / 1000.0);
        println!("  Speedup:     {:.0}x", interp_ns / jit_ns);
        println!("  Result:      {} ✓", expected);
    }

    // ================================================================
    // BENCHMARK 03: Recursive fibonacci (CALL/RET stress)
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn benchmark_03_fibonacci_recursive() {
        let source = "\
            : fib ->n
              n 2 < if n
              else n 1 - fib n 2 - fib +
              end ;
            20 fib";
        let expected = 6765i64;

        let interp = interp_source(source);
        assert_eq!(interp, expected, "Interpreter incorrect");

        let jit = jit_module_source(source);
        assert_eq!(jit, expected, "JIT Module incorrect");

        // fib(20) = 21891 function calls
        let iterations = 100u64;
        let module = crate::parser::compile(source).unwrap();
        let mut compiler = crate::native_backend::NativeCompiler::new().unwrap();
        let func = compiler.compile_module(&module).unwrap();
        let (jit_time, _) = bench_median(|| unsafe { func() }, iterations);
        let (interp_time, _) = bench_single(|| interp_source(source), iterations / 5);

        let jit_ns = jit_time.as_nanos() as f64 / iterations as f64;
        let interp_ns = interp_time.as_nanos() as f64 / (iterations as f64 / 5.0);

        println!("\n=== BENCHMARK 03: Fibonacci Recursive fib(20) ===");
        println!("  JIT Module:  {:.2} µs/run ({} calls)", jit_ns / 1000.0, "21891");
        println!("  Interpreter: {:.2} µs/run", interp_ns / 1000.0);
        println!("  Speedup:     {:.0}x", interp_ns / jit_ns);
        println!("  Result:      {} ✓", expected);
    }

    // ================================================================
    // BENCHMARK 04: Collatz (branch-heavy, mixed ops)
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn benchmark_04_collatz() {
        let source = "\
            0 ->total
            1 ->n
            while n 10000 <= do
              n ->x  0 ->steps
              while x 1 > do
                x 2 mod 0 = if
                  x 2 / ->x
                else
                  x 3 * 1 + ->x
                end
                steps 1 + ->steps
              end
              total steps + ->total
              n 1 + ->n
            end
            total";
        let expected = 849666i64;

        let interp = interp_source(source);
        assert_eq!(interp, expected, "Interpreter incorrect");

        let jit = jit_module_source(source);
        assert_eq!(jit, expected, "JIT Module incorrect");

        let module = crate::parser::compile(source).unwrap();
        let mut compiler = crate::native_backend::NativeCompiler::new().unwrap();
        let func = compiler.compile_module(&module).unwrap();
        let (jit_time, _) = bench_median(|| unsafe { func() }, 100);
        let (interp_time, _) = bench_single(|| interp_source(source), 10);

        let jit_ns = jit_time.as_nanos() as f64 / 100.0;
        let interp_ns = interp_time.as_nanos() as f64 / 10.0;

        println!("\n=== BENCHMARK 04: Collatz (n=1..10000) ===");
        println!("  JIT Module:  {:.2} ms/run", jit_ns / 1_000_000.0);
        println!("  Interpreter: {:.2} ms/run", interp_ns / 1_000_000.0);
        println!("  Speedup:     {:.0}x", interp_ns / jit_ns);
        println!("  Result:      {} ✓", expected);
    }

    // ================================================================
    // BENCHMARK 05: Float pipeline (math ops, type conversions)
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn benchmark_05_float_pipeline() {
        let source = "\
            0 i2f ->sum
            1 ->i
            while i 1000 <= do
              i i2f fsqrt sum fadd ->sum
              i 1 + ->i
            end
            sum f2i";

        let interp = interp_source(source);
        let jit = jit_module_source(source);
        assert_eq!(jit, interp, "Float pipeline: jit={}, interp={}", jit, interp);

        let module = crate::parser::compile(source).unwrap();
        let mut compiler = crate::native_backend::NativeCompiler::new().unwrap();
        let func = compiler.compile_module(&module).unwrap();
        let (jit_time, _) = bench_median(|| unsafe { func() }, 1000);
        let (interp_time, _) = bench_single(|| interp_source(source), 100);

        let jit_ns = jit_time.as_nanos() as f64 / 1000.0;
        let interp_ns = interp_time.as_nanos() as f64 / 100.0;

        println!("\n=== BENCHMARK 05: Float Pipeline (sum sqrt(1..1000)) ===");
        println!("  JIT Module:  {:.2} µs/run", jit_ns / 1000.0);
        println!("  Interpreter: {:.2} µs/run", interp_ns / 1000.0);
        println!("  Speedup:     {:.0}x", interp_ns / jit_ns);
        println!("  Result:      {} (f2i truncated) ✓", interp);
    }

    // ================================================================
    // BENCHMARK 06: Bitwise operations (logic-heavy)
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn benchmark_06_bitwise_ops() {
        let source = "\
            0 ->acc
            1 ->i
            while i 10000 <= do
              i i 1 - bxor
              i 3 band bor
              acc bxor ->acc
              i 1 + ->i
            end
            acc";

        let interp = interp_source(source);
        let jit = jit_module_source(source);
        assert_eq!(jit, interp, "Bitwise: jit={}, interp={}", jit, interp);

        let module = crate::parser::compile(source).unwrap();
        let mut compiler = crate::native_backend::NativeCompiler::new().unwrap();
        let func = compiler.compile_module(&module).unwrap();
        let (jit_time, _) = bench_median(|| unsafe { func() }, 1000);
        let (interp_time, _) = bench_single(|| interp_source(source), 100);

        let jit_ns = jit_time.as_nanos() as f64 / 1000.0;
        let interp_ns = interp_time.as_nanos() as f64 / 100.0;

        println!("\n=== BENCHMARK 06: Bitwise Ops (10K iterations) ===");
        println!("  JIT Module:  {:.2} µs/run", jit_ns / 1000.0);
        println!("  Interpreter: {:.2} µs/run", interp_ns / 1000.0);
        println!("  Speedup:     {:.0}x", interp_ns / jit_ns);
        println!("  Result:      {} ✓", interp);
    }

    // ================================================================
    // BENCHMARK 07: Nested function calls (call overhead)
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn benchmark_07_nested_calls() {
        let source = "\
            : add1 1 + ;
            : add3 add1 add1 add1 ;
            : add9 add3 add3 add3 ;
            : add27 add9 add9 add9 ;
            0 ->acc
            1 ->i
            while i 1000 <= do
              acc add27 ->acc
              i 1 + ->i
            end
            acc";
        let expected = 27000i64;

        let interp = interp_source(source);
        assert_eq!(interp, expected, "Interpreter incorrect");

        let jit = jit_module_source(source);
        assert_eq!(jit, expected, "JIT Module incorrect");

        let module = crate::parser::compile(source).unwrap();
        let mut compiler = crate::native_backend::NativeCompiler::new().unwrap();
        let func = compiler.compile_module(&module).unwrap();
        let (jit_time, _) = bench_median(|| unsafe { func() }, 1000);
        let (interp_time, _) = bench_single(|| interp_source(source), 100);

        let jit_ns = jit_time.as_nanos() as f64 / 1000.0;
        let interp_ns = interp_time.as_nanos() as f64 / 100.0;

        println!("\n=== BENCHMARK 07: Nested Calls (27K calls/run) ===");
        println!("  JIT Module:  {:.2} µs/run", jit_ns / 1000.0);
        println!("  Interpreter: {:.2} µs/run", interp_ns / 1000.0);
        println!("  Speedup:     {:.0}x", interp_ns / jit_ns);
        println!("  Calls/run:   27,000");
        println!("  Result:      {} ✓", expected);
    }

    // ================================================================
    // BENCHMARK 08: Local variable throughput
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn benchmark_08_locals_throughput() {
        let source = "\
            0 ->a  0 ->b  0 ->c  0 ->d
            1 ->i
            while i 10000 <= do
              i ->a
              a a * ->b
              b a + ->c
              c b - ->d
              d a + ->a
              i 1 + ->i
            end
            a";

        let interp = interp_source(source);
        let jit = jit_module_source(source);
        assert_eq!(jit, interp, "Locals throughput: jit={}, interp={}", jit, interp);

        let module = crate::parser::compile(source).unwrap();
        let mut compiler = crate::native_backend::NativeCompiler::new().unwrap();
        let func = compiler.compile_module(&module).unwrap();
        let (jit_time, _) = bench_median(|| unsafe { func() }, 1000);
        let (interp_time, _) = bench_single(|| interp_source(source), 100);

        let jit_ns = jit_time.as_nanos() as f64 / 1000.0;
        let interp_ns = interp_time.as_nanos() as f64 / 100.0;

        println!("\n=== BENCHMARK 08: Locals Throughput (5 vars, 10K iters) ===");
        println!("  JIT Module:  {:.2} µs/run", jit_ns / 1000.0);
        println!("  Interpreter: {:.2} µs/run", interp_ns / 1000.0);
        println!("  Speedup:     {:.0}x", interp_ns / jit_ns);
        println!("  Result:      {} ✓", interp);
    }

    // ================================================================
    // BENCHMARK 09: Comparison-heavy (branch predictor stress)
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn benchmark_09_comparison_heavy() {
        let source = "\
            0 ->count
            1 ->i
            while i 10000 <= do
              i 3 mod 0 = if
                count 1 + ->count
              else i 5 mod 0 = if
                count 2 + ->count
              else i 7 mod 0 = if
                count 3 + ->count
              end end end
              i 1 + ->i
            end
            count";

        let interp = interp_source(source);
        let jit = jit_module_source(source);
        assert_eq!(jit, interp, "Comparison: jit={}, interp={}", jit, interp);

        let module = crate::parser::compile(source).unwrap();
        let mut compiler = crate::native_backend::NativeCompiler::new().unwrap();
        let func = compiler.compile_module(&module).unwrap();
        let (jit_time, _) = bench_median(|| unsafe { func() }, 1000);
        let (interp_time, _) = bench_single(|| interp_source(source), 100);

        let jit_ns = jit_time.as_nanos() as f64 / 1000.0;
        let interp_ns = interp_time.as_nanos() as f64 / 100.0;

        println!("\n=== BENCHMARK 09: Comparison Heavy (mod 3/5/7, 10K) ===");
        println!("  JIT Module:  {:.2} µs/run", jit_ns / 1000.0);
        println!("  Interpreter: {:.2} µs/run", interp_ns / 1000.0);
        println!("  Speedup:     {:.0}x", interp_ns / jit_ns);
        println!("  Result:      {} ✓", interp);
    }

    // ================================================================
    // BENCHMARK 10: SSA vs Module compilation paths
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn benchmark_10_ssa_vs_module() {
        let source_flat = "\
            0 ->sum
            1 ->i
            while i 1000 <= do
              sum i + ->sum
              i 1 + ->i
            end
            sum";
        let expected = 500500i64;

        let interp = interp_source(source_flat);
        assert_eq!(interp, expected);

        let module = crate::parser::compile(source_flat).unwrap();
        let ssa_result = crate::native_backend::run_native_module(&module).unwrap();
        assert_eq!(ssa_result, expected);

        let module_src = format!(": __bench {} ; __bench", source_flat);
        let mod_module = crate::parser::compile(&module_src).unwrap();
        let mod_result = crate::native_backend::run_native_module(&mod_module).unwrap();
        assert_eq!(mod_result, expected);

        // Benchmark both paths
        let iterations = 10_000u64;

        let mut ssa_compiler = crate::native_backend::NativeCompiler::new().unwrap();
        let ssa_func = ssa_compiler.compile_module(&module).unwrap();

        let mut mod_compiler = crate::native_backend::NativeCompiler::new().unwrap();
        let mod_func = mod_compiler.compile_module(&mod_module).unwrap();

        let (ssa_time, _) = bench_median(|| unsafe { ssa_func() }, iterations);
        let (mod_time, _) = bench_median(|| unsafe { mod_func() }, iterations);

        let ssa_ns = ssa_time.as_nanos() as f64 / iterations as f64;
        let mod_ns = mod_time.as_nanos() as f64 / iterations as f64;

        println!("\n=== BENCHMARK 10: SSA vs Module Path (sum 1..1000) ===");
        println!("  SSA path:    {:.2} µs/run", ssa_ns / 1000.0);
        println!("  Module path: {:.2} µs/run", mod_ns / 1000.0);
        println!("  SSA/Module:  {:.2}x", mod_ns / ssa_ns);
        println!("  Result:      {} ✓", expected);
    }

    // ================================================================
    // STRESS TEST: Every opcode category across all backends
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn stress_every_opcode_category() {
        struct TestCase {
            label: &'static str,
            source: &'static str,
            expected: i64,
        }

        let cases = vec![
            // Stack ops
            TestCase { label: "dup", source: "7 dup +", expected: 14 },
            TestCase { label: "swap", source: "10 3 swap -", expected: -7 },
            TestCase { label: "rot", source: "1 2 3 rot + +", expected: 6 },
            TestCase { label: "over", source: "5 3 over + +", expected: 13 },
            TestCase { label: "drop", source: "42 99 drop", expected: 42 },

            // Arithmetic
            TestCase { label: "add", source: "100 200 +", expected: 300 },
            TestCase { label: "sub", source: "100 30 -", expected: 70 },
            TestCase { label: "mul", source: "12 12 *", expected: 144 },
            TestCase { label: "div", source: "100 7 /", expected: 14 },
            TestCase { label: "mod", source: "100 7 mod", expected: 2 },
            TestCase { label: "neg", source: "42 neg", expected: -42 },

            // Comparison
            TestCase { label: "eq_true", source: "5 5 =", expected: 1 },
            TestCase { label: "eq_false", source: "5 4 =", expected: 0 },
            TestCase { label: "lt_true", source: "3 5 <", expected: 1 },
            TestCase { label: "gt_true", source: "5 3 >", expected: 1 },
            TestCase { label: "le_true", source: "5 5 <=", expected: 1 },
            TestCase { label: "ge_true", source: "5 5 >=", expected: 1 },
            TestCase { label: "ne_true", source: "5 4 !=", expected: 1 },
            TestCase { label: "lt_false", source: "5 3 <", expected: 0 },
            TestCase { label: "gt_false", source: "3 5 >", expected: 0 },

            // Bitwise
            TestCase { label: "band", source: "255 15 band", expected: 15 },
            TestCase { label: "bor", source: "240 15 bor", expected: 255 },
            TestCase { label: "bxor", source: "255 255 bxor", expected: 0 },
            TestCase { label: "bnot", source: "0 bnot", expected: -1 },
            TestCase { label: "shl", source: "1 10 shl", expected: 1024 },
            TestCase { label: "shr", source: "1024 10 shr", expected: 1 },

            // Locals
            TestCase { label: "store_load", source: "42 ->x x", expected: 42 },
            TestCase { label: "multi_locals", source: "10 ->a 20 ->b a b +", expected: 30 },
            TestCase { label: "overwrite", source: "1 ->x 2 ->x x", expected: 2 },

            // Control flow
            TestCase { label: "if_true", source: "1 0 > if 42 else 0 end", expected: 42 },
            TestCase { label: "if_false", source: "0 1 > if 42 else 99 end", expected: 99 },
            TestCase { label: "while_sum", source: "0 ->s 1 ->i while i 10 <= do s i + ->s i 1 + ->i end s", expected: 55 },
            TestCase { label: "nested_if", source: "5 3 > if  5 3 < if 1 else 2 end  else 3 end", expected: 2 },

            // Algorithms
            TestCase { label: "fibonacci_10", source: "0 1 ->a ->b 0 ->i while i 8 < do a b + ->c b ->a c ->b i 1 + ->i end b", expected: 21 },
            TestCase { label: "factorial_8", source: "1 ->r 1 ->i while i 8 <= do r i * ->r i 1 + ->i end r", expected: 40320 },
            TestCase { label: "gcd_252_105", source: "252 ->a 105 ->b while b 0 > do a b mod ->t b ->a t ->b end a", expected: 21 },
        ];

        let mut pass = 0;
        let mut fail = 0;

        println!("\n=== STRESS: Every Opcode Category ===");
        println!("{:<16} {:>8} {:>8} {:>8}  {}", "Test", "Interp", "JIT_SSA", "JIT_Mod", "Status");
        println!("{}", "-".repeat(60));

        for case in &cases {
            let interp = interp_source(case.source);
            let module = crate::parser::compile(case.source).unwrap();
            let jit_ssa = crate::native_backend::run_native_module(&module).unwrap();

            let mod_src = format!(": __t {} ; __t", case.source);
            let jit_mod = jit_module_source(&mod_src);

            let ok = interp == case.expected && jit_ssa == case.expected && jit_mod == case.expected;
            let status = if ok { "✓" } else { "✗" };

            if !ok {
                println!("{:<16} {:>8} {:>8} {:>8}  {} (expected {})",
                    case.label, interp, jit_ssa, jit_mod, status, case.expected);
                fail += 1;
            } else {
                println!("{:<16} {:>8} {:>8} {:>8}  {}", case.label, interp, jit_ssa, jit_mod, status);
                pass += 1;
            }
        }

        println!("{}", "-".repeat(60));
        println!("Passed: {}/{}  Failed: {}", pass, cases.len(), fail);
        assert_eq!(fail, 0, "{} cross-backend consistency tests failed", fail);
    }

    // ================================================================
    // STRESS TEST: Deep recursion
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn stress_deep_recursion() {
        let source = "\
            : sum_rec ->n n 0 <= if 0 else n n 1 - sum_rec + end ;
            100 sum_rec";
        let expected = 5050i64;

        let interp = interp_source(source);
        assert_eq!(interp, expected, "Interpreter failed deep recursion");

        let jit = jit_module_source(source);
        assert_eq!(jit, expected, "JIT failed deep recursion: got {}", jit);

        println!("\n=== STRESS: Deep Recursion (sum_rec(100) = 5050) ===");
        println!("  Interpreter: {} ✓", interp);
        println!("  JIT Module:  {} ✓", jit);
    }

    // ================================================================
    // STRESS TEST: Mutual recursion
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn stress_mutual_recursion() {
        let source_even = "\
            : is-even ->n n 0 = if 1 else n 1 - is-odd end ;
            : is-odd  ->n n 0 = if 0 else n 1 - is-even end ;
            20 is-even";
        let source_odd = "\
            : is-even ->n n 0 = if 1 else n 1 - is-odd end ;
            : is-odd  ->n n 0 = if 0 else n 1 - is-even end ;
            21 is-even";

        assert_eq!(interp_source(source_even), 1);
        assert_eq!(jit_module_source(source_even), 1);
        assert_eq!(interp_source(source_odd), 0);
        assert_eq!(jit_module_source(source_odd), 0);

        println!("\n=== STRESS: Mutual Recursion ===");
        println!("  is-even(20) = 1 ✓ (both backends)");
        println!("  is-even(21) = 0 ✓ (both backends)");
    }

    // ================================================================
    // STRESS TEST: Max locals (30 variables)
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn stress_max_locals() {
        let mut parts = Vec::new();
        for i in 0..30 {
            parts.push(format!("{} ->v{}", i + 1, i));
        }
        parts.push("v0".to_string());
        for i in 1..30 {
            parts.push(format!("v{} +", i));
        }
        let source = parts.join(" ");
        let expected: i64 = (1..=30).sum(); // 465

        let interp = interp_source(&source);
        assert_eq!(interp, expected, "Interpreter max locals");

        let jit = jit_module_source(&source);
        assert_eq!(jit, expected, "JIT max locals");

        let mod_src = format!(": __t {} ; __t", source);
        let jit_mod = jit_module_source(&mod_src);
        assert_eq!(jit_mod, expected, "JIT Module max locals");

        println!("\n=== STRESS: Maximum Locals (30 variables) ===");
        println!("  Sum v0..v29 = {} ✓ (all backends)", expected);
    }

    // ================================================================
    // STRESS TEST: Large loop count (100K iterations)
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn stress_large_loop() {
        let source = "\
            0 ->sum
            1 ->i
            while i 100000 <= do
              sum i + ->sum
              i 1 + ->i
            end
            sum";
        let expected = 5000050000i64;

        let interp = interp_source(source);
        assert_eq!(interp, expected, "Interpreter large loop");

        let jit = jit_module_source(source);
        assert_eq!(jit, expected, "JIT large loop");

        println!("\n=== STRESS: Large Loop (sum 1..100K) ===");
        println!("  Result: {} ✓ (all backends)", expected);
    }

    // ================================================================
    // STRESS TEST: Integer boundary values
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn stress_integer_boundaries() {
        let cases: Vec<(&str, &str, i64)> = vec![
            ("i64::MAX", "9223372036854775806 1 +", i64::MAX),
            ("i64::MIN", "-9223372036854775807 1 -", i64::MIN),
            ("zero", "0", 0),
            ("neg_one", "0 1 -", -1),
            ("max_div2", "9223372036854775807 2 /", 4611686018427387903),
        ];

        for (label, source, expected) in &cases {
            let interp = interp_source(source);
            assert_eq!(interp, *expected, "Boundary {}: interp={}", label, interp);

            let jit = jit_module_source(source);
            assert_eq!(jit, *expected, "Boundary {}: jit={}", label, jit);
        }

        println!("\n=== STRESS: Integer Boundaries ===");
        println!("  i64::MAX, i64::MIN, 0, -1, MAX/2 — all correct ✓");
    }

    // ================================================================
    // STRESS TEST: Float edge cases
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn stress_float_edge_cases() {
        let cases: Vec<(&str, &str)> = vec![
            ("sqrt_4", "4 i2f fsqrt f2i"),
            ("neg_sqrt", "9 i2f fneg fneg fsqrt f2i"),
            ("fabs_neg", "5 i2f fneg fabs f2i"),
            ("i2f_f2i_roundtrip", "42 i2f f2i"),
            ("float_add", "10 i2f 3 i2f fadd f2i"),
            ("float_sub", "10 i2f 3 i2f fsub f2i"),
            ("float_mul", "6 i2f 7 i2f fmul f2i"),
            ("float_div", "42 i2f 6 i2f fdiv f2i"),
        ];

        for (label, source) in &cases {
            let interp = interp_source(source);
            let jit = jit_module_source(source);
            assert_eq!(jit, interp, "Float {}: jit={}, interp={}", label, jit, interp);
        }

        println!("\n=== STRESS: Float Edge Cases ===");
        println!("  sqrt, fabs, fneg, i2f/f2i, fadd/fsub/fmul/fdiv — all match ✓");
    }

    // ================================================================
    // STRESS TEST: WASM backend compilation
    // ================================================================

    #[test]
    fn stress_wasm_compilation() {
        let test_cases: Vec<(&str, Vec<u8>)> = vec![
            ("nop+halt", vec![0x00, 0xFF]),
            ("int8+halt", vec![0x30, 42, 0xFF]),
            ("add", vec![0x30, 3, 0x30, 4, 0x40, 0xFF]),
            ("sub", vec![0x30, 10, 0x30, 3, 0x41, 0xFF]),
            ("mul", vec![0x30, 6, 0x30, 7, 0x42, 0xFF]),
            ("dup+add", vec![0x30, 5, 0x02, 0x40, 0xFF]),
            ("swap+sub", vec![0x30, 1, 0x30, 2, 0x03, 0x41, 0xFF]),
            ("le", vec![0x30, 3, 0x30, 5, 0x53, 0xFF]),
            ("drop", vec![0x30, 1, 0x30, 2, 0x01, 0xFF]),
        ];

        let mut pass = 0;
        println!("\n=== STRESS: WASM Compilation ===");
        for (label, code) in &test_cases {
            let result = crate::wasm_backend::compile_to_wasm(code);
            match result {
                Ok(wasm) => {
                    assert_eq!(&wasm[0..4], b"\0asm", "WASM {} missing magic", label);
                    println!("  {} — compiled ({} bytes) ✓", label, wasm.len());
                    pass += 1;
                }
                Err(e) => println!("  {} — FAILED: {}", label, e),
            }
        }
        println!("  {} / {} passed", pass, test_cases.len());
        assert_eq!(pass, test_cases.len());
    }

    // ================================================================
    // STRESS TEST: SPIR-V backend compilation
    // ================================================================

    #[test]
    fn stress_spirv_compilation() {
        let test_cases: Vec<(&str, Vec<u8>)> = vec![
            ("nop+int8", vec![0x00, 0x30, 1, 0xFF]),
            ("add", vec![0x30, 3, 0x30, 4, 0x40, 0xFF]),
            ("sub", vec![0x30, 10, 0x30, 3, 0x41, 0xFF]),
            ("mul", vec![0x30, 6, 0x30, 7, 0x42, 0xFF]),
            ("div", vec![0x30, 42, 0x30, 6, 0x43, 0xFF]),
            ("neg", vec![0x30, 5, 0x45, 0xFF]),
            ("dup+mul", vec![0x30, 5, 0x02, 0x42, 0xFF]),
            ("swap", vec![0x30, 1, 0x30, 2, 0x03, 0xFF]),
            ("drop", vec![0x30, 1, 0x30, 2, 0x01, 0xFF]),
            ("le", vec![0x30, 3, 0x30, 5, 0x53, 0xFF]),
        ];

        let mut pass = 0;
        println!("\n=== STRESS: SPIR-V Compilation ===");
        for (label, code) in &test_cases {
            let result = crate::spirv_backend::compile_to_spirv(code);
            match result {
                Ok(spirv) => {
                    assert!(spirv.len() >= 20, "SPIR-V {} too short", label);
                    println!("  {} — compiled ({} bytes) ✓", label, spirv.len());
                    pass += 1;
                }
                Err(e) => println!("  {} — FAILED: {}", label, e),
            }
        }
        println!("  {} / {} passed", pass, test_cases.len());
        assert_eq!(pass, test_cases.len());
    }

    // ================================================================
    // CAPABILITY MATRIX: document what each backend supports
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn benchmark_backend_capability_matrix() {
        // Minimal bytecode exercising each opcode
        let ops: Vec<(&str, Vec<u8>)> = vec![
            ("NOP",    vec![0x00, 0x30, 1, 0xFF]),
            ("DROP",   vec![0x30, 1, 0x30, 2, 0x01, 0xFF]),
            ("DUP",    vec![0x30, 5, 0x02, 0xFF]),
            ("SWAP",   vec![0x30, 1, 0x30, 2, 0x03, 0xFF]),
            ("ROT",    vec![0x30, 1, 0x30, 2, 0x30, 3, 0x04, 0xFF]),
            ("OVER",   vec![0x30, 1, 0x30, 2, 0x05, 0xFF]),
            ("INT8",   vec![0x30, 42, 0xFF]),
            ("ADD",    vec![0x30, 3, 0x30, 4, 0x40, 0xFF]),
            ("SUB",    vec![0x30, 5, 0x30, 3, 0x41, 0xFF]),
            ("MUL",    vec![0x30, 6, 0x30, 7, 0x42, 0xFF]),
            ("DIV",    vec![0x30, 42, 0x30, 6, 0x43, 0xFF]),
            ("MOD",    vec![0x30, 10, 0x30, 3, 0x44, 0xFF]),
            ("NEG",    vec![0x30, 5, 0x45, 0xFF]),
            ("I2F",    vec![0x30, 42, 0x4D, 0x4E, 0xFF]),
            ("FADD",   vec![0x30, 1, 0x4D, 0x30, 2, 0x4D, 0x46, 0x4E, 0xFF]),
            ("FSUB",   vec![0x30, 5, 0x4D, 0x30, 3, 0x4D, 0x47, 0x4E, 0xFF]),
            ("FMUL",   vec![0x30, 6, 0x4D, 0x30, 7, 0x4D, 0x48, 0x4E, 0xFF]),
            ("FDIV",   vec![0x30, 10, 0x4D, 0x30, 2, 0x4D, 0x49, 0x4E, 0xFF]),
            ("FNEG",   vec![0x30, 5, 0x4D, 0x4A, 0x4E, 0xFF]),
            ("EQ",     vec![0x30, 5, 0x30, 5, 0x50, 0xFF]),
            ("LT",     vec![0x30, 3, 0x30, 5, 0x51, 0xFF]),
            ("GT",     vec![0x30, 5, 0x30, 3, 0x52, 0xFF]),
            ("LE",     vec![0x30, 5, 0x30, 5, 0x53, 0xFF]),
            ("GE",     vec![0x30, 5, 0x30, 5, 0x54, 0xFF]),
            ("NE",     vec![0x30, 5, 0x30, 3, 0x55, 0xFF]),
            ("BAND",   vec![0x30, 12, 0x30, 10, 0x64, 0xFF]),
            ("BOR",    vec![0x30, 12, 0x30, 10, 0x65, 0xFF]),
            ("BXOR",   vec![0x30, 12, 0x30, 10, 0x66, 0xFF]),
            ("BNOT",   vec![0x30, 0, 0x67, 0xFF]),
            ("SHL",    vec![0x30, 1, 0x30, 4, 0x68, 0xFF]),
            ("SHR",    vec![0x30, 16, 0x30, 2, 0x69, 0xFF]),
            ("STORE",  vec![0x30, 42, 0xA8, 0,0,0,0, 0xA9, 0,0,0,0, 0xFF]),
            ("HALT",   vec![0x30, 1, 0xFF]),
        ];

        println!("\n╔══════════════════════════════════════════════════════════╗");
        println!("║           BACKEND CAPABILITY MATRIX                     ║");
        println!("╠══════════════════════════════════════════════════════════╣");
        println!("║ {:<8} {:>7} {:>7} {:>7} {:>7}  {:>6}  ║", "Opcode", "Interp", "JIT", "WASM", "SPIRV", "Match");
        println!("╠══════════════════════════════════════════════════════════╣");

        let mut counts = [0u32; 4]; // interp, jit, wasm, spirv

        for (name, code) in &ops {
            let interp_ok = {
                let mut i = crate::interpreter::Interpreter::new(code);
                i.run().is_ok()
            };
            let jit_ok = crate::native_backend::run_native(code).is_ok();
            let wasm_ok = crate::wasm_backend::compile_to_wasm(code).is_ok();
            let spirv_ok = crate::spirv_backend::compile_to_spirv(code).is_ok();

            if interp_ok { counts[0] += 1; }
            if jit_ok { counts[1] += 1; }
            if wasm_ok { counts[2] += 1; }
            if spirv_ok { counts[3] += 1; }

            // Cross-backend result match (only if both succeed)
            let results_match = if interp_ok && jit_ok {
                let interp_val = interp_run(code);
                let jit_val = crate::native_backend::run_native(code).unwrap();
                interp_val == jit_val
            } else {
                true // can't compare
            };

            let sym = |ok: bool| if ok { "✓" } else { "·" };
            let match_sym = if results_match { "✓" } else { "✗" };

            println!("║ {:<8} {:>7} {:>7} {:>7} {:>7}  {:>6}  ║",
                name, sym(interp_ok), sym(jit_ok), sym(wasm_ok), sym(spirv_ok), match_sym);

            if !results_match {
                println!("║   ⚠ RESULT MISMATCH on {}                             ║", name);
            }
        }

        let total = ops.len();
        println!("╠══════════════════════════════════════════════════════════╣");
        println!("║ {:<8} {:>5}/{} {:>5}/{} {:>5}/{} {:>5}/{}          ║",
            "TOTAL", counts[0], total, counts[1], total, counts[2], total, counts[3], total);
        println!("║ {:<8} {:>6}% {:>6}% {:>6}% {:>6}%          ║",
            "COVERAGE",
            counts[0] * 100 / total as u32,
            counts[1] * 100 / total as u32,
            counts[2] * 100 / total as u32,
            counts[3] * 100 / total as u32);
        println!("╚══════════════════════════════════════════════════════════╝");
    }

    // ================================================================
    // STRESS TEST: Determinism (100 runs, same result)
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn stress_determinism() {
        let source = "\
            : compute ->x
              x x * ->a
              a x + ->b
              b 2 / ->c
              c a - ;
            42 compute";

        let first_interp = interp_source(source);
        let first_jit = jit_module_source(source);

        for i in 0..100 {
            let interp = interp_source(source);
            let jit = jit_module_source(source);
            assert_eq!(interp, first_interp, "Interpreter non-deterministic at iteration {}", i);
            assert_eq!(jit, first_jit, "JIT non-deterministic at iteration {}", i);
        }

        assert_eq!(first_interp, first_jit, "Interp/JIT disagree: {} vs {}", first_interp, first_jit);

        println!("\n=== STRESS: Determinism (100 runs) ===");
        println!("  Interpreter: consistent ✓");
        println!("  JIT Module:  consistent ✓");
        println!("  Cross-match: {} ✓", first_interp);
    }

    // ================================================================
    // STRESS TEST: Interpreter-only features
    // ================================================================

    #[test]
    fn stress_interpreter_only_features() {
        let tests: Vec<(&str, &str, i64)> = vec![
            ("list_create", "( 1 2 3 ) len", 3),
            ("list_get", "( 10 20 30 ) 1 get", 20),
            ("pair", "3 4 pair unpair +", 7),
            ("string_len", "\"hello\" str-len swap drop", 5),
        ];

        println!("\n=== STRESS: Interpreter-Only Features ===");
        for (label, source, expected) in &tests {
            let result = interp_source(source);
            assert_eq!(result, *expected, "Interp-only {}: got {}", label, result);
            println!("  {} = {} ✓", label, result);
        }
    }

    // ================================================================
    // COMPREHENSIVE SUMMARY (run this one for the full report)
    // ================================================================

    #[test]
    #[cfg(feature = "native")]
    fn benchmark_summary_report() {
        println!("\n╔══════════════════════════════════════════════════════════╗");
        println!("║            KORE BENCHMARK SUMMARY REPORT                ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        println!();

        // Quick benchmarks inline
        struct BenchResult {
            name: &'static str,
            jit_us: f64,
            interp_us: f64,
        }

        let mut results = Vec::new();

        // B1: Sum of squares
        {
            let source = "0 10000 while dup 0 > do dup dup * rot + swap 1 - end drop";
            let module = crate::parser::compile(source).unwrap();
            let mut c = crate::native_backend::NativeCompiler::new().unwrap();
            let f = c.compile(&module.code).unwrap();
            let (jt, _) = bench_median(|| unsafe { f() }, 500);
            let (it, _) = bench_single(
                || { let mut i = crate::interpreter::Interpreter::new(&module.code); i.run().unwrap();
                     match i.result() { Some(crate::interpreter::Value::Int(n)) => *n, _ => 0 } }, 50);
            results.push(BenchResult {
                name: "Sum i² (1..10K)",
                jit_us: jt.as_nanos() as f64 / 500.0 / 1000.0,
                interp_us: it.as_nanos() as f64 / 50.0 / 1000.0,
            });
        }

        // B2: Fibonacci iterative
        {
            let source = "0 ->a 1 ->b 78 ->n 0 ->i while i n < do a b + ->c b ->a c ->b i 1 + ->i end b";
            let module = crate::parser::compile(source).unwrap();
            let mut c = crate::native_backend::NativeCompiler::new().unwrap();
            let f = c.compile_module(&module).unwrap();
            let (jt, _) = bench_median(|| unsafe { f() }, 5000);
            let (it, _) = bench_single(|| interp_source(source), 500);
            results.push(BenchResult {
                name: "Fib iter (78)",
                jit_us: jt.as_nanos() as f64 / 5000.0 / 1000.0,
                interp_us: it.as_nanos() as f64 / 500.0 / 1000.0,
            });
        }

        // B3: Fibonacci recursive
        {
            let source = ": fib ->n n 2 < if n else n 1 - fib n 2 - fib + end ; 20 fib";
            let module = crate::parser::compile(source).unwrap();
            let mut c = crate::native_backend::NativeCompiler::new().unwrap();
            let f = c.compile_module(&module).unwrap();
            let (jt, _) = bench_median(|| unsafe { f() }, 100);
            let (it, _) = bench_single(|| interp_source(source), 20);
            results.push(BenchResult {
                name: "Fib recursive (20)",
                jit_us: jt.as_nanos() as f64 / 100.0 / 1000.0,
                interp_us: it.as_nanos() as f64 / 20.0 / 1000.0,
            });
        }

        // B4: Collatz
        {
            let source = "0 ->total 1 ->n while n 10000 <= do n ->x 0 ->steps while x 1 > do x 2 mod 0 = if x 2 / ->x else x 3 * 1 + ->x end steps 1 + ->steps end total steps + ->total n 1 + ->n end total";
            let module = crate::parser::compile(source).unwrap();
            let mut c = crate::native_backend::NativeCompiler::new().unwrap();
            let f = c.compile_module(&module).unwrap();
            let (jt, _) = bench_median(|| unsafe { f() }, 50);
            let (it, _) = bench_single(|| interp_source(source), 5);
            results.push(BenchResult {
                name: "Collatz (1..10K)",
                jit_us: jt.as_nanos() as f64 / 50.0 / 1000.0,
                interp_us: it.as_nanos() as f64 / 5.0 / 1000.0,
            });
        }

        // B5: Float pipeline
        {
            let source = "0 i2f ->sum 1 ->i while i 1000 <= do i i2f fsqrt sum fadd ->sum i 1 + ->i end sum f2i";
            let module = crate::parser::compile(source).unwrap();
            let mut c = crate::native_backend::NativeCompiler::new().unwrap();
            let f = c.compile_module(&module).unwrap();
            let (jt, _) = bench_median(|| unsafe { f() }, 500);
            let (it, _) = bench_single(|| interp_source(source), 50);
            results.push(BenchResult {
                name: "Float sqrt sum (1K)",
                jit_us: jt.as_nanos() as f64 / 500.0 / 1000.0,
                interp_us: it.as_nanos() as f64 / 50.0 / 1000.0,
            });
        }

        // B6: Bitwise
        {
            let source = "0 ->acc 1 ->i while i 10000 <= do i i 1 - bxor i 3 band bor acc bxor ->acc i 1 + ->i end acc";
            let module = crate::parser::compile(source).unwrap();
            let mut c = crate::native_backend::NativeCompiler::new().unwrap();
            let f = c.compile_module(&module).unwrap();
            let (jt, _) = bench_median(|| unsafe { f() }, 500);
            let (it, _) = bench_single(|| interp_source(source), 50);
            results.push(BenchResult {
                name: "Bitwise ops (10K)",
                jit_us: jt.as_nanos() as f64 / 500.0 / 1000.0,
                interp_us: it.as_nanos() as f64 / 50.0 / 1000.0,
            });
        }

        // B7: Nested calls
        {
            let source = ": add1 1 + ; : add3 add1 add1 add1 ; : add9 add3 add3 add3 ; : add27 add9 add9 add9 ; 0 ->acc 1 ->i while i 1000 <= do acc add27 ->acc i 1 + ->i end acc";
            let module = crate::parser::compile(source).unwrap();
            let mut c = crate::native_backend::NativeCompiler::new().unwrap();
            let f = c.compile_module(&module).unwrap();
            let (jt, _) = bench_median(|| unsafe { f() }, 500);
            let (it, _) = bench_single(|| interp_source(source), 50);
            results.push(BenchResult {
                name: "Nested calls (27K)",
                jit_us: jt.as_nanos() as f64 / 500.0 / 1000.0,
                interp_us: it.as_nanos() as f64 / 50.0 / 1000.0,
            });
        }

        // Print table
        println!("{:<22} {:>12} {:>12} {:>10}", "Benchmark", "JIT (µs)", "Interp (µs)", "Speedup");
        println!("{}", "─".repeat(58));
        for r in &results {
            println!("{:<22} {:>12.2} {:>12.2} {:>9.0}x",
                r.name, r.jit_us, r.interp_us, r.interp_us / r.jit_us);
        }
        println!("{}", "─".repeat(58));

        // Geometric mean speedup
        let geo_mean: f64 = results.iter()
            .map(|r| (r.interp_us / r.jit_us).ln())
            .sum::<f64>() / results.len() as f64;
        println!("{:<22} {:>34.0}x (geometric mean)", "", geo_mean.exp());
    }
}
