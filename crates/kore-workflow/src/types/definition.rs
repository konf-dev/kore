//! Workflow definition - the blueprint for a workflow

use kore::{Effect, Op, Value};
use serde::{Deserialize, Serialize};

/// Definition of a workflow - its name, effect signature, and code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDef {
    /// Name of the workflow
    pub name: String,
    
    /// Effect signature (optional - for composition checking)
    pub effect: Option<Effect>,
    
    /// The code to execute
    pub body: Vec<Op>,
    
    /// Optional description
    pub description: Option<String>,
    
    /// Tags for discovery
    pub tags: Vec<String>,
}

impl WorkflowDef {
    /// Create a workflow definition from a quote
    pub fn from_quote(name: impl Into<String>, body: Vec<Op>) -> Self {
        Self {
            name: name.into(),
            effect: None,
            body,
            description: None,
            tags: Vec::new(),
        }
    }

    /// Create with an effect signature
    pub fn with_effect(mut self, effect: Effect) -> Self {
        self.effect = Some(effect);
        self
    }

    /// Add a description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Create an anonymous workflow (for inline spawns)
    pub fn anonymous(body: Vec<Op>) -> Self {
        Self::from_quote(format!("anon-{}", &uuid::Uuid::new_v4().to_string()[..8]), body)
    }
}

/// Extract workflow definition from a kore Value (quote)
impl TryFrom<Value> for WorkflowDef {
    type Error = crate::WorkflowError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Quote(ops) => Ok(WorkflowDef::anonymous(ops)),
            Value::Map(m) => {
                // Allow structured definition
                let name = m.get("name")
                    .and_then(|v| v.as_text().ok())
                    .ok_or_else(|| crate::WorkflowError::InvalidDefinition("missing name".into()))?;
                
                let body = m.get("body")
                    .and_then(|v| match v {
                        Value::Quote(ops) => Some(ops.clone()),
                        _ => None,
                    })
                    .ok_or_else(|| crate::WorkflowError::InvalidDefinition("missing body".into()))?;
                
                let mut def = WorkflowDef::from_quote(name, body);
                
                if let Some(Value::Text(desc)) = m.get("description") {
                    def = def.with_description(desc);
                }
                
                Ok(def)
            }
            _ => Err(crate::WorkflowError::InvalidDefinition(
                format!("expected Quote or Map, got {}", value.type_name())
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definition_from_ops() {
        let ops = vec![Op::push(42), Op::call("dup"), Op::call("add")];
        let def = WorkflowDef::from_quote("double", ops.clone());
        
        assert_eq!(def.name, "double");
        assert_eq!(def.body.len(), 3);
        assert!(def.effect.is_none());
    }

    #[test]
    fn definition_anonymous() {
        let ops = vec![Op::push(1)];
        let def = WorkflowDef::anonymous(ops);
        
        assert!(def.name.starts_with("anon-"));
    }

    #[test]
    fn definition_from_quote_value() {
        let ops = vec![Op::push(42)];
        let value = Value::Quote(ops);
        let def = WorkflowDef::try_from(value).unwrap();
        
        assert!(def.name.starts_with("anon-"));
        assert_eq!(def.body.len(), 1);
    }
}
