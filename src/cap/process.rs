//! Process Control Tools
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | exec | (cmd -- output) | Run shell command |
//! | pid | ( -- id) | Get process ID |
//! | cwd | ( -- path) | Get working directory |
//! | args | ( -- list) | Get command line args |
//! | exit | (code -- ) | Exit process |

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register process tools (5)
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "exec",
        "(cmd -- output)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let cmd = stack.pop()?.into_text()?;
                if !ctx.caps.can_exec() {
                    return Err(crate::error::Error::CapabilityDenied {
                        capability: "exec".into(),
                        tool: "exec".into(),
                    });
                }
                let output = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .output()
                    .await
                    .map_err(|e| crate::error::Error::io(format!("exec '{}': {}", cmd, e)))?;
                
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(crate::error::Error::io(format!(
                        "exec '{}' failed (exit {}): {}",
                        cmd,
                        output.status.code().unwrap_or(-1),
                        stderr.trim()
                    )));
                }
                
                stack.push(Value::Text(String::from_utf8_lossy(&output.stdout).into()))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "pid",
        "( -- id)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                stack.push(Value::Int(std::process::id() as i64))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "cwd",
        "( -- path)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let cwd = std::env::current_dir()
                    .map_err(|e| crate::error::Error::io(format!("cwd: {}", e)))?;
                stack.push(Value::Text(cwd.to_string_lossy().into()))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "args",
        "( -- list)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let args: Vec<Value> = std::env::args().map(Value::Text).collect();
                stack.push(Value::List(args))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "exit",
        "(code -- )",
        |mut stack: Stack, _ctx: Context| {
            Box::pin(async move {
                let code = stack.pop()?.as_int()?;
                std::process::exit(code as i32);
            })
        },
    ));
}
