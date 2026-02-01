//! Messaging infrastructure for workflow communication

mod queue;

pub use queue::{MessageQueue, AsyncMessageQueue, QueueError};
