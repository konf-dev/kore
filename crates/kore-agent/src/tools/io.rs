//! I/O tools: print, read-line, env-get, done
//!
//! Each tool does ONE thing. No global state.

use kore::{Context, Stack, Tool, Value};

/// env-get: (key -- value)
pub fn env_get_tool() -> Tool {
    Tool::native("env-get", "(key:Text -- value:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let key = stack.pop()?.as_text()?.to_string();
            let value = std::env::var(&key).unwrap_or_default();
            stack.push(Value::Text(value))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Read environment variable. Returns empty string if not set.")
}

/// print: (value --)
pub fn print_tool() -> Tool {
    Tool::native("print", "(value:Any --)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?;
            println!("{}", format_value(&value));
            Ok((stack, ctx))
        })
    })
    .with_doc("Print a value to stdout.")
}

/// done: (message --) - Signal completion and EXIT
pub fn done_tool() -> Tool {
    Tool::native("done", "(message:Text --)", |mut stack: Stack, _ctx: Context| {
        Box::pin(async move {
            let message = stack.pop()?.as_text()?.to_string();
            println!("🎉 DONE: {}", message);
            // Exit cleanly - this is how the agent stops
            std::process::exit(0);
        })
    })
    .with_doc("Signal goal completion and exit. Call this when goal is achieved.")
}

/// read-line: (-- text)
pub fn read_line_tool() -> Tool {
    Tool::native("read-line", "(-- input:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            stack.push(Value::Text(input.trim().to_string()))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Read a line from stdin.")
}

/// Format value for printing
fn format_value(value: &Value) -> String {
    match value {
        Value::Text(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::List(l) => format!("{:?}", l),
        Value::Map(m) => serde_json::to_string(m).unwrap_or_else(|_| format!("{:?}", m)),
        Value::Quote(ops) => format!("({})", ops.iter().map(|o| format!("{:?}", o)).collect::<Vec<_>>().join(" ")),
        Value::Handle(h) => format!("<handle:{}>", h.id),
        Value::Error(e) => format!("Error[{}]: {}", e.code, e.message),
    }
}
