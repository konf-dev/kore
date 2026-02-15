//! Definition primitives (3)
//!
//! - def: (quote name -- ) define a new tool
//! - words: ( -- list) list all defined tools
//! - describe: (name -- sig) get a tool's type signature

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

pub fn register(dict: &mut Dictionary) {
    // def: define a new tool
    // Accepts multiple patterns:
    //   "name" [body] def   -- idiomatic (prelude style)
    //   [body] "name" def   -- stack style
    //   value "name" def    -- constant (wraps value in a quote that pushes it)
    dict.register(Tool::native(
        "def",
        "(name:Text body -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let a = stack.pop()?;
                let b = stack.pop()?;
                let (name, body) = match (a, b) {
                    // "name" [body] def → top=Quote, second=Text
                    (Value::Quote(q), Value::Text(n)) => (n, q),
                    // [body] "name" def → top=Text, second=Quote
                    (Value::Text(n), Value::Quote(q)) => (n, q),
                    // value "name" def → constant: wrap value in a push
                    (Value::Text(n), val) => {
                        (n, vec![crate::op::Op::Push(val)])
                    }
                    _ => {
                        return Err(crate::error::Error::TypeError {
                            expected: "Text name and body (Quote or value)".into(),
                            got: "incompatible types".into(),
                        });
                    }
                };
                let tool = Tool::composed(&name, None, body);
                {
                    let mut dict = ctx.dict.write().await;
                    dict.register(tool);
                }
                Ok((stack, ctx))
            })
        },
    ));

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

    // describe: (name -- sig) - get a tool's type signature as text
    //
    // Returns the signature string (e.g., "(a:Int b:Int -- sum:Int)").
    // For tools without a signature, returns "( -- )".
    // Errors if the tool doesn't exist (ToolNotFound).
    dict.register(Tool::native(
        "describe",
        "(name:Text -- sig:Text)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name = stack.pop()?.into_text()?;
                let dict = ctx.dict.read().await;
                let tool = dict.get(&name)?;
                drop(dict);
                let sig = tool.meta.sig.unwrap_or_else(|| "( -- )".to_string());
                stack.push(Value::Text(sig))?;
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

    #[tokio::test]
    async fn test_describe() {
        let ctx = setup().await;
        // "dup" describe -> "( a -- a a )" or similar sig
        let ops = vec![
            Op::Push(Value::Text("dup".into())),
            Op::call("describe"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        let sig = result.values()[0].as_text().unwrap();
        assert!(sig.contains("("), "Expected signature format, got: {sig}");
        assert!(sig.contains(")"), "Expected signature format, got: {sig}");
    }

    #[tokio::test]
    async fn test_describe_unknown_tool() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("nonexistent".into())),
            Op::call("describe"),
        ];
        let result = execute(&ops, Stack::new(), ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_describe_user_defined() {
        let ctx = setup().await;
        // [ dup add ] "double" def  "double" describe -> "( -- )"  (no sig for composed)
        let ops = vec![
            Op::quote(vec![Op::call("dup"), Op::call("add")]),
            Op::Push(Value::Text("double".into())),
            Op::call("def"),
            Op::Push(Value::Text("double".into())),
            Op::call("describe"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        let sig = result.values()[0].as_text().unwrap();
        assert!(sig.contains("("), "Expected signature format, got: {sig}");
    }
}
