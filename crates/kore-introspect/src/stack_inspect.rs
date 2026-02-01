//! # Stack Introspection
//!
//! Inspect the current stack state.
//!
//! ## Purpose
//!
//! Sometimes agents need to understand the current stack:
//! - How many values are on the stack?
//! - What are all the current values?
//!
//! ## Tools
//!
//! | Tool | Effect | Description |
//! |------|--------|-------------|
//! | `depth` | `(-- int)` | Get the number of values on the stack |
//! | `stack-list` | `(-- list)` | Get all stack values as a list (non-destructive) |
//! | `clear` | `(... --)` | Remove all values from the stack |

use kore::{Context, Stack, Tool, Value};

/// Register stack introspection tools into a context.
///
/// # Tools Registered
///
/// - `depth`: Get the number of values on the stack
/// - `stack-list`: Get all stack values as a list
/// - `clear`: Remove all values from the stack
pub async fn register_stack_tools(ctx: &mut Context) {
    // depth: (-- int)
    ctx.dict.write().await.register(
        Tool::native("depth", "(-- int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let len = stack.values().len();
                stack.push(Value::Int(len as i64))?;
                Ok((stack, ctx))
            })
        }),
    );

    // stack-list: (-- list)
    ctx.dict.write().await.register(
        Tool::native("stack-list", "(-- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let values = stack.values().to_vec();
                stack.push(Value::List(values))?;
                Ok((stack, ctx))
            })
        }),
    );

    // clear: (... --)
    ctx.dict.write().await.register(
        Tool::native("clear", "(... --)", |_stack: Stack, ctx: Context| {
            Box::pin(async move {
                // Return a fresh empty stack
                Ok((Stack::new(), ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use kore::{execute, Op, register_builtins};

    #[tokio::test]
    async fn test_depth() {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        register_stack_tools(&mut ctx).await;

        let ops = vec![
            Op::push(1),
            Op::push(2),
            Op::push(3),
            Op::call("depth"),
        ];

        let stack = Stack::new();
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();

        // Stack has: 1, 2, 3, 3 (depth)
        assert_eq!(result.values().len(), 4);
        assert_eq!(result.values()[3], Value::Int(3));
    }

    #[tokio::test]
    async fn test_stack_list() {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        register_stack_tools(&mut ctx).await;

        let ops = vec![
            Op::push(1),
            Op::push(2),
            Op::call("stack-list"),
        ];

        let stack = Stack::new();
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();

        // Stack has: 1, 2, [1, 2]
        assert_eq!(result.values().len(), 3);
        let list = result.values()[2].as_list().unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_clear() {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        register_stack_tools(&mut ctx).await;

        let ops = vec![
            Op::push(1),
            Op::push(2),
            Op::push(3),
            Op::call("clear"),
        ];

        let stack = Stack::new();
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();

        assert!(result.values().is_empty());
    }
}
