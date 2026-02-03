//! Stack primitives (7)
//!
//! - dup: (a -- a a)
//! - drop: (a -- )
//! - swap: (a b -- b a)
//! - rot: (a b c -- b c a)
//! - over: (a b -- a b a)
//! - dip: (a q -- ... a)
//! - depth: ( -- n)

use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::executor::execute;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

pub fn register(dict: &mut Dictionary) {
    // dup: (a -- a a) - REJECTS linear values
    dict.register(Tool::native(
        "dup",
        "(a:Any -- a:Any a:Any)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let a = stack.pop()?;
                // Linear values cannot be duplicated
                if a.is_linear() {
                    return Err(Error::LinearDuplicate(format!("{}", a)));
                }
                stack.push(a.clone())?;
                stack.push(a)?;
                Ok((stack, ctx))
            })
        },
    ));

    // drop: (a -- ) - REJECTS linear values  
    dict.register(Tool::native(
        "drop",
        "(a:Any -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let a = stack.pop()?;
                // Linear values cannot be discarded
                if a.is_linear() {
                    return Err(Error::LinearDiscard(format!("{}", a)));
                }
                Ok((stack, ctx))
            })
        },
    ));

    // swap: (a b -- b a)
    dict.register(Tool::native(
        "swap",
        "(a:Any b:Any -- b:Any a:Any)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(b)?;
                stack.push(a)?;
                Ok((stack, ctx))
            })
        },
    ));

    // rot: (a b c -- b c a)
    dict.register(Tool::native(
        "rot",
        "(a:Any b:Any c:Any -- b:Any c:Any a:Any)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let c = stack.pop()?;
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(b)?;
                stack.push(c)?;
                stack.push(a)?;
                Ok((stack, ctx))
            })
        },
    ));

    // over: (a b -- a b a)
    dict.register(Tool::native(
        "over",
        "(a:Any b:Any -- a:Any b:Any a:Any)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a.clone())?;
                stack.push(b)?;
                stack.push(a)?;
                Ok((stack, ctx))
            })
        },
    ));

    // dip: (a q -- ... a)
    // Execute the quote with the top value temporarily removed,
    // then restore the top value
    dict.register(Tool::native(
        "dip",
        "(a:Any q:Quote -- ...)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                let top = stack.pop()?;
                // Execute quote on remaining stack
                let (mut result_stack, result_ctx) = execute(&quote, stack, ctx).await?;
                // Restore the saved value
                result_stack.push(top)?;
                Ok((result_stack, result_ctx))
            })
        },
    ));

    // depth: ( -- n)
    dict.register(Tool::native(
        "depth",
        "( -- n:Int)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let d = stack.depth();
                stack.push(Value::Int(d as i64))?;
                Ok((stack, ctx))
            })
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::execute;
    use crate::op::Op;

    async fn setup() -> Context {
        let ctx = Context::new();
        let mut dict = ctx.dict.write().await;
        register(&mut dict);
        drop(dict);
        ctx
    }

    #[tokio::test]
    async fn test_dup() {
        let ctx = setup().await;
        let ops = vec![Op::push(42), Op::call("dup")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 2);
        assert_eq!(result.values()[0].as_int().unwrap(), 42);
        assert_eq!(result.values()[1].as_int().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_drop() {
        let ctx = setup().await;
        let ops = vec![Op::push(1), Op::push(2), Op::call("drop")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_swap() {
        let ctx = setup().await;
        let ops = vec![Op::push(1), Op::push(2), Op::call("swap")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 2);
        assert_eq!(result.values()[1].as_int().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_rot() {
        let ctx = setup().await;
        let ops = vec![
            Op::push(1),
            Op::push(2),
            Op::push(3),
            Op::call("rot"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        // 1 2 3 rot -> 2 3 1
        assert_eq!(result.values()[0].as_int().unwrap(), 2);
        assert_eq!(result.values()[1].as_int().unwrap(), 3);
        assert_eq!(result.values()[2].as_int().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_depth() {
        let ctx = setup().await;
        let ops = vec![Op::push(1), Op::push(2), Op::push(3), Op::call("depth")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[3].as_int().unwrap(), 3);
    }
}
