//! # Error Enrichment
//!
//! Consistent error structure with context.
//!
//! ## Purpose
//!
//! Errors need to carry enough context for:
//! - Agents to understand what went wrong
//! - Humans to debug issues
//! - Systems to categorize and handle errors
//!
//! ## Error Structure
//!
//! Every error has:
//! - `code`: A machine-readable error code (e.g., "E_NETWORK_TIMEOUT")
//! - `message`: A human-readable description
//! - `context`: Additional key-value data
//!
//! ## Tools
//!
//! | Tool | Effect | Description |
//! |------|--------|-------------|
//! | `error-wrap` | `(error context -- error)` | Add context to an error |
//! | `error-code` | `(error -- text)` | Get the error code |
//! | `error-message` | `(error -- text)` | Get the error message |
//! | `error-context` | `(error -- map)` | Get the full context |

use indexmap::IndexMap;
use kore::{Context, Stack, Tool, Value};

/// Create an error value with code, message, and context.
pub fn make_error(code: &str, message: &str, context: IndexMap<String, Value>) -> Value {
    let mut error_map = IndexMap::new();
    error_map.insert("code".to_string(), Value::Text(code.to_string()));
    error_map.insert("message".to_string(), Value::Text(message.to_string()));
    error_map.insert("context".to_string(), Value::Map(context));

    Value::Error(Box::new(kore::value::ErrorValue {
        code: code.to_string(),
        message: message.to_string(),
    }))
}

/// Extract the code from an error value.
pub fn get_error_code(error: &Value) -> Option<String> {
    match error {
        Value::Error(e) => Some(e.code.clone()),
        _ => None,
    }
}

/// Extract the message from an error value.
pub fn get_error_message(error: &Value) -> Option<String> {
    match error {
        Value::Error(e) => Some(e.message.clone()),
        _ => None,
    }
}

/// Register error enrichment tools into a context.
///
/// # Tools Registered
///
/// - `error-wrap`: Add context to an error
/// - `error-code`: Get the error code
/// - `error-message`: Get the error message
/// - `error-make`: Create an error from code and message
pub async fn register_error_tools(ctx: &mut Context) {
    // error-make: (code message -- error)
    ctx.dict.write().await.register(
        Tool::native("error-make", "(text text -- error)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let message = stack.pop()?.into_text()?;
                let code = stack.pop()?.into_text()?;

                let error = Value::Error(Box::new(kore::value::ErrorValue {
                    code,
                    message,
                }));
                stack.push(error)?;

                Ok((stack, ctx))
            })
        }),
    );

    // error-code: (error -- text)
    ctx.dict.write().await.register(
        Tool::native("error-code", "(error -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let error = stack.pop()?;
                let code = get_error_code(&error).ok_or_else(|| {
                    kore::Error::TypeError {
                        expected: "Error".into(),
                        got: error.type_name().into(),
                    }
                })?;
                stack.push(Value::Text(code))?;

                Ok((stack, ctx))
            })
        }),
    );

    // error-message: (error -- text)
    ctx.dict.write().await.register(
        Tool::native("error-message", "(error -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let error = stack.pop()?;
                let message = get_error_message(&error).ok_or_else(|| {
                    kore::Error::TypeError {
                        expected: "Error".into(),
                        got: error.type_name().into(),
                    }
                })?;
                stack.push(Value::Text(message))?;

                Ok((stack, ctx))
            })
        }),
    );

    // error-wrap: (error context -- error)
    // Adds context map to the error message
    ctx.dict.write().await.register(
        Tool::native("error-wrap", "(error map -- error)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let context = stack.pop()?.into_map()?;
                let error = stack.pop()?;

                let (code, message) = match &error {
                    Value::Error(e) => (e.code.clone(), e.message.clone()),
                    _ => {
                        return Err(kore::Error::TypeError {
                            expected: "Error".into(),
                            got: error.type_name().into(),
                        });
                    }
                };

                // Format context as additional info
                let context_str: Vec<String> = context
                    .iter()
                    .map(|(k, v)| format!("{}={:?}", k, v))
                    .collect();
                let wrapped_message = if context_str.is_empty() {
                    message
                } else {
                    format!("{} [{}]", message, context_str.join(", "))
                };

                let wrapped = Value::Error(Box::new(kore::value::ErrorValue {
                    code,
                    message: wrapped_message,
                }));
                stack.push(wrapped)?;

                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_error_code() {
        let error = Value::Error(Box::new(kore::value::ErrorValue {
            code: "E_TEST".into(),
            message: "Test error".into(),
        }));

        assert_eq!(get_error_code(&error), Some("E_TEST".into()));
        assert_eq!(get_error_code(&Value::Int(42)), None);
    }

    #[test]
    fn test_get_error_message() {
        let error = Value::Error(Box::new(kore::value::ErrorValue {
            code: "E_TEST".into(),
            message: "Test error".into(),
        }));

        assert_eq!(get_error_message(&error), Some("Test error".into()));
        assert_eq!(get_error_message(&Value::Int(42)), None);
    }
}
