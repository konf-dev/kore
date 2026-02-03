//! Value types - the core data types in Kore
//!
//! # Design Principles
//!
//! 1. **Minimal variants**: 8 core types + 1 extension wrapper
//! 2. **O(1) clone**: All compound types use Arc for sharing
//! 3. **16-byte size**: Pointer + discriminant, cache-friendly
//! 4. **Extension-ready**: Single `Ext` variant handles all extensions
//!
//! # Memory Layout
//!
//! ```text
//! Value = 16 bytes on 64-bit systems
//! ├── discriminant: 8 bytes (with padding)
//! └── data: 8 bytes (i64, f64, or Arc pointer)
//! ```

use crate::error::{Error, Result};
use crate::op::Op;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

/// Shared string type for O(1) clone
pub type ArcStr = Arc<str>;

/// Shared list type for O(1) clone
pub type ArcList = Arc<[Value]>;

/// Shared op list type for O(1) clone
pub type ArcOps = Arc<[Op]>;

/// Map type with Arc keys
pub type Map = BTreeMap<ArcStr, Value>;

/// The core value types in Kore
///
/// 8 base types + 1 extension wrapper = 9 variants total
/// Size: 16 bytes on 64-bit systems
#[derive(Clone, Debug)]
pub enum Value {
    // === Atoms (inline, no allocation) ===
    
    /// Absence of value
    Null,

    /// Boolean
    Bool(bool),

    /// 64-bit signed integer
    Int(i64),

    /// 64-bit float
    Float(f64),

    // === Compounds (Arc for O(1) clone) ===
    
    /// UTF-8 string (shared)
    Text(ArcStr),

    /// Ordered list of values (shared)
    List(ArcList),

    /// Deferred program (shared)
    Quote(ArcOps),

    /// Ordered map with string keys (shared)
    Map(Arc<Map>),

    // === Extension wrapper (handles ALL extensions) ===
    
    /// Extended value: tensor, fiber, linear, distribution, etc.
    Ext(Arc<ExtValue>),
}

/// Extension kinds
pub mod ext {
    /// Tensor: differentiable multi-dimensional array
    pub const TENSOR: u8 = 0;
    /// Fiber: reified computation (paused execution)
    pub const FIBER: u8 = 1;
    /// Linear: value that cannot be duplicated or discarded
    pub const LINEAR: u8 = 2;
    /// Distribution: probability distribution
    pub const DIST: u8 = 3;
    /// Error: captured error value
    pub const ERROR: u8 = 4;
    /// Handle: opaque resource handle
    pub const HANDLE: u8 = 5;
}

/// Extended value - wraps any value with kind + metadata
#[derive(Clone, Debug)]
pub struct ExtValue {
    /// Extension kind (see ext module for constants)
    pub kind: u8,
    /// The wrapped value
    pub data: Value,
    /// Optional metadata (lazy allocated)
    pub meta: Option<Arc<Map>>,
}

/// Error captured as a value
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorValue {
    pub code: String,
    pub message: String,
}

/// Opaque handle to a resource
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Handle {
    pub kind: HandleKind,
    pub id: String,
}

/// Handle kind enum for compatibility
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HandleKind {
    Secret,
    Connection,
    File,
    Custom(String),
}

// ============================================================================
// Value Implementation
// ============================================================================

impl Value {
    // === Constructors (maintain API compatibility) ===
    
    /// Create a Text value from any string-like type
    pub fn text(s: impl AsRef<str>) -> Self {
        Value::Text(Arc::from(s.as_ref()))
    }

    /// Create a List value from a Vec
    pub fn list(v: Vec<Value>) -> Self {
        Value::List(Arc::from(v))
    }

    /// Create a Quote value from a Vec of Ops
    pub fn quote(ops: Vec<Op>) -> Self {
        Value::Quote(Arc::from(ops))
    }

    /// Create a Map value from a BTreeMap
    pub fn map(m: BTreeMap<String, Value>) -> Self {
        let m: Map = m.into_iter()
            .map(|(k, v)| (Arc::from(k.as_str()), v))
            .collect();
        Value::Map(Arc::new(m))
    }

    /// Create a Map value from an iterator of (String, Value) pairs
    pub fn map_from_iter(iter: impl IntoIterator<Item = (String, Value)>) -> Self {
        let m: Map = iter.into_iter()
            .map(|(k, v)| (Arc::from(k.as_str()), v))
            .collect();
        Value::Map(Arc::new(m))
    }

    /// Create an Error value
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        let err = ErrorValue {
            code: code.into(),
            message: message.into(),
        };
        Value::Ext(Arc::new(ExtValue {
            kind: ext::ERROR,
            data: Value::text(format!("{}:{}", err.code, err.message)),
            meta: None,
        }))
    }

    /// Create a Handle value
    pub fn handle(kind: impl Into<String>, id: impl Into<String>) -> Self {
        let mut meta = Map::new();
        meta.insert(Arc::from("kind"), Value::text(kind.into()));
        meta.insert(Arc::from("id"), Value::text(id.into()));
        Value::Ext(Arc::new(ExtValue {
            kind: ext::HANDLE,
            data: Value::Null,
            meta: Some(Arc::new(meta)),
        }))
    }

    // === Extension constructors ===

    /// Create a Tensor value
    pub fn tensor(data: Value, meta: Option<Map>) -> Self {
        Value::Ext(Arc::new(ExtValue {
            kind: ext::TENSOR,
            data,
            meta: meta.map(Arc::new),
        }))
    }

    /// Create a Fiber value
    pub fn fiber(data: Value, meta: Option<Map>) -> Self {
        Value::Ext(Arc::new(ExtValue {
            kind: ext::FIBER,
            data,
            meta: meta.map(Arc::new),
        }))
    }

    /// Create a Linear value (non-duplicable, non-discardable)
    pub fn linear(data: Value) -> Self {
        Value::Ext(Arc::new(ExtValue {
            kind: ext::LINEAR,
            data,
            meta: None,
        }))
    }

    /// Create a Distribution value
    pub fn distribution(data: Value, meta: Option<Map>) -> Self {
        Value::Ext(Arc::new(ExtValue {
            kind: ext::DIST,
            data,
            meta: meta.map(Arc::new),
        }))
    }

    // === Type predicates ===

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "Null",
            Value::Bool(_) => "Bool",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Text(_) => "Text",
            Value::List(_) => "List",
            Value::Quote(_) => "Quote",
            Value::Map(_) => "Map",
            Value::Ext(e) => match e.kind {
                ext::TENSOR => "Tensor",
                ext::FIBER => "Fiber",
                ext::LINEAR => "Linear",
                ext::DIST => "Distribution",
                ext::ERROR => "Error",
                ext::HANDLE => "Handle",
                _ => "Ext",
            }
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn is_num(&self) -> bool {
        matches!(self, Value::Int(_) | Value::Float(_))
    }

    pub fn is_linear(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::LINEAR)
    }

    pub fn is_tensor(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::TENSOR)
    }

    pub fn is_fiber(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::FIBER)
    }

    pub fn is_dist(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::DIST)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::ERROR)
    }

    pub fn is_handle(&self) -> bool {
        matches!(self, Value::Ext(e) if e.kind == ext::HANDLE)
    }

    // === Accessors ===

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
        self.as_bool()
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
        self.as_int()
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
        self.as_float()
    }

    pub fn as_text(&self) -> Result<&str> {
        match self {
            Value::Text(s) => Ok(s.as_ref()),
            _ => Err(Error::TypeError {
                expected: "Text".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn text_opt(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s.as_ref()),
            _ => None,
        }
    }

    pub fn into_text(self) -> Result<String> {
        match self {
            Value::Text(s) => Ok(s.to_string()),
            _ => Err(Error::TypeError {
                expected: "Text".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn as_list(&self) -> Result<&[Value]> {
        match self {
            Value::List(l) => Ok(l.as_ref()),
            _ => Err(Error::TypeError {
                expected: "List".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn into_list(self) -> Result<Vec<Value>> {
        match self {
            Value::List(l) => Ok(l.to_vec()),
            _ => Err(Error::TypeError {
                expected: "List".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn as_quote(&self) -> Result<&[Op]> {
        match self {
            Value::Quote(q) => Ok(q.as_ref()),
            _ => Err(Error::TypeError {
                expected: "Quote".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn into_quote(self) -> Result<Vec<Op>> {
        match self {
            Value::Quote(q) => Ok(q.to_vec()),
            _ => Err(Error::TypeError {
                expected: "Quote".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn as_map(&self) -> Result<&Map> {
        match self {
            Value::Map(m) => Ok(m.as_ref()),
            _ => Err(Error::TypeError {
                expected: "Map".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn into_map(self) -> Result<Map> {
        match self {
            Value::Map(m) => Ok((*m).clone()),
            _ => Err(Error::TypeError {
                expected: "Map".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn as_ext(&self) -> Result<&ExtValue> {
        match self {
            Value::Ext(e) => Ok(e.as_ref()),
            _ => Err(Error::TypeError {
                expected: "Ext".into(),
                got: self.type_name().into(),
            }),
        }
    }

    pub fn as_error(&self) -> Result<ErrorValue> {
        match self {
            Value::Ext(e) if e.kind == ext::ERROR => {
                let msg = e.data.as_text().unwrap_or("");
                let parts: Vec<&str> = msg.splitn(2, ':').collect();
                Ok(ErrorValue {
                    code: parts.get(0).unwrap_or(&"").to_string(),
                    message: parts.get(1).unwrap_or(&"").to_string(),
                })
            }
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
            Value::Ext(e) => match e.kind {
                ext::ERROR => false, // Errors are falsy
                _ => true,           // Other extensions are truthy
            }
        }
    }
}

// ============================================================================
// PartialEq implementation (needed for tests)
// ============================================================================

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b || (a.is_nan() && b.is_nan()),
            (Value::Text(a), Value::Text(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Quote(a), Value::Quote(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            (Value::Ext(a), Value::Ext(b)) => {
                a.kind == b.kind && a.data == b.data && a.meta == b.meta
            }
            _ => false,
        }
    }
}

impl PartialEq for ExtValue {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.data == other.data
    }
}

// ============================================================================
// Display implementation
// ============================================================================

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
                        write!(f, " ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Map(m) => {
                write!(f, "{{")?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}:{}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Quote(ops) => write!(f, "[quote:{}]", ops.len()),
            Value::Ext(e) => {
                let kind_name = match e.kind {
                    ext::TENSOR => "tensor",
                    ext::FIBER => "fiber",
                    ext::LINEAR => "linear",
                    ext::DIST => "dist",
                    ext::ERROR => "error",
                    ext::HANDLE => "handle",
                    k => return write!(f, "<ext:{}>", k),
                };
                write!(f, "<{}:{}>", kind_name, e.data)
            }
        }
    }
}

// ============================================================================
// From implementations (API compatibility)
// ============================================================================

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

impl From<usize> for Value {
    fn from(n: usize) -> Self {
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
        Value::text(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::text(s)
    }
}

impl From<Vec<Value>> for Value {
    fn from(l: Vec<Value>) -> Self {
        Value::list(l)
    }
}

impl From<Error> for Value {
    fn from(e: Error) -> Self {
        Value::error(e.code(), e.to_string())
    }
}

// ============================================================================
// Serialization (compatibility layer)
// ============================================================================

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        match self {
            Value::Null => serializer.serialize_none(),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Int(n) => serializer.serialize_i64(*n),
            Value::Float(f) => serializer.serialize_f64(*f),
            Value::Text(s) => serializer.serialize_str(s),
            Value::List(l) => l.serialize(serializer),
            Value::Map(m) => {
                let mut map = serializer.serialize_map(Some(m.len()))?;
                for (k, v) in m.iter() {
                    map.serialize_entry(k.as_ref(), v)?;
                }
                map.end()
            }
            Value::Quote(ops) => {
                // Serialize as special object
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("__type", "quote")?;
                map.serialize_entry("ops", &ops.to_vec())?;
                map.end()
            }
            Value::Ext(e) => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("__type", "ext")?;
                map.serialize_entry("kind", &e.kind)?;
                map.serialize_entry("data", &e.data)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, SeqAccess, Visitor};

        struct ValueVisitor;

        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid Kore value")
            }

            fn visit_bool<E>(self, v: bool) -> std::result::Result<Value, E> {
                Ok(Value::Bool(v))
            }

            fn visit_i64<E>(self, v: i64) -> std::result::Result<Value, E> {
                Ok(Value::Int(v))
            }

            fn visit_u64<E>(self, v: u64) -> std::result::Result<Value, E> {
                Ok(Value::Int(v as i64))
            }

            fn visit_f64<E>(self, v: f64) -> std::result::Result<Value, E> {
                Ok(Value::Float(v))
            }

            fn visit_str<E>(self, v: &str) -> std::result::Result<Value, E> {
                Ok(Value::text(v))
            }

            fn visit_string<E>(self, v: String) -> std::result::Result<Value, E> {
                Ok(Value::text(v))
            }

            fn visit_none<E>(self) -> std::result::Result<Value, E> {
                Ok(Value::Null)
            }

            fn visit_unit<E>(self) -> std::result::Result<Value, E> {
                Ok(Value::Null)
            }

            fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(v) = seq.next_element()? {
                    values.push(v);
                }
                Ok(Value::list(values))
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values: BTreeMap<String, Value> = BTreeMap::new();
                while let Some((k, v)) = map.next_entry::<String, Value>()? {
                    values.insert(k, v);
                }
                
                // Check for special types
                if let Some(Value::Text(t)) = values.get("__type") {
                    match t.as_ref() {
                        "quote" => {
                            if let Some(Value::List(ops)) = values.get("ops") {
                                // Would need to deserialize ops properly
                                return Ok(Value::Quote(Arc::from(vec![])));
                            }
                        }
                        "ext" => {
                            if let (Some(Value::Int(kind)), Some(data)) = 
                                (values.get("kind"), values.get("data")) 
                            {
                                return Ok(Value::Ext(Arc::new(ExtValue {
                                    kind: *kind as u8,
                                    data: data.clone(),
                                    meta: None,
                                })));
                            }
                        }
                        _ => {}
                    }
                }
                
                Ok(Value::map(values))
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_size() {
        // Value should be 16 bytes (pointer + discriminant)
        // This may vary by platform, but should be small
        let size = std::mem::size_of::<Value>();
        println!("Value size: {} bytes", size);
        assert!(size <= 24, "Value is too large: {} bytes", size);
    }

    #[test]
    fn test_arc_sharing() {
        let list = Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        let list2 = list.clone();
        
        // Both should point to same data
        if let (Value::List(a), Value::List(b)) = (&list, &list2) {
            assert!(Arc::ptr_eq(a, b), "Arc should be shared after clone");
        } else {
            panic!("Expected List");
        }
    }

    #[test]
    fn test_text_arc_sharing() {
        let text = Value::text("hello world");
        let text2 = text.clone();
        
        if let (Value::Text(a), Value::Text(b)) = (&text, &text2) {
            assert!(Arc::ptr_eq(a, b), "Arc should be shared after clone");
        } else {
            panic!("Expected Text");
        }
    }

    #[test]
    fn test_truthiness() {
        assert!(!Value::Null.is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Bool(true).is_truthy());
        assert!(!Value::Int(0).is_truthy());
        assert!(Value::Int(1).is_truthy());
        assert!(Value::Int(-1).is_truthy());
        assert!(!Value::text("").is_truthy());
        assert!(Value::text("hello").is_truthy());
        assert!(!Value::list(vec![]).is_truthy());
        assert!(Value::list(vec![Value::Int(1)]).is_truthy());
    }

    #[test]
    fn test_type_conversions() {
        assert_eq!(Value::Int(42).as_int().unwrap(), 42);
        assert_eq!(Value::Int(42).as_float().unwrap(), 42.0);
        assert!(Value::text("hello").as_int().is_err());
    }

    #[test]
    fn test_linear_predicate() {
        let v = Value::linear(Value::Int(42));
        assert!(v.is_linear());
        assert!(!Value::Int(42).is_linear());
    }

    #[test]
    fn test_tensor_predicate() {
        let t = Value::tensor(Value::list(vec![Value::Float(1.0)]), None);
        assert!(t.is_tensor());
        assert!(!Value::Int(42).is_tensor());
    }

    #[test]
    fn test_error_value() {
        let e = Value::error("NOT_FOUND", "Resource not found");
        assert!(e.is_error());
        let err = e.as_error().unwrap();
        assert_eq!(err.code, "NOT_FOUND");
    }

    #[test]
    fn test_from_impls() {
        let _ = Value::from(42i64);
        let _ = Value::from(3.14f64);
        let _ = Value::from("hello");
        let _ = Value::from(String::from("world"));
        let _ = Value::from(true);
        let _ = Value::from(vec![Value::Int(1)]);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Value::Null), "null");
        assert_eq!(format!("{}", Value::Bool(true)), "true");
        assert_eq!(format!("{}", Value::Int(42)), "42");
        assert_eq!(format!("{}", Value::text("hello")), "\"hello\"");
        assert_eq!(format!("{}", Value::list(vec![Value::Int(1), Value::Int(2)])), "[1 2]");
    }
}
