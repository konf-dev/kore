//! Executor - The ~10 line core execution loop
//!
//! This is the heart of the system. Everything else is just tools.
//!
//! Following Postulate 1: Everything is a Tool.
//! There are only two operations:
//! - Push: Put a value on the stack
//! - Call: Execute a tool by name
//!
//! Conditionals, loops, and everything else are implemented as tools.

use crate::context::Context;
use crate::error::Result;
use crate::op::Op;
use crate::stack::Stack;
use crate::tool::ToolBody;
use crate::value::{ErrorValue, Value};
use std::future::Future;
use std::pin::Pin;
use std::time::Instant;

/// The future type returned by execute
pub type ExecFuture<'a> = Pin<Box<dyn Future<Output = Result<(Stack, Context)>> + Send + 'a>>;

/// Execute a program (list of ops) with given stack and context.
/// 
/// This is the entire execution model: iterate through ops,
/// pushing values or calling tools. Nothing else.
pub fn execute(ops: &[Op], mut stack: Stack, ctx: Context) -> ExecFuture<'_> {
    Box::pin(async move {
        for op in ops {
            (stack, _) = execute_op(op, stack, ctx.clone()).await?;
        }
        Ok((stack, ctx))
    })
}

/// Execute a single operation.
/// 
/// This is the irreducible core - two cases only.
async fn execute_op(op: &Op, mut stack: Stack, ctx: Context) -> Result<(Stack, Context)> {
    match op {
        // Push a literal value onto the stack
        Op::Push(value) => {
            stack.push(value.clone())?;
            Ok((stack, ctx))
        }

        // Call a tool by name
        Op::Call(name) => {
            let dict = ctx.dict.read().await;
            let tool = dict.get(name)?;
            drop(dict); // Release lock before executing

            // Track execution time
            let start = Instant::now();

            // Execute based on tool body
            let result = match &tool.body {
                ToolBody::Native(native_fn) => native_fn.call(stack, ctx.clone()).await,
                ToolBody::Ops(ops) => execute(ops, stack, ctx.clone()).await,
            };

            // Update stats
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

/// Execute the `call` primitive - run a quote from the stack
pub async fn call_quote(mut stack: Stack, ctx: Context) -> Result<(Stack, Context)> {
    let quote = stack.pop()?.into_quote()?;
    execute(&quote, stack, ctx).await
}

/// Execute the `catch` primitive - error handling with stack checkpoint
pub async fn catch_error(mut stack: Stack, ctx: Context) -> Result<(Stack, Context)> {
    let catch_quote = stack.pop()?.into_quote()?;
    let try_quote = stack.pop()?.into_quote()?;

    // Checkpoint the stack BEFORE try
    let checkpoint = stack.checkpoint();

    // Try to execute
    match execute(&try_quote, stack, ctx.clone()).await {
        Ok(result) => Ok(result),
        Err(error) => {
            // Restore stack to checkpoint
            let mut restored = checkpoint;

            // Push error as value
            restored.push(Value::Error(Box::new(ErrorValue {
                code: error.code().to_string(),
                message: error.to_string(),
            })))?;

            // Execute catch quote
            execute(&catch_quote, restored, ctx).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::Tool;

    /// Register minimal tools needed for executor tests
    async fn setup_ctx() -> Context {
        let ctx = Context::new();

        let mut dict = ctx.dict.write().await;

        // dup: (a -- a a)
        dict.register(Tool::native("dup", "", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                stack.push(v.clone())?;
                stack.push(v)?;
                Ok((stack, ctx))
            })
        }));

        // add: (a b -- a+b)
        dict.register(Tool::native("add", "", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                stack.push(Value::Int(a + b))?;
                Ok((stack, ctx))
            })
        }));

        // drop: (a -- )
        dict.register(Tool::native("drop", "", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                stack.pop()?;
                Ok((stack, ctx))
            })
        }));

        // div: (a b -- a/b)
        dict.register(Tool::native("div", "", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.as_int()?;
                let a = stack.pop()?.as_int()?;
                if b == 0 {
                    return Err(crate::error::Error::DivisionByZero);
                }
                stack.push(Value::Int(a / b))?;
                Ok((stack, ctx))
            })
        }));

        // call: run a quote
        dict.register(Tool::native("call", "", |stack: Stack, ctx: Context| {
            Box::pin(async move { call_quote(stack, ctx).await })
        }));

        // catch: error handling
        dict.register(Tool::native("catch", "", |stack: Stack, ctx: Context| {
            Box::pin(async move { catch_error(stack, ctx).await })
        }));

        // if: conditional execution (this is a TOOL, not a primitive!)
        // (condition then_quote else_quote -- result)
        dict.register(Tool::native("if", "", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let else_quote = stack.pop()?.into_quote()?;
                let then_quote = stack.pop()?.into_quote()?;
                let condition = stack.pop()?;
                
                let ops = if condition.is_truthy() {
                    then_quote
                } else {
                    else_quote
                };
                execute(&ops, stack, ctx).await
            })
        }));

        drop(dict);
        ctx
    }

    #[tokio::test]
    async fn test_push() {
        let ctx = setup_ctx().await;
        let stack = Stack::new();

        let ops = vec![Op::push(42), Op::push("hello")];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 2);
        assert_eq!(result.values()[0].as_int().unwrap(), 42);
        assert_eq!(result.values()[1].as_text().unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_call_primitive() {
        let ctx = setup_ctx().await;
        let stack = Stack::new();

        // 5 dup add = 10
        let ops = vec![Op::push(5), Op::call("dup"), Op::call("add")];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 10);
    }

    #[tokio::test]
    async fn test_if_then() {
        let ctx = setup_ctx().await;
        let stack = Stack::new();

        // true [1] [2] if = 1 (using if as a TOOL)
        let ops = vec![
            Op::push(true),
            Op::quote(vec![Op::push(1)]),  // then quote
            Op::quote(vec![Op::push(2)]),  // else quote
            Op::call("if"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_if_else() {
        let ctx = setup_ctx().await;
        let stack = Stack::new();

        // false [1] [2] if = 2 (using if as a TOOL)
        let ops = vec![
            Op::push(false),
            Op::quote(vec![Op::push(1)]),  // then quote
            Op::quote(vec![Op::push(2)]),  // else quote
            Op::call("if"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 2);
    }

    #[tokio::test]
    async fn test_quote_and_call() {
        let ctx = setup_ctx().await;
        let stack = Stack::new();

        // 5 [dup add] call = 10
        // Note: Op::quote creates Push(Value::Quote(...))
        let ops = vec![
            Op::push(5),
            Op::quote(vec![Op::call("dup"), Op::call("add")]),
            Op::call("call"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 10);
    }

    #[tokio::test]
    async fn test_composed_tool() {
        let ctx = setup_ctx().await;

        // Register a composed tool: math/double = dup add
        {
            let mut dict = ctx.dict.write().await;
            dict.register(Tool::composed(
                "math/double",
                Some("(n:Num -- result:Num)"),
                vec![Op::call("dup"), Op::call("add")],
            ));
        }

        let stack = Stack::new();
        let ops = vec![Op::push(21), Op::call("math/double")];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_catch_success() {
        let ctx = setup_ctx().await;
        let stack = Stack::new();

        // [5] [drop 0] catch = 5 (no error, try succeeds)
        let ops = vec![
            Op::quote(vec![Op::push(5)]),
            Op::quote(vec![Op::call("drop"), Op::push(0)]),
            Op::call("catch"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 5);
    }

    #[tokio::test]
    async fn test_catch_error() {
        let ctx = setup_ctx().await;
        let stack = Stack::new();

        // [1 0 div] [drop 999] catch = 999 (div by zero caught)
        let ops = vec![
            Op::quote(vec![Op::push(1), Op::push(0), Op::call("div")]),
            Op::quote(vec![Op::call("drop"), Op::push(999)]),
            Op::call("catch"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 999);
    }
}
