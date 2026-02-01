//! Workflow runtime infrastructure
//!
//! - [`WorkflowState`] - Complete state of a running workflow
//! - [`WorkflowRuntime`] - Manages all workflows
//! - [`execute_workflow`] - Execute a single workflow

mod state;
mod executor;
mod manager;

pub use state::WorkflowState;
pub use executor::{execute_workflow, ExecutionResult};
pub use manager::WorkflowRuntime;
