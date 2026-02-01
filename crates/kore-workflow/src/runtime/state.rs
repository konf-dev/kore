//! Workflow state - the complete state of a running workflow

use crate::types::{WorkflowHandle, WorkflowDef, WorkflowStatus};
use crate::messaging::AsyncMessageQueue;
use kore::{Stack, Context, Value};
use std::time::Instant;

/// Complete state of a running workflow
pub struct WorkflowState {
    /// Unique handle for this workflow
    pub handle: WorkflowHandle,
    
    /// The workflow definition
    pub definition: WorkflowDef,
    
    /// Current execution status
    pub status: WorkflowStatus,
    
    /// The execution stack
    pub stack: Stack,
    
    /// The execution context
    pub context: Context,
    
    /// Current operation index
    pub op_index: usize,
    
    /// Parent workflow handle (None if root)
    pub parent: Option<WorkflowHandle>,
    
    /// Child workflow handles
    pub children: Vec<WorkflowHandle>,
    
    /// Message inbox
    pub inbox: AsyncMessageQueue,
    
    /// When this workflow was created
    pub created_at: Instant,
    
    /// When this workflow was last active
    pub last_active: Instant,
    
    /// Context updates emitted to parent
    pub emitted_context: Vec<Value>,
}

impl WorkflowState {
    /// Create a new workflow state from a definition
    pub fn new(
        handle: WorkflowHandle,
        definition: WorkflowDef,
        parent: Option<WorkflowHandle>,
        context: Context,
    ) -> Self {
        let now = Instant::now();
        Self {
            handle,
            definition,
            status: WorkflowStatus::Running,
            stack: Stack::new(),
            context,
            op_index: 0,
            parent,
            children: Vec::new(),
            inbox: AsyncMessageQueue::new(),
            created_at: now,
            last_active: now,
            emitted_context: Vec::new(),
        }
    }

    /// Check if this workflow is still active
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    /// Check if this workflow has terminated
    pub fn is_terminated(&self) -> bool {
        self.status.is_terminated()
    }

    /// Get the result if completed
    pub fn result(&self) -> Option<&Value> {
        self.status.result()
    }

    /// Complete the workflow with a result
    pub fn complete(&mut self, result: Value) {
        self.status = WorkflowStatus::Complete(result);
        self.last_active = Instant::now();
    }

    /// Fail the workflow with an error
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = WorkflowStatus::Failed(crate::types::WorkflowStatusError {
            code: "execution_error".into(),
            message: error.into(),
            context: None,
        });
        self.last_active = Instant::now();
    }

    /// Set status to waiting for message
    pub fn wait_for_message(&mut self) {
        self.status = WorkflowStatus::WaitingForMessage;
        self.last_active = Instant::now();
    }

    /// Set status to waiting for child
    pub fn wait_for_child(&mut self, child: WorkflowHandle) {
        self.status = WorkflowStatus::WaitingForChild(child);
        self.last_active = Instant::now();
    }

    /// Resume running
    pub fn resume(&mut self) {
        self.status = WorkflowStatus::Running;
        self.last_active = Instant::now();
    }

    /// Add a child workflow
    pub fn add_child(&mut self, child: WorkflowHandle) {
        self.children.push(child);
    }

    /// Emit context update to parent
    pub fn emit(&mut self, context: Value) {
        self.emitted_context.push(context);
    }

    /// Get elapsed time since creation
    pub fn elapsed(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kore::Op;

    #[test]
    fn workflow_state_creation() {
        let handle = WorkflowHandle::new();
        let def = WorkflowDef::from_quote("test", vec![Op::push(42)]);
        let ctx = Context::new();
        
        let state = WorkflowState::new(handle, def, None, ctx);
        
        assert!(state.is_active());
        assert!(!state.is_terminated());
        assert!(state.parent.is_none());
        assert!(state.children.is_empty());
    }

    #[test]
    fn workflow_state_lifecycle() {
        let handle = WorkflowHandle::new();
        let def = WorkflowDef::from_quote("test", vec![]);
        let ctx = Context::new();
        
        let mut state = WorkflowState::new(handle, def, None, ctx);
        
        // Running
        assert!(state.is_active());
        
        // Wait for message
        state.wait_for_message();
        assert!(state.is_active());
        assert!(matches!(state.status, WorkflowStatus::WaitingForMessage));
        
        // Resume
        state.resume();
        assert!(matches!(state.status, WorkflowStatus::Running));
        
        // Complete
        state.complete(Value::Int(42));
        assert!(!state.is_active());
        assert!(state.is_terminated());
        assert_eq!(state.result(), Some(&Value::Int(42)));
    }
}
