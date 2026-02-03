//! Logic primitives (3)
//!
//! - and: (a b -- a&&b)
//! - or: (a b -- a||b)
//! - not: (a -- !a)

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

pub fn register(dict: &mut Dictionary) {
    // and: (a b -- a&&b)
    dict.register(Tool::native(
        "and",
        "(a:Bool b:Bool -- c:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.into_bool()?;
                let a = stack.pop()?.into_bool()?;
                stack.push(Value::Bool(a && b))?;
                Ok((stack, ctx))
            })
        },
    ));

    // or: (a b -- a||b)
    dict.register(Tool::native(
        "or",
        "(a:Bool b:Bool -- c:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.into_bool()?;
                let a = stack.pop()?.into_bool()?;
                stack.push(Value::Bool(a || b))?;
                Ok((stack, ctx))
            })
        },
    ));

    // not: (a -- !a)
    dict.register(Tool::native(
        "not",
        "(a:Bool -- b:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let a = stack.pop()?.into_bool()?;
                stack.push(Value::Bool(!a))?;
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
    async fn test_and() {
        let ctx = setup().await;
        
        // true && true = true
        let ops = vec![Op::Push(Value::Bool(true)), Op::Push(Value::Bool(true)), Op::call("and")];
        let (result, _) = execute(&ops, Stack::new(), ctx.clone()).await.unwrap();
        assert!(result.values()[0].as_bool().unwrap());

        // true && false = false
        let ops = vec![Op::Push(Value::Bool(true)), Op::Push(Value::Bool(false)), Op::call("and")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(!result.values()[0].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_or() {
        let ctx = setup().await;
        
        // false || true = true
        let ops = vec![Op::Push(Value::Bool(false)), Op::Push(Value::Bool(true)), Op::call("or")];
        let (result, _) = execute(&ops, Stack::new(), ctx.clone()).await.unwrap();
        assert!(result.values()[0].as_bool().unwrap());

        // false || false = false
        let ops = vec![Op::Push(Value::Bool(false)), Op::Push(Value::Bool(false)), Op::call("or")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(!result.values()[0].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_not() {
        let ctx = setup().await;
        
        let ops = vec![Op::Push(Value::Bool(true)), Op::call("not")];
        let (result, _) = execute(&ops, Stack::new(), ctx.clone()).await.unwrap();
        assert!(!result.values()[0].as_bool().unwrap());

        let ops = vec![Op::Push(Value::Bool(false)), Op::call("not")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(result.values()[0].as_bool().unwrap());
    }
}
