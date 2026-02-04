//! Error types for Konf Stack

use thiserror::Error;

/// Result type alias for Konf operations
pub type Result<T> = std::result::Result<T, Error>;

/// All possible errors in Konf Stack
#[derive(Error, Debug, Clone)]
pub enum Error {
    // Stack errors
    #[error("Stack underflow: expected {expected}, had {actual}")]
    StackUnderflow { expected: usize, actual: usize },

    #[error("Stack overflow: exceeded maximum depth of {max}")]
    StackOverflow { max: usize },

    // Type errors
    #[error("Type error: expected {expected}, got {got}")]
    TypeError { expected: String, got: String },

    #[error("Type mismatch in effect: tool expects {expected}, stack has {got}")]
    EffectMismatch { expected: String, got: String },

    // Tool errors
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Native function not found: {0}")]
    NativeFunctionNotFound(String),

    #[error("Invalid tool definition: {0}")]
    InvalidToolDefinition(String),

    // Execution errors
    #[error("Division by zero")]
    DivisionByZero,

    #[error("Index out of bounds: {index} (length {length})")]
    IndexOutOfBounds { index: i64, length: usize },

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Assertion failed: {0}")]
    AssertionFailed(String),

    // Runtime error (generic)
    #[error("Runtime error: {0}")]
    Runtime(String),

    // Capability denied
    #[error("Capability denied: '{capability}' required for tool '{tool}'")]
    CapabilityDenied { capability: String, tool: String },

    // Resource exhausted
    #[error("Resource exhausted: {resource} - requested {requested}, available {available}")]
    ResourceExhausted { resource: String, requested: u64, available: u64 },

    // Custom user error
    #[error("{code}: {message}")]
    Custom { code: String, message: String },

    // Parse errors
    #[error("Parse error: {0}")]
    ParseError(String),

    // I/O errors
    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("Shell error: {0}")]
    ShellError(String),

    #[error("I/O error: {0}")]
    IoError(String),

    // Linear type errors (for resource safety)
    #[error("Linear value cannot be duplicated: {0}")]
    LinearDuplicate(String),

    #[error("Linear value cannot be discarded: {0}")]
    LinearDiscard(String),
}

impl Error {
    /// Create a custom error
    pub fn custom(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Custom {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Create an I/O error
    pub fn io(message: impl Into<String>) -> Self {
        Self::IoError(message.into())
    }

    /// Create a type error
    pub fn type_error(expected: &str, got: &crate::value::Value) -> Self {
        Self::TypeError {
            expected: expected.to_string(),
            got: got.type_name().to_string(),
        }
    }

    /// Get error code for matching
    pub fn code(&self) -> &str {
        match self {
            Self::StackUnderflow { .. } => "E_STACK_UNDERFLOW",
            Self::StackOverflow { .. } => "E_STACK_OVERFLOW",
            Self::TypeError { .. } => "E_TYPE",
            Self::EffectMismatch { .. } => "E_EFFECT_MISMATCH",
            Self::ToolNotFound(_) => "E_TOOL_NOT_FOUND",
            Self::NativeFunctionNotFound(_) => "E_NATIVE_NOT_FOUND",
            Self::InvalidToolDefinition(_) => "E_INVALID_TOOL",
            Self::DivisionByZero => "E_DIV_ZERO",
            Self::IndexOutOfBounds { .. } => "E_INDEX_BOUNDS",
            Self::KeyNotFound(_) => "E_KEY_NOT_FOUND",
            Self::AssertionFailed(_) => "E_ASSERTION",
            Self::Runtime(_) => "E_RUNTIME",
            Self::CapabilityDenied { .. } => "E_CAPABILITY_DENIED",
            Self::ResourceExhausted { .. } => "E_RESOURCE_EXHAUSTED",
            Self::Custom { code, .. } => code,
            Self::ParseError(_) => "E_PARSE",
            Self::HttpError(_) => "E_HTTP",
            Self::ShellError(_) => "E_SHELL",
            Self::IoError(_) => "E_IO",
            Self::LinearDuplicate(_) => "E_LINEAR_DUP",
            Self::LinearDiscard(_) => "E_LINEAR_DROP",
        }
    }

    /// Convert error to machine-parseable map (for error-info tool)
    pub fn to_map(&self) -> indexmap::IndexMap<String, crate::value::Value> {
        use crate::value::Value;
        use indexmap::IndexMap;
        
        let mut m = IndexMap::new();
        m.insert("code".into(), Value::Text(self.code().into()));
        m.insert("message".into(), Value::Text(self.to_string()));
        
        // Add structured fields based on variant
        match self {
            Self::StackUnderflow { expected, actual } => {
                m.insert("expected".into(), Value::Int(*expected as i64));
                m.insert("actual".into(), Value::Int(*actual as i64));
            }
            Self::StackOverflow { max } => {
                m.insert("max".into(), Value::Int(*max as i64));
            }
            Self::TypeError { expected, got } => {
                m.insert("expected_type".into(), Value::Text(expected.clone()));
                m.insert("got_type".into(), Value::Text(got.clone()));
            }
            Self::EffectMismatch { expected, got } => {
                m.insert("expected_effect".into(), Value::Text(expected.clone()));
                m.insert("got_effect".into(), Value::Text(got.clone()));
            }
            Self::CapabilityDenied { capability, tool } => {
                m.insert("capability".into(), Value::Text(capability.clone()));
                m.insert("tool".into(), Value::Text(tool.clone()));
            }
            Self::IndexOutOfBounds { index, length } => {
                m.insert("index".into(), Value::Int(*index));
                m.insert("length".into(), Value::Int(*length as i64));
            }
            Self::ResourceExhausted { resource, requested, available } => {
                m.insert("resource".into(), Value::Text(resource.clone()));
                m.insert("requested".into(), Value::Int(*requested as i64));
                m.insert("available".into(), Value::Int(*available as i64));
            }
            Self::ToolNotFound(name) => {
                m.insert("tool".into(), Value::Text(name.clone()));
            }
            Self::KeyNotFound(key) => {
                m.insert("key".into(), Value::Text(key.clone()));
            }
            Self::Custom { code, message } => {
                m.insert("custom_code".into(), Value::Text(code.clone()));
                m.insert("custom_message".into(), Value::Text(message.clone()));
            }
            _ => {}
        }
        
        m
    }
}
