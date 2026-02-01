//! Effect - Type signatures for tools
//!
//! Effect syntax: (input1:Type input2:Type -- output1:Type output2:Type)
//! Example: (a:Num b:Num -- sum:Num)

use crate::error::{Error, Result};
use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A type in the effect system
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    /// Matches any type
    Any,
    /// Null only
    Null,
    /// Boolean
    Bool,
    /// Integer
    Int,
    /// Float
    Float,
    /// Num = Int | Float
    Num,
    /// String
    Text,
    /// List of any type
    List,
    /// List of specific type
    ListOf(Box<Type>),
    /// Map with any value type
    Map,
    /// Map with specific value type
    MapOf(Box<Type>),
    /// Quote (deferred program)
    Quote,
    /// Handle (opaque resource)
    Handle,
    /// Error
    Error,
    /// Optional (Null | T)
    Optional(Box<Type>),
    /// Union of types
    Union(Vec<Type>),
}

impl Type {
    /// Check if a value matches this type
    pub fn matches(&self, value: &Value) -> bool {
        match (self, value) {
            (Type::Any, _) => true,
            (Type::Null, Value::Null) => true,
            (Type::Bool, Value::Bool(_)) => true,
            (Type::Int, Value::Int(_)) => true,
            (Type::Float, Value::Float(_)) => true,
            (Type::Num, Value::Int(_) | Value::Float(_)) => true,
            (Type::Text, Value::Text(_)) => true,
            (Type::List, Value::List(_)) => true,
            (Type::ListOf(inner), Value::List(items)) => {
                items.iter().all(|v| inner.matches(v))
            }
            (Type::Map, Value::Map(_)) => true,
            (Type::MapOf(inner), Value::Map(m)) => {
                m.values().all(|v| inner.matches(v))
            }
            (Type::Quote, Value::Quote(_)) => true,
            (Type::Handle, Value::Handle(_)) => true,
            (Type::Error, Value::Error(_)) => true,
            (Type::Optional(_), Value::Null) => true,
            (Type::Optional(inner), v) => inner.matches(v),
            (Type::Union(types), v) => types.iter().any(|t| t.matches(v)),
            _ => false,
        }
    }

    /// Parse a type from string
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        
        // Optional: ?T
        if let Some(inner) = s.strip_prefix('?') {
            return Ok(Type::Optional(Box::new(Type::parse(inner)?)));
        }

        // Parameterized: List<T> or Map<T>
        if let Some(rest) = s.strip_prefix("List<") {
            if let Some(inner) = rest.strip_suffix('>') {
                return Ok(Type::ListOf(Box::new(Type::parse(inner)?)));
            }
        }
        if let Some(rest) = s.strip_prefix("Map<") {
            if let Some(inner) = rest.strip_suffix('>') {
                return Ok(Type::MapOf(Box::new(Type::parse(inner)?)));
            }
        }

        // Union: T|U|V (simple case, no nested unions)
        if s.contains('|') && !s.contains('<') {
            let types: Result<Vec<_>> = s.split('|').map(|t| Type::parse(t.trim())).collect();
            return Ok(Type::Union(types?));
        }

        // Simple types
        match s {
            "Any" => Ok(Type::Any),
            "Null" => Ok(Type::Null),
            "Bool" => Ok(Type::Bool),
            "Int" => Ok(Type::Int),
            "Float" => Ok(Type::Float),
            "Num" => Ok(Type::Num),
            "Text" => Ok(Type::Text),
            "List" => Ok(Type::List),
            "Map" => Ok(Type::Map),
            "Quote" => Ok(Type::Quote),
            "Handle" => Ok(Type::Handle),
            "Error" => Ok(Type::Error),
            _ => Err(Error::ParseError(format!("Unknown type: {}", s))),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Any => write!(f, "Any"),
            Type::Null => write!(f, "Null"),
            Type::Bool => write!(f, "Bool"),
            Type::Int => write!(f, "Int"),
            Type::Float => write!(f, "Float"),
            Type::Num => write!(f, "Num"),
            Type::Text => write!(f, "Text"),
            Type::List => write!(f, "List"),
            Type::ListOf(inner) => write!(f, "List<{}>", inner),
            Type::Map => write!(f, "Map"),
            Type::MapOf(inner) => write!(f, "Map<{}>", inner),
            Type::Quote => write!(f, "Quote"),
            Type::Handle => write!(f, "Handle"),
            Type::Error => write!(f, "Error"),
            Type::Optional(inner) => write!(f, "?{}", inner),
            Type::Union(types) => {
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, "|")?;
                    }
                    write!(f, "{}", t)?;
                }
                Ok(())
            }
        }
    }
}

/// Named parameter in an effect signature
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    /// Parameter name (for documentation)
    pub name: String,
    /// Parameter type
    pub typ: Type,
}

/// Effect signature: (inputs -- outputs)
/// 
/// Describes what a tool consumes from the stack and what it produces.
/// Example: `(a:Num b:Num -- sum:Num)` means "pop two numbers, push their sum"
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    /// Input parameters (consumed from stack, rightmost = top)
    pub inputs: Vec<Param>,
    /// Output parameters (pushed to stack, rightmost = top)
    pub outputs: Vec<Param>,
}

impl Effect {
    /// Create an empty effect (no inputs, no outputs)
    pub fn empty() -> Self {
        Self {
            inputs: vec![],
            outputs: vec![],
        }
    }

    /// Parse effect from string: "(a:Type b:Type -- c:Type)"
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        
        // Remove outer parens
        let s = s
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| Error::ParseError("Effect must be wrapped in ()".into()))?;

        // Split on --
        let parts: Vec<&str> = s.split("--").collect();
        if parts.len() != 2 {
            return Err(Error::ParseError("Effect must contain --".into()));
        }

        let inputs = Self::parse_params(parts[0].trim())?;
        let outputs = Self::parse_params(parts[1].trim())?;

        Ok(Self { inputs, outputs })
    }

    fn parse_params(s: &str) -> Result<Vec<Param>> {
        if s.is_empty() {
            return Ok(vec![]);
        }

        let mut params = vec![];
        for part in s.split_whitespace() {
            // Check if it's a named:Type param or just a reference name
            if let Some((name, typ)) = part.split_once(':') {
                params.push(Param {
                    name: name.to_string(),
                    typ: Type::parse(typ)?,
                });
            } else {
                // Just a name reference (for outputs referring to input types)
                // Treat as Any for now - could be enhanced to track type variables
                params.push(Param {
                    name: part.to_string(),
                    typ: Type::Any,
                });
            }
        }
        Ok(params)
    }

    /// Validate that stack matches input types
    pub fn validate_inputs(&self, stack: &[Value]) -> Result<()> {
        if stack.len() < self.inputs.len() {
            return Err(Error::EffectMismatch {
                expected: format!("{} inputs", self.inputs.len()),
                got: format!("{} values on stack", stack.len()),
            });
        }

        // Check types (inputs are listed left-to-right, rightmost is top of stack)
        for (i, param) in self.inputs.iter().rev().enumerate() {
            let value = &stack[stack.len() - 1 - i];
            if !param.typ.matches(value) {
                return Err(Error::EffectMismatch {
                    expected: format!("{}:{}", param.name, param.typ),
                    got: value.type_name().to_string(),
                });
            }
        }

        Ok(())
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, p) in self.inputs.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{}:{}", p.name, p.typ)?;
        }
        write!(f, " -- ")?;
        for (i, p) in self.outputs.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{}:{}", p.name, p.typ)?;
        }
        write!(f, ")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_parse() {
        assert_eq!(Type::parse("Any").unwrap(), Type::Any);
        assert_eq!(Type::parse("Int").unwrap(), Type::Int);
        assert_eq!(Type::parse("Num").unwrap(), Type::Num);
        assert_eq!(
            Type::parse("List<Int>").unwrap(),
            Type::ListOf(Box::new(Type::Int))
        );
        assert_eq!(
            Type::parse("?Text").unwrap(),
            Type::Optional(Box::new(Type::Text))
        );
        assert_eq!(
            Type::parse("Int|Text").unwrap(),
            Type::Union(vec![Type::Int, Type::Text])
        );
    }

    #[test]
    fn test_type_matches() {
        assert!(Type::Any.matches(&Value::Int(42)));
        assert!(Type::Int.matches(&Value::Int(42)));
        assert!(!Type::Int.matches(&Value::Text("hello".into())));
        assert!(Type::Num.matches(&Value::Int(42)));
        assert!(Type::Num.matches(&Value::Float(3.14)));
        assert!(Type::Optional(Box::new(Type::Int)).matches(&Value::Null));
        assert!(Type::Optional(Box::new(Type::Int)).matches(&Value::Int(42)));
    }

    #[test]
    fn test_effect_parse() {
        let effect = Effect::parse("(a:Num b:Num -- sum:Num)").unwrap();
        assert_eq!(effect.inputs.len(), 2);
        assert_eq!(effect.outputs.len(), 1);
        assert_eq!(effect.inputs[0].name, "a");
        assert_eq!(effect.inputs[0].typ, Type::Num);
    }

    #[test]
    fn test_effect_validate() {
        let effect = Effect::parse("(a:Int b:Int -- sum:Int)").unwrap();
        
        // Valid
        assert!(effect.validate_inputs(&[Value::Int(1), Value::Int(2)]).is_ok());
        
        // Wrong type
        assert!(effect.validate_inputs(&[Value::Int(1), Value::Text("x".into())]).is_err());
        
        // Not enough values
        assert!(effect.validate_inputs(&[Value::Int(1)]).is_err());
    }
}
