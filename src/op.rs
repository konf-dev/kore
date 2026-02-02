//! Op - The Two Operations
//!
//! Following Postulate 1 (Everything is a Tool), there are only two operations:
//! - Push: Put a value on the stack
//! - Call: Execute a tool by name
//!
//! Everything else (conditionals, loops, etc.) is implemented as tools.

use crate::value::Value;
use serde::{Deserialize, Serialize};

/// The two fundamental operations.
/// 
/// This is the irreducible core. Everything else is built from tools.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    /// Push a literal value onto the stack
    Push(Value),

    /// Call a tool by name
    Call(String),
}

impl Op {
    /// Create a Push operation
    pub fn push(value: impl Into<Value>) -> Self {
        Op::Push(value.into())
    }

    /// Create a Call operation
    pub fn call(name: impl Into<String>) -> Self {
        Op::Call(name.into())
    }

    /// Create a Quote push (convenience for pushing a program as a value)
    /// This is just Push(Value::Quote(...)), not a special operation.
    pub fn quote(ops: Vec<Op>) -> Self {
        Op::Push(Value::Quote(ops))
    }

    /// Parse a string into a sequence of operations.
    /// 
    /// Syntax:
    /// - Integers: `42`, `-7`
    /// - Floats: `3.14`, `-0.5`
    /// - Booleans: `true`, `false`
    /// - Null: `null`
    /// - Text: `"hello"` or `'hello'`
    /// - Quotes: `[ ... ]`
    /// - Tool calls: any other word
    /// - Comments: `# ...` until end of line
    pub fn parse(input: &str) -> crate::error::Result<Vec<Op>> {
        let tokens = Self::tokenize(input);
        Self::parse_tokens(&tokens)
    }

    fn parse_tokens(tokens: &[String]) -> crate::error::Result<Vec<Op>> {
        let mut ops = Vec::new();
        let mut i = 0;

        while i < tokens.len() {
            let token = &tokens[i];

            // Quote start - this becomes Push(Value::Quote(...))
            if token == "[" {
                let mut depth = 1;
                let mut end = i + 1;
                while end < tokens.len() && depth > 0 {
                    if tokens[end] == "[" {
                        depth += 1;
                    } else if tokens[end] == "]" {
                        depth -= 1;
                    }
                    if depth > 0 {
                        end += 1;
                    }
                }
                let inner_ops = Self::parse_tokens(&tokens[i + 1..end])?;
                ops.push(Op::quote(inner_ops));  // Uses Push(Value::Quote(...))
                i = end + 1;
                continue;
            }
            // Skip ] (handled by quote parsing)
            else if token == "]" {
                i += 1;
                continue;
            }
            // Integer
            else if let Ok(n) = token.parse::<i64>() {
                ops.push(Op::push(n));
            }
            // Float (must come after integer check)
            else if let Ok(f) = token.parse::<f64>() {
                ops.push(Op::push(f));
            }
            // Boolean true
            else if token == "true" {
                ops.push(Op::push(true));
            }
            // Boolean false
            else if token == "false" {
                ops.push(Op::push(false));
            }
            // Null
            else if token == "null" {
                ops.push(Op::Push(Value::Null));
            }
            // String literal
            else if (token.starts_with('"') && token.ends_with('"') && token.len() >= 2)
                 || (token.starts_with('\'') && token.ends_with('\'') && token.len() >= 2) {
                let s = &token[1..token.len() - 1];
                let s = unescape(s);
                ops.push(Op::push(s));
            }
            // Tool name (anything else)
            else {
                ops.push(Op::call(token.clone()));
            }

            i += 1;
        }

        Ok(ops)
    }

    fn tokenize(input: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut chars = input.chars().peekable();
        let mut current = String::new();

        while let Some(c) = chars.next() {
            match c {
                // Comment
                '#' => {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                    for c2 in chars.by_ref() {
                        if c2 == '\n' {
                            break;
                        }
                    }
                }
                // Whitespace
                ' ' | '\t' | '\n' | '\r' => {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                }
                // Brackets
                '[' | ']' => {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                    tokens.push(c.to_string());
                }
                // Single-quoted string
                '\'' => {
                    current.push(c);
                    for c2 in chars.by_ref() {
                        current.push(c2);
                        if c2 == '\'' {
                            break;
                        }
                    }
                }
                // Double-quoted string with escapes
                '"' => {
                    current.push(c);
                    while let Some(c2) = chars.next() {
                        current.push(c2);
                        if c2 == '\\' {
                            if let Some(c3) = chars.next() {
                                current.push(c3);
                            }
                        } else if c2 == '"' {
                            break;
                        }
                    }
                }
                // Other characters
                _ => {
                    current.push(c);
                }
            }
        }

        if !current.is_empty() {
            tokens.push(current);
        }

        tokens
    }
}

/// Unescape string escape sequences
fn unescape(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('\'') => result.push('\''),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    
    result
}

impl std::fmt::Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Op::Push(v) => write!(f, "push({})", v),
            Op::Call(name) => write!(f, "{}", name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push() {
        let op = Op::push(42);
        assert!(matches!(op, Op::Push(Value::Int(42))));
    }

    #[test]
    fn test_call() {
        let op = Op::call("add");
        assert!(matches!(op, Op::Call(name) if name == "add"));
    }

    #[test]
    fn test_quote_is_push() {
        let op = Op::quote(vec![Op::call("dup")]);
        // Quote is just Push(Value::Quote(...)), not a special variant
        assert!(matches!(op, Op::Push(Value::Quote(_))));
    }

    #[test]
    fn test_parse_simple() {
        let ops = Op::parse("1 2 add").unwrap();
        assert_eq!(ops.len(), 3);
        assert!(matches!(&ops[0], Op::Push(Value::Int(1))));
        assert!(matches!(&ops[1], Op::Push(Value::Int(2))));
        assert!(matches!(&ops[2], Op::Call(name) if name == "add"));
    }

    #[test]
    fn test_parse_quote() {
        let ops = Op::parse("[ dup add ]").unwrap();
        assert_eq!(ops.len(), 1);
        // Quote becomes Push(Value::Quote(...))
        match &ops[0] {
            Op::Push(Value::Quote(inner)) => {
                assert_eq!(inner.len(), 2);
            }
            _ => panic!("Expected Push(Quote)"),
        }
    }
}
