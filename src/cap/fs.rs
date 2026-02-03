//! File System Capability Tools
//!
//! Tools that interact with the filesystem.
//! All operations require appropriate fs:read or fs:write capabilities.
//!
//! | Tool | Signature | Capability |
//! |------|-----------|------------|
//! | fs-read | (path -- text) | fs:read:PATH |
//! | fs-write | (path text -- ) | fs:write:PATH |
//! | fs-append | (path text -- ) | fs:write:PATH |
//! | fs-exists | (path -- bool) | fs:read:PATH |
//! | fs-list | (path -- list) | fs:read:PATH |
//! | fs-rm | (path -- ) | fs:write:PATH |
//! | fs-mkdir | (path -- ) | fs:write:PATH |

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register file system tools (7)
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "fs-read",
        "(path -- text)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let path = stack.pop()?.into_text()?;
                if !ctx.caps.can_read_path(std::path::Path::new(&path)) {
                    return Err(crate::error::Error::CapabilityDenied {
                        capability: format!("fs:read:{}", path),
                        tool: "fs-read".into(),
                    });
                }
                let contents = tokio::fs::read_to_string(&path)
                    .await
                    .map_err(|e| crate::error::Error::io(format!("fs-read '{}': {}", path, e)))?;
                stack.push(Value::Text(contents))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "fs-write",
        "(path text -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let contents = stack.pop()?.into_text()?;
                let path = stack.pop()?.into_text()?;
                if !ctx.caps.can_write_path(std::path::Path::new(&path)) {
                    return Err(crate::error::Error::CapabilityDenied {
                        capability: format!("fs:write:{}", path),
                        tool: "fs-write".into(),
                    });
                }
                tokio::fs::write(&path, &contents)
                    .await
                    .map_err(|e| crate::error::Error::io(format!("fs-write '{}': {}", path, e)))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "fs-append",
        "(path text -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                use tokio::io::AsyncWriteExt;
                let contents = stack.pop()?.into_text()?;
                let path = stack.pop()?.into_text()?;
                if !ctx.caps.can_write_path(std::path::Path::new(&path)) {
                    return Err(crate::error::Error::CapabilityDenied {
                        capability: format!("fs:write:{}", path),
                        tool: "fs-append".into(),
                    });
                }
                let mut file = tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .await
                    .map_err(|e| crate::error::Error::io(format!("fs-append '{}': {}", path, e)))?;
                file.write_all(contents.as_bytes())
                    .await
                    .map_err(|e| crate::error::Error::io(format!("fs-append '{}': {}", path, e)))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "fs-exists",
        "(path -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let path = stack.pop()?.into_text()?;
                if !ctx.caps.can_read_path(std::path::Path::new(&path)) {
                    return Err(crate::error::Error::CapabilityDenied {
                        capability: format!("fs:read:{}", path),
                        tool: "fs-exists".into(),
                    });
                }
                let exists = tokio::fs::metadata(&path).await.is_ok();
                stack.push(Value::Bool(exists))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "fs-list",
        "(path -- list)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let path = stack.pop()?.into_text()?;
                if !ctx.caps.can_read_path(std::path::Path::new(&path)) {
                    return Err(crate::error::Error::CapabilityDenied {
                        capability: format!("fs:read:{}", path),
                        tool: "fs-list".into(),
                    });
                }
                let mut entries = Vec::new();
                let mut dir = tokio::fs::read_dir(&path)
                    .await
                    .map_err(|e| crate::error::Error::io(format!("fs-list '{}': {}", path, e)))?;
                while let Some(entry) = dir
                    .next_entry()
                    .await
                    .map_err(|e| crate::error::Error::io(format!("fs-list '{}': {}", path, e)))?
                {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                    let display = if is_dir { format!("{}/", name) } else { name };
                    entries.push(Value::Text(display));
                }
                stack.push(Value::List(entries))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "fs-rm",
        "(path -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let path = stack.pop()?.into_text()?;
                if !ctx.caps.can_write_path(std::path::Path::new(&path)) {
                    return Err(crate::error::Error::CapabilityDenied {
                        capability: format!("fs:write:{}", path),
                        tool: "fs-rm".into(),
                    });
                }
                // Try file first, then directory
                if tokio::fs::remove_file(&path).await.is_err() {
                    tokio::fs::remove_dir_all(&path)
                        .await
                        .map_err(|e| crate::error::Error::io(format!("fs-rm '{}': {}", path, e)))?;
                }
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "fs-mkdir",
        "(path -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let path = stack.pop()?.into_text()?;
                if !ctx.caps.can_write_path(std::path::Path::new(&path)) {
                    return Err(crate::error::Error::CapabilityDenied {
                        capability: format!("fs:write:{}", path),
                        tool: "fs-mkdir".into(),
                    });
                }
                tokio::fs::create_dir_all(&path)
                    .await
                    .map_err(|e| crate::error::Error::io(format!("fs-mkdir '{}': {}", path, e)))?;
                Ok((stack, ctx))
            })
        },
    ));
}
