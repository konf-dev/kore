//! Console I/O Tools
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | print | (text -- ) | Print without newline |
//! | println | (text -- ) | Print with newline |
//! | read-line | ( -- text) | Read line from stdin |
//! | log | (level msg -- ) | Log to stderr |

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register I/O tools (4)
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "print",
        "(text -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let text = stack.pop()?.into_text()?;
                use std::io::Write;
                print!("{}", text);
                std::io::stdout().flush().ok();
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "println",
        "(text -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let text = stack.pop()?.into_text()?;
                println!("{}", text);
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "read-line",
        "( -- text)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let mut line = String::new();
                std::io::stdin()
                    .read_line(&mut line)
                    .map_err(|e| crate::error::Error::io(format!("read-line: {}", e)))?;
                // Remove trailing newline
                line.truncate(line.trim_end_matches(&['\r', '\n'][..]).len());
                stack.push(Value::Text(line))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "log",
        "(level msg -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let msg = stack.pop()?.into_text()?;
                let level = stack.pop()?.into_text()?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                eprintln!("[{}] {} {}", level.to_uppercase(), now, msg);
                Ok((stack, ctx))
            })
        },
    ));
}
