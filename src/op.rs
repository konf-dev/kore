//! Op - The 4 operation types

use crate::value::Value;
use serde::{Deserialize, Serialize};

/// The 4 operation types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    /// Push a literal value onto the stack
    Push(Value),

    /// Call a tool by name
    Call(String),

    /// Push a quote (deferred program) onto the stack
    Quote(Vec<Op>),

    /// Conditional execution
    If {
        then_ops: Vec<Op>,
        else_ops: Vec<Op>,
    },
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

    /// Create a Quote operation
    pub fn quote(ops: Vec<Op>) -> Self {
        Op::Quote(ops)
    }

    /// Create an If operation
    pub fn if_then_else(then_ops: Vec<Op>, else_ops: Vec<Op>) -> Self {
        Op::If { then_ops, else_ops }
    }

    /// Simple parser for testing - not for production use
    /// Supports: integers, floats, "strings", 'strings', true/false/null, quotes, tool-names
    
    pub fn parse(input: &str) -> crate::error::Result<Vec<Op>> {
        let tokens = Self::tokenize(input);
        Self::parse_tokens(&tokens)
    }

    
    fn parse_tokens(tokens: &[String]) -> crate::error::Result<Vec<Op>> {
        let mut ops = Vec::new();
        let mut i = 0;

        while i < tokens.len() {
            let token = &tokens[i];

            // Quote start
            if token == "[" {
                // Find matching ]
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
                // Parse the inner tokens as a quote
                let inner_ops = Self::parse_tokens(&tokens[i + 1..end])?;
                ops.push(Op::quote(inner_ops));
                i = end + 1;
                continue;
            }
            // Skip ] (handled by quote parsing)
            else if token == "]" {
                i += 1;
                continue;
            }
            // Try to parse as integer
            else if let Ok(n) = token.parse::<i64>() {
                ops.push(Op::push(n));
            }
            // Try to parse as float
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
            // String literal with double quotes
            else if token.starts_with('"') && token.ends_with('"') && token.len() >= 2 {
                let s = &token[1..token.len() - 1];
                // Handle escape sequences
                let s = unescape(s);
                ops.push(Op::push(s));
            }
            // String literal with single quotes
            else if token.starts_with('\'') && token.ends_with('\'') && token.len() >= 2 {
                let s = &token[1..token.len() - 1];
                // Handle escape sequences
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

    /// Simple tokenizer that respects quoted strings and brackets
    
    fn tokenize(input: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut chars = input.chars().peekable();
        let mut current = String::new();

        while let Some(c) = chars.next() {
            match c {
                // Whitespace - end current token
                ' ' | '\t' | '\n' | '\r' => {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                }
                // Brackets - separate tokens
                '[' | ']' => {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                    tokens.push(c.to_string());
                }
                // Single quote - read until closing quote
                '\'' => {
                    current.push(c);
                    while let Some(c2) = chars.next() {
                        current.push(c2);
                        if c2 == '\'' {
                            break;
                        }
                    }
                }
                // Double quote - read until closing quote (handle escaped quotes)
                '"' => {
                    current.push(c);
                    while let Some(c2) = chars.next() {
                        current.push(c2);
                        if c2 == '\\' {
                            // Escape sequence - consume next char
                            if let Some(c3) = chars.next() {
                                current.push(c3);
                            }
                        } else if c2 == '"' {
                            break;
                        }
                    }
                }
                // Any other character
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

/// Unescape string escape sequences like \n, \t, \\
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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    

    #[test]
    fn test_op_creation() {
        let push = Op::push(42);
        assert!(matches!(push, Op::Push(Value::Int(42))));

        let call = Op::call("add");
        assert!(matches!(call, Op::Call(name) if name == "add"));

        let quote = Op::quote(vec![Op::call("dup"), Op::call("add")]);
        assert!(matches!(quote, Op::Quote(ops) if ops.len() == 2));
    }
}
