//! Value types - the core types that can exist on the stack
//!
//! # Design
//! 
//! 10 base types + 1 extension wrapper for future pillars (Tensor, Fiber, etc.)

use crate::error::{Error, Result};
use crate::op::Op;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// Extension kinds for the Ext variant
pub mod ext {
    /// Tensor: differentiable multi-dimensional array
    pub const TENSOR: u8 = 0;
    /// Fiber: reified computation (paused execution)
    pub const FIBER: u8 = 1;
    /// Linear: value that cannot be duplicated or discarded
    pub const LINEAR: u8 = 2;
    /// Distribution: probability distribution
    pub const DIST: u8 = 3;
}

/// Extended value - wraps any value with kind + metadata
#[derive(Debug, Clone)]
pub struct ExtValue {
    /// Extension kind (see ext module for constants)
    pub kind: u8,
    /// The wrapped value
    pub data: Box<Value>,
    /// Optional metadata
    pub meta: Option<IndexMap<String, Value>>,
}

impl PartialEq for ExtValue {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.data == other.data
    }
}

/// The 11 value types in Kore (10 base + 1 extension wrapper)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    /// Absence of value
    Null,

    /// Boolean
    Bool(bool),

    /// 64-bit signed integer
    Int(i64),

    /// 64-bit float
    Float(f64),

    /// UTF-8 string
    Text(String),

    /// Ordered list of values
    List(Vec<Value>),

    /// Ordered map with string keys
    Map(IndexMap<String, Value>),

    /// Deferred program (quote)
    Quote(Vec<Op>),

    /// Opaque handle to a resource
    Handle(Handle),

    /// Captured error
    Error(Box<ErrorValue>),

    /// Extension value: tensor, fiber, linear, distribution, etc.
    #[serde(skip)]
    Ext(Arc<ExtValue>),
}

/// Opaque handle to a resource (secrets, connections, files)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Handle {
    pub kind: HandleKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HandleKind {
    Secret,
    Connection,
    File,
    Custom(String),
}

/// Error captured as a value (for catch)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorValue {
    pub code: String,
    pub message: String,
}

impl Value {
    // Type checking methods

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "Null",
            Value::Bool(_) => "Bool",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Text(_) => "Text",
            Value::List(_) => "List",
            Value::Map(_) => "Map",
            Value::Quote(_) => "Quote",
            Value::Handle(_) => "Handle",
            Value::Error(_) => "Error",
            Value::Ext(e) => match e.kind {
                ext::TENSOR => "Tensor",
                ext::FIBER => "Fiber",
                ext::LINEAR => "Linear",
                ext::DIST => "Distribution",
                _ => "Ext",
            },
        }
    }

    // === Extension predicates ===

    /// Check if this is a linear value (cannot be duplicated or discarded)
    pub fn is_linear(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::LINEAR)
    }

    /// Check if this is a tensor value
    pub fn is_tensor(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::TENSOR)
    }

    /// Check if this is a fiber value
    pub fn is_fiber(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::FIBER)
    }

    /// Check if this is a distribution value
    pub fn is_dist(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::DIST)
    }

    // === Extension constructors ===

    /// Create a Tensor value
    pub fn tensor(data: Value, meta: Option<IndexMap<String, Value>>) -> Self {
        Value::Ext(Arc::new(ExtValue {
            kind: ext::TENSOR,
            data: Box::new(data),
            meta,
        }))
    }

    /// Create a Fiber value
    pub fn fiber(data: Value, meta: Option<IndexMap<String, Value>>) -> Self {
        Value::Ext(Arc::new(ExtValue {
            kind: ext::FIBER,
            data: Box::new(data),
            meta,
        }))
    }

    /// Create a Linear value (non-duplicable, non-discardable)
    pub fn linear(data: Value) -> Self {
        Value::Ext(Arc::new(ExtValue {
            kind: ext::LINEAR,
            data: Box::new(data),
            meta: None,
        }))
    }

    /// Create a Distribution value
    pub fn distribution(data: Value, meta: Option<IndexMap<String, Value>>) -> Self {
        Value::Ext(Arc::new(ExtValue {
            kind: ext::DIST,
            data: Box::new(data),
            meta,
        }))
    }

    /// Access extension value
    pub fn as_ext(&self) -> Result<&ExtValue> {
        match self {
            Value::Ext(e) => Ok(e.as_ref()),
            _ => Err(Error::TypeError {
                expected: "Ext".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn is_num(&self) -> bool {
        matches!(self, Value::Int(_) | Value::Float(_))
    }

    // Conversion methods

    pub fn as_bool(&self) -> Result<bool> {
        match self {
            Value::Bool(b) => Ok(*b),
            _ => Err(Error::TypeError {
                expected: "Bool".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn into_bool(self) -> Result<bool> {
        match self {
            Value::Bool(b) => Ok(b),
            _ => Err(Error::TypeError {
                expected: "Bool".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn as_int(&self) -> Result<i64> {
        match self {
            Value::Int(n) => Ok(*n),
            _ => Err(Error::TypeError {
                expected: "Int".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn into_int(self) -> Result<i64> {
        match self {
            Value::Int(n) => Ok(n),
            _ => Err(Error::TypeError {
                expected: "Int".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn as_float(&self) -> Result<f64> {
        match self {
            Value::Float(f) => Ok(*f),
            Value::Int(n) => Ok(*n as f64), // Int promotes to Float
            _ => Err(Error::TypeError {
                expected: "Float".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn as_num(&self) -> Result<f64> {
        match self {
            Value::Int(n) => Ok(*n as f64),
            Value::Float(f) => Ok(*f),
            _ => Err(Error::TypeError {
                expected: "Num".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn as_text(&self) -> Result<&str> {
        match self {
            Value::Text(s) => Ok(s),
            _ => Err(Error::TypeError {
                expected: "Text".into(),
                got: self.type_name().into(),
            }),
        }
    }

    /// Get text as Option (for chaining with Option methods)
    pub fn text_opt(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn into_text(self) -> Result<String> {
        match self {
            Value::Text(s) => Ok(s),
            _ => Err(Error::TypeError {
                expected: "Text".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn as_list(&self) -> Result<&Vec<Value>> {
        match self {
            Value::List(l) => Ok(l),
            _ => Err(Error::TypeError {
                expected: "List".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn into_list(self) -> Result<Vec<Value>> {
        match self {
            Value::List(l) => Ok(l),
            _ => Err(Error::TypeError {
                expected: "List".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn as_map(&self) -> Result<&IndexMap<String, Value>> {
        match self {
            Value::Map(m) => Ok(m),
            _ => Err(Error::TypeError {
                expected: "Map".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn into_map(self) -> Result<IndexMap<String, Value>> {
        match self {
            Value::Map(m) => Ok(m),
            _ => Err(Error::TypeError {
                expected: "Map".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn as_quote(&self) -> Result<&Vec<Op>> {
        match self {
            Value::Quote(q) => Ok(q),
            _ => Err(Error::TypeError {
                expected: "Quote".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn into_quote(self) -> Result<Vec<Op>> {
        match self {
            Value::Quote(q) => Ok(q),
            _ => Err(Error::TypeError {
                expected: "Quote".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn as_error(&self) -> Result<&ErrorValue> {
        match self {
            Value::Error(e) => Ok(e),
            _ => Err(Error::TypeError {
                expected: "Error".into(),
                got: self.type_name().into(),
            }),
        }
    }

    /// Truthiness for conditionals
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::Float(f) => *f != 0.0 && !f.is_nan(),
            Value::Text(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Map(m) => !m.is_empty(),
            Value::Quote(q) => !q.is_empty(),
            Value::Handle(_) => true,
            Value::Error(_) => true,
            Value::Ext(_) => true, // Extensions are truthy by default
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::Text(s) => write!(f, "\"{}\"", s),
            Value::List(l) => {
                write!(f, "[")?;
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Map(m) => {
                write!(f, "{{")?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Quote(ops) => write!(f, "[quote: {} ops]", ops.len()),
            Value::Handle(h) => write!(f, "<handle:{:?}:{}>", h.kind, h.id),
            Value::Error(e) => write!(f, "<error:{}:{}>", e.code, e.message),
            Value::Ext(e) => {
                let kind_name = match e.kind {
                    ext::TENSOR => "tensor",
                    ext::FIBER => "fiber",
                    ext::LINEAR => "linear",
                    ext::DIST => "dist",
                    k => return write!(f, "<ext:{}>", k),
                };
                write!(f, "<{}:{}>", kind_name, e.data)
            }
        }
    }
}

// Convenient From implementations
impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Int(n)
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Value::Int(n as i64)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Text(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Text(s.to_string())
    }
}

impl From<Vec<Value>> for Value {
    fn from(l: Vec<Value>) -> Self {
        Value::List(l)
    }
}

impl From<IndexMap<String, Value>> for Value {
    fn from(m: IndexMap<String, Value>) -> Self {
        Value::Map(m)
    }
}

impl From<Error> for Value {
    fn from(e: Error) -> Self {
        Value::Error(Box::new(ErrorValue {
            code: e.code().to_string(),
            message: e.to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truthiness() {
        assert!(!Value::Null.is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Bool(true).is_truthy());
        assert!(!Value::Int(0).is_truthy());
        assert!(Value::Int(1).is_truthy());
        assert!(Value::Int(-1).is_truthy());
        assert!(!Value::Text("".into()).is_truthy());
        assert!(Value::Text("hello".into()).is_truthy());
        assert!(!Value::List(vec![]).is_truthy());
        assert!(Value::List(vec![Value::Int(1)]).is_truthy());
    }

    #[test]
    fn test_type_conversions() {
        assert_eq!(Value::Int(42).as_int().unwrap(), 42);
        assert_eq!(Value::Int(42).as_float().unwrap(), 42.0); // Int promotes
        assert!(Value::Text("hello".into()).as_int().is_err());
    }

    // === Extension type tests ===

    #[test]
    fn test_tensor_creation() {
        let data = Value::List(vec![
            Value::Float(1.0),
            Value::Float(2.0),
            Value::Float(3.0),
        ]);
        let tensor = Value::tensor(data.clone(), None);
        assert!(tensor.is_tensor());
        assert!(!tensor.is_linear());
        assert!(!tensor.is_fiber());
        assert_eq!(tensor.type_name(), "Tensor");
    }

    #[test]
    fn test_linear_creation() {
        let inner = Value::Text("unique-resource".into());
        let linear = Value::linear(inner);
        assert!(linear.is_linear());
        assert!(!linear.is_tensor());
        assert_eq!(linear.type_name(), "Linear");
    }

    #[test]
    fn test_fiber_creation() {
        let state = Value::Map(IndexMap::new());
        let fiber = Value::fiber(state, None);
        assert!(fiber.is_fiber());
        assert!(!fiber.is_linear());
        assert_eq!(fiber.type_name(), "Fiber");
    }

    #[test]
    fn test_distribution_creation() {
        let params = Value::Map(IndexMap::new());
        let dist = Value::distribution(params, None);
        assert!(dist.is_dist());
        assert_eq!(dist.type_name(), "Distribution");
    }

    #[test]
    fn test_ext_value_display() {
        let tensor = Value::tensor(Value::Int(42), None);
        let s = format!("{}", tensor);
        assert!(s.contains("tensor"));
    }

    #[test]
    fn test_ext_arc_sharing() {
        let tensor = Value::tensor(Value::List(vec![Value::Float(1.0)]), None);
        let tensor2 = tensor.clone();
        
        // Both should share the same Arc
        if let (Value::Ext(a), Value::Ext(b)) = (&tensor, &tensor2) {
            assert!(Arc::ptr_eq(a, b), "Arc should be shared after clone");
        } else {
            panic!("Expected Ext");
        }
    }
}
