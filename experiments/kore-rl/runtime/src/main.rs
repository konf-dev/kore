//! Kore Runtime Server
//!
//! HTTP/JSON server for executing Kore programs with full tracing.
//! Designed for RL training: receives programs, returns traces.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio::time::timeout;

use kore::context::Context;
use kore::op::Op;
use kore::stack::Stack;
use kore::value::Value;

/// Maximum concurrent executions
const MAX_CONCURRENT: usize = 100;

/// Default timeout per execution (ms)
const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Maximum program length
const MAX_PROGRAM_LENGTH: usize = 10000;

/// Maximum steps per execution
const MAX_STEPS: u32 = 10000;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct ExecuteRequest {
    program: String,
    #[serde(default)]
    initial_stack: Vec<JsonValue>,
    #[serde(default = "default_max_steps")]
    max_steps: u32,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}

fn default_max_steps() -> u32 { MAX_STEPS }
fn default_timeout() -> u64 { DEFAULT_TIMEOUT_MS }

#[derive(Debug, Serialize)]
struct ExecuteResponse {
    success: bool,
    final_stack: Vec<JsonValue>,
    trace: Vec<TraceEntry>,
    error: Option<String>,
    error_at_step: Option<usize>,
    steps_executed: usize,
    execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
struct TraceEntry {
    step: usize,
    op: String,
    op_type: String,  // "push" or "call"
    stack_before: Vec<JsonValue>,
    stack_after: Vec<JsonValue>,
    success: bool,
    error: Option<String>,
}

/// JSON-serializable value wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum JsonValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    List(Vec<JsonValue>),
    Map(std::collections::HashMap<String, JsonValue>),
    Quote(String),
    Handle(String),
    Error { code: String, message: String },
    Ext { kind: String, data: Box<JsonValue> },
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
            Value::Map(map) => JsonValue::Map(
                map.iter().map(|(k, v)| (k.clone(), v.into())).collect()
            ),
            Value::Quote(ops) => JsonValue::Quote(
                format!("[{}]", ops.iter().map(|o| format!("{}", o)).collect::<Vec<_>>().join(" "))
            ),
            Value::Handle(h) => JsonValue::Handle(format!("{:?}:{}", h.kind, h.id)),
            Value::Error(e) => JsonValue::Error { 
                code: e.code.clone(), 
                message: e.message.clone() 
            },
            Value::Ext(e) => {
                let kind = match e.kind {
                    0 => "tensor",
                    1 => "fiber",
                    2 => "linear",
                    3 => "dist",
                    4 => "affine",
                    k => return JsonValue::Text(format!("<ext:{}>", k)),
                };
                JsonValue::Ext {
                    kind: kind.to_string(),
                    data: Box::new(e.data.as_ref().into()),
                }
            }
        }
    }
}

// ============================================================================
// Server State
// ============================================================================

struct AppState {
    semaphore: Semaphore,
}

// ============================================================================
// Handlers
// ============================================================================

async fn health() -> &'static str {
    "ok"
}

async fn execute_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, (StatusCode, String)> {
    // Validate request
    if req.program.len() > MAX_PROGRAM_LENGTH {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Program too long: {} > {}", req.program.len(), MAX_PROGRAM_LENGTH),
        ));
    }

    // Acquire semaphore permit
    let _permit = state.semaphore.acquire().await.map_err(|e| {
        (StatusCode::SERVICE_UNAVAILABLE, e.to_string())
    })?;

    // Execute with timeout
    let timeout_duration = Duration::from_millis(req.timeout_ms.min(DEFAULT_TIMEOUT_MS));
    
    let result = timeout(timeout_duration, async {
        execute_program(&req).await
    }).await;

    match result {
        Ok(response) => Ok(Json(response)),
        Err(_) => Err((
            StatusCode::REQUEST_TIMEOUT,
            "Execution timeout".to_string(),
        )),
    }
}

async fn execute_program(req: &ExecuteRequest) -> ExecuteResponse {
    let start = Instant::now();
    
    // Parse program
    let ops = match Op::parse(&req.program) {
        Ok(ops) => ops,
        Err(e) => {
            return ExecuteResponse {
                success: false,
                final_stack: vec![],
                trace: vec![],
                error: Some(format!("Parse error: {}", e)),
                error_at_step: None,
                steps_executed: 0,
                execution_time_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    // Create context with limited capabilities (sandbox)
    let ctx = Context::new();
    
    // Initialize stack
    let mut stack = Stack::new();
    // TODO: Convert initial_stack from JSON to Kore values
    
    // Execute with tracing
    let mut trace = Vec::new();
    let mut error: Option<String> = None;
    let mut error_at_step: Option<usize> = None;
    let mut step = 0;
    
    for op in &ops {
        if step >= req.max_steps as usize {
            error = Some("Max steps exceeded".to_string());
            error_at_step = Some(step);
            break;
        }
        
        let stack_before: Vec<JsonValue> = stack.values().iter().map(Into::into).collect();
        
        let (op_str, op_type) = match op {
            Op::Push(v) => (format!("{}", v), "push".to_string()),
            Op::Call(name) => (name.clone(), "call".to_string()),
        };
        
        // Execute single operation (context unused for now, keeping for future use)
        let _ = &ctx;
        let result = execute_single_op_sync(&mut stack, op);
        
        let stack_after: Vec<JsonValue> = stack.values().iter().map(Into::into).collect();
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
    
    let final_stack: Vec<JsonValue> = stack.values().iter().map(Into::into).collect();
    
    ExecuteResponse {
        success: error.is_none(),
        final_stack,
        trace,
        error,
        error_at_step,
        steps_executed: step,
        execution_time_ms: start.elapsed().as_millis() as u64,
    }
}

/// Execute built-in tools (subset for training)
fn execute_builtin(stack: &mut Stack, name: &str) -> kore::Result<()> {
    use kore::error::Error;
    
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
            stack.push(a)?;  // Keep value on stack
            stack.push(Value::Text(t))?;
            Ok(())
        }
        "is-null" => {
            let a = stack.pop()?;
            stack.push(Value::Bool(matches!(a, Value::Null)))?;
            Ok(())
        }
        "is-int" => {
            let a = stack.pop()?;
            stack.push(Value::Bool(matches!(a, Value::Int(_))))?;
            Ok(())
        }
        "is-bool" => {
            let a = stack.pop()?;
            stack.push(Value::Bool(matches!(a, Value::Bool(_))))?;
            Ok(())
        }
        "is-list" => {
            let a = stack.pop()?;
            stack.push(Value::Bool(matches!(a, Value::List(_))))?;
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
                execute_single_op_sync(stack, op)?;
            }
            Ok(())
        }
        "if" => {
            let else_quote = stack.pop()?.into_quote()?;
            let then_quote = stack.pop()?.into_quote()?;
            let cond = stack.pop()?;
            
            let chosen = if cond.is_truthy() { then_quote } else { else_quote };
            
            for op in &chosen {
                execute_single_op_sync(stack, op)?;
            }
            Ok(())
        }
        "times" => {
            let quote = stack.pop()?.into_quote()?;
            let n = stack.pop()?.as_int()?;
            
            for _ in 0..n {
                for op in &quote {
                    execute_single_op_sync(stack, op)?;
                }
            }
            Ok(())
        }
        "while" => {
            let body = stack.pop()?.into_quote()?;
            let cond = stack.pop()?.into_quote()?;
            
            loop {
                // Evaluate condition
                for op in &cond {
                    execute_single_op_sync(stack, op)?;
                }
                
                let should_continue = stack.pop()?.as_bool()?;
                if !should_continue {
                    break;
                }
                
                // Execute body
                for op in &body {
                    execute_single_op_sync(stack, op)?;
                }
            }
            Ok(())
        }
        "dip" => {
            let quote = stack.pop()?.into_quote()?;
            let a = stack.pop()?;
            
            for op in &quote {
                execute_single_op_sync(stack, op)?;
            }
            
            stack.push(a)?;
            Ok(())
        }
        
        // Unknown tool
        _ => Err(Error::ToolNotFound(name.to_string())),
    }
}

/// Sync version for nested calls
fn execute_single_op_sync(stack: &mut Stack, op: &Op) -> kore::Result<()> {
    match op {
        Op::Push(value) => {
            stack.push(value.clone())?;
            Ok(())
        }
        Op::Call(name) => execute_builtin(stack, name),
    }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let host = args.iter()
        .position(|a| a == "--host")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("127.0.0.1");
    let port: u16 = args.iter()
        .position(|a| a == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    let state = Arc::new(AppState {
        semaphore: Semaphore::new(MAX_CONCURRENT),
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/execute", post(execute_handler))
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", host, port).parse().unwrap();
    println!("Kore Runtime Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
