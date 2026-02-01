//! # Serialization
//!
//! Convert between Value and wire formats.
//!
//! ## Purpose
//!
//! Values need to be serialized for:
//! - HTTP request/response bodies
//! - Database storage
//! - File I/O
//! - Inter-process communication
//!
//! ## Formats
//!
//! | Format | Use Case |
//! |--------|----------|
//! | JSON | HTTP APIs, config files |
//!
//! ## Tools
//!
//! | Tool | Effect | Description |
//! |------|--------|-------------|
//! | `to-json` | `(value -- text)` | Serialize value to JSON string |
//! | `from-json` | `(text -- value)` | Parse JSON string to value |

use kore::{Context, Stack, Tool, Value};

/// Convert a Kore Value to JSON string.
pub fn value_to_json(value: &Value) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| format!("JSON serialization failed: {}", e))
}

/// Convert a Kore Value to pretty JSON string.
pub fn value_to_json_pretty(value: &Value) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|e| format!("JSON serialization failed: {}", e))
}

/// Parse a JSON string to a Kore Value.
pub fn json_to_value(json: &str) -> Result<Value, String> {
    serde_json::from_str(json).map_err(|e| format!("JSON parse failed: {}", e))
}

/// Register serialization tools into a context.
///
/// # Tools Registered
///
/// - `to-json`: Serialize value to JSON string
/// - `to-json-pretty`: Serialize value to pretty JSON string
/// - `from-json`: Parse JSON string to value
pub async fn register_serial_tools(ctx: &mut Context) {
    // to-json: (value -- text)
    ctx.dict.write().await.register(
        Tool::native("to-json", "(any -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                let json = value_to_json(&value).map_err(kore::Error::Runtime)?;
                stack.push(Value::Text(json))?;
                Ok((stack, ctx))
            })
        }),
    );

    // to-json-pretty: (value -- text)
    ctx.dict.write().await.register(
        Tool::native("to-json-pretty", "(any -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                let json = value_to_json_pretty(&value).map_err(kore::Error::Runtime)?;
                stack.push(Value::Text(json))?;
                Ok((stack, ctx))
            })
        }),
    );

    // from-json: (text -- value)
    ctx.dict.write().await.register(
        Tool::native("from-json", "(text -- any)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let json = stack.pop()?.into_text()?;
                let value = json_to_value(&json).map_err(kore::Error::Runtime)?;
                stack.push(value)?;
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::indexmap;

    #[test]
    fn test_value_to_json() {
        assert_eq!(value_to_json(&Value::Null).unwrap(), "null");
        assert_eq!(value_to_json(&Value::Bool(true)).unwrap(), "true");
        assert_eq!(value_to_json(&Value::Int(42)).unwrap(), "42");
        assert_eq!(value_to_json(&Value::Float(3.14)).unwrap(), "3.14");
        assert_eq!(value_to_json(&Value::Text("hello".into())).unwrap(), "\"hello\"");
    }

    #[test]
    fn test_json_to_value() {
        assert_eq!(json_to_value("null").unwrap(), Value::Null);
        assert_eq!(json_to_value("true").unwrap(), Value::Bool(true));
        assert_eq!(json_to_value("42").unwrap(), Value::Int(42));
        assert_eq!(json_to_value("\"hello\"").unwrap(), Value::Text("hello".into()));
    }

    #[test]
    fn test_roundtrip() {
        let value = Value::Map(indexmap! {
            "name".to_string() => Value::Text("test".into()),
            "count".to_string() => Value::Int(42),
            "items".to_string() => Value::List(vec![Value::Int(1), Value::Int(2)]),
        });

        let json = value_to_json(&value).unwrap();
        let parsed = json_to_value(&json).unwrap();

        assert_eq!(value, parsed);
    }

    #[test]
    fn test_invalid_json() {
        let result = json_to_value("not valid json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("JSON parse failed"));
    }
}
