//! Comparison primitives (2)
//!
//! Only the irreducible:
//! - eq: (a b -- bool) structural equality
//! - lt: (a b -- bool) less than
//!
//! The rest compose from these:
//! - gt  = swap lt
//! - le  = gt not
//! - ge  = lt not
//! - neq = eq not

use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

pub fn register(dict: &mut Dictionary) {
    // eq: (a b -- bool)
    dict.register(Tool::native(
        "eq",
        "(a:Any b:Any -- result:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(Value::Bool(a == b))?;
                Ok((stack, ctx))
            })
        },
    ));

    // lt: (a b -- bool)
    dict.register(Tool::native(
        "lt",
        "(a:Num b:Num -- result:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?;
                let a = stack.pop()?;
                let result = match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => x < y,
                    (Value::Float(x), Value::Float(y)) => x < y,
                    (Value::Int(x), Value::Float(y)) => (*x as f64) < *y,
                    (Value::Float(x), Value::Int(y)) => *x < (*y as f64),
                    (Value::Text(x), Value::Text(y)) => x < y,
                    _ => return Err(Error::type_error("Num|Text", &a)),
                };
                stack.push(Value::Bool(result))?;
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
    async fn test_eq_true() {
        let ctx = setup().await;
        let ops = vec![Op::push(42), Op::push(42), Op::call("eq")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(result.values()[0].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_eq_false() {
        let ctx = setup().await;
        let ops = vec![Op::push(42), Op::push(43), Op::call("eq")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(!result.values()[0].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_lt_true() {
        let ctx = setup().await;
        let ops = vec![Op::push(3), Op::push(5), Op::call("lt")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(result.values()[0].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_lt_false() {
        let ctx = setup().await;
        let ops = vec![Op::push(5), Op::push(3), Op::call("lt")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(!result.values()[0].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_lt_equal() {
        let ctx = setup().await;
        let ops = vec![Op::push(5), Op::push(5), Op::call("lt")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(!result.values()[0].as_bool().unwrap());
    }
}
