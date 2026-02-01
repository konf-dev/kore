//! Workflow execution engine

use crate::error::WorkflowError as WfError;
use crate::error::Result;
use crate::types::{WorkflowHandle, WorkflowStatus};
use crate::types::status::FailureInfo;
use crate::runtime::state::WorkflowState;
use kore::{Op, Value, execute};
use std::time::Duration;

/// Execute a workflow until it completes, waits for message, or fails
pub async fn execute_workflow(state: &mut WorkflowState) -> Result<ExecutionResult> {
    // Get remaining operations
    let ops = &state.definition.body[state.op_index..];
    
    if ops.is_empty() {
        // No more ops - complete with top of stack or Null
        let result = state.stack.pop().unwrap_or(Value::Null);
        state.status = WorkflowStatus::Complete(result.clone());
        return Ok(ExecutionResult::Complete(result));
    }

    // Execute operations one by one
    for (i, op) in ops.iter().enumerate() {
        // Check for special operations
        match op {
            Op::Call(name) if name == "recv" => {
                // Try to get message from inbox
                if let Some(msg) = state.inbox.try_recv().await {
                    let _ = state.stack.push(msg.into_payload());
                    state.op_index += i + 1;
                    continue;
                } else {
                    // No message - wait
                    state.op_index += i;
                    state.status = WorkflowStatus::WaitingForMessage;
                    return Ok(ExecutionResult::WaitingForMessage);
                }
            }
            Op::Call(name) if name == "recv-timeout" => {
                // Pop timeout from stack
                let timeout_ms = state.stack.pop()
                    .map_err(|e| WfError::ExecutionFailed(e.to_string()))?
                    .as_int().map_err(|e| WfError::ExecutionFailed(e.to_string()))?;
                
                let timeout = Duration::from_millis(timeout_ms as u64);
                
                if let Some(msg) = state.inbox.recv_timeout(timeout).await {
                    let _ = state.stack.push(msg.into_payload());
                } else {
                    let _ = state.stack.push(Value::Null);
                }
                state.op_index += i + 1;
                continue;
            }
            Op::Call(name) if name == "await" => {
                // Pop handle from stack
                let handle_val = state.stack.pop()
                    .map_err(|e| WfError::ExecutionFailed(e.to_string()))?;
                
                let child_handle = WorkflowHandle::try_from(&handle_val)
                    .map_err(|e| WfError::ExecutionFailed(e.to_string()))?;
                
                // Check if we're tracking this child
                if state.children.contains(&child_handle) {
                    state.op_index += i;
                    state.status = WorkflowStatus::WaitingForChild(child_handle);
                    return Ok(ExecutionResult::WaitingForChild(child_handle));
                } else {
                    return Err(WfError::ExecutionFailed(
                        format!("Cannot await non-child workflow: {}", child_handle)
                    ));
                }
            }
            _ => {
                // Regular operation - execute via kore
                let single_op = std::slice::from_ref(op);
                
                let stack = std::mem::take(&mut state.stack);
                let ctx = std::mem::take(&mut state.context);
                
                match execute(single_op, stack, ctx).await {
                    Ok((new_stack, new_ctx)) => {
                        state.stack = new_stack;
                        state.context = new_ctx;
                        state.op_index += i + 1;
                    }
                    Err(e) => {
                        let err = FailureInfo {
                            code: "EXECUTION_ERROR".to_string(),
                            message: e.to_string(),
                            context: None,
                        };
                        state.status = WorkflowStatus::Failed(err);
                        return Ok(ExecutionResult::Failed(e.to_string()));
                    }
                }
            }
        }
    }

    // All ops executed - complete
    let result = state.stack.pop().unwrap_or(Value::Null);
    state.status = WorkflowStatus::Complete(result.clone());
    Ok(ExecutionResult::Complete(result))
}

/// Result of workflow execution
#[derive(Debug, Clone)]
pub enum ExecutionResult {
    /// Workflow completed with a result
    Complete(Value),
    /// Workflow is waiting for a message
    WaitingForMessage,
    /// Workflow is waiting for a child to complete
    WaitingForChild(WorkflowHandle),
    /// Workflow failed with an error
    Failed(String),
    /// Workflow was terminated
    Terminated,
    /// Workflow yielded control
    Yielded,
}

impl ExecutionResult {
    pub fn is_complete(&self) -> bool {
        matches!(self, ExecutionResult::Complete(_))
    }

    pub fn is_waiting(&self) -> bool {
        matches!(self, ExecutionResult::WaitingForMessage | ExecutionResult::WaitingForChild(_))
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, ExecutionResult::Failed(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{WorkflowDef, WorkflowHandle, WorkflowStatus, Message};
    use kore::{Context, register_builtins};

    async fn test_context() -> Context {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn execute_simple_workflow() {
        let handle = WorkflowHandle::new();
        // Simple workflow: push 42, dup, drop (result: 42)
        let def = WorkflowDef::from_quote("test", vec![
            Op::push(42),
            Op::call("dup"),
            Op::call("drop"),
        ]);
        let ctx = test_context().await;
        
        let mut state = WorkflowState::new(handle, def, None, ctx);
        
        let result = execute_workflow(&mut state).await.unwrap();
        
        assert!(result.is_complete(), "Expected complete, got: {:?}", result);
        if let ExecutionResult::Complete(v) = result {
            assert_eq!(v, Value::Int(42));
        }
    }

    #[tokio::test]
    async fn execute_workflow_waits_for_message() {
        let handle = WorkflowHandle::new();
        let def = WorkflowDef::from_quote("test", vec![
            Op::call("recv"),  // Will wait here
            Op::call("dup"),
            Op::call("drop"),
        ]);
        let ctx = test_context().await;
        
        let mut state = WorkflowState::new(handle, def, None, ctx);
        
        let result = execute_workflow(&mut state).await.unwrap();
        
        assert!(result.is_waiting());
        assert!(matches!(result, ExecutionResult::WaitingForMessage));
        assert!(matches!(state.status, WorkflowStatus::WaitingForMessage));
    }

    #[tokio::test]
    async fn execute_workflow_receives_message() {
        let handle = WorkflowHandle::new();
        let def = WorkflowDef::from_quote("test", vec![
            Op::call("recv"),
            Op::call("dup"),
            Op::call("drop"),
        ]);
        let ctx = test_context().await;
        
        let mut state = WorkflowState::new(handle, def, None, ctx);
        
        // Send a message first
        let sender = WorkflowHandle::new();
        state.inbox.send(Message::new(Value::Int(42), sender)).await.unwrap();
        
        // Now execute - should complete
        let result = execute_workflow(&mut state).await.unwrap();
        
        assert!(result.is_complete(), "Expected complete, got: {:?}", result);
        if let ExecutionResult::Complete(v) = result {
            assert_eq!(v, Value::Int(42));
        }
    }
}
