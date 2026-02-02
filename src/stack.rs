//! Stack - the only way to pass data between tools

use crate::error::{Error, Result};
use crate::value::Value;

/// Maximum stack depth (configurable)
const DEFAULT_MAX_DEPTH: usize = 10_000;

/// The stack - a LIFO collection of Values
#[derive(Debug, Clone)]
pub struct Stack {
    values: Vec<Value>,
    max_depth: usize,
}

impl Stack {
    /// Create an empty stack
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    /// Create a stack with custom max depth
    pub fn with_max_depth(max_depth: usize) -> Self {
        Self {
            values: Vec::new(),
            max_depth,
        }
    }

    /// Create a stack from initial values (bottom to top)
    pub fn from_values(values: Vec<Value>) -> Self {
        Self {
            values,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    /// Push a value onto the stack
    pub fn push(&mut self, value: Value) -> Result<()> {
        if self.values.len() >= self.max_depth {
            return Err(Error::StackOverflow { max: self.max_depth });
        }
        self.values.push(value);
        Ok(())
    }

    /// Pop a value from the stack
    pub fn pop(&mut self) -> Result<Value> {
        self.values.pop().ok_or(Error::StackUnderflow { expected: 1, actual: 0 })
    }

    /// Peek at the top value without removing it
    pub fn peek(&self) -> Result<&Value> {
        self.values.last().ok_or(Error::StackUnderflow { expected: 1, actual: 0 })
    }

    /// Peek at the nth value from top (0 = top)
    pub fn peek_n(&self, n: usize) -> Result<&Value> {
        if n >= self.values.len() {
            return Err(Error::StackUnderflow { expected: n + 1, actual: self.values.len() });
        }
        Ok(&self.values[self.values.len() - 1 - n])
    }

    /// Get current stack depth
    pub fn depth(&self) -> usize {
        self.values.len()
    }

    /// Check if stack is empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Get all values (bottom to top)
    pub fn values(&self) -> &[Value] {
        &self.values
    }

    /// Alias for values() - returns slice of stack values
    pub fn as_slice(&self) -> &[Value] {
        &self.values
    }

    /// Convert to vector of values
    pub fn into_values(self) -> Vec<Value> {
        self.values
    }

    /// Clone the stack (for checkpointing in catch)
    pub fn checkpoint(&self) -> Self {
        self.clone()
    }

    /// Restore from checkpoint
    pub fn restore(&mut self, checkpoint: Stack) {
        self.values = checkpoint.values;
    }

    /// Clear the stack
    pub fn clear(&mut self) {
        self.values.clear();
    }

    // === Typed pop helpers ===

    /// Pop and expect an Int
    pub fn pop_int(&mut self) -> Result<i64> {
        let v = self.pop()?;
        v.into_int()
    }

    /// Pop and expect a Text (String)
    pub fn pop_text(&mut self) -> Result<String> {
        let v = self.pop()?;
        v.into_text()
    }

    /// Pop and expect a Bool
    pub fn pop_bool(&mut self) -> Result<bool> {
        let v = self.pop()?;
        v.into_bool()
    }

    /// Pop and expect a List
    pub fn pop_list(&mut self) -> Result<Vec<Value>> {
        let v = self.pop()?;
        v.into_list()
    }

    /// Pop and expect a Map
    pub fn pop_map(&mut self) -> Result<indexmap::IndexMap<String, Value>> {
        let v = self.pop()?;
        v.into_map()
    }

    /// Pop and expect a Quote (a deferred program)
    pub fn pop_quote(&mut self) -> Result<Vec<crate::op::Op>> {
        let v = self.pop()?;
        v.into_quote()
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}

// Convenient ways to create a stack
impl FromIterator<Value> for Stack {
    fn from_iter<I: IntoIterator<Item = Value>>(iter: I) -> Self {
        Self {
            values: iter.into_iter().collect(),
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_pop() {
        let mut stack = Stack::new();
        stack.push(Value::Int(1)).unwrap();
        stack.push(Value::Int(2)).unwrap();
        stack.push(Value::Int(3)).unwrap();

        assert_eq!(stack.depth(), 3);
        assert_eq!(stack.pop().unwrap().as_int().unwrap(), 3);
        assert_eq!(stack.pop().unwrap().as_int().unwrap(), 2);
        assert_eq!(stack.pop().unwrap().as_int().unwrap(), 1);
        assert!(stack.pop().is_err()); // Underflow
    }

    #[test]
    fn test_peek() {
        let mut stack = Stack::new();
        stack.push(Value::Int(1)).unwrap();
        stack.push(Value::Int(2)).unwrap();

        assert_eq!(stack.peek().unwrap().as_int().unwrap(), 2);
        assert_eq!(stack.peek_n(0).unwrap().as_int().unwrap(), 2);
        assert_eq!(stack.peek_n(1).unwrap().as_int().unwrap(), 1);
        assert!(stack.peek_n(2).is_err());
    }

    #[test]
    fn test_overflow() {
        let mut stack = Stack::with_max_depth(2);
        stack.push(Value::Int(1)).unwrap();
        stack.push(Value::Int(2)).unwrap();
        assert!(stack.push(Value::Int(3)).is_err()); // Overflow
    }

    #[test]
    fn test_checkpoint_restore() {
        let mut stack = Stack::new();
        stack.push(Value::Int(1)).unwrap();
        stack.push(Value::Int(2)).unwrap();

        let checkpoint = stack.checkpoint();

        stack.push(Value::Int(3)).unwrap();
        stack.push(Value::Int(4)).unwrap();
        assert_eq!(stack.depth(), 4);

        stack.restore(checkpoint);
        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.pop().unwrap().as_int().unwrap(), 2);
    }
}
