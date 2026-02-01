//! Workflow runtime - manages all running workflows

use crate::types::{WorkflowHandle, WorkflowDef, WorkflowStatus, Message};
use crate::runtime::{WorkflowState, ExecutionResult, execute_workflow};
use crate::{WorkflowError, Result};
use kore::{Context, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// The workflow runtime - manages all workflows
pub struct WorkflowRuntime {
    /// All workflow states, keyed by handle
    workflows: Arc<RwLock<HashMap<WorkflowHandle, Arc<Mutex<WorkflowState>>>>>,
    
    /// The current workflow (for self/parent tools)
    current: Arc<RwLock<Option<WorkflowHandle>>>,
}

impl WorkflowRuntime {
    /// Create a new workflow runtime
    pub fn new() -> Self {
        Self {
            workflows: Arc::new(RwLock::new(HashMap::new())),
            current: Arc::new(RwLock::new(None)),
        }
    }

    /// Spawn a new workflow from a definition
    pub async fn spawn(
        &self,
        def: WorkflowDef,
        parent: Option<WorkflowHandle>,
        context: Context,
    ) -> Result<WorkflowHandle> {
        let handle = WorkflowHandle::new();
        
        let state = WorkflowState::new(handle, def, parent, context);
        
        // Register the workflow
        {
            let mut workflows = self.workflows.write().await;
            workflows.insert(handle, Arc::new(Mutex::new(state)));
        }
        
        // If there's a parent, add this as a child
        if let Some(parent_handle) = parent {
            if let Some(parent_state) = self.get_workflow(&parent_handle).await {
                let mut parent = parent_state.lock().await;
                parent.add_child(handle);
            }
        }
        
        Ok(handle)
    }

    /// Spawn and immediately start running a workflow
    pub async fn spawn_and_run(
        &self,
        def: WorkflowDef,
        parent: Option<WorkflowHandle>,
        context: Context,
    ) -> Result<WorkflowHandle> {
        let handle = self.spawn(def, parent, context).await?;
        
        // Start execution in background
        let runtime = self.clone();
        tokio::spawn(async move {
            runtime.run_workflow(handle).await
        });
        
        Ok(handle)
    }

    /// Run a workflow until it completes or waits
    pub async fn run_workflow(&self, handle: WorkflowHandle) -> Result<ExecutionResult> {
        let state_arc = self.get_workflow(&handle).await
            .ok_or_else(|| WorkflowError::NotFound(handle.to_string()))?;
        
        // Set as current workflow
        {
            let mut current = self.current.write().await;
            *current = Some(handle);
        }
        
        let result = {
            let mut state = state_arc.lock().await;
            execute_workflow(&mut state).await?
        };
        
        // Clear current workflow
        {
            let mut current = self.current.write().await;
            *current = None;
        }
        
        Ok(result)
    }

    /// Run a workflow in a loop until it terminates
    pub async fn run_to_completion(&self, handle: WorkflowHandle) -> Result<Value> {
        loop {
            let result = self.run_workflow(handle).await?;
            
            match result {
                ExecutionResult::Complete(v) => return Ok(v),
                ExecutionResult::Failed(e) => return Err(WorkflowError::ExecutionFailed(e)),
                ExecutionResult::Terminated => {
                    // Get the result from the state
                    let state_arc = self.get_workflow(&handle).await
                        .ok_or_else(|| WorkflowError::NotFound(handle.to_string()))?;
                    let state = state_arc.lock().await;
                    return state.result()
                        .cloned()
                        .ok_or_else(|| WorkflowError::ExecutionFailed("no result".into()));
                }
                ExecutionResult::WaitingForMessage => {
                    // Wait for a message to arrive
                    let state_arc = self.get_workflow(&handle).await
                        .ok_or_else(|| WorkflowError::NotFound(handle.to_string()))?;
                    let state = state_arc.lock().await;
                    
                    // Wait for inbox to have a message
                    drop(state); // Release lock while waiting
                    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                }
                ExecutionResult::WaitingForChild(child) => {
                    // Wait for child to complete
                    let child_result = Box::pin(self.run_to_completion(child)).await?;
                    
                    // Push child result onto parent stack
                    let state_arc = self.get_workflow(&handle).await
                        .ok_or_else(|| WorkflowError::NotFound(handle.to_string()))?;
                    let mut state = state_arc.lock().await;
                    state.stack.push(child_result)?;
                    state.resume();
                }
                ExecutionResult::Yielded => {
                    // Give other tasks a chance
                    tokio::task::yield_now().await;
                }
            }
        }
    }

    /// Await a workflow's completion
    pub async fn await_workflow(&self, handle: WorkflowHandle) -> Result<Value> {
        // First check if already complete
        if let Some(state_arc) = self.get_workflow(&handle).await {
            let state = state_arc.lock().await;
            if let Some(result) = state.result() {
                return Ok(result.clone());
            }
            if state.is_terminated() {
                return Err(WorkflowError::ExecutionFailed("workflow terminated without result".into()));
            }
        } else {
            return Err(WorkflowError::NotFound(handle.to_string()));
        }
        
        // Run to completion
        self.run_to_completion(handle).await
    }

    /// Send a message to a workflow
    pub async fn send(&self, to: WorkflowHandle, msg: Message) -> Result<()> {
        let state_arc = self.get_workflow(&to).await
            .ok_or_else(|| WorkflowError::NotFound(to.to_string()))?;
        
        let state = state_arc.lock().await;
        state.inbox.send(msg).await
            .map_err(|e| WorkflowError::QueueFull(e.to_string()))?;
        
        Ok(())
    }

    /// Get workflow status
    pub async fn status(&self, handle: WorkflowHandle) -> Result<WorkflowStatus> {
        let state_arc = self.get_workflow(&handle).await
            .ok_or_else(|| WorkflowError::NotFound(handle.to_string()))?;
        
        let state = state_arc.lock().await;
        Ok(state.status.clone())
    }

    /// Cancel a workflow
    pub async fn cancel(&self, handle: WorkflowHandle) -> Result<()> {
        let state_arc = self.get_workflow(&handle).await
            .ok_or_else(|| WorkflowError::NotFound(handle.to_string()))?;
        
        let mut state = state_arc.lock().await;
        state.status = WorkflowStatus::Cancelled;
        Ok(())
    }

    /// Get the current workflow handle
    pub async fn current(&self) -> Option<WorkflowHandle> {
        let current = self.current.read().await;
        *current
    }

    /// Get the parent of a workflow
    pub async fn parent(&self, handle: WorkflowHandle) -> Result<Option<WorkflowHandle>> {
        let state_arc = self.get_workflow(&handle).await
            .ok_or_else(|| WorkflowError::NotFound(handle.to_string()))?;
        
        let state = state_arc.lock().await;
        Ok(state.parent)
    }

    /// Get a workflow state by handle
    async fn get_workflow(&self, handle: &WorkflowHandle) -> Option<Arc<Mutex<WorkflowState>>> {
        let workflows = self.workflows.read().await;
        workflows.get(handle).cloned()
    }

    /// Get count of active workflows
    pub async fn active_count(&self) -> usize {
        let workflows = self.workflows.read().await;
        let mut count = 0;
        for state_arc in workflows.values() {
            let state = state_arc.lock().await;
            if state.is_active() {
                count += 1;
            }
        }
        count
    }

    /// Get all workflow handles
    pub async fn all_handles(&self) -> Vec<WorkflowHandle> {
        let workflows = self.workflows.read().await;
        workflows.keys().cloned().collect()
    }
}

impl Default for WorkflowRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for WorkflowRuntime {
    fn clone(&self) -> Self {
        Self {
            workflows: Arc::clone(&self.workflows),
            current: Arc::clone(&self.current),
        }
    }
}

// Make runtime available in kore Context
impl WorkflowRuntime {
    /// Store runtime in a kore Context
    pub fn attach_to_context(&self, _ctx: &mut Context) {
        // Store as a custom entry - workflows can access via special tool
        // This is a placeholder - actual implementation would use Context extensions
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use kore::{Op, register_builtins};

    async fn test_context() -> Context {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn runtime_spawn_and_run() {
        let runtime = WorkflowRuntime::new();
        
        // Simple workflow: push 42, dup, drop (result: 42)
        let def = WorkflowDef::from_quote("test", vec![
            Op::push(42),
            Op::call("dup"),
            Op::call("drop"),
        ]);
        
        let handle = runtime.spawn(def, None, test_context().await).await.unwrap();
        let result = runtime.run_to_completion(handle).await.unwrap();
        
        assert_eq!(result, Value::Int(42));
    }

    #[tokio::test]
    async fn runtime_spawn_with_parent() {
        let runtime = WorkflowRuntime::new();
        
        // Spawn parent
        let parent_def = WorkflowDef::from_quote("parent", vec![Op::push(1)]);
        let parent = runtime.spawn(parent_def, None, test_context().await).await.unwrap();
        
        // Spawn child
        let child_def = WorkflowDef::from_quote("child", vec![Op::push(2)]);
        let child = runtime.spawn(child_def, Some(parent), test_context().await).await.unwrap();
        
        // Check parent relationship
        let child_parent = runtime.parent(child).await.unwrap();
        assert_eq!(child_parent, Some(parent));
    }

    #[tokio::test]
    async fn runtime_send_recv() {
        let runtime = WorkflowRuntime::new();
        
        // Workflow that waits for a message, dups it, drops (returns same value)
        let def = WorkflowDef::from_quote("echo", vec![
            Op::call("recv"),
            Op::call("dup"),
            Op::call("drop"),
        ]);
        
        let handle = runtime.spawn(def, None, test_context().await).await.unwrap();
        
        // Send a message
        let sender = WorkflowHandle::new();
        runtime.send(handle, Message::new(Value::Int(42), sender)).await.unwrap();
        
        // Run to completion
        let result = runtime.run_to_completion(handle).await.unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[tokio::test]
    async fn runtime_cancel() {
        let runtime = WorkflowRuntime::new();
        
        let def = WorkflowDef::from_quote("long", vec![
            Op::call("recv"),  // Will wait forever
        ]);
        
        let handle = runtime.spawn(def, None, test_context().await).await.unwrap();
        
        // Cancel
        runtime.cancel(handle).await.unwrap();
        
        // Check status
        let status = runtime.status(handle).await.unwrap();
        assert!(matches!(status, WorkflowStatus::Cancelled));
    }
}
