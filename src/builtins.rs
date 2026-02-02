//! Built-in tools - the minimal set that makes kore a complete language
//!
//! These are language primitives, not library functions.
//! Each tool does exactly one thing.
//!
//! ## Execution
//! - `call`: Run a quote
//! - `try`: Run a quote, capture errors as Error values
//!
//! ## Error inspection
//! - `is-error`: Check if a value is an Error
//! - `unwrap`: Extract value, or stop if Error
//!
//! ## Stack manipulation
//! - `dup`: Duplicate top value
//! - `drop`: Remove top value
//! - `swap`: Swap top two values
//! - `over`: Copy second value to top
//! - `rot`: Rotate top three values

use crate::context::Context;
use crate::executor::execute;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::{ErrorValue, Value};

/// Register all built-in tools
pub async fn register_builtins(ctx: &mut Context) {
    let mut dict = ctx.dict.write().await;

    // === Execution ===

    // call: (quote -- ...) - run a quote
    dict.register(Tool::native("call", "(q:Quote -- ...)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let quote = stack.pop()?.into_quote()?;
            execute(&quote, stack, ctx).await
        })
    }));

    // try: (quote -- value-or-error) - run a quote, capture errors
    dict.register(Tool::native("try", "(q:Quote -- result:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let quote = stack.pop()?.into_quote()?;
            
            match execute(&quote, stack.clone(), ctx.clone()).await {
                Ok((result_stack, new_ctx)) => Ok((result_stack, new_ctx)),
                Err(error) => {
                    // Convert error to Error value
                    stack.push(Value::Error(Box::new(ErrorValue {
                        code: error.code().to_string(),
                        message: error.to_string(),
                    })))?;
                    Ok((stack, ctx))
                }
            }
        })
    }));

    // === Error inspection ===

    // is-error: (value -- bool) - check if value is an Error
    dict.register(Tool::native("is-error", "(v:Any -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?;
            let is_error = matches!(value, Value::Error(_));
            stack.push(Value::Bool(is_error))?;
            Ok((stack, ctx))
        })
    }));

    // unwrap: (value-or-error -- value) - extract value, or stop if Error
    dict.register(Tool::native("unwrap", "(v:Any -- result:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?;
            match value {
                Value::Error(e) => Err(crate::error::Error::Custom {
                    code: e.code,
                    message: e.message,
                }),
                other => {
                    stack.push(other)?;
                    Ok((stack, ctx))
                }
            }
        })
    }));

    // === Stack manipulation ===

    // dup: (a -- a a) - duplicate top value
    dict.register(Tool::native("dup", "(a:Any -- a:Any a:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?;
            stack.push(value.clone())?;
            stack.push(value)?;
            Ok((stack, ctx))
        })
    }));

    // drop: (a -- ) - remove top value
    dict.register(Tool::native("drop", "(a:Any -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            stack.pop()?;
            Ok((stack, ctx))
        })
    }));

    // swap: (a b -- b a) - swap top two values
    dict.register(Tool::native("swap", "(a:Any b:Any -- b:Any a:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?;
            let a = stack.pop()?;
            stack.push(b)?;
            stack.push(a)?;
            Ok((stack, ctx))
        })
    }));

    // over: (a b -- a b a) - copy second value to top
    dict.register(Tool::native("over", "(a:Any b:Any -- a:Any b:Any a:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?;
            let a = stack.pop()?;
            stack.push(a.clone())?;
            stack.push(b)?;
            stack.push(a)?;
            Ok((stack, ctx))
        })
    }));

    // rot: (a b c -- b c a) - rotate top three values
    dict.register(Tool::native("rot", "(a:Any b:Any c:Any -- b:Any c:Any a:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let c = stack.pop()?;
            let b = stack.pop()?;
            let a = stack.pop()?;
            stack.push(b)?;
            stack.push(c)?;
            stack.push(a)?;
            Ok((stack, ctx))
        })
    }));

    // === Control Flow ===

    // if: (condition then-quote else-quote -- ...) - conditional execution
    dict.register(Tool::native("if", "(cond:Bool then:Quote else:Quote -- ...)", |mut stack: Stack, ctx: Context| {
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

    // loop: (quote -- ...) - repeat until false on stack
    dict.register(Tool::native("loop", "(body:Quote -- ...)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let quote = stack.pop()?.into_quote()?;
            
            loop {
                let (new_stack, new_ctx) = execute(&quote, stack, ctx.clone()).await?;
                stack = new_stack;
                
                // Check condition on top of stack
                let condition = stack.pop()?;
                if !condition.is_truthy() {
                    break;
                }
            }
            Ok((stack, ctx))
        })
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::Op;

    async fn setup() -> Context {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn test_call() {
        let ctx = setup().await;
        let stack = Stack::new();

        // 5 [dup] call -> 5 5
        let ops = vec![
            Op::push(5),
            Op::Quote(vec![Op::call("dup")]),
            Op::call("call"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 2);
        assert_eq!(result.values()[0].as_int().unwrap(), 5);
        assert_eq!(result.values()[1].as_int().unwrap(), 5);
    }

    #[tokio::test]
    async fn test_try_success() {
        let ctx = setup().await;
        let stack = Stack::new();

        // [5] try -> 5
        let ops = vec![
            Op::Quote(vec![Op::push(5)]),
            Op::call("try"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 5);
    }

    #[tokio::test]
    async fn test_try_error() {
        let ctx = setup().await;
        let stack = Stack::new();

        // [drop] try -> Error (stack underflow)
        let ops = vec![
            Op::Quote(vec![Op::call("drop")]),
            Op::call("try"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert!(matches!(result.values()[0], Value::Error(_)));
    }

    #[tokio::test]
    async fn test_is_error() {
        let ctx = setup().await;
        let stack = Stack::new();

        // 5 is-error -> false
        let ops = vec![Op::push(5), Op::call("is-error")];
        let (result, _) = execute(&ops, stack, ctx.clone()).await.unwrap();
        assert_eq!(result.values()[0].as_bool().unwrap(), false);

        // [drop] try is-error -> true
        let stack = Stack::new();
        let ops = vec![
            Op::Quote(vec![Op::call("drop")]),
            Op::call("try"),
            Op::call("is-error"),
        ];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_bool().unwrap(), true);
    }

    #[tokio::test]
    async fn test_unwrap_value() {
        let ctx = setup().await;
        let stack = Stack::new();

        // 5 unwrap -> 5
        let ops = vec![Op::push(5), Op::call("unwrap")];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 5);
    }

    #[tokio::test]
    async fn test_unwrap_error() {
        let ctx = setup().await;
        let stack = Stack::new();

        // [drop] try unwrap -> stops with error
        let ops = vec![
            Op::Quote(vec![Op::call("drop")]),
            Op::call("try"),
            Op::call("unwrap"),
        ];
        let result = execute(&ops, stack, ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dup() {
        let ctx = setup().await;
        let stack = Stack::new();

        let ops = vec![Op::push(42), Op::call("dup")];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 2);
        assert_eq!(result.values()[0].as_int().unwrap(), 42);
        assert_eq!(result.values()[1].as_int().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_drop() {
        let ctx = setup().await;
        let stack = Stack::new();

        let ops = vec![Op::push(1), Op::push(2), Op::call("drop")];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_swap() {
        let ctx = setup().await;
        let stack = Stack::new();

        let ops = vec![Op::push(1), Op::push(2), Op::call("swap")];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 2);
        assert_eq!(result.values()[1].as_int().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_over() {
        let ctx = setup().await;
        let stack = Stack::new();

        let ops = vec![Op::push(1), Op::push(2), Op::call("over")];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 3);
        assert_eq!(result.values()[0].as_int().unwrap(), 1);
        assert_eq!(result.values()[1].as_int().unwrap(), 2);
        assert_eq!(result.values()[2].as_int().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_rot() {
        let ctx = setup().await;
        let stack = Stack::new();

        // 1 2 3 rot -> 2 3 1
        let ops = vec![
            Op::push(1),
            Op::push(2),
            Op::push(3),
            Op::call("rot"),
        ];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 2);
        assert_eq!(result.values()[1].as_int().unwrap(), 3);
        assert_eq!(result.values()[2].as_int().unwrap(), 1);
    }
}
