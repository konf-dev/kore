//! Introspection tools: list-tools, tool-help

use kore::{Context, Stack, Tool, Value};

/// list-tools: (-- names)
pub fn list_tools_tool() -> Tool {
    Tool::native("list-tools", "(-- names:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let names: Vec<Value> = {
                let dict = ctx.dict.read().await;
                dict.list()
                    .into_iter()
                    .map(|k| Value::Text(k))
                    .collect()
            };
            stack.push(Value::List(names))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("List all available tool names.")
}

/// tool-help: (name -- info)
pub fn tool_help_tool() -> Tool {
    Tool::native("tool-help", "(name:Text -- info:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let name = stack.pop()?.as_text()?.to_string();
            
            let result = {
                let dict = ctx.dict.read().await;
                dict.get(&name)
            };
            
            match result {
                Ok(tool) => {
                    let effect_str = tool.effect.as_ref()
                        .map(|e| e.to_string())
                        .unwrap_or_default();
                    let doc_str = tool.doc.clone().unwrap_or_default();
                    
                    let info = Value::Map(indexmap::indexmap! {
                        "name".into() => Value::Text(name),
                        "effect".into() => Value::Text(effect_str),
                        "doc".into() => Value::Text(doc_str),
                    });
                    stack.push(info)?;
                }
                Err(_) => {
                    return Err(kore::Error::Runtime(format!("Tool not found: {}", name)));
                }
            }
            
            Ok((stack, ctx))
        })
    })
    .with_doc("Get tool effect signature and documentation.")
}
