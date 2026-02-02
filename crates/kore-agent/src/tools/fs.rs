//! File system tools: file-read, file-write, file-append, file-exists, file-delete
//! dir-list, dir-create, dir-delete
//!
//! All paths are relative to workspace root.

use kore::{Context, Stack, Tool, Value};
use crate::tools::workspace_path;

/// file-read: (path -- content)
pub fn file_read_tool() -> Tool {
    Tool::native("file-read", "(path:Text -- content:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let path = stack.pop()?.as_text()?.to_string();
            let full_path = workspace_path(&path);
            
            let content = tokio::fs::read_to_string(&full_path).await
                .map_err(|e| kore::Error::Runtime(format!("file-read '{}': {}", path, e)))?;
            
            stack.push(Value::Text(content))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Read file content. Path is relative to workspace.")
}

/// file-write: (path content --)
pub fn file_write_tool() -> Tool {
    Tool::native("file-write", "(path:Text content:Text --)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let content = stack.pop()?.as_text()?.to_string();
            let path = stack.pop()?.as_text()?.to_string();
            let full_path = workspace_path(&path);
            
            // Ensure parent directory exists
            if let Some(parent) = full_path.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            
            tokio::fs::write(&full_path, content).await
                .map_err(|e| kore::Error::Runtime(format!("file-write '{}': {}", path, e)))?;
            
            Ok((stack, ctx))
        })
    })
    .with_doc("Write content to file. Creates parent directories. Path is relative to workspace.")
}

/// file-append: (path content --)
pub fn file_append_tool() -> Tool {
    Tool::native("file-append", "(path:Text content:Text --)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let content = stack.pop()?.as_text()?.to_string();
            let path = stack.pop()?.as_text()?.to_string();
            let full_path = workspace_path(&path);
            
            // Ensure parent directory exists
            if let Some(parent) = full_path.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&full_path)
                .await
                .map_err(|e| kore::Error::Runtime(format!("file-append '{}': {}", path, e)))?;
            
            file.write_all(content.as_bytes()).await
                .map_err(|e| kore::Error::Runtime(format!("file-append '{}': {}", path, e)))?;
            
            Ok((stack, ctx))
        })
    })
    .with_doc("Append content to file. Creates file if not exists. Path is relative to workspace.")
}

/// file-exists: (path -- bool)
pub fn file_exists_tool() -> Tool {
    Tool::native("file-exists", "(path:Text -- exists:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let path = stack.pop()?.as_text()?.to_string();
            let full_path = workspace_path(&path);
            
            let exists = tokio::fs::metadata(&full_path).await.is_ok();
            stack.push(Value::Bool(exists))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Check if file exists. Path is relative to workspace.")
}

/// file-delete: (path --)
pub fn file_delete_tool() -> Tool {
    Tool::native("file-delete", "(path:Text --)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let path = stack.pop()?.as_text()?.to_string();
            let full_path = workspace_path(&path);
            
            tokio::fs::remove_file(&full_path).await
                .map_err(|e| kore::Error::Runtime(format!("file-delete '{}': {}", path, e)))?;
            
            Ok((stack, ctx))
        })
    })
    .with_doc("Delete a file. Path is relative to workspace.")
}

/// dir-list: (path -- entries)
pub fn dir_list_tool() -> Tool {
    Tool::native("dir-list", "(path:Text -- entries:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let path = stack.pop()?.as_text()?.to_string();
            let full_path = workspace_path(&path);
            
            let mut entries = Vec::new();
            let mut dir = tokio::fs::read_dir(&full_path).await
                .map_err(|e| kore::Error::Runtime(format!("dir-list '{}': {}", path, e)))?;
            
            while let Some(entry) = dir.next_entry().await
                .map_err(|e| kore::Error::Runtime(format!("dir-list '{}': {}", path, e)))? {
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = entry.file_type().await
                    .map(|ft| ft.is_dir())
                    .unwrap_or(false);
                
                // Append / to directories
                let display = if is_dir { format!("{}/", name) } else { name };
                entries.push(Value::Text(display));
            }
            
            stack.push(Value::List(entries))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("List directory contents. Directories end with /. Path is relative to workspace.")
}

/// dir-create: (path --)
pub fn dir_create_tool() -> Tool {
    Tool::native("dir-create", "(path:Text --)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let path = stack.pop()?.as_text()?.to_string();
            let full_path = workspace_path(&path);
            
            tokio::fs::create_dir_all(&full_path).await
                .map_err(|e| kore::Error::Runtime(format!("dir-create '{}': {}", path, e)))?;
            
            Ok((stack, ctx))
        })
    })
    .with_doc("Create directory (and parents). Path is relative to workspace.")
}

/// dir-delete: (path --)
pub fn dir_delete_tool() -> Tool {
    Tool::native("dir-delete", "(path:Text --)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let path = stack.pop()?.as_text()?.to_string();
            let full_path = workspace_path(&path);
            
            tokio::fs::remove_dir_all(&full_path).await
                .map_err(|e| kore::Error::Runtime(format!("dir-delete '{}': {}", path, e)))?;
            
            Ok((stack, ctx))
        })
    })
    .with_doc("Delete directory recursively. Path is relative to workspace.")
}
