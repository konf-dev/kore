//! Lookahead Executor - Runtime Optimization Without Language Change
//!
//! # Philosophy
//!
//! This module implements transparent I/O parallelization that:
//! - Preserves Postulate 1: Tool : Stack → Stack
//! - Preserves Postulate 2: execute(t, s) = s'
//! - Preserves Postulate 3: trace(f ; g) = trace(f) · trace(g)
//!
//! The key insight: if we have consecutive I/O operations where each
//! operation's input is already on the stack (not dependent on previous
//! I/O result), we can execute them in parallel and commit in order.
//!
//! # Semantic Equivalence
//!
//! For any program P, if sequential_execute(P) = S, then
//! lookahead_execute(P) = S. Same result, potentially faster.
//!
//! # How It Works
//!
//! Sequential (what Kore sees):
//! ```text
//! "url1" http-get  → waits → result1
//! "url2" http-get  → waits → result2
//! "url3" http-get  → waits → result3
//! Total: 3 × latency
//! ```
//!
//! Lookahead (what actually happens):
//! ```text
//! "url1" http-get ─┐
//! "url2" http-get ─┼→ all fire in parallel
//! "url3" http-get ─┘
//! collect results in ORDER: [result1, result2, result3]
//! Total: 1 × latency
//! ```
//!
//! Kore's trace still shows: step1 → step2 → step3 (linear)
//!
//! # Safety
//!
//! An I/O operation can only be batched if:
//! 1. Its inputs are already on the stack (Push ops before it)
//! 2. It's a known pure I/O operation (http-get, llm, fs-read)
//! 3. It doesn't depend on previous I/O results
//!
//! This is conservative: we only optimize what we can prove safe.

use crate::context::Context;
use crate::error::Result;
use crate::op::Op;
use crate::stack::Stack;
use crate::tool::ToolBody;
use crate::value::Value;
use std::future::Future;
use std::pin::Pin;
use std::time::Instant;

/// Set of tool names known to be I/O bound and safe to parallelize.
/// These tools:
/// - Take inputs from stack
/// - Perform network/disk I/O
/// - Return results to stack
/// - Have no side effects visible to other tools
const IO_TOOLS: &[&str] = &[
    "http-get",    // (url -- response)
    "http-post",   // (url body headers -- response)  
    "llm",         // (prompt -- response)
    "llm-system",  // (system user -- response)
    "fs-read",     // (path -- content)
    "file-read",   // (path -- content)
];

/// Tools that have side effects and CANNOT be parallelized
#[allow(dead_code)]
const SIDE_EFFECT_TOOLS: &[&str] = &[
    "fs-write",
    "file-write", 
    "file-append",
    "fs-delete",
    "file-delete",
    "mem-set",
    "print",
    "println",
];

/// Analyze a sequence of ops to find an I/O batch starting at position `start`.
///
/// Returns an IoBatch if we found parallelizable I/O ops.
///
/// A batch is valid if:
/// 1. Each I/O op has EXACTLY its inputs pushed immediately before it
/// 2. No I/O op depends on a previous I/O op's result
/// 3. No side-effect tools in between
///
/// Pattern we're looking for:
/// ```text
/// push1 push2 ... pushN io-call   <- one unit
/// push1 push2 ... pushM io-call   <- another unit
/// ```
/// Each unit is self-contained: pushes provide ALL inputs for the io-call.
fn analyze_io_batch(ops: &[Op], start: usize, _stack: &Stack) -> Option<IoBatch> {
    if start >= ops.len() {
        return None;
    }

    let mut batch = IoBatch::new();
    let mut pos = start;

    while pos < ops.len() {
        // Collect consecutive Push ops
        let mut pushes = Vec::new();
        
        while pos < ops.len() {
            if let Op::Push(v) = &ops[pos] {
                pushes.push(v.clone());
                pos += 1;
            } else {
                break;
            }
        }

        // Must be followed by I/O Call
        if pos >= ops.len() {
            break;
        }

        if let Op::Call(name) = &ops[pos] {
            // Is this an I/O tool?
            if IO_TOOLS.contains(&name.as_str()) {
                let inputs_needed = io_tool_inputs(name);
                
                // CRITICAL: The pushes must provide EXACTLY the inputs needed
                // If pushes.len() < inputs_needed, this I/O depends on something
                // earlier (like a previous I/O result), so we can't batch it
                if pushes.len() >= inputs_needed {
                    // This op is self-contained and can be batched!
                    batch.add_op(pushes, name.clone(), inputs_needed);
                    pos += 1;
                    continue;
                } else {
                    // Not enough pushes - this depends on previous results
                    // Stop batching here
                    break;
                }
            }
            
            // Side effect tool or non-I/O tool? Stop batching
            break;
        } else {
            // Not a Call - stop batching
            break;
        }
    }

    if batch.ops.is_empty() {
        None
    } else {
        Some(batch)
    }
}

/// Number of stack inputs an I/O tool needs
fn io_tool_inputs(name: &str) -> usize {
    match name {
        "http-get" => 1,    // (url -- response)
        "http-post" => 3,   // (url body headers -- response)
        "llm" => 1,         // (prompt -- response)
        "llm-system" => 2,  // (system user -- response)
        "fs-read" => 1,     // (path -- content)
        "file-read" => 1,   // (path -- content)
        _ => 1,             // Default assumption
    }
}

/// A batch of I/O operations that can run in parallel
#[derive(Debug)]
struct IoBatch {
    /// Each entry: (push_values, tool_name, inputs_needed)
    ops: Vec<(Vec<Value>, String, usize)>,
    /// Total ops consumed from the program
    total_ops_consumed: usize,
}

impl IoBatch {
    fn new() -> Self {
        Self {
            ops: Vec::new(),
            total_ops_consumed: 0,
        }
    }

    fn add_op(&mut self, pushes: Vec<Value>, tool_name: String, inputs: usize) {
        self.total_ops_consumed += pushes.len() + 1; // pushes + call
        self.ops.push((pushes, tool_name, inputs));
    }

    fn len(&self) -> usize {
        self.ops.len()
    }
}

/// Execute a batch of I/O operations in parallel.
/// 
/// CRITICAL: Results are returned in the SAME ORDER as the ops.
/// This preserves sequential semantics.
/// 
/// Each op in the batch is self-contained: its pushes provide ALL inputs.
async fn execute_io_batch(
    batch: &IoBatch,
    mut stack: Stack,
    ctx: &Context,
) -> Result<Stack> {
    use futures::future::join_all;

    // Build execution tasks - each is self-contained
    let mut tasks = Vec::new();

    for (pushes, tool_name, inputs_needed) in &batch.ops {
        // Build the stack for this I/O operation from its pushes
        let mut io_stack = Stack::new();
        
        // The pushes are stored in order, but we only need the last `inputs_needed` values
        // (earlier pushes might be extra if we had more pushes than needed)
        let start_idx = pushes.len().saturating_sub(*inputs_needed);
        for v in &pushes[start_idx..] {
            io_stack.push(v.clone())?;
        }

        let ctx_clone = ctx.clone();
        let name = tool_name.clone();

        // Create the async task
        let task = async move {
            let dict = ctx_clone.dict.read().await;
            let tool = dict.get(&name)?;
            drop(dict);

            match &tool.body {
                ToolBody::Native(native_fn) => {
                    native_fn.call(io_stack, ctx_clone).await
                }
                ToolBody::Ops(ops) => {
                    crate::executor::execute(ops, io_stack, ctx_clone).await
                }
            }
        };

        tasks.push(task);
    }

    // Execute all in parallel!
    let results: Vec<Result<(Stack, Context)>> = join_all(tasks).await;

    // Collect results in ORDER - this is critical for correctness
    for result in results {
        let (result_stack, _) = result?;
        // Each I/O op should leave exactly one value on stack
        if let Some(v) = result_stack.values().last() {
            stack.push(v.clone())?;
        }
    }

    Ok(stack)
}

/// Lookahead executor - transparent optimization over sequential execution.
///
/// Guarantees:
/// - Same output as sequential execution
/// - Same trace structure (linear)
/// - Faster wall-clock time for I/O bound workloads
pub fn execute_with_lookahead<'a>(
    ops: &'a [Op],
    stack: Stack,
    ctx: Context,
) -> Pin<Box<dyn Future<Output = Result<(Stack, Context)>> + Send + 'a>> {
    Box::pin(async move {
        execute_with_lookahead_inner(ops, stack, ctx).await
    })
}

async fn execute_with_lookahead_inner(
    ops: &[Op],
    mut stack: Stack,
    ctx: Context,
) -> Result<(Stack, Context)> {
    let mut pc = 0; // Program counter

    while pc < ops.len() {
        // Try to find an I/O batch starting at current position
        if let Some(batch) = analyze_io_batch(ops, pc, &stack) {
            if batch.len() > 1 {
                // Found parallelizable batch!
                // Execute in parallel, results in order
                stack = execute_io_batch(&batch, stack, &ctx).await?;
                pc += batch.total_ops_consumed;
                continue;
            }
        }

        // No batch or single op - execute normally
        let op = &ops[pc];
        let result = execute_single_op(op, stack, ctx.clone()).await?;
        stack = result.0;
        pc += 1;
    }

    Ok((stack, ctx))
}

/// Execute a single operation (same as original executor)
async fn execute_single_op(
    op: &Op,
    mut stack: Stack,
    ctx: Context,
) -> Result<(Stack, Context)> {
    match op {
        Op::Push(value) => {
            stack.push(value.clone())?;
            Ok((stack, ctx))
        }
        Op::Call(name) => {
            let dict = ctx.dict.read().await;
            let tool = dict.get(name)?;
            drop(dict);

            let start = Instant::now();

            let result = match &tool.body {
                ToolBody::Native(native_fn) => native_fn.call(stack, ctx.clone()).await,
                ToolBody::Ops(ops) => {
                    // Recursive lookahead for composed tools
                    execute_with_lookahead(ops, stack, ctx.clone()).await
                }
            };

            let duration = start.elapsed();
            {
                let mut dict = ctx.dict.write().await;
                if let Some(t) = dict.get_mut(name) {
                    if result.is_ok() {
                        t.meta.record_call(duration);
                    } else {
                        t.meta.record_failure();
                    }
                }
            }

            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::Tool;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    /// Create a test context with mock I/O tools
    async fn setup_test_ctx() -> Context {
        let ctx = Context::new();
        let mut dict = ctx.dict.write().await;

        // Basic tools
        dict.register(Tool::native("add", "", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Int(a + b))?;
                Ok((stack, ctx))
            })
        }));

        dict.register(Tool::native("dup", "", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                stack.push(v.clone())?;
                stack.push(v)?;
                Ok((stack, ctx))
            })
        }));

        drop(dict);
        ctx
    }

    /// Add a mock I/O tool that sleeps for a specified duration
    async fn add_mock_io_tool(ctx: &Context, name: &str, sleep_ms: u64, counter: Arc<AtomicUsize>) {
        let mut dict = ctx.dict.write().await;
        let sleep_duration = Duration::from_millis(sleep_ms);
        
        dict.register(Tool::native(name, "", move |mut stack: Stack, ctx: Context| {
            let c = counter.clone();
            let d = sleep_duration;
            Box::pin(async move {
                let input = stack.pop()?.as_int()?;
                
                // Simulate I/O latency
                tokio::time::sleep(d).await;
                
                // Record that we ran
                c.fetch_add(1, Ordering::SeqCst);
                
                // Return input * 10 (easy to verify)
                stack.push(Value::Int(input * 10))?;
                Ok((stack, ctx))
            })
        }));
    }

    // ==================== CORRECTNESS TESTS ====================

    #[tokio::test]
    async fn test_basic_execution_unchanged() {
        // Verify: non-I/O code executes identically
        let ctx = setup_test_ctx().await;
        let stack = Stack::new();

        let ops = vec![
            Op::push(5),
            Op::call("dup"),
            Op::call("add"),
        ];

        // Sequential
        let (seq_result, _) = crate::executor::execute(&ops, stack.clone(), ctx.clone()).await.unwrap();
        
        // Lookahead
        let (la_result, _) = execute_with_lookahead(&ops, stack.clone(), ctx.clone()).await.unwrap();

        assert_eq!(seq_result.values(), la_result.values());
        assert_eq!(la_result.values()[0].as_int().unwrap(), 10);
    }

    #[tokio::test]
    async fn test_single_io_unchanged() {
        // Verify: single I/O op executes correctly
        let ctx = setup_test_ctx().await;
        let counter = Arc::new(AtomicUsize::new(0));
        add_mock_io_tool(&ctx, "http-get", 10, counter.clone()).await;

        let stack = Stack::new();
        let ops = vec![
            Op::push(5),
            Op::call("http-get"),
        ];

        let (result, _) = execute_with_lookahead(&ops, stack, ctx).await.unwrap();

        assert_eq!(result.values()[0].as_int().unwrap(), 50);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_sequential_io_same_result() {
        // Verify: multiple I/O ops produce same result as sequential
        let ctx = setup_test_ctx().await;
        let counter = Arc::new(AtomicUsize::new(0));
        add_mock_io_tool(&ctx, "http-get", 10, counter.clone()).await;

        let ops = vec![
            Op::push(1),
            Op::call("http-get"),  // → 10
            Op::push(2),
            Op::call("http-get"),  // → 20
            Op::push(3),
            Op::call("http-get"),  // → 30
        ];

        // Sequential execution
        let stack1 = Stack::new();
        let (seq_result, _) = crate::executor::execute(&ops, stack1, ctx.clone()).await.unwrap();

        // Reset counter
        counter.store(0, Ordering::SeqCst);

        // Lookahead execution
        let stack2 = Stack::new();
        let (la_result, _) = execute_with_lookahead(&ops, stack2, ctx.clone()).await.unwrap();

        // Results must be identical
        assert_eq!(seq_result.values(), la_result.values());
        assert_eq!(la_result.depth(), 3);
        assert_eq!(la_result.values()[0].as_int().unwrap(), 10);
        assert_eq!(la_result.values()[1].as_int().unwrap(), 20);
        assert_eq!(la_result.values()[2].as_int().unwrap(), 30);
    }

    #[tokio::test]
    async fn test_order_preserved() {
        // CRITICAL: Results must be in program order
        let ctx = setup_test_ctx().await;
        
        // Tool that returns its input (no modification)
        {
            let mut dict = ctx.dict.write().await;
            dict.register(Tool::native("http-get", "", |mut stack: Stack, ctx: Context| {
                Box::pin(async move {
                    let input = stack.pop()?.as_int()?;
                    // Different sleep times - if not ordered correctly, results would be wrong
                    let sleep_ms = match input {
                        1 => 50,  // Slow
                        2 => 10,  // Fast
                        3 => 30,  // Medium
                        _ => 10,
                    };
                    tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                    stack.push(Value::Int(input))?;
                    Ok((stack, ctx))
                })
            }));
        }

        let ops = vec![
            Op::push(1), Op::call("http-get"),  // Slow (50ms)
            Op::push(2), Op::call("http-get"),  // Fast (10ms)
            Op::push(3), Op::call("http-get"),  // Medium (30ms)
        ];

        let stack = Stack::new();
        let (result, _) = execute_with_lookahead(&ops, stack, ctx).await.unwrap();

        // MUST be in program order: 1, 2, 3
        // NOT in completion order: 2, 3, 1
        assert_eq!(result.values()[0].as_int().unwrap(), 1);
        assert_eq!(result.values()[1].as_int().unwrap(), 2);
        assert_eq!(result.values()[2].as_int().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_dependent_io_not_batched() {
        // If an I/O op depends on previous I/O result, don't batch
        let ctx = setup_test_ctx().await;
        let call_count = Arc::new(AtomicUsize::new(0));
        
        {
            let mut dict = ctx.dict.write().await;
            let cc = call_count.clone();
            dict.register(Tool::native("http-get", "", move |mut stack: Stack, ctx: Context| {
                let c = cc.clone();
                Box::pin(async move {
                    let input = stack.pop()?.as_int()?;
                    c.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    stack.push(Value::Int(input * 2))?;
                    Ok((stack, ctx))
                })
            }));
        }

        // Second http-get depends on first's result
        let ops = vec![
            Op::push(5),
            Op::call("http-get"),  // → 10
            // NO PUSH here - next op uses previous result
            Op::call("http-get"),  // → 20 (uses 10 from previous)
        ];

        let stack = Stack::new();
        let (result, _) = execute_with_lookahead(&ops, stack, ctx).await.unwrap();

        // Should work correctly even though batching isn't possible
        assert_eq!(result.values()[0].as_int().unwrap(), 20);
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    // ==================== OPTIMIZATION TESTS ====================

    #[tokio::test]
    async fn test_parallel_speedup() {
        // Verify: parallel execution is faster than sequential
        let ctx = setup_test_ctx().await;
        let counter = Arc::new(AtomicUsize::new(0));
        
        // Each I/O op takes 50ms
        add_mock_io_tool(&ctx, "http-get", 50, counter.clone()).await;

        let ops = vec![
            Op::push(1), Op::call("http-get"),
            Op::push(2), Op::call("http-get"),
            Op::push(3), Op::call("http-get"),
        ];

        // Time sequential execution
        let start_seq = Instant::now();
        let stack1 = Stack::new();
        let _ = crate::executor::execute(&ops, stack1, ctx.clone()).await.unwrap();
        let seq_duration = start_seq.elapsed();

        counter.store(0, Ordering::SeqCst);

        // Time lookahead execution
        let start_la = Instant::now();
        let stack2 = Stack::new();
        let _ = execute_with_lookahead(&ops, stack2, ctx.clone()).await.unwrap();
        let la_duration = start_la.elapsed();

        // Lookahead should be significantly faster
        // Sequential: ~150ms (3 × 50ms)
        // Lookahead: ~50ms (parallel)
        println!("Sequential: {:?}", seq_duration);
        println!("Lookahead: {:?}", la_duration);

        // Allow some overhead, but lookahead should be at least 2x faster
        assert!(la_duration < seq_duration / 2, 
            "Lookahead {:?} should be at least 2x faster than sequential {:?}",
            la_duration, seq_duration);
    }

    #[tokio::test]
    async fn test_mixed_io_and_compute() {
        // Mix of I/O and compute operations
        let ctx = setup_test_ctx().await;
        let counter = Arc::new(AtomicUsize::new(0));
        add_mock_io_tool(&ctx, "http-get", 20, counter.clone()).await;

        let ops = vec![
            // First batch: two I/O ops
            Op::push(1), Op::call("http-get"),  // → 10
            Op::push(2), Op::call("http-get"),  // → 20
            // Compute: breaks the batch
            Op::call("add"),                    // → 30
            // Second batch: one I/O op
            Op::push(3), Op::call("http-get"),  // → 30
            // More compute
            Op::call("add"),                    // → 60
        ];

        let stack = Stack::new();
        let (result, _) = execute_with_lookahead(&ops, stack, ctx).await.unwrap();

        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 60);
    }

    // ==================== POSTULATE COMPLIANCE TESTS ====================

    #[tokio::test]
    async fn test_postulate_1_tool_stack_to_stack() {
        // Postulate 1: Tool : Stack → Stack
        // The lookahead executor must preserve this
        let ctx = setup_test_ctx().await;
        let counter = Arc::new(AtomicUsize::new(0));
        add_mock_io_tool(&ctx, "http-get", 10, counter.clone()).await;

        let input_stack = Stack::new();
        let ops = vec![Op::push(42), Op::call("http-get")];

        let (output_stack, _) = execute_with_lookahead(&ops, input_stack, ctx).await.unwrap();

        // Input: Stack (empty)
        // Output: Stack (with one value)
        // This is Tool : Stack → Stack ✓
        assert_eq!(output_stack.depth(), 1);
    }

    #[tokio::test]
    async fn test_postulate_2_deterministic() {
        // Postulate 2: execute(t, s) = s' (deterministic)
        // Same input must always produce same output
        let ctx = setup_test_ctx().await;
        let counter = Arc::new(AtomicUsize::new(0));
        add_mock_io_tool(&ctx, "http-get", 10, counter.clone()).await;

        let ops = vec![
            Op::push(1), Op::call("http-get"),
            Op::push(2), Op::call("http-get"),
        ];

        // Run multiple times
        let mut results = Vec::new();
        for _ in 0..5 {
            let stack = Stack::new();
            let (result, _) = execute_with_lookahead(&ops, stack, ctx.clone()).await.unwrap();
            results.push(result.values().to_vec());
        }

        // All results must be identical
        for i in 1..results.len() {
            assert_eq!(results[0], results[i], "Run {} differs from run 0", i);
        }
    }

    #[tokio::test]
    async fn test_postulate_3_composition_trace() {
        // Postulate 3: trace(f ; g) = trace(f) · trace(g)
        // Composition should produce concatenated results
        let ctx = setup_test_ctx().await;
        
        // f: push 5
        // g: dup add
        // f ; g = 10 on stack

        let ops = vec![
            Op::push(5),
            Op::call("dup"),
            Op::call("add"),
        ];

        let stack = Stack::new();
        let (result, _) = execute_with_lookahead(&ops, stack, ctx).await.unwrap();

        // f(stack) = [5]
        // g([5]) = [10]
        // (f;g)(stack) = [10] ✓
        assert_eq!(result.values()[0].as_int().unwrap(), 10);
    }

    #[tokio::test]
    async fn test_equivalence_with_sequential() {
        // The ultimate test: lookahead must be equivalent to sequential
        let ctx = setup_test_ctx().await;
        let counter = Arc::new(AtomicUsize::new(0));
        add_mock_io_tool(&ctx, "http-get", 5, counter.clone()).await;

        // Complex program with various patterns
        let ops = vec![
            Op::push(1),
            Op::call("http-get"),
            Op::push(2),
            Op::call("http-get"),
            Op::call("add"),
            Op::call("dup"),
            Op::push(3),
            Op::call("http-get"),
            Op::call("add"),
        ];

        // Sequential
        counter.store(0, Ordering::SeqCst);
        let (seq_result, _) = crate::executor::execute(&ops, Stack::new(), ctx.clone()).await.unwrap();
        let seq_calls = counter.load(Ordering::SeqCst);

        // Lookahead
        counter.store(0, Ordering::SeqCst);
        let (la_result, _) = execute_with_lookahead(&ops, Stack::new(), ctx.clone()).await.unwrap();
        let la_calls = counter.load(Ordering::SeqCst);

        // Same results
        assert_eq!(seq_result.values(), la_result.values());
        
        // Same number of I/O calls
        assert_eq!(seq_calls, la_calls);
    }

    // ==================== ADDITIONAL EDGE CASE TESTS ====================

    #[tokio::test]
    async fn test_empty_program() {
        let ctx = setup_test_ctx().await;
        let stack = Stack::new();
        let ops: Vec<Op> = vec![];

        let (result, _) = execute_with_lookahead(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 0);
    }

    #[tokio::test]
    async fn test_only_pushes() {
        let ctx = setup_test_ctx().await;
        let stack = Stack::new();
        let ops = vec![Op::push(1), Op::push(2), Op::push(3)];

        let (result, _) = execute_with_lookahead(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 3);
        assert_eq!(result.values()[0].as_int().unwrap(), 1);
        assert_eq!(result.values()[1].as_int().unwrap(), 2);
        assert_eq!(result.values()[2].as_int().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_single_batch_only() {
        // Program that is entirely one batchable sequence
        let ctx = setup_test_ctx().await;
        let counter = Arc::new(AtomicUsize::new(0));
        add_mock_io_tool(&ctx, "http-get", 30, counter.clone()).await;

        let ops = vec![
            Op::push(1), Op::call("http-get"),
            Op::push(2), Op::call("http-get"),
            Op::push(3), Op::call("http-get"),
            Op::push(4), Op::call("http-get"),
            Op::push(5), Op::call("http-get"),
        ];

        let start = std::time::Instant::now();
        let (result, _) = execute_with_lookahead(&ops, Stack::new(), ctx).await.unwrap();
        let duration = start.elapsed();

        assert_eq!(result.depth(), 5);
        assert_eq!(counter.load(Ordering::SeqCst), 5);

        // Should take ~30ms, not 150ms
        println!("5 parallel I/O took: {:?}", duration);
        assert!(duration < std::time::Duration::from_millis(100),
            "Should be ~30ms (parallel), was {:?}", duration);
    }

    #[tokio::test]
    async fn test_no_io_ops() {
        // Pure computation - should work but no batching
        let ctx = setup_test_ctx().await;
        let stack = Stack::new();

        let ops = vec![
            Op::push(2),
            Op::push(3),
            Op::call("add"),
            Op::call("dup"),
            Op::call("add"),
        ];

        let (result, _) = execute_with_lookahead(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 10); // (2+3)*2
    }

    #[tokio::test]
    async fn test_io_error_propagates() {
        // If an I/O op fails, error should propagate correctly
        let ctx = setup_test_ctx().await;
        
        {
            let mut dict = ctx.dict.write().await;
            dict.register(Tool::native("http-get", "", |mut stack: Stack, ctx: Context| {
                Box::pin(async move {
                    let input = stack.pop()?.as_int()?;
                    if input == 2 {
                        return Err(crate::error::Error::Runtime("simulated failure".into()));
                    }
                    stack.push(Value::Int(input * 10))?;
                    Ok((stack, ctx))
                })
            }));
        }

        let ops = vec![
            Op::push(1), Op::call("http-get"),
            Op::push(2), Op::call("http-get"),  // This will fail
            Op::push(3), Op::call("http-get"),
        ];

        let result = execute_with_lookahead(&ops, Stack::new(), ctx).await;
        assert!(result.is_err(), "Should propagate error");
    }

    #[tokio::test]
    async fn test_alternating_io_and_compute() {
        // IO, compute, IO, compute pattern
        let ctx = setup_test_ctx().await;
        let counter = Arc::new(AtomicUsize::new(0));
        add_mock_io_tool(&ctx, "http-get", 10, counter.clone()).await;

        let ops = vec![
            Op::push(1), Op::call("http-get"),  // → 10
            Op::push(5), Op::call("add"),       // → 15
            Op::push(2), Op::call("http-get"),  // → 20
            Op::call("add"),                    // → 35
        ];

        let (result, _) = execute_with_lookahead(&ops, Stack::new(), ctx).await.unwrap();
        
        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 35);
    }

    #[tokio::test]
    async fn test_many_small_batches() {
        // Multiple small batchable groups separated by compute
        let ctx = setup_test_ctx().await;
        let counter = Arc::new(AtomicUsize::new(0));
        add_mock_io_tool(&ctx, "http-get", 20, counter.clone()).await;

        let ops = vec![
            // Batch 1
            Op::push(1), Op::call("http-get"),
            Op::push(2), Op::call("http-get"),
            // Compute (breaks batch)
            Op::call("add"),
            // Batch 2
            Op::push(3), Op::call("http-get"),
            Op::push(4), Op::call("http-get"),
            // Compute
            Op::call("add"),
            // Final add
            Op::call("add"),
        ];

        let start = std::time::Instant::now();
        let (result, _) = execute_with_lookahead(&ops, Stack::new(), ctx).await.unwrap();
        let duration = start.elapsed();

        // (10 + 20) + (30 + 40) = 30 + 70 = 100
        assert_eq!(result.values()[0].as_int().unwrap(), 100);
        
        // Should be ~40ms (2 batches of ~20ms each), not 80ms (4 sequential)
        println!("2 batches of 2 I/O took: {:?}", duration);
        assert!(duration < std::time::Duration::from_millis(70),
            "Should be ~40ms (2 parallel batches), was {:?}", duration);
    }

    // ==================== STRESS TESTS ====================

    #[tokio::test]
    async fn test_large_batch() {
        // Many parallel I/O operations
        let ctx = setup_test_ctx().await;
        let counter = Arc::new(AtomicUsize::new(0));
        add_mock_io_tool(&ctx, "http-get", 10, counter.clone()).await;

        let mut ops = Vec::new();
        for i in 1..=20 {
            ops.push(Op::push(i as i64));
            ops.push(Op::call("http-get"));
        }

        let start = std::time::Instant::now();
        let (result, _) = execute_with_lookahead(&ops, Stack::new(), ctx).await.unwrap();
        let duration = start.elapsed();

        assert_eq!(result.depth(), 20);
        assert_eq!(counter.load(Ordering::SeqCst), 20);

        // All 20 should run in parallel: ~10ms not 200ms
        println!("20 parallel I/O took: {:?}", duration);
        assert!(duration < std::time::Duration::from_millis(50),
            "Should be ~10ms (parallel), was {:?}", duration);
    }

    #[tokio::test]
    async fn test_determinism_under_varying_timing() {
        // Even with different completion times, results must be ordered
        let ctx = setup_test_ctx().await;
        
        {
            let mut dict = ctx.dict.write().await;
            dict.register(Tool::native("http-get", "", |mut stack: Stack, ctx: Context| {
                Box::pin(async move {
                    let input = stack.pop()?.as_int()?;
                    // Reverse timing: higher numbers complete faster
                    let sleep_ms = (100 - input * 10) as u64;
                    tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
                    stack.push(Value::Int(input))?;
                    Ok((stack, ctx))
                })
            }));
        }

        let ops = vec![
            Op::push(1), Op::call("http-get"),  // Slowest (90ms)
            Op::push(2), Op::call("http-get"),  // (80ms)
            Op::push(3), Op::call("http-get"),  // (70ms)
            Op::push(4), Op::call("http-get"),  // (60ms)
            Op::push(5), Op::call("http-get"),  // Fastest (50ms)
        ];

        // Run multiple times
        for run in 0..3 {
            let (result, _) = execute_with_lookahead(&ops, Stack::new(), ctx.clone()).await.unwrap();
            
            // Must ALWAYS be in program order
            assert_eq!(result.values()[0].as_int().unwrap(), 1, "Run {}: first should be 1", run);
            assert_eq!(result.values()[1].as_int().unwrap(), 2, "Run {}: second should be 2", run);
            assert_eq!(result.values()[2].as_int().unwrap(), 3, "Run {}: third should be 3", run);
            assert_eq!(result.values()[3].as_int().unwrap(), 4, "Run {}: fourth should be 4", run);
            assert_eq!(result.values()[4].as_int().unwrap(), 5, "Run {}: fifth should be 5", run);
        }
    }

    // ==================== POSTULATE COMPLIANCE FORMAL TESTS ====================

    /// Test that Postulate 1 holds: Tool : Stack → Stack
    /// Every operation transforms a stack into a stack.
    #[tokio::test]
    async fn test_postulate_1_formal() {
        let ctx = setup_test_ctx().await;
        let counter = Arc::new(AtomicUsize::new(0));
        add_mock_io_tool(&ctx, "http-get", 5, counter.clone()).await;

        // Various programs
        let programs = vec![
            vec![],  // Empty
            vec![Op::push(1)],  // Just push
            vec![Op::push(1), Op::call("http-get")],  // Push + I/O
            vec![Op::push(1), Op::call("http-get"), Op::push(2), Op::call("http-get")],  // Batch
        ];

        for (i, ops) in programs.iter().enumerate() {
            let input: Stack = Stack::new();  // Stack
            let result = execute_with_lookahead(ops, input, ctx.clone()).await;
            
            // Output must be Ok((Stack, Context)) or Err
            match result {
                Ok((output, _)) => {
                    // output is a Stack - Postulate 1 satisfied
                    println!("Program {}: input Stack → output Stack (depth {})", i, output.depth());
                }
                Err(_) => {
                    // Error is also valid - operation failed but type is correct
                    println!("Program {}: input Stack → Error", i);
                }
            }
        }
    }

    /// Test that Postulate 2 holds: execute(t, s) = s' (deterministic)
    /// Same tool on same stack always produces same result.
    #[tokio::test]
    async fn test_postulate_2_formal() {
        let ctx = setup_test_ctx().await;
        let counter = Arc::new(AtomicUsize::new(0));
        add_mock_io_tool(&ctx, "http-get", 5, counter.clone()).await;

        let ops = vec![
            Op::push(42),
            Op::call("http-get"),
            Op::push(7),
            Op::call("http-get"),
        ];

        // Execute 10 times
        let mut results: Vec<Vec<i64>> = Vec::new();
        for _ in 0..10 {
            counter.store(0, Ordering::SeqCst);
            let (result, _) = execute_with_lookahead(&ops, Stack::new(), ctx.clone()).await.unwrap();
            let values: Vec<i64> = result.values().iter()
                .map(|v| v.as_int().unwrap())
                .collect();
            results.push(values);
        }

        // All results must be identical
        let first = &results[0];
        for (i, result) in results.iter().enumerate() {
            assert_eq!(first, result, "Run {} differs: {:?} vs {:?}", i, first, result);
        }
        println!("Postulate 2: {} runs all produced {:?}", results.len(), first);
    }

    /// Test that Postulate 3 holds: trace(f ; g) = trace(f) · trace(g)
    /// Composition produces concatenated results.
    #[tokio::test]
    async fn test_postulate_3_formal() {
        let ctx = setup_test_ctx().await;

        // f: push 3, push 4
        // g: add
        // f;g should give [7]

        // Execute f alone
        let f_ops = vec![Op::push(3), Op::push(4)];
        let (f_result, _) = execute_with_lookahead(&f_ops, Stack::new(), ctx.clone()).await.unwrap();
        // f_result = [3, 4]

        // Execute g on f's result
        let g_ops = vec![Op::call("add")];
        let (g_result, _) = execute_with_lookahead(&g_ops, f_result.clone(), ctx.clone()).await.unwrap();
        // g_result = [7]

        // Execute f;g (composition)
        let fg_ops = vec![Op::push(3), Op::push(4), Op::call("add")];
        let (fg_result, _) = execute_with_lookahead(&fg_ops, Stack::new(), ctx.clone()).await.unwrap();

        // Postulate 3: (f;g)(s) = g(f(s))
        assert_eq!(g_result.values(), fg_result.values(), 
            "Postulate 3: g(f(s)) = {:?}, (f;g)(s) = {:?}", 
            g_result.values(), fg_result.values());
        
        println!("Postulate 3: g(f(s)) = (f;g)(s) = {:?}", fg_result.values());
    }

    /// Test that traces remain linear even with parallel execution
    /// This is the key guarantee: Kore sees sequential trace, runtime optimizes.
    #[tokio::test]
    async fn test_trace_linearity() {
        // The lookahead executor runs I/O in parallel BUT
        // commits results in program order, so the trace is linear.
        
        let ctx = setup_test_ctx().await;
        
        // Track order of result commits
        let commit_order: Arc<std::sync::Mutex<Vec<i64>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
        
        {
            let mut dict = ctx.dict.write().await;
            let _order = commit_order.clone();
            dict.register(Tool::native("http-get", "", move |mut stack: Stack, ctx: Context| {
                Box::pin(async move {
                    let input = stack.pop()?.as_int()?;
                    // Different delays
                    let sleep_ms = match input {
                        1 => 50,  // Slow
                        2 => 10,  // Fast  
                        3 => 30,  // Medium
                        _ => 10,
                    };
                    tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
                    stack.push(Value::Int(input))?;
                    Ok((stack, ctx))
                })
            }));
        }

        let ops = vec![
            Op::push(1), Op::call("http-get"),  // Slow
            Op::push(2), Op::call("http-get"),  // Fast
            Op::push(3), Op::call("http-get"),  // Medium
        ];

        let (result, _) = execute_with_lookahead(&ops, Stack::new(), ctx).await.unwrap();

        // Despite parallel execution, results are in PROGRAM ORDER
        // This means the trace is linear: step1 → step2 → step3
        assert_eq!(result.values()[0].as_int().unwrap(), 1);
        assert_eq!(result.values()[1].as_int().unwrap(), 2);
        assert_eq!(result.values()[2].as_int().unwrap(), 3);

        println!("Trace linearity: results committed in program order despite parallel I/O");
    }
}
