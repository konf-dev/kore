//! Higher-Order Combinators
//!
//! Functional programming combinators for transforming lists.
//! These are native because they require internal iteration.
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | map | (list quote -- list) | Transform each element |
//! | filter | (list quote -- list) | Keep elements matching predicate |
//! | fold | (list init quote -- value) | Reduce to single value |
//! | each | (list quote -- ) | Execute for each, discard results |

use crate::context::{Context, Dictionary};
use crate::executor::execute;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register combinator tools (4)
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "map",
        "(list quote -- list)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                let list = stack.pop()?.into_list()?;
                let mut result = Vec::with_capacity(list.len());
                
                for item in list {
                    let mut item_stack = Stack::new();
                    item_stack.push(item)?;
                    let (result_stack, _) = execute(&quote, item_stack, ctx.clone()).await?;
                    if let Some(v) = result_stack.values().last() {
                        result.push(v.clone());
                    }
                }
                
                stack.push(Value::List(result))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "filter",
        "(list quote -- list)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                let list = stack.pop()?.into_list()?;
                let mut result = Vec::new();
                
                for item in list {
                    let mut item_stack = Stack::new();
                    item_stack.push(item.clone())?;
                    let (result_stack, _) = execute(&quote, item_stack, ctx.clone()).await?;
                    if let Some(v) = result_stack.values().last() {
                        if v.is_truthy() {
                            result.push(item);
                        }
                    }
                }
                
                stack.push(Value::List(result))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "fold",
        "(list init quote -- value)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                let init = stack.pop()?;
                let list = stack.pop()?.into_list()?;
                
                let mut acc = init;
                for item in list {
                    let mut fold_stack = Stack::new();
                    fold_stack.push(acc)?;
                    fold_stack.push(item)?;
                    let (result_stack, _) = execute(&quote, fold_stack, ctx.clone()).await?;
                    acc = result_stack
                        .values()
                        .last()
                        .ok_or_else(|| {
                            crate::error::Error::Runtime(
                                "fold: quote must return a value".into(),
                            )
                        })?
                        .clone();
                }
                
                stack.push(acc)?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "each",
        "(list quote -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                let list = stack.pop()?.into_list()?;
                
                for item in list {
                    let mut item_stack = Stack::new();
                    item_stack.push(item)?;
                    execute(&quote, item_stack, ctx.clone()).await?;
                }
                
                Ok((stack, ctx))
            })
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::Op;

    async fn setup() -> Context {
        let mut ctx = Context::new();
        crate::core::register_core(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn test_map() {
        let ctx = setup().await;
        // [1 2 3] [ 2 mul ] map → [2 4 6]
        let ops = vec![
            Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
            Op::Push(Value::Quote(vec![Op::push(2), Op::call("mul")])),
            Op::call("map"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(
            result.values()[0],
            Value::List(vec![Value::Int(2), Value::Int(4), Value::Int(6)])
        );
    }

    #[tokio::test]
    async fn test_filter() {
        let ctx = setup().await;
        // [1 2 3 4] [ 2 mod 0 eq ] filter → [2 4]
        let ops = vec![
            Op::Push(Value::List(vec![
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
                Value::Int(4),
            ])),
            Op::Push(Value::Quote(vec![
                Op::push(2),
                Op::call("mod"),
                Op::push(0),
                Op::call("eq"),
            ])),
            Op::call("filter"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(
            result.values()[0],
            Value::List(vec![Value::Int(2), Value::Int(4)])
        );
    }

    #[tokio::test]
    async fn test_fold() {
        let ctx = setup().await;
        // [1 2 3] 0 [ add ] fold → 6
        let ops = vec![
            Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
            Op::push(0),
            Op::Push(Value::Quote(vec![Op::call("add")])),
            Op::call("fold"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Int(6));
    }

    #[tokio::test]
    async fn test_each() {
        let ctx = setup().await;
        // [1 2 3] [ drop ] each → (empty)
        let ops = vec![
            Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
            Op::Push(Value::Quote(vec![Op::call("drop")])),
            Op::call("each"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(result.values().is_empty());
    }
}
