//! Workflow tools for kore
//!
//! These tools enable concurrent workflow execution within kore programs.

use crate::runtime::WorkflowRuntime;
use crate::types::{WorkflowHandle, WorkflowDef, Message};
use crate::{WorkflowError, Result};
use kore::{Context, Stack, Tool, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Global runtime storage for workflow tools
/// 
/// This uses a thread-local or context-stored runtime.
/// For now, we use a simple Arc-based approach.
static RUNTIME: once_cell::sync::Lazy<Arc<RwLock<Option<WorkflowRuntime>>>> = 
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(None)));

/// Initialize the global workflow runtime
pub async fn init_runtime() -> WorkflowRuntime {
    let runtime = WorkflowRuntime::new();
    {
        let mut global = RUNTIME.write().await;
        *global = Some(runtime.clone());
    }
    runtime
}

/// Get the current workflow runtime
pub async fn get_runtime() -> Result<WorkflowRuntime> {
    let global = RUNTIME.read().await;
    global.clone().ok_or(WorkflowError::RuntimeNotAvailable)
}

/// Create the spawn tool: (quote -- handle)
pub fn spawn_tool() -> Tool {
    Tool::native("spawn", "(code:Quote -- handle:Handle)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let quote = stack.pop()?;
            let ops = quote.as_quote()?;
            
            let def = WorkflowDef::anonymous(ops.to_vec());
            let runtime = get_runtime().await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            // Get current workflow as parent
            let parent = runtime.current().await;
            
            let handle = runtime.spawn_and_run(def, parent, ctx.clone()).await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            stack.push(Value::from(handle))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Spawn a new concurrent workflow from a quote. Returns a handle to the workflow.")
}

/// Create the await tool: (handle -- result)
pub fn await_tool() -> Tool {
    Tool::native("await", "(handle:Handle -- result:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let handle_val = stack.pop()?;
            let handle = WorkflowHandle::try_from(&handle_val)
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            let runtime = get_runtime().await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            let result = runtime.await_workflow(handle).await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            stack.push(result)?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Wait for a workflow to complete and push its result.")
}

/// Create the send tool: (handle message --)
pub fn send_tool() -> Tool {
    Tool::native("send", "(handle:Handle message:Any --)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let message_val = stack.pop()?;
            let handle_val = stack.pop()?;
            let handle = WorkflowHandle::try_from(&handle_val)
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            let runtime = get_runtime().await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            // Get sender (current workflow or synthetic handle)
            let sender = runtime.current().await.unwrap_or_else(WorkflowHandle::new);
            
            let msg = Message::new(message_val, sender);
            
            runtime.send(handle, msg).await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            Ok((stack, ctx))
        })
    })
    .with_doc("Send a message to a workflow.")
}

/// Create the recv tool: (-- message)
/// Note: This is handled specially by the executor - this is a fallback
pub fn recv_tool() -> Tool {
    Tool::native("recv", "(-- message:Any)", |_stack: Stack, _ctx: Context| {
        Box::pin(async move {
            // The actual recv is handled by the workflow executor
            // This tool is only called when not in a workflow context
            Err(kore::Error::Runtime(
                "recv can only be used within a workflow".into()
            ))
        })
    })
    .with_doc("Receive the next message sent to this workflow. Blocks until a message arrives.")
}

/// Create the recv-timeout tool: (ms -- message-or-null)
pub fn recv_timeout_tool() -> Tool {
    Tool::native("recv-timeout", "(timeout_ms:Int -- message:Any)", |_stack: Stack, _ctx: Context| {
        Box::pin(async move {
            // The actual recv-timeout is handled by the workflow executor
            Err(kore::Error::Runtime(
                "recv-timeout can only be used within a workflow".into()
            ))
        })
    })
    .with_doc("Receive a message with timeout. Returns null if timeout expires.")
}

/// Create the self tool: (-- handle)
pub fn self_tool() -> Tool {
    Tool::native("self", "(-- handle:Handle)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let runtime = get_runtime().await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            let handle = runtime.current().await
                .ok_or_else(|| kore::Error::Runtime("not in a workflow context".into()))?;
            
            stack.push(Value::from(handle))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Get the current workflow's handle.")
}

/// Create the parent tool: (-- handle-or-null)
pub fn parent_tool() -> Tool {
    Tool::native("parent", "(-- handle:Handle)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let runtime = get_runtime().await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            let current = runtime.current().await
                .ok_or_else(|| kore::Error::Runtime("not in a workflow context".into()))?;
            
            let parent = runtime.parent(current).await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            let value = match parent {
                Some(h) => Value::from(h),
                None => Value::Null,
            };
            
            stack.push(value)?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Get the parent workflow's handle, or null if this is a root workflow.")
}

/// Create the status tool: (handle -- status)
pub fn status_tool() -> Tool {
    Tool::native("status", "(handle:Handle -- status:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let handle_val = stack.pop()?;
            let handle = WorkflowHandle::try_from(&handle_val)
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            let runtime = get_runtime().await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            let status = runtime.status(handle).await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            stack.push(Value::from(status))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Get the status of a workflow.")
}

/// Create the cancel tool: (handle --)
pub fn cancel_tool() -> Tool {
    Tool::native("cancel", "(handle:Handle --)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let handle_val = stack.pop()?;
            let handle = WorkflowHandle::try_from(&handle_val)
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            let runtime = get_runtime().await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            runtime.cancel(handle).await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            Ok((stack, ctx))
        })
    })
    .with_doc("Cancel a running workflow.")
}

/// Get all workflow tools
pub fn all_tools() -> Vec<Tool> {
    vec![
        spawn_tool(),
        await_tool(),
        send_tool(),
        recv_tool(),
        recv_timeout_tool(),
        self_tool(),
        parent_tool(),
        status_tool(),
        cancel_tool(),
    ]
}

/// Register all workflow tools into a context
pub async fn register_all(ctx: &mut Context) {
    // Initialize runtime if not done
    let _ = init_runtime().await;
    
    let mut dict = ctx.dict.write().await;
    for tool in all_tools() {
        dict.register(tool);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tools_created() {
        let tools = all_tools();
        assert_eq!(tools.len(), 9);
        
        let names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"spawn"));
        assert!(names.contains(&"await"));
        assert!(names.contains(&"send"));
        assert!(names.contains(&"recv"));
        assert!(names.contains(&"recv-timeout"));
        assert!(names.contains(&"self"));
        assert!(names.contains(&"parent"));
        assert!(names.contains(&"status"));
        assert!(names.contains(&"cancel"));
    }
}
