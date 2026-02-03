//! Arithmetic primitives (6)
//!
//! - add: (a b -- a+b)
//! - sub: (a b -- a-b)
//! - mul: (a b -- a*b)
//! - div: (a b -- a/b)
//! - mod: (a b -- a%b)
//! - neg: (a -- -a)

use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

pub fn register(dict: &mut Dictionary) {
    // add: (a b -- a+b)
    dict.register(Tool::native(
        "add",
        "(a:Num b:Num -- c:Num)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?;
                let a = stack.pop()?;
                let result = match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => Value::Int(x + y),
                    (Value::Float(x), Value::Float(y)) => Value::Float(x + y),
                    (Value::Int(x), Value::Float(y)) => Value::Float(*x as f64 + y),
                    (Value::Float(x), Value::Int(y)) => Value::Float(x + *y as f64),
                    _ => return Err(Error::type_error("Num", &a)),
                };
                stack.push(result)?;
                Ok((stack, ctx))
            })
        },
    ));

    // sub: (a b -- a-b)
    dict.register(Tool::native(
        "sub",
        "(a:Num b:Num -- c:Num)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?;
                let a = stack.pop()?;
                let result = match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => Value::Int(x - y),
                    (Value::Float(x), Value::Float(y)) => Value::Float(x - y),
                    (Value::Int(x), Value::Float(y)) => Value::Float(*x as f64 - y),
                    (Value::Float(x), Value::Int(y)) => Value::Float(x - *y as f64),
                    _ => return Err(Error::type_error("Num", &a)),
                };
                stack.push(result)?;
                Ok((stack, ctx))
            })
        },
    ));

    // mul: (a b -- a*b)
    dict.register(Tool::native(
        "mul",
        "(a:Num b:Num -- c:Num)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?;
                let a = stack.pop()?;
                let result = match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => Value::Int(x * y),
                    (Value::Float(x), Value::Float(y)) => Value::Float(x * y),
                    (Value::Int(x), Value::Float(y)) => Value::Float(*x as f64 * y),
                    (Value::Float(x), Value::Int(y)) => Value::Float(x * *y as f64),
                    _ => return Err(Error::type_error("Num", &a)),
                };
                stack.push(result)?;
                Ok((stack, ctx))
            })
        },
    ));

    // div: (a b -- a/b)
    dict.register(Tool::native(
        "div",
        "(a:Num b:Num -- c:Num)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?;
                let a = stack.pop()?;
                let result = match (&a, &b) {
                    (Value::Int(_), Value::Int(0)) => {
                        return Err(Error::Runtime("division by zero".into()))
                    }
                    (Value::Int(x), Value::Int(y)) => Value::Int(x / y),
                    (Value::Float(x), Value::Float(y)) => Value::Float(x / y),
                    (Value::Int(x), Value::Float(y)) => Value::Float(*x as f64 / y),
                    (Value::Float(x), Value::Int(y)) => Value::Float(x / *y as f64),
                    _ => return Err(Error::type_error("Num", &a)),
                };
                stack.push(result)?;
                Ok((stack, ctx))
            })
        },
    ));

    // mod: (a b -- a%b)
    dict.register(Tool::native(
        "mod",
        "(a:Int b:Int -- c:Int)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?.into_int()?;
                let a = stack.pop()?.into_int()?;
                if b == 0 {
                    return Err(Error::Runtime("modulo by zero".into()));
                }
                stack.push(Value::Int(a % b))?;
                Ok((stack, ctx))
            })
        },
    ));

    // neg: (a -- -a)
    dict.register(Tool::native(
        "neg",
        "(a:Num -- b:Num)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let a = stack.pop()?;
                let result = match a {
                    Value::Int(x) => Value::Int(-x),
                    Value::Float(x) => Value::Float(-x),
                    _ => return Err(Error::type_error("Num", &a)),
                };
                stack.push(result)?;
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
    async fn test_add() {
        let ctx = setup().await;
        let ops = vec![Op::push(3), Op::push(5), Op::call("add")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 8);
    }

    #[tokio::test]
    async fn test_sub() {
        let ctx = setup().await;
        let ops = vec![Op::push(10), Op::push(3), Op::call("sub")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 7);
    }

    #[tokio::test]
    async fn test_mul() {
        let ctx = setup().await;
        let ops = vec![Op::push(4), Op::push(5), Op::call("mul")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 20);
    }

    #[tokio::test]
    async fn test_div() {
        let ctx = setup().await;
        let ops = vec![Op::push(20), Op::push(4), Op::call("div")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 5);
    }

    #[tokio::test]
    async fn test_mod() {
        let ctx = setup().await;
        let ops = vec![Op::push(17), Op::push(5), Op::call("mod")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 2);
    }

    #[tokio::test]
    async fn test_neg() {
        let ctx = setup().await;
        let ops = vec![Op::push(42), Op::call("neg")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), -42);
    }

    #[tokio::test]
    async fn test_mixed_types() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Int(3)),
            Op::Push(Value::Float(2.5)),
            Op::call("add"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_float().unwrap(), 5.5);
    }
}
