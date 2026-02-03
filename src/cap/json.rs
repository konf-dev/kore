//! JSON Tools
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | json-parse | (text -- value) | Parse JSON |
//! | json-encode | (value -- text) | Encode to JSON |

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;
use indexmap::IndexMap;

/// Register JSON tools (2)
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "json-parse",
        "(text -- value)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let text = stack.pop()?.into_text()?;
                let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
                    crate::error::Error::Runtime(format!(
                        "json-parse: invalid JSON at position {}: {}",
                        e.column(),
                        e
                    ))
                })?;
                stack.push(json_to_value(json))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "json-encode",
        "(value -- text)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                let json = value_to_json(&value);
                let text = serde_json::to_string(&json).map_err(|e| {
                    crate::error::Error::Runtime(format!("json-encode: {}", e))
                })?;
                stack.push(Value::Text(text))?;
                Ok((stack, ctx))
            })
        },
    ));
}

/// Convert serde_json::Value to kore::Value
fn json_to_value(json: serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Float(0.0)
            }
        }
        serde_json::Value::String(s) => Value::Text(s),
        serde_json::Value::Array(arr) => {
            Value::List(arr.into_iter().map(json_to_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let mut map = IndexMap::new();
            for (k, v) in obj {
                map.insert(k, json_to_value(v));
            }
            Value::Map(map)
        }
    }
}

/// Convert kore::Value to serde_json::Value
fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::Value::Number((*i).into()),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::Text(s) => serde_json::Value::String(s.clone()),
        Value::List(l) => {
            serde_json::Value::Array(l.iter().map(value_to_json).collect())
        }
        Value::Map(m) => {
            let obj: serde_json::Map<String, serde_json::Value> = m
                .iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
        Value::Quote(_) => serde_json::Value::String("[quote]".into()),
        Value::Handle(h) => serde_json::Value::String(format!("[handle:{}]", h.id)),
        Value::Error(e) => {
            let mut obj = serde_json::Map::new();
            obj.insert("error".into(), serde_json::Value::String(e.code.clone()));
            obj.insert("message".into(), serde_json::Value::String(e.message.clone()));
            serde_json::Value::Object(obj)
        }
        Value::Ext(e) => {
            let mut obj = serde_json::Map::new();
            obj.insert("ext".into(), serde_json::Value::Number(e.kind.into()));
            obj.insert("data".into(), value_to_json(&e.data));
            serde_json::Value::Object(obj)
        }
    }
}
