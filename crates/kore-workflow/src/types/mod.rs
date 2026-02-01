//! Core types for kore-workflow
//!
//! - [`WorkflowHandle`] - Unique identifier for a workflow instance
//! - [`Message`] - Communication between workflows
//! - [`WorkflowStatus`] - Lifecycle state of a workflow
//! - [`WorkflowDef`] - Definition/blueprint for a workflow

mod handle;
mod message;
pub mod status;
mod definition;

pub use handle::WorkflowHandle;
pub use message::Message;
pub use status::{WorkflowStatus, WorkflowError as WorkflowStatusError};
pub use definition::WorkflowDef;
