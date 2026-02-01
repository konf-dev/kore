//! # kore-workflow
//!
//! Concurrent workflow primitives for kore. Enables spawning, messaging,
//! and coordination of concurrent workflows that compose like regular tools.
//!
//! ## Core Concepts
//!
//! - **Workflow**: A running instance of kore code with its own stack and context
//! - **WorkflowHandle**: Unique identifier for a workflow (can be on the stack)
//! - **Message**: Communication between workflows
//! - **Runtime**: Manages all running workflows
//!
//! ## Tools Provided
//!
//! | Tool | Effect | Description |
//! |------|--------|-------------|
//! | `spawn` | `(quote -- handle)` | Start a new workflow |
//! | `await` | `(handle -- result)` | Wait for workflow completion |
//! | `send` | `(handle message --)` | Send message to workflow |
//! | `recv` | `(-- message)` | Receive next message (blocks) |
//! | `recv-timeout` | `(ms -- message-or-null)` | Receive with timeout |
//! | `self` | `(-- handle)` | Get current workflow's handle |
//! | `parent` | `(-- handle-or-null)` | Get parent workflow's handle |
//! | `status` | `(handle -- status)` | Get workflow status |
//! | `cancel` | `(handle --)` | Cancel a workflow |
//!
//! ## Example
//!
//! ```ignore
//! // Spawn a workflow that doubles its input
//! (recv dup add) spawn   // -> handle
//! 21 swap send           // send 21 to it
//! await                  // -> 42
//! ```
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    WorkflowRuntime                       │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │
//! │  │ Workflow A  │  │ Workflow B  │  │ Workflow C  │      │
//! │  │ (running)   │  │ (waiting)   │  │ (complete)  │      │
//! │  └──────┬──────┘  └──────┬──────┘  └─────────────┘      │
//! │         │                │                               │
//! │         └────────────────┘                               │
//! │              Messages                                    │
//! └─────────────────────────────────────────────────────────┘
//! ```

mod error;
pub mod types;
pub mod runtime;
pub mod messaging;
pub mod tools;

pub use error::{WorkflowError, Result};
pub use types::{WorkflowHandle, Message, WorkflowStatus, WorkflowDef};

// Re-export tools for easy registration
pub use tools::all_tools;
