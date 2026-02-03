//! List primitives (8)
//!
//! Operations on List values that require internal access.
//! These cannot be composed from other primitives.

use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register all list primitives
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "list-len",
        "(list -- n)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let val = stack.pop()?;
                let list = match &val {
                    Value::List(l) => l,
                    _ => return Err(Error::type_error("list", &val)),
                };
                stack.push(Value::Int(list.len() as i64))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "list-get",
        "(list n -- item)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let n = stack.pop()?.as_int()? as usize;
                let val = stack.pop()?;
                let list = match &val {
                    Value::List(l) => l,
                    _ => return Err(Error::type_error("list", &val)),
                };
                if n >= list.len() {
                    return Err(Error::Runtime(format!(
                        "list-get: index {} out of bounds for list of length {}",
                        n, list.len()
                    )));
                }
                stack.push(list[n].clone())?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "list-set",
        "(list n item -- list')",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let item = stack.pop()?;
                let n = stack.pop()?.as_int()? as usize;
                let val = stack.pop()?;
                let mut list = match val {
                    Value::List(l) => l,
                    _ => return Err(Error::type_error("list", &val)),
                };
                if n >= list.len() {
                    return Err(Error::Runtime(format!(
                        "list-set: index {} out of bounds for list of length {}",
                        n, list.len()
                    )));
                }
                list[n] = item;
                stack.push(Value::List(list))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "list-push",
        "(list item -- list')",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let item = stack.pop()?;
                let val = stack.pop()?;
                let mut list = match val {
                    Value::List(l) => l,
                    _ => return Err(Error::type_error("list", &val)),
                };
                list.push(item);
                stack.push(Value::List(list))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "list-pop",
        "(list -- list' item)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let val = stack.pop()?;
                let mut list = match val {
                    Value::List(l) => l,
                    _ => return Err(Error::type_error("list", &val)),
                };
                if list.is_empty() {
                    return Err(Error::Runtime("list-pop: empty list".to_string()));
                }
                let item = list.pop().unwrap();
                stack.push(Value::List(list))?;
                stack.push(item)?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "list-slice",
        "(list start end -- list')",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let end = stack.pop()?.as_int()? as usize;
                let start = stack.pop()?.as_int()? as usize;
                let val = stack.pop()?;
                let list = match &val {
                    Value::List(l) => l,
                    _ => return Err(Error::type_error("list", &val)),
                };
                let end = end.min(list.len());
                let start = start.min(end);
                stack.push(Value::List(list[start..end].to_vec()))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "list-concat",
        "(list1 list2 -- list)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let val2 = stack.pop()?;
                let list2 = match val2 {
                    Value::List(l) => l,
                    _ => return Err(Error::type_error("list", &val2)),
                };
                let val1 = stack.pop()?;
                let mut list1 = match val1 {
                    Value::List(l) => l,
                    _ => return Err(Error::type_error("list", &val1)),
                };
                list1.extend(list2);
                stack.push(Value::List(list1))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "list-reverse",
        "(list -- list')",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let val = stack.pop()?;
                let mut list = match val {
                    Value::List(l) => l,
                    _ => return Err(Error::type_error("list", &val)),
                };
                list.reverse();
                stack.push(Value::List(list))?;
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
        let mut ctx = Context::new();
        crate::core::register_core(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn test_list_len() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
            Op::call("list-len"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Int(3));
    }

    #[tokio::test]
    async fn test_list_get() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::List(vec![Value::Int(10), Value::Int(20), Value::Int(30)])),
            Op::push(1),
            Op::call("list-get"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Int(20));
    }

    #[tokio::test]
    async fn test_list_push_pop() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::List(vec![Value::Int(1)])),
            Op::push(2),
            Op::call("list-push"),
            Op::call("list-pop"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::List(vec![Value::Int(1)]));
        assert_eq!(result.values()[1], Value::Int(2));
    }

    #[tokio::test]
    async fn test_list_reverse() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
            Op::call("list-reverse"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(
            result.values()[0],
            Value::List(vec![Value::Int(3), Value::Int(2), Value::Int(1)])
        );
    }
}
