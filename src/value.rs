//! Value types - the 10 types that can exist on the stack

use crate::error::{Error, Result};
use crate::op::Op;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The 10 value types in Konf Stack
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
}
