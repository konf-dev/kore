//! Shell execution tools: shell, shell-dir
//!
//! Execute commands in the sandbox.

use kore::{Context, Stack, Tool, Value};
use crate::tools::workspace_path;

/// shell: (cmd -- result)
pub fn shell_tool() -> Tool {
    Tool::native("shell", "(cmd:Text -- result:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let cmd = stack.pop()?.as_text()?.to_string();
            
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .current_dir(workspace_path(""))
                .output()
                .await
                .map_err(|e| kore::Error::Runtime(format!("shell: {}", e)))?;
            
            let result = Value::Map(indexmap::indexmap! {
                "stdout".into() => Value::Text(String::from_utf8_lossy(&output.stdout).to_string()),
                "stderr".into() => Value::Text(String::from_utf8_lossy(&output.stderr).to_string()),
                "code".into() => Value::Int(output.status.code().unwrap_or(-1) as i64),
            });
            
            stack.push(result)?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Run shell command in workspace. Returns {stdout, stderr, code}.")
}

/// shell-dir: (cmd dir -- result)
pub fn shell_dir_tool() -> Tool {
    Tool::native("shell-dir", "(cmd:Text dir:Text -- result:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let dir = stack.pop()?.as_text()?.to_string();
            let cmd = stack.pop()?.as_text()?.to_string();
            
            let full_dir = workspace_path(&dir);
            
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .current_dir(&full_dir)
                .output()
                .await
                .map_err(|e| kore::Error::Runtime(format!("shell-dir: {}", e)))?;
            
            let result = Value::Map(indexmap::indexmap! {
                "stdout".into() => Value::Text(String::from_utf8_lossy(&output.stdout).to_string()),
                "stderr".into() => Value::Text(String::from_utf8_lossy(&output.stderr).to_string()),
                "code".into() => Value::Int(output.status.code().unwrap_or(-1) as i64),
            });
            
            stack.push(result)?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Run shell command in specified directory. Returns {stdout, stderr, code}.")
}
