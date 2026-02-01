//! # Schema Validation
//!
//! Validate values against JSON Schema.
//!
//! ## Purpose
//!
//! Schema validation is essential for:
//! - Validating MCP tool inputs/outputs
//! - API request/response validation
//! - Configuration validation
//! - Agent-provided data validation
//!
//! ## Tools
//!
//! | Tool | Effect | Description |
//! |------|--------|-------------|
//! | `schema-validate` | `(schema value -- bool)` | Check if value matches schema |
//! | `schema-errors` | `(schema value -- list)` | Get validation error messages |
//! | `schema-parse` | `(text -- schema)` | Parse a JSON Schema from text |

use kore::{Context, Stack, Tool, Value};
use jsonschema::JSONSchema;

/// Parse a JSON Schema from a string.
fn parse_schema(schema_str: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(schema_str).map_err(|e| format!("Invalid JSON Schema: {}", e))
}

/// Validate a value against a JSON Schema.
///
/// Returns Ok(true) if valid, Ok(false) if invalid.
pub fn validate_value(schema: &serde_json::Value, value: &Value) -> Result<bool, String> {
    let compiled = JSONSchema::compile(schema)
        .map_err(|e| format!("Failed to compile schema: {}", e))?;

    let value_json: serde_json::Value = serde_json::to_value(value)
        .map_err(|e| format!("Failed to convert value: {}", e))?;

    Ok(compiled.is_valid(&value_json))
}

/// Get validation errors for a value against a schema.
///
/// Returns a list of error messages.
pub fn get_validation_errors(schema: &serde_json::Value, value: &Value) -> Result<Vec<String>, String> {
    let compiled = JSONSchema::compile(schema)
        .map_err(|e| format!("Failed to compile schema: {}", e))?;

    let value_json: serde_json::Value = serde_json::to_value(value)
        .map_err(|e| format!("Failed to convert value: {}", e))?;

    let errors: Vec<String> = compiled
        .validate(&value_json)
        .err()
        .map(|iter| iter.map(|e| e.to_string()).collect())
        .unwrap_or_default();

    Ok(errors)
}

/// Register schema validation tools into a context.
///
/// # Tools Registered
///
/// - `schema-validate`: Check if value matches schema
/// - `schema-errors`: Get validation error messages
/// - `schema-parse`: Parse a JSON Schema from text
pub async fn register_schema_tools(ctx: &mut Context) {
    // schema-parse: (text -- map)
    // Parse a JSON Schema string into a map
    ctx.dict.write().await.register(
        Tool::native("schema-parse", "(text -- map)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let schema_str = stack.pop()?.into_text()?;
                let schema = parse_schema(&schema_str).map_err(kore::Error::Runtime)?;

                // Convert to Value
                let value: Value = serde_json::from_value(schema)
                    .map_err(|e| kore::Error::Runtime(format!("Failed to convert schema: {}", e)))?;

                stack.push(value)?;
                Ok((stack, ctx))
            })
        }),
    );

    // schema-validate: (schema value -- bool)
    ctx.dict.write().await.register(
        Tool::native("schema-validate", "(map any -- bool)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                let schema_value = stack.pop()?;

                // Convert schema to serde_json::Value
                let schema_json: serde_json::Value = serde_json::to_value(&schema_value)
                    .map_err(|e| kore::Error::Runtime(format!("Invalid schema: {}", e)))?;

                let valid = validate_value(&schema_json, &value)
                    .map_err(kore::Error::Runtime)?;

                stack.push(Value::Bool(valid))?;
                Ok((stack, ctx))
            })
        }),
    );

    // schema-errors: (schema value -- list)
    ctx.dict.write().await.register(
        Tool::native("schema-errors", "(map any -- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                let schema_value = stack.pop()?;

                // Convert schema to serde_json::Value
                let schema_json: serde_json::Value = serde_json::to_value(&schema_value)
                    .map_err(|e| kore::Error::Runtime(format!("Invalid schema: {}", e)))?;

                let errors = get_validation_errors(&schema_json, &value)
                    .map_err(kore::Error::Runtime)?;

                let error_values: Vec<Value> = errors
                    .into_iter()
                    .map(Value::Text)
                    .collect();

                stack.push(Value::List(error_values))?;
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_value_valid() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" }
            },
            "required": ["name"]
        });

        let value = Value::Map(indexmap::indexmap! {
            "name".to_string() => Value::Text("Alice".into()),
            "age".to_string() => Value::Int(30),
        });

        assert!(validate_value(&schema, &value).unwrap());
    }

    #[test]
    fn test_validate_value_invalid() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "required": ["name"]
        });

        let value = Value::Map(indexmap::indexmap! {
            "age".to_string() => Value::Int(30),
        });

        assert!(!validate_value(&schema, &value).unwrap());
    }

    #[test]
    fn test_get_validation_errors() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "required": ["name"]
        });

        let value = Value::Map(indexmap::indexmap! {
            "age".to_string() => Value::Int(30),
        });

        let errors = get_validation_errors(&schema, &value).unwrap();
        assert!(!errors.is_empty());
        assert!(errors[0].contains("name"));
    }

    #[test]
    fn test_parse_schema() {
        let schema_str = r#"{"type": "string"}"#;
        let schema = parse_schema(schema_str).unwrap();
        assert_eq!(schema["type"], "string");
    }

    #[test]
    fn test_parse_invalid_schema() {
        let result = parse_schema("not json");
        assert!(result.is_err());
    }
}
