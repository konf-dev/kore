//! Introspection tools (3)
//!
//! Tools for reflecting on the tool dictionary and metadata.
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | meta | (name -- map) | Get tool metadata |
//! | meta! | (name key value -- ) | Set metadata field |
//! | defined? | (name -- bool) | Check if tool exists |

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register introspection tools (3)
pub fn register(dict: &mut Dictionary) {
    // meta: (name -- map) - get metadata for a tool
    dict.register(Tool::native(
        "meta",
        "(name:Text -- meta:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name = stack.pop()?.into_text()?;
                let dict = ctx.dict.read().await;
                let tool = dict.get(&name)?;
                drop(dict);
                
                stack.push(tool.meta.to_value())?;
                Ok((stack, ctx))
            })
        },
    ));

    // meta!: (name key value -- ) - set metadata field
    dict.register(Tool::native(
        "meta!",
        "(name:Text key:Text val:Text -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?.into_text()?;
                let key = stack.pop()?.into_text()?;
                let name = stack.pop()?.into_text()?;
                
                let mut dict = ctx.dict.write().await;
                if let Ok(mut tool) = dict.get(&name) {
                    match key.as_str() {
                        "doc" => tool.meta = tool.meta.clone().with_doc(&value),
                        "status" => tool.meta.status = value,
                        _ => tool.meta.set(&key, &value),
                    }
                    dict.register(tool);
                }
                drop(dict);
                
                Ok((stack, ctx))
            })
        },
    ));

    // defined?: (name -- bool) - check if tool exists
    dict.register(Tool::native(
        "defined?",
        "(name:Text -- exists:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name = stack.pop()?.into_text()?;
                let dict = ctx.dict.read().await;
                let exists = dict.get(&name).is_ok();
                drop(dict);
                
                stack.push(Value::Bool(exists))?;
                Ok((stack, ctx))
            })
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::core::register_core;
    use crate::executor::execute;
    use crate::op::Op;

    async fn setup() -> Context {
        let mut ctx = Context::new();
        register_core(&mut ctx).await;
        let mut dict = ctx.dict.write().await;
        register(&mut dict);
        drop(dict);
        ctx
    }

    #[tokio::test]
    async fn test_defined() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("dup".into())),
            Op::call("defined?"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(result.values()[0].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_defined_false() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("nonexistent".into())),
            Op::call("defined?"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(!result.values()[0].as_bool().unwrap());
    }
}
