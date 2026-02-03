s//! PyO3 Python bindings for Kore runtime
//!
//! Exposes the Kore stack machine to Python for training LLMs.
//!
//! Usage from Python:
//! ```python
//! from kore_py import KoreRuntime
//! 
//! rt = KoreRuntime()
//! result = rt.execute("3 4 add 2 mul")
//! print(result.stack)  # [14]
//! print(result.trace)  # Full execution trace
//! ```

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3::exceptions::PyRuntimeError;

use crate::error::Error;
use crate::op::Op;
use crate::stack::Stack;
use crate::value::Value;

/// Convert Kore Value to Python object
fn value_to_py(py: Python<'_>, value: &Value) -> PyObject {
    match value {
        Value::Null => py.None(),
        Value::Bool(b) => b.into_py(py),
        Value::Int(n) => n.into_py(py),
        Value::Float(f) => f.into_py(py),
        Value::Text(s) => s.into_py(py),
        Value::List(items) => {
            let py_list = PyList::new_bound(py, items.iter().map(|v| value_to_py(py, v)));
            py_list.into_py(py)
        }
        Value::Map(map) => {
            let py_dict = PyDict::new_bound(py);
            for (k, v) in map {
                py_dict.set_item(k, value_to_py(py, v)).unwrap();
            }
            py_dict.into_py(py)
        }
        Value::Quote(ops) => {
            let s = format!("[{}]", ops.iter().map(|op| format!("{}", op)).collect::<Vec<_>>().join(" "));
            s.into_py(py)
        }
        Value::Handle(h) => format!("<Handle:{:?}:{}>", h.kind, h.id).into_py(py),
        Value::Error(e) => {
            let py_dict = PyDict::new_bound(py);
            py_dict.set_item("code", &e.code).unwrap();
            py_dict.set_item("message", &e.message).unwrap();
            py_dict.into_py(py)
        }
        Value::Ext(e) => format!("<Ext:kind={}>", e.kind).into_py(py),
    }
}

/// A single trace entry
#[pyclass]
#[derive(Clone)]
pub struct TraceEntry {
    #[pyo3(get)]
    pub step: usize,
    #[pyo3(get)]
    pub op: String,
    #[pyo3(get)]
    pub op_type: String,  // "push" or "call"
    #[pyo3(get)]
    pub stack_before: Vec<PyObject>,
    #[pyo3(get)]
    pub stack_after: Vec<PyObject>,
    #[pyo3(get)]
    pub success: bool,
    #[pyo3(get)]
    pub error: Option<String>,
}

/// Result of executing a Kore program
#[pyclass]
#[derive(Clone)]
pub struct ExecutionResult {
    #[pyo3(get)]
    pub success: bool,
    #[pyo3(get)]
    pub stack: Vec<PyObject>,
    #[pyo3(get)]
    pub trace: Vec<TraceEntry>,
    #[pyo3(get)]
    pub error: Option<String>,
    #[pyo3(get)]
    pub error_at_step: Option<usize>,
    #[pyo3(get)]
    pub tokens_executed: usize,
}

#[pymethods]
impl ExecutionResult {
    fn __repr__(&self) -> String {
        format!(
            "ExecutionResult(success={}, stack_len={}, tokens={})",
            self.success,
            self.stack.len(),
            self.tokens_executed
        )
    }
}

/// Kore Runtime with tracing support
#[pyclass]
pub struct KoreRuntime {
    // Context is created fresh for each execution in async mode
    // For synchronous tracing, we use a simple stack-based execution
}

#[pymethods]
impl KoreRuntime {
    #[new]
    pub fn new() -> Self {
        KoreRuntime {}
    }

    /// Execute a complete Kore program with full tracing
    pub fn execute(&self, py: Python<'_>, program: &str) -> PyResult<ExecutionResult> {
        // Parse program
        let ops = Op::parse(program).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        
        // Execute with tracing
        let mut stack = Stack::new();
        let mut trace = Vec::new();
        let mut step = 0;
        let mut error: Option<String> = None;
        let mut error_at_step: Option<usize> = None;
        
        for op in &ops {
            let stack_before: Vec<PyObject> = stack.values().iter().map(|v| value_to_py(py, v)).collect();
            
            let (op_str, op_type) = match op {
                Op::Push(v) => (format!("{}", v), "push".to_string()),
                Op::Call(name) => (name.clone(), "call".to_string()),
            };
            
            // Execute single op synchronously (simplified - no async tools)
            let result = execute_op_sync(&mut stack, op);
            
            let stack_after: Vec<PyObject> = stack.values().iter().map(|v| value_to_py(py, v)).collect();
            
            let success = result.is_ok();
            let err_msg = result.err().map(|e| e.to_string());
            
            if !success {
                error = err_msg.clone();
                error_at_step = Some(step);
            }
            
            trace.push(TraceEntry {
                step,
                op: op_str,
                op_type,
                stack_before,
                stack_after,
                success,
                error: err_msg,
            });
            
            step += 1;
            
            if !success {
                break;
            }
        }
        
        let final_stack: Vec<PyObject> = stack.values().iter().map(|v| value_to_py(py, v)).collect();
        
        Ok(ExecutionResult {
            success: error.is_none(),
            stack: final_stack,
            trace,
            error,
            error_at_step,
            tokens_executed: step,
        })
    }
    
    /// Execute a single token and return the result
    /// For incremental execution during LLM generation
    pub fn step(&self, py: Python<'_>, current_stack: Vec<PyObject>, token: &str) -> PyResult<ExecutionResult> {
        // Parse the single token
        let ops = Op::parse(token).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        
        if ops.is_empty() {
            return Err(PyRuntimeError::new_err("Empty token"));
        }
        
        // TODO: Convert Python stack to Kore stack
        // For now, start fresh and use the full program approach
        let mut stack = Stack::new();
        
        // Execute the ops
        let mut trace = Vec::new();
        let mut error: Option<String> = None;
        
        for (step, op) in ops.iter().enumerate() {
            let stack_before: Vec<PyObject> = stack.values().iter().map(|v| value_to_py(py, v)).collect();
            
            let (op_str, op_type) = match op {
                Op::Push(v) => (format!("{}", v), "push".to_string()),
                Op::Call(name) => (name.clone(), "call".to_string()),
            };
            
            let result = execute_op_sync(&mut stack, op);
            let stack_after: Vec<PyObject> = stack.values().iter().map(|v| value_to_py(py, v)).collect();
            let success = result.is_ok();
            
            if let Err(e) = result {
                error = Some(e.to_string());
            }
            
            trace.push(TraceEntry {
                step,
                op: op_str,
                op_type,
                stack_before,
                stack_after,
                success,
                error: error.clone(),
            });
            
            if !success {
                break;
            }
        }
        
        let final_stack: Vec<PyObject> = stack.values().iter().map(|v| value_to_py(py, v)).collect();
        
        Ok(ExecutionResult {
            success: error.is_none(),
            stack: final_stack,
            trace,
            error,
            error_at_step: None,
            tokens_executed: ops.len(),
        })
    }
    
    /// Get the stack effect of a token (consumes, produces)
    pub fn effect_of(&self, token: &str) -> PyResult<(u32, u32)> {
        // Use the analyzer to get effect
        let ops = Op::parse(token).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        
        let analysis = kore::analyzer::analyze(&ops);
        
        Ok((analysis.effect.consumes, analysis.effect.produces))
    }
    
    /// Check if a token can be executed with given stack depth
    pub fn can_execute(&self, stack_depth: usize, token: &str) -> PyResult<bool> {
        let (consumes, _) = self.effect_of(token)?;
        Ok(stack_depth >= consumes as usize)
    }
}

/// Synchronous execution of a single op (no async tools)
fn execute_op_sync(stack: &mut Stack, op: &Op) -> kore::Result<()> {
    match op {
        Op::Push(value) => {
            stack.push(value.clone())?;
            Ok(())
        }
        Op::Call(name) => {
            execute_builtin_sync(stack, name)
        }
    }
}

/// Execute a built-in tool synchronously
fn execute_builtin_sync(stack: &mut Stack, name: &str) -> kore::Result<()> {
    match name {
        // Stack manipulation
        "dup" => {
            let v = stack.pop()?;
            stack.push(v.clone())?;
            stack.push(v)?;
            Ok(())
        }
        "drop" => {
            stack.pop()?;
            Ok(())
        }
        "swap" => {
            let b = stack.pop()?;
            let a = stack.pop()?;
            stack.push(b)?;
            stack.push(a)?;
            Ok(())
        }
        "over" => {
            let b = stack.pop()?;
            let a = stack.pop()?;
            stack.push(a.clone())?;
            stack.push(b)?;
            stack.push(a)?;
            Ok(())
        }
        "rot" => {
            let c = stack.pop()?;
            let b = stack.pop()?;
            let a = stack.pop()?;
            stack.push(b)?;
            stack.push(c)?;
            stack.push(a)?;
            Ok(())
        }
        "nip" => {
            let b = stack.pop()?;
            stack.pop()?;  // drop a
            stack.push(b)?;
            Ok(())
        }
        
        // Arithmetic
        "add" => {
            let b = stack.pop()?;
            let a = stack.pop()?;
            match (&a, &b) {
                (Value::Int(x), Value::Int(y)) => stack.push(Value::Int(x + y))?,
                (Value::Float(x), Value::Float(y)) => stack.push(Value::Float(x + y))?,
                (Value::Int(x), Value::Float(y)) => stack.push(Value::Float(*x as f64 + y))?,
                (Value::Float(x), Value::Int(y)) => stack.push(Value::Float(x + *y as f64))?,
                (Value::Text(x), Value::Text(y)) => stack.push(Value::Text(format!("{}{}", x, y)))?,
                _ => return Err(kore::Error::TypeError { 
                    expected: "numbers or text".into(), 
                    actual: format!("{} and {}", a.type_name(), b.type_name())
                }),
            }
            Ok(())
        }
        "sub" => {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a - b))?;
            Ok(())
        }
        "mul" => {
            let b = stack.pop()?;
            let a = stack.pop()?;
            match (&a, &b) {
                (Value::Int(x), Value::Int(y)) => stack.push(Value::Int(x * y))?,
                (Value::Float(x), Value::Float(y)) => stack.push(Value::Float(x * y))?,
                (Value::Int(x), Value::Float(y)) => stack.push(Value::Float(*x as f64 * y))?,
                (Value::Float(x), Value::Int(y)) => stack.push(Value::Float(x * *y as f64))?,
                _ => return Err(kore::Error::TypeError { 
                    expected: "numbers".into(), 
                    actual: format!("{} and {}", a.type_name(), b.type_name())
                }),
            }
            Ok(())
        }
        "div" => {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            if b == 0 {
                return Err(kore::Error::DivisionByZero);
            }
            stack.push(Value::Int(a / b))?;
            Ok(())
        }
        "mod" => {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            if b == 0 {
                return Err(kore::Error::DivisionByZero);
            }
            stack.push(Value::Int(a % b))?;
            Ok(())
        }
        "neg" => {
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(-a))?;
            Ok(())
        }
        
        // Comparison
        "eq" => {
            let b = stack.pop()?;
            let a = stack.pop()?;
            stack.push(Value::Bool(a == b))?;
            Ok(())
        }
        "neq" => {
            let b = stack.pop()?;
            let a = stack.pop()?;
            stack.push(Value::Bool(a != b))?;
            Ok(())
        }
        "lt" => {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Bool(a < b))?;
            Ok(())
        }
        "gt" => {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Bool(a > b))?;
            Ok(())
        }
        "le" => {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Bool(a <= b))?;
            Ok(())
        }
        "ge" => {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Bool(a >= b))?;
            Ok(())
        }
        
        // Logic
        "and" => {
            let b = stack.pop()?.as_bool()?;
            let a = stack.pop()?.as_bool()?;
            stack.push(Value::Bool(a && b))?;
            Ok(())
        }
        "or" => {
            let b = stack.pop()?.as_bool()?;
            let a = stack.pop()?.as_bool()?;
            stack.push(Value::Bool(a || b))?;
            Ok(())
        }
        "not" => {
            let a = stack.pop()?.as_bool()?;
            stack.push(Value::Bool(!a))?;
            Ok(())
        }
        
        // Type operations
        "type" => {
            let a = stack.pop()?;
            stack.push(Value::Text(a.type_name().to_string()))?;
            Ok(())
        }
        
        // List operations
        "list-len" => {
            let list = stack.pop()?.into_list()?;
            stack.push(Value::Int(list.len() as i64))?;
            Ok(())
        }
        "list-wrap" => {
            let a = stack.pop()?;
            stack.push(Value::List(vec![a]))?;
            Ok(())
        }
        
        // Control flow - call
        "call" => {
            let quote = stack.pop()?.into_quote()?;
            for op in &quote {
                execute_op_sync(stack, op)?;
            }
            Ok(())
        }
        
        // Control flow - if
        "if" => {
            let else_quote = stack.pop()?.into_quote()?;
            let then_quote = stack.pop()?.into_quote()?;
            let cond = stack.pop()?;
            
            let chosen = if cond.is_truthy() { then_quote } else { else_quote };
            
            for op in &chosen {
                execute_op_sync(stack, op)?;
            }
            Ok(())
        }
        
        // Unknown tool
        _ => Err(kore::Error::ToolNotFound(name.to_string())),
    }
}

/// Incremental executor for step-by-step execution
#[pyclass]
pub struct IncrementalExecutor {
    stack: Stack,
    trace: Vec<TraceEntry>,
    step_count: usize,
    error: Option<String>,
    quote_depth: usize,
    pending_tokens: Vec<String>,
}

#[pymethods]
impl IncrementalExecutor {
    #[new]
    pub fn new() -> Self {
        IncrementalExecutor {
            stack: Stack::new(),
            trace: Vec::new(),
            step_count: 0,
            error: None,
            quote_depth: 0,
            pending_tokens: Vec::new(),
        }
    }
    
    /// Reset the executor
    pub fn reset(&mut self) {
        self.stack = Stack::new();
        self.trace = Vec::new();
        self.step_count = 0;
        self.error = None;
        self.quote_depth = 0;
        self.pending_tokens = Vec::new();
    }
    
    /// Execute a single token
    pub fn step(&mut self, py: Python<'_>, token: &str) -> PyResult<ExecutionResult> {
        if self.error.is_some() {
            return Err(PyRuntimeError::new_err("Previous error not cleared"));
        }
        
        // Handle quote brackets
        if token == "[" {
            self.quote_depth += 1;
            self.pending_tokens.push(token.to_string());
            return Ok(self.make_result(py, true));
        }
        
        if token == "]" {
            self.quote_depth -= 1;
            self.pending_tokens.push(token.to_string());
            
            if self.quote_depth == 0 {
                // Quote complete - parse and push
                let quote_str = self.pending_tokens.join(" ");
                self.pending_tokens.clear();
                
                let ops = Op::parse(&quote_str).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
                
                // Should be a single Push(Quote(...)) op
                if let Some(Op::Push(value)) = ops.first() {
                    let stack_before: Vec<PyObject> = self.stack.values().iter().map(|v| value_to_py(py, v)).collect();
                    
                    self.stack.push(value.clone()).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
                    
                    let stack_after: Vec<PyObject> = self.stack.values().iter().map(|v| value_to_py(py, v)).collect();
                    
                    self.trace.push(TraceEntry {
                        step: self.step_count,
                        op: quote_str,
                        op_type: "push".to_string(),
                        stack_before,
                        stack_after,
                        success: true,
                        error: None,
                    });
                    self.step_count += 1;
                }
            }
            
            return Ok(self.make_result(py, true));
        }
        
        // Inside a quote - buffer
        if self.quote_depth > 0 {
            self.pending_tokens.push(token.to_string());
            return Ok(self.make_result(py, true));
        }
        
        // Regular token - execute immediately
        let stack_before: Vec<PyObject> = self.stack.values().iter().map(|v| value_to_py(py, v)).collect();
        
        let ops = Op::parse(token).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        
        let mut success = true;
        let mut error_msg: Option<String> = None;
        
        for op in &ops {
            let result = execute_op_sync(&mut self.stack, op);
            if let Err(e) = result {
                success = false;
                error_msg = Some(e.to_string());
                self.error = error_msg.clone();
                break;
            }
        }
        
        let stack_after: Vec<PyObject> = self.stack.values().iter().map(|v| value_to_py(py, v)).collect();
        
        let (op_str, op_type) = if let Some(op) = ops.first() {
            match op {
                Op::Push(v) => (format!("{}", v), "push".to_string()),
                Op::Call(name) => (name.clone(), "call".to_string()),
            }
        } else {
            (token.to_string(), "unknown".to_string())
        };
        
        self.trace.push(TraceEntry {
            step: self.step_count,
            op: op_str,
            op_type,
            stack_before,
            stack_after,
            success,
            error: error_msg,
        });
        self.step_count += 1;
        
        Ok(self.make_result(py, success))
    }
    
    /// Get current stack as Python list
    pub fn get_stack(&self, py: Python<'_>) -> Vec<PyObject> {
        self.stack.values().iter().map(|v| value_to_py(py, v)).collect()
    }
    
    /// Get stack depth
    pub fn stack_depth(&self) -> usize {
        self.stack.depth()
    }
    
    /// Check if inside a quote
    pub fn in_quote(&self) -> bool {
        self.quote_depth > 0
    }
    
    /// Get error if any
    pub fn get_error(&self) -> Option<String> {
        self.error.clone()
    }
    
    fn make_result(&self, py: Python<'_>, success: bool) -> ExecutionResult {
        ExecutionResult {
            success,
            stack: self.stack.values().iter().map(|v| value_to_py(py, v)).collect(),
            trace: self.trace.clone(),
            error: self.error.clone(),
            error_at_step: if self.error.is_some() { Some(self.step_count.saturating_sub(1)) } else { None },
            tokens_executed: self.step_count,
        }
    }
}

/// Python module
#[pymodule]
fn kore_py(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<KoreRuntime>()?;
    m.add_class::<IncrementalExecutor>()?;
    m.add_class::<ExecutionResult>()?;
    m.add_class::<TraceEntry>()?;
    Ok(())
}
