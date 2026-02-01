//! Error types for Konf Stack

use thiserror::Error;

/// Result type alias for Konf operations
pub type Result<T> = std::result::Result<T, Error>;

/// All possible errors in Konf Stack
#[derive(Error, Debug, Clone)]
pub enum Error {
    // Stack errors
    #[error("Stack underflow: tried to pop from empty stack")]
    StackUnderflow,

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
}

impl Error {
    /// Create a custom error
    pub fn custom(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Custom {
            code: code.into(),
            message: message.into(),
        }
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
            Self::StackUnderflow => "E_STACK_UNDERFLOW",
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
            Self::Custom { code, .. } => code,
            Self::ParseError(_) => "E_PARSE",
            Self::HttpError(_) => "E_HTTP",
            Self::ShellError(_) => "E_SHELL",
        }
    }
}
