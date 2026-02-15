//! Compose primitive (1)
//!
//! - compose: (q1 q2 -- q3) concatenate two quotes into one
//!
//! This is Postulate 3 made concrete: composition IS concatenation.
//! Since quotes are `Vec<Op>`, compose is literally `Vec::extend`.

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

pub fn register(dict: &mut Dictionary) {
    // compose: (q1 q2 -- q3) - concatenate two quotes
    //
    // Applying q3 is equivalent to applying q1 then q2.
    // This lets a model build complex quotes incrementally:
    //   [dup] [add] compose  →  [dup add]
    dict.register(Tool::native(
        "compose",
        "(q1:Quote q2:Quote -- q3:Quote)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let q2 = stack.pop()?.into_quote()?;
                let q1 = stack.pop()?.into_quote()?;
                let mut combined = q1;
                combined.extend(q2);
                stack.push(Value::Quote(combined))?;
                Ok((stack, ctx))
            })
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core;
    use crate::executor::execute;
    use crate::op::Op;

    async fn setup() -> Context {
        let ctx = Context::new();
        {
            let mut dict = ctx.dict.write().await;
            register(&mut dict);
            core::stack::register(&mut dict);
            core::arithmetic::register(&mut dict);
            core::execution::register(&mut dict);
        }
        ctx
    }

    #[tokio::test]
    async fn test_compose_basic() {
        let ctx = setup().await;
        // [dup] [add] compose call on 5  →  10
        let ops = vec![
            Op::push(5),
            Op::quote(vec![Op::call("dup")]),
            Op::quote(vec![Op::call("add")]),
            Op::call("compose"),
            Op::call("call"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 10);
    }

    #[tokio::test]
    async fn test_compose_two_empties() {
        let ctx = setup().await;
        // [] [] compose  →  []
        let ops = vec![
            Op::quote(vec![]),
            Op::quote(vec![]),
            Op::call("compose"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        let q = result.values()[0].as_quote().unwrap();
        assert!(q.is_empty());
    }

    #[tokio::test]
    async fn test_compose_identity_left() {
        let ctx = setup().await;
        // [] [dup] compose call on 7  →  7 7
        let ops = vec![
            Op::push(7),
            Op::quote(vec![]),
            Op::quote(vec![Op::call("dup")]),
            Op::call("compose"),
            Op::call("call"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 2);
        assert_eq!(result.values()[0].as_int().unwrap(), 7);
        assert_eq!(result.values()[1].as_int().unwrap(), 7);
    }

    #[tokio::test]
    async fn test_compose_identity_right() {
        let ctx = setup().await;
        // [dup] [] compose call on 7  →  7 7
        let ops = vec![
            Op::push(7),
            Op::quote(vec![Op::call("dup")]),
            Op::quote(vec![]),
            Op::call("compose"),
            Op::call("call"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 2);
        assert_eq!(result.values()[0].as_int().unwrap(), 7);
        assert_eq!(result.values()[1].as_int().unwrap(), 7);
    }

    #[tokio::test]
    async fn test_compose_nested_quotes() {
        let ctx = setup().await;
        // [5 [dup]] [add] compose  →  quote containing [5 [dup] add]
        let ops = vec![
            Op::quote(vec![
                Op::push(5),
                Op::quote(vec![Op::call("dup")]),
            ]),
            Op::quote(vec![Op::call("add")]),
            Op::call("compose"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        let q = result.values()[0].as_quote().unwrap();
        assert_eq!(q.len(), 3); // push(5), push(Quote([dup])), call("add")
    }

    #[tokio::test]
    async fn test_compose_chaining() {
        let ctx = setup().await;
        // Build [dup add 1 add] incrementally: [dup] [add] compose [1] compose [add] compose
        // Then call on 5  →  11
        let ops = vec![
            Op::push(5),
            Op::quote(vec![Op::call("dup")]),
            Op::quote(vec![Op::call("add")]),
            Op::call("compose"),
            Op::quote(vec![Op::push(1)]),
            Op::call("compose"),
            Op::quote(vec![Op::call("add")]),
            Op::call("compose"),
            Op::call("call"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 11);
    }

    #[tokio::test]
    async fn test_compose_non_quote_errors() {
        let ctx = setup().await;
        // 1 [add] compose  →  TypeError
        let ops = vec![
            Op::push(1),
            Op::quote(vec![Op::call("add")]),
            Op::call("compose"),
        ];
        let result = execute(&ops, Stack::new(), ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_compose_underflow() {
        let ctx = setup().await;
        // Just one quote → StackUnderflow
        let ops = vec![
            Op::quote(vec![Op::call("add")]),
            Op::call("compose"),
        ];
        let result = execute(&ops, Stack::new(), ctx).await;
        assert!(result.is_err());
    }
}
