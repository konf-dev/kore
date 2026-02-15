//! Kore Execution Benchmarks
//!
//! Measures execution time across different scenarios:
//! 1. Single-shot execution (functional executor)
//! 2. Session step-by-step execution
//! 3. Session run (batch)
//! 4. Snapshot + restore cycles
//! 5. Fork + parallel exploration (MCTS pattern)
//! 6. Parallel independent sessions (training parallelism)
//! 7. Incremental program building (compile_op pattern)
//!
//! Run with: cargo test --release -p kore --test benchmarks -- --nocapture

use kore::builtins::register_builtins;
use kore::context::Context;
use kore::executor::execute;
use kore::op::Op;
use kore::session::Session;
use kore::stack::Stack;
use kore::value::Value;

use std::time::Instant;

// ============================================================
// Helpers
// ============================================================

async fn setup_ctx() -> Context {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    ctx
}

/// Format duration in human-readable form
fn fmt_duration(d: std::time::Duration) -> String {
    let us = d.as_micros();
    if us < 1000 {
        format!("{}µs", us)
    } else if us < 1_000_000 {
        format!("{:.2}ms", us as f64 / 1000.0)
    } else {
        format!("{:.3}s", us as f64 / 1_000_000.0)
    }
}

/// Create a compute-bound program of N push+add ops
fn make_sum_program(n: usize) -> Vec<Op> {
    let mut ops = vec![Op::push(0)];
    for i in 1..=n {
        ops.push(Op::Push(Value::Int(i as i64)));
        ops.push(Op::call("add"));
    }
    ops
}

/// Create a program using various ops (push, dup, add, mul, drop)
fn make_mixed_program(n: usize) -> Vec<Op> {
    // Mix of arithmetic, stack, and comparison ops.
    // Uses swap+drop to replace top with fresh small values periodically,
    // preventing unbounded growth that overflows i64 in debug mode.
    let mut ops = vec![Op::push(1), Op::push(2)];
    for i in 0..n {
        match i % 6 {
            0 => {
                // Reset top to a small value to prevent overflow
                ops.push(Op::call("drop"));
                ops.push(Op::Push(Value::Int((i % 100) as i64)));
            }
            1 => {
                ops.push(Op::Push(Value::Int(1)));
                ops.push(Op::call("add"));
            }
            2 => {
                ops.push(Op::call("dup"));
                ops.push(Op::call("drop"));
            }
            3 => {
                ops.push(Op::Push(Value::Int(2)));
                ops.push(Op::call("add"));
            }
            4 => {
                ops.push(Op::call("dup"));
                ops.push(Op::Push(Value::Int(10)));
                ops.push(Op::call("lt"));
                ops.push(Op::call("drop")); // drop the bool
            }
            _ => {
                ops.push(Op::call("dup"));
                ops.push(Op::call("swap"));
                ops.push(Op::call("drop"));
            }
        }
    }
    ops
}

/// Create a string manipulation program
fn make_string_program(n: usize) -> Vec<Op> {
    let mut ops = vec![Op::Push(Value::Text("hello".into()))];
    for _ in 0..n {
        ops.push(Op::Push(Value::Text(" world".into())));
        ops.push(Op::call("str-concat"));
    }
    ops
}

/// Create a list building program
fn make_list_program(n: usize) -> Vec<Op> {
    let mut ops = vec![];
    for i in 0..n {
        ops.push(Op::Push(Value::Int(i as i64)));
    }
    ops.push(Op::Push(Value::Int(n as i64)));
    ops.push(Op::call("list"));
    ops
}

// ============================================================
// Benchmark 1: Single-shot functional execution
// ============================================================

#[tokio::test]
async fn bench_01_single_shot_execution() {
    let ctx = setup_ctx().await;

    println!("\n{}", "=".repeat(60));
    println!("BENCHMARK 1: Single-Shot Functional Execution");
    println!("{}", "=".repeat(60));

    let scenarios: Vec<(&str, Vec<Op>)> = vec![
        ("sum_100", make_sum_program(100)),
        ("sum_1000", make_sum_program(1000)),
        ("sum_10000", make_sum_program(10000)),
        ("mixed_100", make_mixed_program(100)),
        ("mixed_1000", make_mixed_program(1000)),
        ("string_50", make_string_program(50)),
        ("string_200", make_string_program(200)),
        ("list_100", make_list_program(100)),
        ("list_500", make_list_program(500)),
    ];

    println!("{:<20} {:>8} {:>10} {:>12}", "scenario", "ops", "time", "ops/sec");
    println!("{:-<56}", "");

    for (name, ops) in &scenarios {
        let op_count = ops.len();
        let start = Instant::now();
        let (result, _) = execute(ops, Stack::new(), ctx.clone()).await.unwrap();
        let elapsed = start.elapsed();

        let ops_per_sec = op_count as f64 / elapsed.as_secs_f64();
        println!(
            "{:<20} {:>8} {:>10} {:>12.0}",
            name,
            op_count,
            fmt_duration(elapsed),
            ops_per_sec
        );

        // Verify results
        assert!(result.depth() >= 1, "{} should produce at least one value", name);
    }
}

// ============================================================
// Benchmark 2: Session step-by-step vs run
// ============================================================

#[tokio::test]
async fn bench_02_session_step_vs_run() {
    let ctx = setup_ctx().await;

    println!("\n{}", "=".repeat(60));
    println!("BENCHMARK 2: Session Step-by-Step vs Run");
    println!("{}", "=".repeat(60));

    let sizes = [100, 1000, 5000];

    println!(
        "{:<12} {:>8} {:>10} {:>10} {:>8}",
        "size", "ops", "step()", "run()", "ratio"
    );
    println!("{:-<54}", "");

    for &size in &sizes {
        let ops = make_sum_program(size);
        let op_count = ops.len();

        // Measure step-by-step
        let mut session = Session::new(ops.clone(), Stack::new(), ctx.clone());
        let start = Instant::now();
        while session.status().is_running() {
            session.step().await;
        }
        let step_time = start.elapsed();
        assert!(session.status().is_halted());
        let step_result = session.stack().values()[0].as_int().unwrap();

        // Measure run()
        let mut session = Session::new(ops, Stack::new(), ctx.clone());
        let start = Instant::now();
        session.run().await;
        let run_time = start.elapsed();
        assert!(session.status().is_halted());
        let run_result = session.stack().values()[0].as_int().unwrap();

        // Results should match
        assert_eq!(step_result, run_result);

        let ratio = step_time.as_secs_f64() / run_time.as_secs_f64();
        println!(
            "{:<12} {:>8} {:>10} {:>10} {:>7.2}x",
            format!("sum_{}", size),
            op_count,
            fmt_duration(step_time),
            fmt_duration(run_time),
            ratio
        );
    }
}

// ============================================================
// Benchmark 3: Snapshot + Restore cycles
// ============================================================

#[tokio::test]
async fn bench_03_snapshot_restore() {
    let ctx = setup_ctx().await;

    println!("\n{}", "=".repeat(60));
    println!("BENCHMARK 3: Snapshot + Restore Cycles");
    println!("{}", "=".repeat(60));

    let ops = make_sum_program(500);
    let mut session = Session::new(ops, Stack::new(), ctx.clone());

    // Run to midpoint
    session.step_n(250).await;

    // Measure snapshot creation
    let snap_count = 1000;
    let start = Instant::now();
    let mut snaps = Vec::with_capacity(snap_count);
    for _ in 0..snap_count {
        snaps.push(session.snapshot());
    }
    let snap_time = start.elapsed();

    // Measure restore
    let start = Instant::now();
    for snap in &snaps {
        session.restore(snap);
    }
    let restore_time = start.elapsed();

    println!(
        "Snapshot ×{}: total={}, per_snap={}",
        snap_count,
        fmt_duration(snap_time),
        fmt_duration(snap_time / snap_count as u32)
    );
    println!(
        "Restore  ×{}: total={}, per_restore={}",
        snap_count,
        fmt_duration(restore_time),
        fmt_duration(restore_time / snap_count as u32)
    );
    println!();

    // Measure snapshot + diverge + restore cycle (MCTS pattern)
    let cycle_count = 100;
    let ctx2 = setup_ctx().await;
    let ops = make_sum_program(200);
    let mut session = Session::new(ops, Stack::new(), ctx2);
    session.step_n(100).await;
    let root = session.snapshot();

    let start = Instant::now();
    for i in 0..cycle_count {
        session.restore(&root);
        session.append_ops(vec![Op::Push(Value::Int(i as i64)), Op::call("add")]);
        session.run().await;
    }
    let cycle_time = start.elapsed();

    println!(
        "MCTS cycle (restore+append+run) ×{}: total={}, per_cycle={}",
        cycle_count,
        fmt_duration(cycle_time),
        fmt_duration(cycle_time / cycle_count as u32)
    );
}

// ============================================================
// Benchmark 4: Fork + parallel exploration
// ============================================================

#[tokio::test]
async fn bench_04_fork_parallel() {
    let ctx = setup_ctx().await;

    println!("\n{}", "=".repeat(60));
    println!("BENCHMARK 4: Fork + Parallel Exploration");
    println!("{}", "=".repeat(60));

    // Create base session
    let ops = make_sum_program(100);
    let mut session = Session::new(ops, Stack::new(), ctx.clone());
    session.step_n(50).await;

    // Measure fork creation
    let fork_count = 100;
    let start = Instant::now();
    let mut forks: Vec<Session> = Vec::with_capacity(fork_count);
    for _ in 0..fork_count {
        forks.push(session.fork());
    }
    let fork_time = start.elapsed();

    println!(
        "Fork ×{}: total={}, per_fork={}",
        fork_count,
        fmt_duration(fork_time),
        fmt_duration(fork_time / fork_count as u32)
    );

    // Sequential: run all forks to completion
    let start = Instant::now();
    for fork in &mut forks {
        fork.append_ops(vec![Op::push(999), Op::call("add")]);
        fork.run().await;
    }
    let seq_time = start.elapsed();

    println!(
        "Sequential run ×{}: total={}, per_session={}",
        fork_count,
        fmt_duration(seq_time),
        fmt_duration(seq_time / fork_count as u32)
    );

    // Parallel: run forks concurrently with tokio::join
    let mut forks: Vec<Session> = Vec::with_capacity(fork_count);
    for _ in 0..fork_count {
        let mut f = session.fork();
        f.append_ops(vec![Op::push(999), Op::call("add")]);
        forks.push(f);
    }

    let start = Instant::now();
    let handles: Vec<_> = forks
        .into_iter()
        .map(|mut f| {
            let _ctx = ctx.clone();
            tokio::spawn(async move {
                f.run().await;
                f
            })
        })
        .collect();

    let mut results = Vec::with_capacity(fork_count);
    for h in handles {
        results.push(h.await.unwrap());
    }
    let par_time = start.elapsed();

    println!(
        "Parallel run  ×{}: total={}, per_session={}",
        fork_count,
        fmt_duration(par_time),
        fmt_duration(par_time / fork_count as u32)
    );
    println!(
        "Speedup: {:.2}x",
        seq_time.as_secs_f64() / par_time.as_secs_f64()
    );

    // Verify all results are the same
    let expected = results[0].stack().values()[0].as_int().unwrap();
    for r in &results {
        assert_eq!(r.stack().values()[0].as_int().unwrap(), expected);
    }
}

// ============================================================
// Benchmark 5: Parallel independent sessions (training scenario)
// ============================================================

#[tokio::test]
async fn bench_05_parallel_independent_sessions() {
    let ctx = setup_ctx().await;

    println!("\n{}", "=".repeat(60));
    println!("BENCHMARK 5: Parallel Independent Sessions (Training)");
    println!("{}", "=".repeat(60));

    let session_count = 50;
    let ops_per_session = 1000;

    // Sequential baseline
    let start = Instant::now();
    for _i in 0..session_count {
        let ops = make_sum_program(ops_per_session);
        let mut session = Session::new(ops, Stack::new(), ctx.clone());
        session.run().await;
        assert!(session.status().is_halted());
    }
    let seq_time = start.elapsed();

    // Parallel execution
    let start = Instant::now();
    let handles: Vec<_> = (0..session_count)
        .map(|_i| {
            let ctx = ctx.clone();
            tokio::spawn(async move {
                let ops = make_sum_program(ops_per_session);
                let mut session = Session::new(ops, Stack::new(), ctx);
                session.run().await;
                assert!(session.status().is_halted());
                session.stack().values()[0].as_int().unwrap()
            })
        })
        .collect();

    let mut par_results = Vec::new();
    for h in handles {
        par_results.push(h.await.unwrap());
    }
    let par_time = start.elapsed();

    let expected_sum: i64 = (1..=ops_per_session as i64).sum();
    for &r in &par_results {
        assert_eq!(r, expected_sum);
    }

    println!(
        "Sessions: {}, Ops/session: {}",
        session_count, ops_per_session
    );
    println!("Sequential: {}", fmt_duration(seq_time));
    println!("Parallel:   {}", fmt_duration(par_time));
    println!(
        "Speedup:    {:.2}x",
        seq_time.as_secs_f64() / par_time.as_secs_f64()
    );
    println!(
        "Throughput: {:.0} sessions/sec (parallel)",
        session_count as f64 / par_time.as_secs_f64()
    );
    println!(
        "Total ops:  {:.0} ops/sec (parallel)",
        (session_count * ops_per_session * 2) as f64 / par_time.as_secs_f64()
    );
}

// ============================================================
// Benchmark 6: Incremental program building (compile_op)
// ============================================================

#[tokio::test]
async fn bench_06_incremental_build() {
    let ctx = setup_ctx().await;

    println!("\n{}", "=".repeat(60));
    println!("BENCHMARK 6: Incremental Program Building");
    println!("{}", "=".repeat(60));

    let iterations = 200;

    // Pattern: append small code chunks and run, simulating model actions
    let mut session = Session::new(vec![], Stack::new(), ctx.clone());
    session.append_ops(vec![Op::push(0)]);
    session.run().await;

    let start = Instant::now();
    for i in 1..=iterations {
        // Model generates code → parse → append → run
        let code = format!("{} add", i);
        let ops = Op::parse(&code).unwrap();
        session.append_ops(ops);
        session.run().await;
    }
    let total_time = start.elapsed();

    let expected: i64 = (1..=iterations as i64).sum();
    assert_eq!(session.stack().values()[0].as_int().unwrap(), expected);

    println!(
        "Iterations: {}, Total: {}, Per iteration: {}",
        iterations,
        fmt_duration(total_time),
        fmt_duration(total_time / iterations as u32)
    );
    println!(
        "This includes: Op::parse() + append_ops() + run() per iteration"
    );
}

// ============================================================
// Benchmark 7: Mixed workload (realistic MCTS simulation)
// ============================================================

#[tokio::test]
async fn bench_07_mcts_simulation() {
    let ctx = setup_ctx().await;

    println!("\n{}", "=".repeat(60));
    println!("BENCHMARK 7: Realistic MCTS Simulation");
    println!("{}", "=".repeat(60));

    // Simulate MCTS tree search with N rollouts
    let rollouts = 100;
    let depth = 5; // actions per rollout

    // Root program
    let mut session = Session::new(vec![Op::push(0)], Stack::new(), ctx.clone());
    session.run().await;
    let root_snap = session.snapshot();

    let actions = ["1 add", "2 add", "dup add", "3 mul", "1 add"];
    let mut best_value = i64::MIN;
    let mut best_path = String::new();

    let start = Instant::now();
    for rollout in 0..rollouts {
        session.restore(&root_snap);

        let mut path = Vec::new();
        for d in 0..depth {
            let action_idx = (rollout * 7 + d * 13) % actions.len(); // deterministic "random"
            let action = actions[action_idx];
            path.push(action);
            session.append_ops(Op::parse(action).unwrap());
            session.run().await;
        }

        let value = session.stack().values()[0].as_int().unwrap();
        if value > best_value {
            best_value = value;
            best_path = path.join(" → ");
        }
    }
    let total_time = start.elapsed();

    println!(
        "Rollouts: {}, Depth: {}, Best value: {}",
        rollouts, depth, best_value
    );
    println!("Best path: {}", best_path);
    println!(
        "Total: {}, Per rollout: {}",
        fmt_duration(total_time),
        fmt_duration(total_time / rollouts as u32)
    );
    println!(
        "Rollouts/sec: {:.0}",
        rollouts as f64 / total_time.as_secs_f64()
    );
}

// ============================================================
// Benchmark 8: Parallel MCTS (multiple trees)
// ============================================================

#[tokio::test]
async fn bench_08_parallel_mcts() {
    let ctx = setup_ctx().await;

    println!("\n{}", "=".repeat(60));
    println!("BENCHMARK 8: Parallel MCTS Trees");
    println!("{}", "=".repeat(60));

    let tree_count = 16;
    let rollouts_per_tree = 50;
    let depth = 5;
    let actions = ["1 add", "2 add", "dup add", "3 mul", "1 add"];

    // Sequential
    let start = Instant::now();
    let mut seq_best = i64::MIN;
    for tree in 0..tree_count {
        let mut session = Session::new(vec![Op::push(0)], Stack::new(), ctx.clone());
        session.run().await;
        let root = session.snapshot();

        for rollout in 0..rollouts_per_tree {
            session.restore(&root);
            for d in 0..depth {
                let idx = (tree * 11 + rollout * 7 + d * 13) % actions.len();
                session.append_ops(Op::parse(actions[idx]).unwrap());
                session.run().await;
            }
            let v = session.stack().values()[0].as_int().unwrap();
            if v > seq_best {
                seq_best = v;
            }
        }
    }
    let seq_time = start.elapsed();

    // Parallel
    let start = Instant::now();
    let handles: Vec<_> = (0..tree_count)
        .map(|tree| {
            let ctx = ctx.clone();
            tokio::spawn(async move {
                let mut session = Session::new(vec![Op::push(0)], Stack::new(), ctx);
                session.run().await;
                let root = session.snapshot();

                let mut best = i64::MIN;
                for rollout in 0..rollouts_per_tree {
                    session.restore(&root);
                    for d in 0..depth {
                        let idx = (tree * 11 + rollout * 7 + d * 13) % actions.len();
                        session.append_ops(Op::parse(actions[idx]).unwrap());
                        session.run().await;
                    }
                    let v = session.stack().values()[0].as_int().unwrap();
                    if v > best {
                        best = v;
                    }
                }
                best
            })
        })
        .collect();

    let mut par_best = i64::MIN;
    for h in handles {
        let v = h.await.unwrap();
        if v > par_best {
            par_best = v;
        }
    }
    let par_time = start.elapsed();

    let total_rollouts = tree_count * rollouts_per_tree;
    println!(
        "Trees: {}, Rollouts/tree: {}, Total rollouts: {}",
        tree_count, rollouts_per_tree, total_rollouts
    );
    println!(
        "Sequential: {} ({:.0} rollouts/sec)",
        fmt_duration(seq_time),
        total_rollouts as f64 / seq_time.as_secs_f64()
    );
    println!(
        "Parallel:   {} ({:.0} rollouts/sec)",
        fmt_duration(par_time),
        total_rollouts as f64 / par_time.as_secs_f64()
    );
    println!(
        "Speedup:    {:.2}x",
        seq_time.as_secs_f64() / par_time.as_secs_f64()
    );
    assert_eq!(seq_best, par_best, "parallel and sequential should find same best");
}

// ============================================================
// Summary: run all and print table
// ============================================================

#[tokio::test]
async fn bench_00_summary() {
    let ctx = setup_ctx().await;

    println!("\n{}", "=".repeat(70));
    println!("  KORE EXECUTION BENCHMARK SUMMARY");
    println!("  Hardware: see `lscpu` and `free -h`");
    println!("  Date: (run `date` for timestamp)");
    println!("{}\n", "=".repeat(70));

    // Quick representative benchmarks for the summary table
    let scenarios: Vec<(&str, usize)> = vec![
        ("100 ops", 100),
        ("1K ops", 1000),
        ("10K ops", 10000),
        ("50K ops", 50000),
    ];

    println!("{:<12} {:>10} {:>10} {:>12}", "workload", "executor", "session", "ops/sec");
    println!("{:-<48}", "");

    for (label, size) in &scenarios {
        let ops = make_sum_program(*size);
        let op_count = ops.len();

        // Functional executor
        let start = Instant::now();
        execute(&ops, Stack::new(), ctx.clone()).await.unwrap();
        let exec_time = start.elapsed();

        // Session.run()
        let mut session = Session::new(ops, Stack::new(), ctx.clone());
        let start = Instant::now();
        session.run().await;
        let sess_time = start.elapsed();

        let ops_sec = op_count as f64 / sess_time.as_secs_f64();
        println!(
            "{:<12} {:>10} {:>10} {:>12.0}",
            label,
            fmt_duration(exec_time),
            fmt_duration(sess_time),
            ops_sec,
        );
    }

    // Parallel scaling
    println!("\n{:<12} {:>10} {:>10} {:>8}", "parallel", "sequential", "parallel", "speedup");
    println!("{:-<44}", "");

    for &par_count in &[4, 16, 64] {
        let ops_per = 1000;

        let start = Instant::now();
        for _ in 0..par_count {
            let ops = make_sum_program(ops_per);
            let mut s = Session::new(ops, Stack::new(), ctx.clone());
            s.run().await;
        }
        let seq_time = start.elapsed();

        let start = Instant::now();
        let handles: Vec<_> = (0..par_count)
            .map(|_| {
                let ctx = ctx.clone();
                tokio::spawn(async move {
                    let ops = make_sum_program(ops_per);
                    let mut s = Session::new(ops, Stack::new(), ctx);
                    s.run().await;
                })
            })
            .collect();
        for h in handles {
            h.await.unwrap();
        }
        let par_time = start.elapsed();

        let speedup = seq_time.as_secs_f64() / par_time.as_secs_f64();
        println!(
            "{:<12} {:>10} {:>10} {:>7.2}x",
            format!("{}×1K", par_count),
            fmt_duration(seq_time),
            fmt_duration(par_time),
            speedup,
        );
    }
}
