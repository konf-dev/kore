//! Error types for kore-workflow

use thiserror::Error;

/// Errors that can occur in workflow operations
#[derive(Debug, Error, Clone)]
pub enum WorkflowError {
    #[error("workflow not found: {0}")]
    NotFound(String),

    #[error("invalid workflow handle: {0}")]
    InvalidHandle(String),

    #[error("value is not a workflow handle: {0}")]
    NotAWorkflowHandle(String),

    #[error("invalid workflow definition: {0}")]
    InvalidDefinition(String),

    #[error("workflow already terminated: {0}")]
    AlreadyTerminated(String),

    #[error("message queue full for workflow: {0}")]
    QueueFull(String),

    #[error("message receive timeout")]
    ReceiveTimeout,

    #[error("no parent workflow")]
    NoParent,

    #[error("workflow spawn failed: {0}")]
    SpawnFailed(String),

    #[error("workflow execution failed: {0}")]
    ExecutionFailed(String),

    #[error("runtime not available")]
    RuntimeNotAvailable,

    #[error("kore error: {0}")]
    Kore(String),
}

impl From<kore::Error> for WorkflowError {
    fn from(e: kore::Error) -> Self {
        WorkflowError::Kore(e.to_string())
    }
}

/// Result type for workflow operations
pub type Result<T> = std::result::Result<T, WorkflowError>;
