//! Workflow handle - unique identifier for a running workflow

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for a workflow instance
/// 
/// Internally uses UUID v4 but serializes as string for portability.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkflowHandle(Uuid);

// Custom serialization as string
impl Serialize for WorkflowHandle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for WorkflowHandle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Uuid::parse_str(&s)
            .map(WorkflowHandle)
            .map_err(serde::de::Error::custom)
    }
}

impl WorkflowHandle {
    /// Create a new unique workflow handle
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from an existing UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Get as string for display/serialization
    pub fn to_string_id(&self) -> String {
        self.0.to_string()
    }

    /// Parse from string
    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for WorkflowHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for WorkflowHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WorkflowHandle({})", &self.0.to_string()[..8])
    }
}

impl fmt::Display for WorkflowHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.0.to_string()[..8])
    }
}

/// Convert to kore Handle for stack operations
impl From<WorkflowHandle> for kore::Handle {
    fn from(wh: WorkflowHandle) -> Self {
        kore::Handle {
            kind: kore::HandleKind::Custom("Workflow".into()),
            id: wh.to_string_id(),
        }
    }
}

/// Convert to kore Value
impl From<WorkflowHandle> for kore::Value {
    fn from(wh: WorkflowHandle) -> Self {
        kore::Value::Handle(wh.into())
    }
}

/// Try to extract WorkflowHandle from kore Value
impl TryFrom<&kore::Value> for WorkflowHandle {
    type Error = crate::WorkflowError;

    fn try_from(value: &kore::Value) -> Result<Self, Self::Error> {
        match value {
            kore::Value::Handle(h) => {
                if h.kind == kore::HandleKind::Custom("Workflow".into()) {
                    WorkflowHandle::parse(&h.id)
                        .map_err(|_| crate::WorkflowError::InvalidHandle(h.id.clone()))
                } else {
                    Err(crate::WorkflowError::NotAWorkflowHandle(format!("{:?}", h.kind)))
                }
            }
            _ => Err(crate::WorkflowError::NotAWorkflowHandle(value.type_name().into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_creation() {
        let h1 = WorkflowHandle::new();
        let h2 = WorkflowHandle::new();
        assert_ne!(h1, h2);
    }

    #[test]
    fn handle_display() {
        let h = WorkflowHandle::new();
        let s = format!("{}", h);
        assert_eq!(s.len(), 8); // Short form
    }

    #[test]
    fn handle_roundtrip() {
        let h = WorkflowHandle::new();
        let s = h.to_string_id();
        let h2 = WorkflowHandle::parse(&s).unwrap();
        assert_eq!(h, h2);
    }

    #[test]
    fn handle_to_kore_value() {
        let h = WorkflowHandle::new();
        let v: kore::Value = h.into();
        assert_eq!(v.type_name(), "Handle");
    }
}
