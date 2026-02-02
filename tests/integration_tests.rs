//! Integration Tests for Kore
//!
//! These tests verify:
//! 1. All tools work correctly
//! 2. Tools compose correctly (output of one feeds input of another)
//! 3. Edge cases are handled properly
//! 4. Error propagation works
//! 5. Type matching for tool composition

use kore::{execute, register_builtins, Context, Effect, Op, Stack, Tool, Value};
use indexmap::IndexMap;

// =============================================================================
// Test Helpers
// =============================================================================

async fn run(code: &str) -> Vec<Value> {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    let ops = Op::parse(code).expect("parse failed");
    let stack = Stack::new();
    let (result, _) = execute(&ops, stack, ctx).await.expect("execute failed");
    result.values().to_vec()
}

#[allow(dead_code)] // Useful helper for tests that need to inspect context after execution
async fn run_with_ctx(code: &str, ctx: Context) -> (Vec<Value>, Context) {
    let ops = Op::parse(code).expect("parse failed");
    let stack = Stack::new();
    let (result, ctx) = execute(&ops, stack, ctx).await.expect("execute failed");
    (result.values().to_vec(), ctx)
}

async fn run_ops(ops: Vec<Op>) -> Vec<Value> {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    let stack = Stack::new();
    let (result, _) = execute(&ops, stack, ctx).await.expect("execute failed");
    result.values().to_vec()
}

async fn run_ops_with_tools(ops: Vec<Op>, tools: Vec<Tool>) -> Vec<Value> {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    for tool in tools {
        ctx.dict.write().await.register(tool);
    }
    let stack = Stack::new();
    let (result, _) = execute(&ops, stack, ctx).await.expect("execute failed");
    result.values().to_vec()
}

async fn run_should_fail(code: &str) -> String {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    let ops = Op::parse(code).expect("parse failed");
    let stack = Stack::new();
    match execute(&ops, stack, ctx).await {
        Err(e) => e.to_string(),
        Ok(_) => panic!("Expected error but got success"),
    }
}

fn effect(s: &str) -> Option<Effect> {
    Effect::parse(s).ok()
}

// =============================================================================
// Core Language Tests
// =============================================================================

mod core_ops {
    use super::*;

    #[tokio::test]
    async fn push_literals() {
        // Integers
        assert_eq!(run("42").await, vec![Value::Int(42)]);
        assert_eq!(run("-17").await, vec![Value::Int(-17)]);
        assert_eq!(run("0").await, vec![Value::Int(0)]);
        
        // Floats
        assert_eq!(run("3.14").await, vec![Value::Float(3.14)]);
        assert_eq!(run("-2.5").await, vec![Value::Float(-2.5)]);
        
        // Booleans
        assert_eq!(run("true").await, vec![Value::Bool(true)]);
        assert_eq!(run("false").await, vec![Value::Bool(false)]);
        
        // Null
        assert_eq!(run("null").await, vec![Value::Null]);
        
        // Strings
        assert_eq!(run(r#""hello""#).await, vec![Value::Text("hello".into())]);
        assert_eq!(run(r#"'world'"#).await, vec![Value::Text("world".into())]);
        assert_eq!(run(r#""""#).await, vec![Value::Text("".into())]);
    }

    #[tokio::test]
    async fn push_multiple() {
        assert_eq!(
            run("1 2 3").await,
            vec![Value::Int(1), Value::Int(2), Value::Int(3)]
        );
    }

    #[tokio::test]
    async fn quotes() {
        // Quote is pushed as a value
        let result = run("[ 1 2 ]").await;
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], Value::Quote(_)));
        
        // Nested quotes
        let result = run("[ [ 1 ] ]").await;
        assert_eq!(result.len(), 1);
        if let Value::Quote(outer) = &result[0] {
            assert_eq!(outer.len(), 1);
            assert!(matches!(outer[0], Op::Quote(_)));
        } else {
            panic!("Expected quote");
        }
    }

    #[tokio::test]
    async fn quote_call() {
        // Execute a quote with call
        assert_eq!(run("[ 42 ] call").await, vec![Value::Int(42)]);
        assert_eq!(run("[ 1 2 ] call").await, vec![Value::Int(1), Value::Int(2)]);
    }

    #[tokio::test]
    async fn if_true() {
        let ops = vec![
            Op::push(true),
            Op::if_then_else(
                vec![Op::push(1)],
                vec![Op::push(0)],
            ),
        ];
        assert_eq!(run_ops(ops).await, vec![Value::Int(1)]);
    }

    #[tokio::test]
    async fn if_false() {
        let ops = vec![
            Op::push(false),
            Op::if_then_else(
                vec![Op::push(1)],
                vec![Op::push(0)],
            ),
        ];
        assert_eq!(run_ops(ops).await, vec![Value::Int(0)]);
    }

    #[tokio::test]
    async fn if_truthy_int() {
        // Non-zero int is truthy
        let ops = vec![
            Op::push(5),
            Op::if_then_else(
                vec![Op::push("yes")],
                vec![Op::push("no")],
            ),
        ];
        assert_eq!(run_ops(ops).await, vec![Value::Text("yes".into())]);
        
        // Zero is falsy
        let ops = vec![
            Op::push(0),
            Op::if_then_else(
                vec![Op::push("yes")],
                vec![Op::push("no")],
            ),
        ];
        assert_eq!(run_ops(ops).await, vec![Value::Text("no".into())]);
    }

    #[tokio::test]
    async fn if_truthy_string() {
        // Non-empty string is truthy
        let ops = vec![
            Op::push("hello"),
            Op::if_then_else(
                vec![Op::push(1)],
                vec![Op::push(0)],
            ),
        ];
        assert_eq!(run_ops(ops).await, vec![Value::Int(1)]);
        
        // Empty string is falsy
        let ops = vec![
            Op::push(""),
            Op::if_then_else(
                vec![Op::push(1)],
                vec![Op::push(0)],
            ),
        ];
        assert_eq!(run_ops(ops).await, vec![Value::Int(0)]);
    }

    #[tokio::test]
    async fn if_truthy_null() {
        // Null is falsy
        let ops = vec![
            Op::Push(Value::Null),
            Op::if_then_else(
                vec![Op::push(1)],
                vec![Op::push(0)],
            ),
        ];
        assert_eq!(run_ops(ops).await, vec![Value::Int(0)]);
    }

    #[tokio::test]
    async fn if_truthy_list() {
        // Non-empty list is truthy
        let ops = vec![
            Op::Push(Value::List(vec![Value::Int(1)])),
            Op::if_then_else(
                vec![Op::push("yes")],
                vec![Op::push("no")],
            ),
        ];
        assert_eq!(run_ops(ops).await, vec![Value::Text("yes".into())]);
        
        // Empty list is falsy
        let ops = vec![
            Op::Push(Value::List(vec![])),
            Op::if_then_else(
                vec![Op::push("yes")],
                vec![Op::push("no")],
            ),
        ];
        assert_eq!(run_ops(ops).await, vec![Value::Text("no".into())]);
    }
}

// =============================================================================
// Stack Manipulation Tests
// =============================================================================

mod stack_ops {
    use super::*;

    #[tokio::test]
    async fn dup() {
        assert_eq!(run("42 dup").await, vec![Value::Int(42), Value::Int(42)]);
        assert_eq!(
            run(r#""hello" dup"#).await,
            vec![Value::Text("hello".into()), Value::Text("hello".into())]
        );
    }

    #[tokio::test]
    async fn drop() {
        assert_eq!(run("1 2 drop").await, vec![Value::Int(1)]);
        assert_eq!(run("1 drop").await, vec![]);
    }

    #[tokio::test]
    async fn swap() {
        assert_eq!(run("1 2 swap").await, vec![Value::Int(2), Value::Int(1)]);
        assert_eq!(
            run(r#""a" "b" swap"#).await,
            vec![Value::Text("b".into()), Value::Text("a".into())]
        );
    }

    #[tokio::test]
    async fn over() {
        assert_eq!(
            run("1 2 over").await,
            vec![Value::Int(1), Value::Int(2), Value::Int(1)]
        );
    }

    #[tokio::test]
    async fn rot() {
        assert_eq!(
            run("1 2 3 rot").await,
            vec![Value::Int(2), Value::Int(3), Value::Int(1)]
        );
    }

    #[tokio::test]
    async fn stack_underflow_dup() {
        let err = run_should_fail("dup").await;
        assert!(err.contains("underflow") || err.contains("empty") || err.contains("effect") || err.contains("inputs"), "Expected stack error: {}", err);
    }

    #[tokio::test]
    async fn stack_underflow_drop() {
        let err = run_should_fail("drop").await;
        assert!(err.contains("underflow") || err.contains("empty") || err.contains("effect") || err.contains("inputs"), "Expected stack error: {}", err);
    }

    #[tokio::test]
    async fn stack_underflow_swap() {
        let err = run_should_fail("1 swap").await;
        assert!(err.contains("underflow") || err.contains("empty") || err.contains("effect") || err.contains("inputs"), "Expected stack error: {}", err);
    }

    #[tokio::test]
    async fn stack_underflow_over() {
        let err = run_should_fail("1 over").await;
        assert!(err.contains("underflow") || err.contains("empty") || err.contains("effect") || err.contains("inputs"), "Expected stack error: {}", err);
    }

    #[tokio::test]
    async fn stack_underflow_rot() {
        let err = run_should_fail("1 2 rot").await;
        assert!(err.contains("underflow") || err.contains("empty") || err.contains("effect") || err.contains("inputs"), "Expected stack error: {}", err);
    }

    #[tokio::test]
    async fn complex_stack_manipulation() {
        // 1 2 3 swap rot => 1 3 2 rot => 3 2 1
        assert_eq!(
            run("1 2 3 swap rot").await,
            vec![Value::Int(3), Value::Int(2), Value::Int(1)]
        );
        
        // dup dup: 5 => 5 5 5
        assert_eq!(
            run("5 dup dup").await,
            vec![Value::Int(5), Value::Int(5), Value::Int(5)]
        );
    }
}

// =============================================================================
// Error Handling Tests
// =============================================================================

mod error_handling {
    use super::*;

    #[tokio::test]
    async fn try_success() {
        // try with successful quote returns the result
        assert_eq!(run("[ 42 ] try").await, vec![Value::Int(42)]);
    }

    #[tokio::test]
    async fn try_failure() {
        // try with failing quote returns Error value
        let result = run("[ drop ] try").await;
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], Value::Error(_)));
    }

    #[tokio::test]
    async fn is_error_true() {
        let result = run("[ drop ] try is-error").await;
        assert_eq!(result, vec![Value::Bool(true)]);
    }

    #[tokio::test]
    async fn is_error_false() {
        assert_eq!(run("42 is-error").await, vec![Value::Bool(false)]);
        assert_eq!(run("null is-error").await, vec![Value::Bool(false)]);
        assert_eq!(run(r#""hello" is-error"#).await, vec![Value::Bool(false)]);
    }

    #[tokio::test]
    async fn unwrap_non_error() {
        assert_eq!(run("42 unwrap").await, vec![Value::Int(42)]);
        assert_eq!(run(r#""test" unwrap"#).await, vec![Value::Text("test".into())]);
    }

    #[tokio::test]
    async fn unwrap_error_propagates() {
        let err = run_should_fail("[ drop ] try unwrap").await;
        assert!(!err.is_empty(), "Expected error message");
    }

    #[tokio::test]
    async fn nested_try() {
        // Nested try: outer try catches inner failure
        let result = run("[ [ drop ] try ] try").await;
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], Value::Error(_)));
    }

    #[tokio::test]
    async fn error_recovery_pattern() {
        // Pattern: try, check if error, provide default
        // This simulates: x.unwrap_or(default)
        let ops = vec![
            // Try something that fails
            Op::Quote(vec![Op::call("drop")]),
            Op::call("try"),
            Op::call("is-error"),
            Op::if_then_else(
                vec![Op::push(0)],    // default on error
                vec![],               // keep result
            ),
        ];
        let result = run_ops(ops).await;
        assert_eq!(result, vec![Value::Int(0)]);
    }
}

// =============================================================================
// Tool Composition Tests
// =============================================================================

mod composition {
    use super::*;

    fn make_add() -> Tool {
        Tool::native("add", "(a:Int b:Int -- sum:Int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Int(a + b))?;
                Ok((stack, ctx))
            })
        })
    }

    fn make_mul() -> Tool {
        Tool::native("mul", "(a:Int b:Int -- product:Int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Int(a * b))?;
                Ok((stack, ctx))
            })
        })
    }

    fn make_neg() -> Tool {
        Tool::native("neg", "(a:Int -- negated:Int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Int(-a))?;
                Ok((stack, ctx))
            })
        })
    }

    fn make_len() -> Tool {
        Tool::native("len", "(s:Text -- length:Int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let s = stack.pop()?.into_text()?;
                stack.push(Value::Int(s.len() as i64))?;
                Ok((stack, ctx))
            })
        })
    }

    fn make_concat() -> Tool {
        Tool::native("concat", "(a:Text b:Text -- result:Text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.into_text()?;
                let a = stack.pop()?.into_text()?;
                stack.push(Value::Text(format!("{}{}", a, b)))?;
                Ok((stack, ctx))
            })
        })
    }

    #[tokio::test]
    async fn simple_composition() {
        // add output (Int) feeds into neg input (Int)
        let ops = vec![Op::push(3), Op::push(4), Op::call("add"), Op::call("neg")];
        let result = run_ops_with_tools(ops, vec![make_add(), make_neg()]).await;
        assert_eq!(result, vec![Value::Int(-7)]);
    }

    #[tokio::test]
    async fn chained_same_type() {
        // mul -> add -> neg: all work on Int
        let ops = vec![
            Op::push(2),
            Op::push(3),
            Op::call("mul"),   // 6
            Op::push(4),
            Op::call("add"),   // 10
            Op::call("neg"),   // -10
        ];
        let result = run_ops_with_tools(ops, vec![make_add(), make_mul(), make_neg()]).await;
        assert_eq!(result, vec![Value::Int(-10)]);
    }

    #[tokio::test]
    async fn text_to_int_composition() {
        // concat output (Text) feeds into len input (Text), len output (Int) is final
        let ops = vec![
            Op::push("hello"),
            Op::push("world"),
            Op::call("concat"),  // "helloworld"
            Op::call("len"),     // 10
        ];
        let result = run_ops_with_tools(ops, vec![make_concat(), make_len()]).await;
        assert_eq!(result, vec![Value::Int(10)]);
    }

    #[tokio::test]
    async fn composed_tool() {
        // double = dup add
        let double = Tool::composed(
            "double",
            effect("(n:Int -- doubled:Int)"),
            vec![Op::call("dup"), Op::call("add")],
        );

        let ops = vec![Op::push(21), Op::call("double")];
        let result = run_ops_with_tools(ops, vec![make_add(), double]).await;
        assert_eq!(result, vec![Value::Int(42)]);
    }

    #[tokio::test]
    async fn nested_composed_tools() {
        // add is native
        // double = dup add
        // quadruple = double double
        let double = Tool::composed(
            "double",
            effect("(n:Int -- doubled:Int)"),
            vec![Op::call("dup"), Op::call("add")],
        );

        let quadruple = Tool::composed(
            "quadruple",
            effect("(n:Int -- quadrupled:Int)"),
            vec![Op::call("double"), Op::call("double")],
        );

        let ops = vec![Op::push(5), Op::call("quadruple")];
        let result = run_ops_with_tools(ops, vec![make_add(), double, quadruple]).await;
        assert_eq!(result, vec![Value::Int(20)]);
    }

    #[tokio::test]
    async fn type_mismatch_fails() {
        // len expects Text, but we give it Int
        let ops = vec![Op::push(42), Op::call("len")];
        
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        ctx.dict.write().await.register(make_len());
        
        let stack = Stack::new();
        let result = execute(&ops, stack, ctx).await;
        assert!(result.is_err(), "Expected type error");
    }

    #[tokio::test]
    async fn composition_with_stack_ops() {
        // Use stack ops to prepare data for tools
        // 5 dup add => 10, then neg => -10
        let ops = vec![
            Op::push(5),
            Op::call("dup"),
            Op::call("add"),
            Op::call("neg"),
        ];
        let result = run_ops_with_tools(ops, vec![make_add(), make_neg()]).await;
        assert_eq!(result, vec![Value::Int(-10)]);
    }

    #[tokio::test]
    async fn composition_with_conditionals() {
        // abs = dup 0 less-than (neg) () if
        let lt = Tool::native("less-than", "(a:Int b:Int -- result:Bool)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Bool(a < b))?;
                Ok((stack, ctx))
            })
        });

        let abs = Tool::composed(
            "abs",
            effect("(n:Int -- absolute:Int)"),
            vec![
                Op::call("dup"),
                Op::push(0),
                Op::call("less-than"),
                Op::if_then_else(
                    vec![Op::call("neg")],
                    vec![],
                ),
            ],
        );

        let ops = vec![Op::push(-5), Op::call("abs")];
        let result = run_ops_with_tools(ops, vec![make_neg(), lt.clone(), abs.clone()]).await;
        assert_eq!(result, vec![Value::Int(5)]);

        let ops = vec![Op::push(7), Op::call("abs")];
        let result = run_ops_with_tools(ops, vec![make_neg(), lt, abs]).await;
        assert_eq!(result, vec![Value::Int(7)]);
    }
}

// =============================================================================
// Recursion and Turing Completeness Tests
// =============================================================================

mod recursion {
    use super::*;

    #[tokio::test]
    async fn factorial() {
        let sub = Tool::native("-", "(a:Int b:Int -- diff:Int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Int(a - b))?;
                Ok((stack, ctx))
            })
        });

        let mul = Tool::native("*", "(a:Int b:Int -- product:Int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Int(a * b))?;
                Ok((stack, ctx))
            })
        });

        let le = Tool::native("<=", "(a:Int b:Int -- result:Bool)", |mut stack: Stack, ctx: Context| {
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
            effect("(n:Int -- result:Int)"),
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

        let ops = vec![Op::push(5), Op::call("factorial")];
        let result = run_ops_with_tools(ops, vec![sub, mul, le, factorial]).await;
        assert_eq!(result, vec![Value::Int(120)]); // 5! = 120

        let ops = vec![Op::push(0), Op::call("factorial")];
        let result = run_ops_with_tools(
            ops,
            vec![
                Tool::native("-", "(a:Int b:Int -- diff:Int)", |mut stack: Stack, ctx: Context| {
                    Box::pin(async move {
                        let b = stack.pop()?.as_int()?;
                        let a = stack.pop()?.as_int()?;
                        stack.push(Value::Int(a - b))?;
                        Ok((stack, ctx))
                    })
                }),
                Tool::native("*", "(a:Int b:Int -- product:Int)", |mut stack: Stack, ctx: Context| {
                    Box::pin(async move {
                        let b = stack.pop()?.as_int()?;
                        let a = stack.pop()?.as_int()?;
                        stack.push(Value::Int(a * b))?;
                        Ok((stack, ctx))
                    })
                }),
                Tool::native("<=", "(a:Int b:Int -- result:Bool)", |mut stack: Stack, ctx: Context| {
                    Box::pin(async move {
                        let b = stack.pop()?.as_int()?;
                        let a = stack.pop()?.as_int()?;
                        stack.push(Value::Bool(a <= b))?;
                        Ok((stack, ctx))
                    })
                }),
                Tool::composed(
                    "factorial",
                    effect("(n:Int -- result:Int)"),
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
                ),
            ],
        ).await;
        assert_eq!(result, vec![Value::Int(1)]); // 0! = 1
    }

    #[tokio::test]
    async fn fibonacci() {
        let sub = Tool::native("-", "(a:Int b:Int -- diff:Int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Int(a - b))?;
                Ok((stack, ctx))
            })
        });

        let add = Tool::native("+", "(a:Int b:Int -- sum:Int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Int(a + b))?;
                Ok((stack, ctx))
            })
        });

        let le = Tool::native("<=", "(a:Int b:Int -- result:Bool)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Bool(a <= b))?;
                Ok((stack, ctx))
            })
        });

        // fib = dup 1 <= () (dup 1 - fib swap 2 - fib +) if
        let fib = Tool::composed(
            "fib",
            effect("(n:Int -- result:Int)"),
            vec![
                Op::call("dup"),
                Op::push(1),
                Op::call("<="),
                Op::if_then_else(
                    vec![], // n <= 1: return n
                    vec![
                        Op::call("dup"),
                        Op::push(1),
                        Op::call("-"),
                        Op::call("fib"),
                        Op::call("swap"),
                        Op::push(2),
                        Op::call("-"),
                        Op::call("fib"),
                        Op::call("+"),
                    ],
                ),
            ],
        );

        // fib(0) = 0, fib(1) = 1, fib(2) = 1, fib(3) = 2, fib(4) = 3, fib(5) = 5, fib(6) = 8
        let ops = vec![Op::push(6), Op::call("fib")];
        let result = run_ops_with_tools(ops, vec![sub, add, le, fib]).await;
        assert_eq!(result, vec![Value::Int(8)]);
    }

    #[tokio::test]
    async fn sum_to_n() {
        let sub = Tool::native("-", "(a:Int b:Int -- diff:Int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Int(a - b))?;
                Ok((stack, ctx))
            })
        });

        let add = Tool::native("+", "(a:Int b:Int -- sum:Int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Int(a + b))?;
                Ok((stack, ctx))
            })
        });

        let le = Tool::native("<=", "(a:Int b:Int -- result:Bool)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Bool(a <= b))?;
                Ok((stack, ctx))
            })
        });

        // sum-to = dup 0 <= (drop 0) (dup 1 - sum-to +) if
        let sum_to = Tool::composed(
            "sum-to",
            effect("(n:Int -- sum:Int)"),
            vec![
                Op::call("dup"),
                Op::push(0),
                Op::call("<="),
                Op::if_then_else(
                    vec![Op::call("drop"), Op::push(0)],
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

        // sum(10) = 55
        let ops = vec![Op::push(10), Op::call("sum-to")];
        let result = run_ops_with_tools(ops, vec![sub, add, le, sum_to]).await;
        assert_eq!(result, vec![Value::Int(55)]);
    }
}

// =============================================================================
// Value Type Tests
// =============================================================================

mod value_types {
    use super::*;

    #[tokio::test]
    async fn list_operations() {
        // Create a list
        let result = run_ops(vec![
            Op::Push(Value::List(vec![
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
            ])),
        ]).await;
        assert_eq!(result.len(), 1);
        if let Value::List(l) = &result[0] {
            assert_eq!(l.len(), 3);
        } else {
            panic!("Expected list");
        }
    }

    #[tokio::test]
    async fn map_operations() {
        let mut map = IndexMap::new();
        map.insert("name".to_string(), Value::Text("Alice".into()));
        map.insert("age".to_string(), Value::Int(30));

        let result = run_ops(vec![Op::Push(Value::Map(map))]).await;
        assert_eq!(result.len(), 1);
        if let Value::Map(m) = &result[0] {
            assert_eq!(m.get("name"), Some(&Value::Text("Alice".into())));
            assert_eq!(m.get("age"), Some(&Value::Int(30)));
        } else {
            panic!("Expected map");
        }
    }

    #[tokio::test]
    async fn handle_type() {
        let handle = Value::Handle(kore::value::Handle {
            kind: kore::value::HandleKind::Custom("test".into()),
            id: "test-123".into(),
        });

        let result = run_ops(vec![Op::Push(handle.clone())]).await;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], handle);
    }

    #[tokio::test]
    async fn dup_preserves_type() {
        // All types should be duplicatable
        let test_values = vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(42),
            Value::Float(3.14),
            Value::Text("hello".into()),
            Value::List(vec![Value::Int(1)]),
            Value::Map(IndexMap::new()),
            Value::Quote(vec![Op::push(1)]),
        ];

        for val in test_values {
            let result = run_ops(vec![Op::Push(val.clone()), Op::call("dup")]).await;
            assert_eq!(result.len(), 2);
            assert_eq!(result[0], val);
            assert_eq!(result[1], val);
        }
    }
}

// =============================================================================
// Edge Cases
// =============================================================================

mod edge_cases {
    use super::*;

    #[tokio::test]
    async fn empty_program() {
        assert_eq!(run("").await, vec![]);
    }

    #[tokio::test]
    async fn empty_quote() {
        let result = run("[ ] call").await;
        assert_eq!(result, vec![]);
    }

    #[tokio::test]
    async fn deeply_nested_quotes() {
        // [[[ 42 ]]] call call call
        let ops = vec![
            Op::Quote(vec![
                Op::Quote(vec![
                    Op::Quote(vec![Op::push(42)]),
                ]),
            ]),
            Op::call("call"),
            Op::call("call"),
            Op::call("call"),
        ];
        let result = run_ops(ops).await;
        assert_eq!(result, vec![Value::Int(42)]);
    }

    #[tokio::test]
    async fn many_operations() {
        // 100 push operations
        let ops: Vec<Op> = (0..100).map(|i| Op::push(i as i64)).collect();
        
        let result = run_ops(ops).await;
        assert_eq!(result.len(), 100);
        for (i, val) in result.iter().enumerate() {
            assert_eq!(*val, Value::Int(i as i64));
        }
    }

    #[tokio::test]
    async fn unknown_tool_error() {
        let err = run_should_fail("nonexistent-tool").await;
        assert!(
            err.contains("not found") || err.contains("undefined") || err.contains("unknown"),
            "Expected 'not found' error: {}",
            err
        );
    }

    #[tokio::test]
    async fn call_non_quote_error() {
        let err = run_should_fail("42 call").await;
        assert!(
            err.contains("type") || err.contains("Quote") || err.contains("expected"),
            "Expected type error: {}",
            err
        );
    }

    #[tokio::test]
    async fn if_with_non_bool_works() {
        // Non-bool values use truthiness
        let ops = vec![
            Op::Push(Value::List(vec![Value::Int(1)])), // truthy
            Op::if_then_else(vec![Op::push(1)], vec![Op::push(0)]),
        ];
        assert_eq!(run_ops(ops).await, vec![Value::Int(1)]);
    }

    #[tokio::test]
    async fn large_integers() {
        let ops = vec![
            Op::push(i64::MAX),
            Op::call("dup"),
        ];
        let result = run_ops(ops).await;
        assert_eq!(result, vec![Value::Int(i64::MAX), Value::Int(i64::MAX)]);
    }

    #[tokio::test]
    async fn float_precision() {
        let ops = vec![
            Op::Push(Value::Float(0.1)),
            Op::Push(Value::Float(0.2)),
        ];
        let result = run_ops(ops).await;
        assert_eq!(result.len(), 2);
        if let Value::Float(f) = result[0] {
            assert!((f - 0.1).abs() < 1e-10);
        }
    }

    #[tokio::test]
    async fn unicode_strings() {
        let result = run(r#""こんにちは""#).await;
        assert_eq!(result, vec![Value::Text("こんにちは".into())]);
        
        let result = run(r#""🚀🎉""#).await;
        assert_eq!(result, vec![Value::Text("🚀🎉".into())]);
    }

    #[tokio::test]
    async fn special_characters_in_strings() {
        // Test various special characters that might cause parsing issues
        let tests = vec![
            (r#""hello world""#, "hello world"),
            // Parser interprets escape sequences, so \t becomes a tab
            (r#""tab\there""#, "tab\there"),
        ];
        
        for (input, expected) in tests {
            let result = run(input).await;
            assert_eq!(result.len(), 1);
            if let Value::Text(s) = &result[0] {
                assert_eq!(s, expected);
            }
        }
    }
}

// =============================================================================
// Quote Semantics Tests
// =============================================================================

mod quote_semantics {
    use super::*;

    #[tokio::test]
    async fn quote_is_data() {
        // A quote on the stack is just data until called
        let result = run("[ 1 2 3 ]").await;
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], Value::Quote(_)));
    }

    #[tokio::test]
    async fn quote_dup() {
        // Quotes can be duplicated
        let result = run("[ 42 ] dup").await;
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], Value::Quote(_)));
        assert!(matches!(result[1], Value::Quote(_)));
    }

    #[tokio::test]
    async fn quote_preserves_state() {
        // Quote should capture the operations, not values
        // When called, it operates on current stack
        let ops = vec![
            Op::push(10),
            Op::Quote(vec![Op::call("dup")]),
            Op::call("call"),
        ];
        let result = run_ops(ops).await;
        assert_eq!(result, vec![Value::Int(10), Value::Int(10)]);
    }

    #[tokio::test]
    async fn quote_in_if() {
        // Quotes as branches
        let ops = vec![
            Op::push(true),
            Op::if_then_else(
                vec![Op::push("then")],
                vec![Op::push("else")],
            ),
        ];
        let result = run_ops(ops).await;
        assert_eq!(result, vec![Value::Text("then".into())]);
    }

    #[tokio::test]
    async fn higher_order_tools() {
        // A tool that takes a quote and applies it twice
        let apply_twice = Tool::native(
            "apply-twice",
            "(q:Quote -- ...)",
            |mut stack: Stack, ctx: Context| {
                Box::pin(async move {
                    let quote = stack.pop()?.into_quote()?;
                    let (stack, ctx) = kore::execute(&quote, stack, ctx).await?;
                    kore::execute(&quote, stack, ctx).await
                })
            },
        );

        let inc = Tool::native("inc", "(n:Int -- n+1:Int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let n = stack.pop()?.as_int()?;
                stack.push(Value::Int(n + 1))?;
                Ok((stack, ctx))
            })
        });

        let ops = vec![
            Op::push(5),
            Op::Quote(vec![Op::call("inc")]),
            Op::call("apply-twice"),
        ];
        let result = run_ops_with_tools(ops, vec![apply_twice, inc]).await;
        assert_eq!(result, vec![Value::Int(7)]); // 5 + 1 + 1 = 7
    }
}

// =============================================================================
// Context and Dictionary Tests
// =============================================================================

mod context_tests {
    use super::*;

    #[tokio::test]
    async fn tool_registration() {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;

        // Custom tool
        let custom = Tool::native("my-tool", "(-- result:Int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                stack.push(Value::Int(42))?;
                Ok((stack, ctx))
            })
        });
        ctx.dict.write().await.register(custom);

        let ops = vec![Op::call("my-tool")];
        let stack = Stack::new();
        let (result, _) = execute(&ops, stack, ctx).await.expect("execute failed");
        assert_eq!(result.values(), vec![Value::Int(42)]);
    }

    #[tokio::test]
    async fn tool_override() {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;

        // Register a tool
        ctx.dict.write().await.register(
            Tool::native("test", "(--)", |stack: Stack, ctx: Context| {
                Box::pin(async move { Ok((stack, ctx)) })
            })
        );

        // Override it
        ctx.dict.write().await.register(
            Tool::native("test", "(-- n:Int)", |mut stack: Stack, ctx: Context| {
                Box::pin(async move {
                    stack.push(Value::Int(100))?;
                    Ok((stack, ctx))
                })
            })
        );

        let ops = vec![Op::call("test")];
        let stack = Stack::new();
        let (result, _) = execute(&ops, stack, ctx).await.expect("execute failed");
        assert_eq!(result.values(), vec![Value::Int(100)]);
    }
}
