//! Kore Training CLI
//!
//! Optimized CLI for RL training - outputs JSON with full traces.
//! Reads program from stdin, outputs structured result to stdout.
//!
//! Usage:
//!   echo "3 4 add" | kore-train
//!   echo "3 4 add" | kore-train --no-trace
//!   echo "3 4 add" | kore-train --max-steps 1000

use std::io::{self, Read};
use std::time::Instant;

use serde::Serialize;

use kore::error::Error;
use kore::op::Op;
use kore::stack::Stack;
use kore::value::Value;

/// JSON output format
#[derive(Serialize)]
struct Output {
    success: bool,
    final_stack: Vec<JsonValue>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    trace: Vec<TraceEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_at_step: Option<usize>,
    steps_executed: usize,
    execution_time_us: u64,
}

#[derive(Serialize)]
struct TraceEntry {
    step: usize,
    op: String,
    op_type: String,
    stack_before: Vec<JsonValue>,
    stack_after: Vec<JsonValue>,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// JSON-serializable value
#[derive(Serialize, Clone)]
#[serde(untagged)]
enum JsonValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    List(Vec<JsonValue>),
}

impl From<&Value> for JsonValue {
    fn from(v: &Value) -> Self {
        match v {
            Value::Null => JsonValue::Null,
            Value::Bool(b) => JsonValue::Bool(*b),
            Value::Int(n) => JsonValue::Int(*n),
            Value::Float(f) => JsonValue::Float(*f),
            Value::Text(s) => JsonValue::Text(s.clone()),
            Value::List(items) => JsonValue::List(items.iter().map(Into::into).collect()),
            Value::Map(m) => JsonValue::Text(format!("{{map:{}}}", m.len())),
            Value::Quote(ops) => JsonValue::Text(format!("[quote:{}]", ops.len())),
            Value::Handle(h) => JsonValue::Text(format!("<handle:{:?}>", h.kind)),
            Value::Error(e) => JsonValue::Text(format!("<error:{}>", e.code)),
            Value::Ext(e) => JsonValue::Text(format!("<ext:{}>", e.kind)),
        }
    }
}

fn main() {
    // Parse args
    let args: Vec<String> = std::env::args().collect();
    
    let mut trace_enabled = true;
    let mut max_steps: usize = 10000;
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--no-trace" => trace_enabled = false,
            "--trace" => trace_enabled = true,
            "--max-steps" => {
                i += 1;
                if i < args.len() {
                    max_steps = args[i].parse().unwrap_or(10000);
                }
            }
            "-h" | "--help" => {
                eprintln!("Usage: kore-train [--trace|--no-trace] [--max-steps N]");
                eprintln!("Reads Kore program from stdin, outputs JSON to stdout.");
                return;
            }
            _ => {}
        }
        i += 1;
    }
    
    // Read program from stdin
    let mut program = String::new();
    if io::stdin().read_to_string(&mut program).is_err() {
        output_error("Failed to read stdin");
        return;
    }
    
    // Execute
    let start = Instant::now();
    let result = execute_with_trace(&program, max_steps, trace_enabled);
    let elapsed_us = start.elapsed().as_micros() as u64;
    
    // Output JSON
    let output = Output {
        success: result.0,
        final_stack: result.1,
        trace: result.2,
        error: result.3,
        error_at_step: result.4,
        steps_executed: result.5,
        execution_time_us: elapsed_us,
    };
    
    println!("{}", serde_json::to_string(&output).unwrap());
}

fn output_error(msg: &str) {
    let output = Output {
        success: false,
        final_stack: vec![],
        trace: vec![],
        error: Some(msg.to_string()),
        error_at_step: None,
        steps_executed: 0,
        execution_time_us: 0,
    };
    println!("{}", serde_json::to_string(&output).unwrap());
}

fn execute_with_trace(
    program: &str,
    max_steps: usize,
    trace_enabled: bool,
) -> (bool, Vec<JsonValue>, Vec<TraceEntry>, Option<String>, Option<usize>, usize) {
    // Parse
    let ops = match Op::parse(program) {
        Ok(ops) => ops,
        Err(e) => return (false, vec![], vec![], Some(e.to_string()), None, 0),
    };
    
    // Execute
    let mut stack = Stack::new();
    let mut trace = Vec::new();
    let mut error: Option<String> = None;
    let mut error_at_step: Option<usize> = None;
    let mut step = 0;
    
    for op in &ops {
        if step >= max_steps {
            error = Some("Max steps exceeded".to_string());
            error_at_step = Some(step);
            break;
        }
        
        let stack_before: Vec<JsonValue> = if trace_enabled {
            stack.values().iter().map(Into::into).collect()
        } else {
            vec![]
        };
        
        let (op_str, op_type) = match op {
            Op::Push(v) => (format!("{}", v), "push".to_string()),
            Op::Call(name) => (name.clone(), "call".to_string()),
        };
        
        let result = execute_op(&mut stack, op);
        
        let stack_after: Vec<JsonValue> = if trace_enabled {
            stack.values().iter().map(Into::into).collect()
        } else {
            vec![]
        };
        
        let success = result.is_ok();
        let err_msg = result.err().map(|e| e.to_string());
        
        if !success {
            error = err_msg.clone();
            error_at_step = Some(step);
        }
        
        if trace_enabled {
            trace.push(TraceEntry {
                step,
                op: op_str,
                op_type,
                stack_before,
                stack_after,
                success,
                error: err_msg,
            });
        }
        
        step += 1;
        
        if !success {
            break;
        }
    }
    
    let final_stack: Vec<JsonValue> = stack.values().iter().map(Into::into).collect();
    
    (error.is_none(), final_stack, trace, error, error_at_step, step)
}

fn execute_op(stack: &mut Stack, op: &Op) -> kore::Result<()> {
    match op {
        Op::Push(value) => {
            stack.push(value.clone())?;
            Ok(())
        }
        Op::Call(name) => execute_builtin(stack, name),
    }
}

fn execute_builtin(stack: &mut Stack, name: &str) -> kore::Result<()> {
    match name {
        // Stack manipulation
        "dup" => {
            let v = stack.pop()?;
            if v.is_non_duplicable() {
                return Err(Error::LinearDuplicate(format!("{}", v)));
            }
            stack.push(v.clone())?;
            stack.push(v)?;
            Ok(())
        }
        "drop" => {
            let v = stack.pop()?;
            if v.is_linear() {
                return Err(Error::LinearDiscard(format!("{}", v)));
            }
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
            if a.is_non_duplicable() {
                return Err(Error::LinearDuplicate(format!("{}", a)));
            }
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
            let a = stack.pop()?;
            if a.is_linear() {
                return Err(Error::LinearDiscard(format!("{}", a)));
            }
            stack.push(b)?;
            Ok(())
        }
        "depth" => {
            stack.push(Value::Int(stack.depth() as i64))?;
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
                _ => return Err(Error::TypeError { 
                    expected: "numbers or text".into(), 
                    got: format!("{} and {}", a.type_name(), b.type_name())
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
                _ => return Err(Error::TypeError { 
                    expected: "numbers".into(), 
                    got: format!("{} and {}", a.type_name(), b.type_name())
                }),
            }
            Ok(())
        }
        "div" => {
            let b = stack.pop()?.as_int()?;
            if b == 0 {
                return Err(Error::DivisionByZero);
            }
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a / b))?;
            Ok(())
        }
        "mod" => {
            let b = stack.pop()?.as_int()?;
            if b == 0 {
                return Err(Error::DivisionByZero);
            }
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a % b))?;
            Ok(())
        }
        "neg" => {
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(-a))?;
            Ok(())
        }
        "abs" => {
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a.abs()))?;
            Ok(())
        }
        "min" => {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a.min(b)))?;
            Ok(())
        }
        "max" => {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a.max(b)))?;
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
            let t = a.type_name().to_string();
            stack.push(a)?;
            stack.push(Value::Text(t))?;
            Ok(())
        }
        
        // List operations
        "list-len" => {
            let list = stack.pop()?.into_list()?;
            stack.push(Value::Int(list.len() as i64))?;
            Ok(())
        }
        "list-empty" => {
            stack.push(Value::List(vec![]))?;
            Ok(())
        }
        "list-wrap" => {
            let a = stack.pop()?;
            stack.push(Value::List(vec![a]))?;
            Ok(())
        }
        "list-push" => {
            let item = stack.pop()?;
            let mut list = stack.pop()?.into_list()?;
            list.push(item);
            stack.push(Value::List(list))?;
            Ok(())
        }
        "list-pop" => {
            let mut list = stack.pop()?.into_list()?;
            if let Some(item) = list.pop() {
                stack.push(Value::List(list))?;
                stack.push(item)?;
                Ok(())
            } else {
                Err(Error::Runtime("Cannot pop from empty list".into()))
            }
        }
        "list-get" => {
            let idx = stack.pop()?.as_int()?;
            let list = stack.pop()?.into_list()?;
            if idx < 0 || idx as usize >= list.len() {
                return Err(Error::IndexOutOfBounds { index: idx, length: list.len() });
            }
            stack.push(list[idx as usize].clone())?;
            Ok(())
        }
        "list-concat" => {
            let b = stack.pop()?.into_list()?;
            let mut a = stack.pop()?.into_list()?;
            a.extend(b);
            stack.push(Value::List(a))?;
            Ok(())
        }
        
        // Control flow
        "call" => {
            let quote = stack.pop()?.into_quote()?;
            for op in &quote {
                execute_op(stack, op)?;
            }
            Ok(())
        }
        "if" => {
            let else_quote = stack.pop()?.into_quote()?;
            let then_quote = stack.pop()?.into_quote()?;
            let cond = stack.pop()?;
            
            let chosen = if cond.is_truthy() { then_quote } else { else_quote };
            
            for op in &chosen {
                execute_op(stack, op)?;
            }
            Ok(())
        }
        "times" => {
            let quote = stack.pop()?.into_quote()?;
            let n = stack.pop()?.as_int()?;
            
            for _ in 0..n {
                for op in &quote {
                    execute_op(stack, op)?;
                }
            }
            Ok(())
        }
        "while" => {
            let body = stack.pop()?.into_quote()?;
            let cond = stack.pop()?.into_quote()?;
            
            loop {
                for op in &cond {
                    execute_op(stack, op)?;
                }
                
                let should_continue = stack.pop()?.as_bool()?;
                if !should_continue {
                    break;
                }
                
                for op in &body {
                    execute_op(stack, op)?;
                }
            }
            Ok(())
        }
        "dip" => {
            let quote = stack.pop()?.into_quote()?;
            let a = stack.pop()?;
            
            for op in &quote {
                execute_op(stack, op)?;
            }
            
            stack.push(a)?;
            Ok(())
        }
        
        // Unknown tool
        _ => Err(Error::ToolNotFound(name.to_string())),
    }
}
