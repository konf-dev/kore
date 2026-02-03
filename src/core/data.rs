//! Data primitives (3)
//!
//! - list: (n -- [items]) collect n items from stack into list
//! - unlist: ([items] -- ...items) spread list onto stack
//! - map-new: ( -- {}) create empty map

use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

pub fn register(dict: &mut Dictionary) {
    // list: (n -- [items])
    dict.register(Tool::native(
        "list",
        "(n:Int -- l:List)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let n = stack.pop()?.into_int()? as usize;
                if n > stack.depth() {
                    return Err(Error::StackUnderflow {
                        expected: n,
                        actual: stack.depth(),
                    });
                }
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(stack.pop()?);
                }
                items.reverse(); // Stack order to list order
                stack.push(Value::List(items))?;
                Ok((stack, ctx))
            })
        },
    ));

    // unlist: ([items] -- ...items)
    dict.register(Tool::native(
        "unlist",
        "(l:List -- ...)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let list = stack.pop()?.into_list()?;
                for item in list {
                    stack.push(item)?;
                }
                Ok((stack, ctx))
            })
        },
    ));

    // map-new: ( -- {})
    dict.register(Tool::native(
        "map-new",
        "( -- m:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                stack.push(Value::Map(indexmap::IndexMap::new()))?;
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
    async fn test_list() {
        let ctx = setup().await;
        // 1 2 3 3 list -> [1, 2, 3]
        let ops = vec![
            Op::push(1),
            Op::push(2),
            Op::push(3),
            Op::push(3),
            Op::call("list"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        let list = result.values()[0].as_list().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].as_int().unwrap(), 1);
        assert_eq!(list[1].as_int().unwrap(), 2);
        assert_eq!(list[2].as_int().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_unlist() {
        let ctx = setup().await;
        // [1, 2, 3] unlist -> 1 2 3
        let ops = vec![
            Op::Push(Value::List(vec![
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
            ])),
            Op::call("unlist"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 3);
        assert_eq!(result.values()[0].as_int().unwrap(), 1);
        assert_eq!(result.values()[1].as_int().unwrap(), 2);
        assert_eq!(result.values()[2].as_int().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_map_new() {
        let ctx = setup().await;
        let ops = vec![Op::call("map-new")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        let map = result.values()[0].as_map().unwrap();
        assert!(map.is_empty());
    }

    #[tokio::test]
    async fn test_list_unlist_roundtrip() {
        let ctx = setup().await;
        // 1 2 3 3 list unlist -> 1 2 3
        let ops = vec![
            Op::push(1),
            Op::push(2),
            Op::push(3),
            Op::push(3),
            Op::call("list"),
            Op::call("unlist"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 3);
        assert_eq!(result.values()[0].as_int().unwrap(), 1);
        assert_eq!(result.values()[1].as_int().unwrap(), 2);
        assert_eq!(result.values()[2].as_int().unwrap(), 3);
    }
}
