//! Definition primitives (3)
//!
//! - def: (quote name -- ) define a new tool
//! - def-verified: (quote name sig -- ) define with effect verification
//! - words: ( -- list) list all defined tools

use crate::analyzer;
use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::types::Effect;
use crate::value::Value;

pub fn register(dict: &mut Dictionary) {
    // def: (quote name -- )
    // Usage: [ body ] "name" def
    dict.register(Tool::native(
        "def",
        "(body:Quote name:Text -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name = stack.pop()?.into_text()?;
                let body = stack.pop()?.into_quote()?;
                let tool = Tool::composed(&name, None, body);
                {
                    let mut dict = ctx.dict.write().await;
                    dict.register(tool);
                }
                Ok((stack, ctx))
            })
        },
    ));

    // def-verified: (quote name sig -- )
    // Defines tool ONLY if inferred effect matches declared signature
    // Usage: [ dup mul ] "square" "(n -- n)" def-verified
    dict.register(Tool::native(
        "def-verified",
        "(code:Quote name:Text sig:Text -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let sig = stack.pop()?.into_text()?;
                let name = stack.pop()?.into_text()?;
                let quote = stack.pop()?.into_quote()?;
                
                // Parse declared effect
                let declared = Effect::parse(&sig)
                    .map_err(|e| Error::Runtime(format!("def-verified: invalid signature: {}", e)))?;
                
                // Infer actual effect
                let analysis = analyzer::analyze(&quote);
                
                // Check for stack errors
                if analysis.has_errors() {
                    let error_msgs: Vec<String> = analysis.errors.iter()
                        .map(|e| e.message.clone())
                        .collect();
                    return Err(Error::Runtime(format!(
                        "def-verified: quote has stack errors: {}", 
                        error_msgs.join(", ")
                    )));
                }
                
                // Compare effects
                if analysis.effect != declared {
                    return Err(Error::EffectMismatch {
                        expected: format!("{}", declared),
                        got: format!("{}", analysis.effect),
                    });
                }
                
                // Define the tool with verified signature
                let tool = Tool::composed(&name, Some(&sig), quote);
                {
                    let mut dict = ctx.dict.write().await;
                    dict.register(tool);
                }
                
                Ok((stack, ctx))
            })
        },
    ).with_doc("Define tool only if inferred effect matches declared signature"));

    // words: ( -- list)
    dict.register(Tool::native(
        "words",
        "( -- names:List)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let dict = ctx.dict.read().await;
                let names: Vec<Value> = dict.list().into_iter().map(Value::Text).collect();
                drop(dict);
                stack.push(Value::List(names))?;
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
        }
        ctx
    }

    #[tokio::test]
    async fn test_def() {
        let ctx = setup().await;
        // [ dup add ] "double" def  5 double -> 10
        let ops = vec![
            Op::quote(vec![Op::call("dup"), Op::call("add")]),
            Op::Push(Value::Text("double".into())),
            Op::call("def"),
            Op::push(5),
            Op::call("double"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 10);
    }

    #[tokio::test]
    async fn test_words() {
        let ctx = setup().await;
        let ops = vec![Op::call("words")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        let names = result.values()[0].as_list().unwrap();
        // Should contain at least def, words, dup, add
        let name_strs: Vec<_> = names.iter().filter_map(|v| v.as_text().ok()).collect();
        assert!(name_strs.contains(&"def"));
        assert!(name_strs.contains(&"words"));
        assert!(name_strs.contains(&"dup"));
    }
}
