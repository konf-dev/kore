//! Error primitives (4)
//!
//! - try: (quote -- ...| error) execute, catch errors as values
//! - fail: (msg -- !) raise error, never returns
//! - is-error: (a -- bool) check if value is an error
//! - error-info: (error -- map) convert error to structured map

use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::executor::execute;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::{ErrorValue, Value};

pub fn register(dict: &mut Dictionary) {
    // try: (quote -- ...| error)
    dict.register(Tool::native(
        "try",
        "(q:Quote -- ... | err:Error)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                match execute(&quote, stack, ctx.clone()).await {
                    Ok((new_stack, _)) => Ok((new_stack, ctx)),
                    Err(e) => {
                        let mut err_stack = Stack::new();
                        err_stack.push(Value::Error(Box::new(ErrorValue {
                            code: e.code().to_string(),
                            message: e.to_string(),
                        })))?;
                        Ok((err_stack, ctx))
                    }
                }
            })
        },
    ));

    // fail: (msg -- !)
    dict.register(Tool::native(
        "fail",
        "(msg:Text -- !)",
        |mut stack: Stack, _ctx: Context| {
            Box::pin(async move {
                let msg = stack.pop()?.into_text()?;
                Err(Error::Runtime(msg))
            })
        },
    ));

    // is-error: (a -- bool)
    dict.register(Tool::native(
        "is-error",
        "(v:Any -- result:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                stack.push(Value::Bool(matches!(v, Value::Error(_))))?;
                Ok((stack, ctx))
            })
        },
    ));

    // error-info: (error -- map)
    // Convert error to structured map for machine parsing
    dict.register(Tool::native(
        "error-info",
        "(err:Error -- info:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let val = stack.pop()?;
                match val {
                    Value::Error(e) => {
                        // Convert ErrorValue to structured map
                        use indexmap::IndexMap;
                        let mut m = IndexMap::new();
                        m.insert("code".into(), Value::Text(e.code.clone()));
                        m.insert("message".into(), Value::Text(e.message.clone()));
                        stack.push(Value::Map(m))?;
                    }
                    _ => {
                        return Err(Error::TypeError {
                            expected: "Error".to_string(),
                            got: val.type_name().to_string(),
                        });
                    }
                }
                Ok((stack, ctx))
            })
        },
    ).with_doc("Convert error to structured map for machine parsing"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core;
    use crate::op::Op;

    async fn setup() -> Context {
        let ctx = Context::new();
        {
            let mut dict = ctx.dict.write().await;
            register(&mut dict);
            // Need stack ops for some tests
            core::stack::register(&mut dict);
        }
        ctx
    }

    #[tokio::test]
    async fn test_try_success() {
        let ctx = setup().await;
        // [5] try -> 5
        let ops = vec![
            Op::quote(vec![Op::push(5)]),
            Op::call("try"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 5);
    }

    #[tokio::test]
    async fn test_try_error() {
        let ctx = setup().await;
        // [drop] try -> Error (stack underflow)
        let ops = vec![
            Op::quote(vec![Op::call("drop")]),
            Op::call("try"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert!(matches!(result.values()[0], Value::Error(_)));
    }

    #[tokio::test]
    async fn test_fail() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("boom".into())),
            Op::call("fail"),
        ];
        let result = execute(&ops, Stack::new(), ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_is_error_true() {
        let ctx = setup().await;
        // [drop] try is-error -> true
        let ops = vec![
            Op::quote(vec![Op::call("drop")]),
            Op::call("try"),
            Op::call("is-error"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(result.values()[0].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_is_error_false() {
        let ctx = setup().await;
        let ops = vec![Op::push(42), Op::call("is-error")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(!result.values()[0].as_bool().unwrap());
    }
}
