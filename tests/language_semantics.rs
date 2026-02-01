//! # Language Semantics Tests
//!
//! These tests verify that Kore is a theoretically and practically sound
//! programming language. They test fundamental properties that any correct
//! implementation must satisfy.

use kore::{Context, Stack, Op, Value, execute, register_builtins, Tool};

/// Helper to run a program and get the final stack
async fn run(ops: Vec<Op>) -> Vec<Value> {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    let stack = Stack::new();
    let (result, _) = execute(&ops, stack, ctx).await.unwrap();
    result.values().to_vec()
}

/// Helper to run a program with additional tools
async fn run_with_tools(ops: Vec<Op>, tools: Vec<Tool>) -> Vec<Value> {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    for tool in tools {
        ctx.dict.write().await.register(tool);
    }
    let stack = Stack::new();
    let (result, _) = execute(&ops, stack, ctx).await.unwrap();
    result.values().to_vec()
}

/// Helper that expects an error
async fn run_expect_error(ops: Vec<Op>) -> String {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    let stack = Stack::new();
    match execute(&ops, stack, ctx).await {
        Err(e) => e.to_string(),
        Ok(_) => panic!("Expected error but got success"),
    }
}

// ============================================================================
// STACK INVARIANTS
// ============================================================================

#[tokio::test]
async fn stack_is_lifo() {
    // Push 1, 2, 3 - should be [1, 2, 3] with 3 on top
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::push(3),
    ]).await;
    
    // Values are stored bottom-to-top
    assert_eq!(result, vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
}

#[tokio::test]
async fn empty_program_produces_empty_stack() {
    let result = run(vec![]).await;
    assert!(result.is_empty());
}

#[tokio::test]
async fn push_is_non_destructive() {
    // Pushing should never remove existing values
    let result = run(vec![
        Op::push(1),
        Op::push(2),
    ]).await;
    
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], Value::Int(1)); // Original still there
}

// ============================================================================
// QUOTE SEMANTICS
// ============================================================================

#[tokio::test]
async fn quote_is_value() {
    // A quote by itself is just a value on the stack
    let result = run(vec![
        Op::quote(vec![Op::push(42)]),
    ]).await;
    
    assert_eq!(result.len(), 1);
    assert!(result[0].as_quote().is_ok());
}

#[tokio::test]
async fn quote_defers_execution() {
    // The ops inside a quote should not execute until called
    let result = run(vec![
        Op::quote(vec![
            Op::call("nonexistent-tool"), // Would fail if executed
        ]),
    ]).await;
    
    // Quote is on stack, no error
    assert_eq!(result.len(), 1);
    assert!(result[0].as_quote().is_ok());
}

#[tokio::test]
async fn nested_quotes_preserve_structure() {
    // Quotes can contain quotes
    let result = run(vec![
        Op::quote(vec![
            Op::quote(vec![Op::push(1)]),
        ]),
    ]).await;
    
    assert_eq!(result.len(), 1);
    assert!(result[0].as_quote().is_ok());
}

#[tokio::test]
async fn call_executes_quote() {
    // call should execute a quote's contents
    let result = run(vec![
        Op::quote(vec![Op::push(42)]),
        Op::call("call"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}

// ============================================================================
// CONDITIONAL SEMANTICS
// ============================================================================

#[tokio::test]
async fn if_true_executes_then_branch() {
    let result = run(vec![
        Op::push(true),
        Op::if_then_else(
            vec![Op::push(1)],
            vec![Op::push(2)],
        ),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(1)]);
}

#[tokio::test]
async fn if_false_executes_else_branch() {
    let result = run(vec![
        Op::push(false),
        Op::if_then_else(
            vec![Op::push(1)],
            vec![Op::push(2)],
        ),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(2)]);
}

#[tokio::test]
async fn if_consumes_condition() {
    // The boolean should be consumed by if
    let result = run(vec![
        Op::push(999),
        Op::push(true),
        Op::if_then_else(vec![], vec![]),
    ]).await;
    
    // Only 999 remains, true was consumed
    assert_eq!(result, vec![Value::Int(999)]);
}

#[tokio::test]
async fn truthy_values() {
    // Non-false, non-null, non-zero, non-empty values are truthy
    
    // Int 1 is truthy
    let result = run(vec![
        Op::push(1),
        Op::if_then_else(vec![Op::push("yes")], vec![Op::push("no")]),
    ]).await;
    assert_eq!(result, vec![Value::Text("yes".into())]);
    
    // Int 0 is falsy
    let result = run(vec![
        Op::push(0),
        Op::if_then_else(vec![Op::push("yes")], vec![Op::push("no")]),
    ]).await;
    assert_eq!(result, vec![Value::Text("no".into())]);
    
    // Non-empty string is truthy
    let result = run(vec![
        Op::push("hello"),
        Op::if_then_else(vec![Op::push("yes")], vec![Op::push("no")]),
    ]).await;
    assert_eq!(result, vec![Value::Text("yes".into())]);
    
    // Empty string is falsy
    let result = run(vec![
        Op::push(""),
        Op::if_then_else(vec![Op::push("yes")], vec![Op::push("no")]),
    ]).await;
    assert_eq!(result, vec![Value::Text("no".into())]);
}

// ============================================================================
// ERROR HANDLING SEMANTICS
// ============================================================================

#[tokio::test]
async fn try_captures_error() {
    // try should capture errors as Error values
    let result = run(vec![
        Op::quote(vec![Op::call("nonexistent")]),
        Op::call("try"),
    ]).await;
    
    assert_eq!(result.len(), 1);
    assert!(result[0].as_error().is_ok());
}

#[tokio::test]
async fn try_returns_value_on_success() {
    let result = run(vec![
        Op::quote(vec![Op::push(42)]),
        Op::call("try"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}

#[tokio::test]
async fn is_error_distinguishes_errors() {
    // Error value
    let result = run(vec![
        Op::quote(vec![Op::call("nonexistent")]),
        Op::call("try"),
        Op::call("is-error"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
    
    // Non-error value
    let result = run(vec![
        Op::push(42),
        Op::call("is-error"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(false)]);
}

#[tokio::test]
async fn unwrap_extracts_value() {
    let result = run(vec![
        Op::push(42),
        Op::call("unwrap"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}

#[tokio::test]
async fn unwrap_stops_on_error() {
    let err = run_expect_error(vec![
        Op::quote(vec![Op::call("nonexistent")]),
        Op::call("try"),
        Op::call("unwrap"),
    ]).await;
    
    assert!(err.contains("Tool not found") || err.contains("nonexistent"));
}

#[tokio::test]
async fn errors_are_inspectable() {
    // Full error handling pattern
    let result = run(vec![
        // Try something that fails
        Op::quote(vec![Op::call("nonexistent")]),
        Op::call("try"),
        
        // Check if error
        Op::call("dup"),
        Op::call("is-error"),
        
        // Branch based on error
        Op::if_then_else(
            vec![Op::call("drop"), Op::push("recovered")],
            vec![Op::call("unwrap")],
        ),
    ]).await;
    
    assert_eq!(result, vec![Value::Text("recovered".into())]);
}

// ============================================================================
// STACK MANIPULATION
// ============================================================================

#[tokio::test]
async fn dup_duplicates() {
    let result = run(vec![
        Op::push(42),
        Op::call("dup"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42), Value::Int(42)]);
}

#[tokio::test]
async fn drop_removes() {
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::call("drop"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(1)]);
}

#[tokio::test]
async fn swap_swaps() {
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::call("swap"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(2), Value::Int(1)]);
}

#[tokio::test]
async fn over_copies_second() {
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::call("over"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(1), Value::Int(2), Value::Int(1)]);
}

#[tokio::test]
async fn rot_rotates() {
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::push(3),
        Op::call("rot"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(2), Value::Int(3), Value::Int(1)]);
}

// ============================================================================
// COMPOSITION (TURING COMPLETENESS)
// ============================================================================

#[tokio::test]
async fn tools_can_be_composed() {
    // A composed tool is just a sequence of ops
    let double = Tool::composed(
        "double",
        None,
        vec![Op::call("dup"), Op::call("add")],
    );
    
    let add = Tool::native("add", "(int int -- int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a + b))?;
            Ok((stack, ctx))
        })
    });
    
    let result = run_with_tools(
        vec![Op::push(21), Op::call("double")],
        vec![double, add],
    ).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}

#[tokio::test]
async fn recursive_tool_works() {
    // factorial = dup 1 <= (drop 1) (dup 1 - factorial *) if
    
    let sub = Tool::native("-", "(int int -- int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a - b))?;
            Ok((stack, ctx))
        })
    });
    
    let mul = Tool::native("*", "(int int -- int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a * b))?;
            Ok((stack, ctx))
        })
    });
    
    let le = Tool::native("<=", "(int int -- bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Bool(a <= b))?;
            Ok((stack, ctx))
        })
    });
    
    // factorial = dup 1 <= (drop 1) (dup 1 - factorial *) if
    let factorial = Tool::composed(
        "factorial",
        None,
        vec![
            Op::call("dup"),
            Op::push(1),
            Op::call("<="),
            Op::if_then_else(
                vec![Op::call("drop"), Op::push(1)],
                vec![
                    Op::call("dup"),
                    Op::push(1),
                    Op::call("-"),
                    Op::call("factorial"),
                    Op::call("*"),
                ],
            ),
        ],
    );
    
    let result = run_with_tools(
        vec![Op::push(5), Op::call("factorial")],
        vec![sub, mul, le, factorial],
    ).await;
    
    assert_eq!(result, vec![Value::Int(120)]); // 5! = 120
}

#[tokio::test]
async fn looping_via_recursion() {
    // sum-to: n -> n + (n-1) + ... + 1
    // sum-to = dup 1 <= () (dup 1 - sum-to +) if
    
    let sub = Tool::native("-", "(int int -- int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a - b))?;
            Ok((stack, ctx))
        })
    });
    
    let add = Tool::native("+", "(int int -- int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a + b))?;
            Ok((stack, ctx))
        })
    });
    
    let le = Tool::native("<=", "(int int -- bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Bool(a <= b))?;
            Ok((stack, ctx))
        })
    });
    
    let sum_to = Tool::composed(
        "sum-to",
        None,
        vec![
            Op::call("dup"),
            Op::push(1),
            Op::call("<="),
            Op::if_then_else(
                vec![], // base case: just return n
                vec![
                    Op::call("dup"),
                    Op::push(1),
                    Op::call("-"),
                    Op::call("sum-to"),
                    Op::call("+"),
                ],
            ),
        ],
    );
    
    let result = run_with_tools(
        vec![Op::push(10), Op::call("sum-to")],
        vec![sub, add, le, sum_to],
    ).await;
    
    assert_eq!(result, vec![Value::Int(55)]); // 10+9+8+...+1 = 55
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[tokio::test]
async fn call_on_empty_stack_is_error() {
    let err = run_expect_error(vec![
        Op::call("call"), // no quote on stack
    ]).await;
    
    // The error message should indicate stack underflow or similar
    assert!(
        err.to_lowercase().contains("underflow") || 
        err.to_lowercase().contains("empty") || 
        err.to_lowercase().contains("stack") ||
        err.to_lowercase().contains("pop"),
        "Expected stack underflow error, got: {}", err
    );
}

#[tokio::test]
async fn deeply_nested_calls_work() {
    // ((((42))))  - 4 levels of quoting, 4 calls
    let result = run(vec![
        Op::quote(vec![
            Op::quote(vec![
                Op::quote(vec![
                    Op::quote(vec![Op::push(42)]),
                ]),
            ]),
        ]),
        Op::call("call"),
        Op::call("call"),
        Op::call("call"),
        Op::call("call"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}

#[tokio::test]
async fn if_with_empty_branches() {
    let result = run(vec![
        Op::push(1),
        Op::push(true),
        Op::if_then_else(vec![], vec![]),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(1)]);
}

// ============================================================================
// VALUE TYPE CORRECTNESS
// ============================================================================

#[tokio::test]
async fn all_value_types_can_be_pushed() {
    let result = run(vec![
        Op::Push(Value::Null),
        Op::push(true),
        Op::push(42),
        Op::push(3.14),
        Op::push("hello"),
        Op::Push(Value::List(vec![Value::Int(1)])),
        Op::Push(Value::Map(indexmap::indexmap! {
            "key".to_string() => Value::Int(1)
        })),
        Op::quote(vec![]),
    ]).await;
    
    assert_eq!(result.len(), 8);
    assert!(result[0].is_null());
    assert!(result[1].as_bool().is_ok());
    assert!(result[2].as_int().is_ok());
    assert!(result[3].as_float().is_ok());
    assert!(result[4].as_text().is_ok());
    assert!(result[5].as_list().is_ok());
    assert!(result[6].as_map().is_ok());
    assert!(result[7].as_quote().is_ok());
}

// ============================================================================
// IDENTITY LAWS (Mathematical Soundness)
// ============================================================================

#[tokio::test]
async fn drop_dup_is_identity() {
    // dup drop = identity (for any value)
    let result = run(vec![
        Op::push(42),
        Op::call("dup"),
        Op::call("drop"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}

#[tokio::test]
async fn swap_swap_is_identity() {
    // swap swap = identity
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::call("swap"),
        Op::call("swap"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(1), Value::Int(2)]);
}

#[tokio::test]
async fn rot_rot_rot_is_identity() {
    // rot rot rot = identity (3 rotations returns to original)
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::push(3),
        Op::call("rot"),
        Op::call("rot"),
        Op::call("rot"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
}

#[tokio::test]
async fn try_success_is_transparent() {
    // (push 42) try = push 42 (for successful execution)
    let result = run(vec![
        Op::quote(vec![Op::push(42)]),
        Op::call("try"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}

#[tokio::test]
async fn unwrap_non_error_is_transparent() {
    // x unwrap = x (when x is not an error)
    let result = run(vec![
        Op::push(42),
        Op::call("unwrap"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}
