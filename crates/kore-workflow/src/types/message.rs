//! Message - typed communication between workflows

use kore::Value;
use serde::{Deserialize, Serialize};
use std::time::Instant;

use super::WorkflowHandle;

/// A message sent between workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The payload
    pub payload: Value,
    
    /// Who sent this message
    pub sender: WorkflowHandle,
    
    /// When it was sent (not serialized - for local timing only)
    #[serde(skip)]
    pub timestamp: Option<Instant>,
    
    /// Optional correlation ID for request-response patterns
    pub correlation_id: Option<String>,
}

impl Message {
    /// Create a new message with just a payload
    pub fn new(payload: Value, sender: WorkflowHandle) -> Self {
        Self {
            payload,
            sender,
            timestamp: Some(Instant::now()),
            correlation_id: None,
        }
    }

    /// Create a message with a correlation ID (for request-response)
    pub fn with_correlation(payload: Value, sender: WorkflowHandle, correlation_id: String) -> Self {
        Self {
            payload,
            sender,
            timestamp: Some(Instant::now()),
            correlation_id: Some(correlation_id),
        }
    }

    /// Get the payload value
    pub fn into_payload(self) -> Value {
        self.payload
    }

    /// Check if this is a response to a request
    pub fn is_response(&self) -> bool {
        self.correlation_id.is_some()
    }
}

/// Convert Message to kore Value (for stack operations)
impl From<Message> for Value {
    fn from(msg: Message) -> Self {
        // Represent as a Map for introspection
        Value::Map(indexmap::indexmap! {
            "payload".into() => msg.payload,
            "sender".into() => Value::Text(msg.sender.to_string_id()),
            "correlation_id".into() => msg.correlation_id
                .map(Value::Text)
                .unwrap_or(Value::Null),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_creation() {
        let sender = WorkflowHandle::new();
        let msg = Message::new(Value::Int(42), sender);
        
        assert_eq!(msg.payload, Value::Int(42));
        assert!(msg.timestamp.is_some());
        assert!(msg.correlation_id.is_none());
    }

    #[test]
    fn message_with_correlation() {
        let sender = WorkflowHandle::new();
        let msg = Message::with_correlation(
            Value::Text("hello".into()),
            sender,
            "req-123".into(),
        );
        
        assert!(msg.is_response());
        assert_eq!(msg.correlation_id, Some("req-123".into()));
    }

    #[test]
    fn message_to_value() {
        let sender = WorkflowHandle::new();
        let msg = Message::new(Value::Int(42), sender);
        let v: Value = msg.into();
        
        assert_eq!(v.type_name(), "Map");
    }
}
