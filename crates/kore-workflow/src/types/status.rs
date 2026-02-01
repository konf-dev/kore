//! Workflow status - lifecycle states for a workflow

use serde::{Deserialize, Serialize};
use kore::Value;

/// The lifecycle state of a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowStatus {
    /// Workflow is executing
    Running,
    
    /// Workflow is waiting for a message
    WaitingForMessage,
    
    /// Workflow is waiting for a child to complete
    WaitingForChild(super::WorkflowHandle),
    
    /// Workflow completed successfully with a result
    Complete(Value),
    
    /// Workflow failed with an error
    Failed(FailureInfo),
    
    /// Workflow was cancelled
    Cancelled,
}

/// Error information for failed workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureInfo {
    pub code: String,
    pub message: String,
    pub context: Option<String>,
}

impl WorkflowStatus {
    /// Check if workflow is still running (not terminated)
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            WorkflowStatus::Running
                | WorkflowStatus::WaitingForMessage
                | WorkflowStatus::WaitingForChild(_)
        )
    }

    /// Check if workflow has terminated
    pub fn is_terminated(&self) -> bool {
        matches!(
            self,
            WorkflowStatus::Complete(_) | WorkflowStatus::Failed(_) | WorkflowStatus::Cancelled
        )
    }

    /// Check if workflow completed successfully
    pub fn is_success(&self) -> bool {
        matches!(self, WorkflowStatus::Complete(_))
    }

    /// Get the result if completed
    pub fn result(&self) -> Option<&Value> {
        match self {
            WorkflowStatus::Complete(v) => Some(v),
            _ => None,
        }
    }

    /// Get the error if failed
    pub fn error(&self) -> Option<&FailureInfo> {
        match self {
            WorkflowStatus::Failed(e) => Some(e),
            _ => None,
        }
    }

    /// Convert status to a string for display
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkflowStatus::Running => "running",
            WorkflowStatus::WaitingForMessage => "waiting-message",
            WorkflowStatus::WaitingForChild(_) => "waiting-child",
            WorkflowStatus::Complete(_) => "complete",
            WorkflowStatus::Failed(_) => "failed",
            WorkflowStatus::Cancelled => "cancelled",
        }
    }
}

/// Convert status to kore Value for stack operations
impl From<WorkflowStatus> for Value {
    fn from(status: WorkflowStatus) -> Self {
        match status {
            WorkflowStatus::Running => Value::Text("running".into()),
            WorkflowStatus::WaitingForMessage => Value::Text("waiting-message".into()),
            WorkflowStatus::WaitingForChild(h) => Value::Map(indexmap::indexmap! {
                "status".into() => Value::Text("waiting-child".into()),
                "child".into() => Value::Text(h.to_string_id()),
            }),
            WorkflowStatus::Complete(v) => Value::Map(indexmap::indexmap! {
                "status".into() => Value::Text("complete".into()),
                "result".into() => v,
            }),
            WorkflowStatus::Failed(e) => Value::Map(indexmap::indexmap! {
                "status".into() => Value::Text("failed".into()),
                "code".into() => Value::Text(e.code),
                "message".into() => Value::Text(e.message),
            }),
            WorkflowStatus::Cancelled => Value::Text("cancelled".into()),
        }
    }
}

impl From<kore::Error> for FailureInfo {
    fn from(e: kore::Error) -> Self {
        FailureInfo {
            code: "kore_error".into(),
            message: e.to_string(),
            context: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_active_states() {
        assert!(WorkflowStatus::Running.is_active());
        assert!(WorkflowStatus::WaitingForMessage.is_active());
        assert!(!WorkflowStatus::Complete(Value::Null).is_active());
        assert!(!WorkflowStatus::Cancelled.is_active());
    }

    #[test]
    fn status_to_value() {
        let v: Value = WorkflowStatus::Running.into();
        assert_eq!(v, Value::Text("running".into()));
    }
}
