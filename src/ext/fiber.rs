//! Fiber Extension: Reified Computations
//!
//! A Fiber is an **immutable value** representing a suspended computation.
//! This preserves all three postulates:
//!
//! - P1 (Tool): Each fiber operation is Stack → Stack
//! - P2 (Stack): No hidden state; fiber state is explicit (stack + code)
//! - P3 (Composition): Fibers are values; operations compose via concatenation
//!
//! # Mathematical Model
//!
//! A Fiber is a triple: `F = (S, C, σ)` where:
//! - S: Stack (list of values)
//! - C: Code (remaining operations)
//! - σ: Status ∈ {pending, done, error}
//!
//! The step function is:
//! ```text
//! step : Fiber → Fiber
//! step(S, op·C, pending) = (op(S), C, pending)   -- execute one op
//! step(S, ε, pending) = (S, ε, done)             -- empty code = done
//! step(S, C, done) = (S, C, done)                -- done is fixed point
//! step(S, C, error) = (S, C, error)              -- error is fixed point
//! ```
//!
//! # Key Properties
//!
//! - **Immutable**: Operations return NEW fibers, old fiber unchanged
//! - **Fork = dup**: Since fibers are values, duplication is just `dup`
//! - **Checkpoint = value**: Keep the fiber value, use it later
//! - **Deterministic**: step(F) always produces the same F'
//!
//! # Stack Effects
//!
//! | Tool | Stack Effect | Description |
//! |------|-------------|-------------|
//! | fiber-new | (quote -- fiber) | Create fiber from quotation |
//! | fiber-step | (fiber -- fiber') | Execute one operation |
//! | fiber-run | (fiber -- fiber') | Run until done or error |
//! | fiber-stack | (fiber -- list) | Get fiber's stack as list |
//! | fiber-code | (fiber -- quote) | Get remaining code |
//! | fiber-status | (fiber -- text) | Get "pending"/"done"/"error" |
//! | fiber-inject | (fiber value -- fiber') | Push value onto fiber's stack |
//! | fiber-result | (fiber -- value) | Get result from done fiber |
//! | is-fiber | (value -- bool) | Check if value is a fiber |

use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::executor::execute;
use crate::op::Op;
use crate::stack::Stack;
use crate::tool::{Tool, ToolBody};
use crate::value::{ext, ExtValue, Value, ErrorValue};
use indexmap::IndexMap;
use std::sync::Arc;

/// Fiber status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiberStatus {
    Pending,
    Done,
    Error,
}

impl FiberStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FiberStatus::Pending => "pending",
            FiberStatus::Done => "done",
            FiberStatus::Error => "error",
        }
    }
}

/// Create a Fiber value from components
pub fn make_fiber(stack: Vec<Value>, code: Vec<Op>, status: FiberStatus) -> Value {
    let mut meta = IndexMap::new();
    
    // Store stack as a list
    meta.insert("stack".to_string(), Value::List(stack));
    
    // Store code as a quote
    meta.insert("code".to_string(), Value::Quote(code));
    
    // Store status as text
    meta.insert("status".to_string(), Value::Text(status.as_str().to_string()));
    
    Value::Ext(Arc::new(ExtValue {
        kind: ext::FIBER,
        data: Box::new(Value::Null), // Marker
        meta: Some(meta),
    }))
}

/// Extract fiber components from a Value
pub fn extract_fiber(v: &Value) -> Result<(Vec<Value>, Vec<Op>, FiberStatus), Error> {
    match v {
        Value::Ext(ext) if ext.kind == ext::FIBER => {
            let meta = ext.meta.as_ref()
                .ok_or_else(|| Error::Runtime("fiber missing metadata".into()))?;
            
            // Extract stack
            let stack = match meta.get("stack") {
                Some(Value::List(l)) => l.clone(),
                _ => return Err(Error::Runtime("fiber missing stack".into())),
            };
            
            // Extract code
            let code = match meta.get("code") {
                Some(Value::Quote(q)) => q.clone(),
                _ => return Err(Error::Runtime("fiber missing code".into())),
            };
            
            // Extract status
            let status = match meta.get("status") {
                Some(Value::Text(s)) => match s.as_str() {
                    "pending" => FiberStatus::Pending,
                    "done" => FiberStatus::Done,
                    "error" => FiberStatus::Error,
                    _ => return Err(Error::Runtime("fiber invalid status".into())),
                },
                _ => return Err(Error::Runtime("fiber missing status".into())),
            };
            
            Ok((stack, code, status))
        }
        _ => Err(Error::Runtime("expected fiber".into())),
    }
}

/// Check if a value is a fiber
pub fn is_fiber(v: &Value) -> bool {
    matches!(v, Value::Ext(ext) if ext.kind == ext::FIBER)
}

/// Register all fiber tools
pub fn register(dict: &mut Dictionary) {
    // fiber-new: (quote -- fiber)
    // Creates a new fiber from a quotation with empty stack
    dict.register(Tool::native(
        "fiber-new",
        "(code:Quote -- fiber:Fiber)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let code = stack.pop()?;
                match code {
                    Value::Quote(ops) => {
                        let fiber = make_fiber(vec![], ops, FiberStatus::Pending);
                        stack.push(fiber)?;
                        Ok((stack, ctx))
                    }
                    _ => Err(Error::Runtime("fiber-new requires a quotation".into())),
                }
            })
        },
    ));

    // fiber-stack: (fiber -- list)
    // Get the fiber's internal stack as a list (non-destructive)
    dict.register(Tool::native(
        "fiber-stack",
        "(fiber:Fiber -- stack:List)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let fiber_val = stack.pop()?;
                let (fiber_stack, _, _) = extract_fiber(&fiber_val)?;
                stack.push(Value::List(fiber_stack))?;
                Ok((stack, ctx))
            })
        },
    ));

    // fiber-code: (fiber -- quote)
    // Get the fiber's remaining code as a quotation
    dict.register(Tool::native(
        "fiber-code",
        "(fiber:Fiber -- code:Quote)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let fiber_val = stack.pop()?;
                let (_, code, _) = extract_fiber(&fiber_val)?;
                stack.push(Value::Quote(code))?;
                Ok((stack, ctx))
            })
        },
    ));

    // fiber-status: (fiber -- text)
    // Get the fiber's status as text: "pending", "done", or "error"
    dict.register(Tool::native(
        "fiber-status",
        "(fiber:Fiber -- status:Text)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let fiber_val = stack.pop()?;
                let (_, _, status) = extract_fiber(&fiber_val)?;
                stack.push(Value::Text(status.as_str().to_string()))?;
                Ok((stack, ctx))
            })
        },
    ));

    // fiber-inject: (fiber value -- fiber')
    // Push a value onto the fiber's internal stack
    dict.register(Tool::native(
        "fiber-inject",
        "(fiber:Fiber value:Any -- fiber':Fiber)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                let fiber_val = stack.pop()?;
                let (mut fiber_stack, code, status) = extract_fiber(&fiber_val)?;
                
                fiber_stack.push(value);
                
                let fiber = make_fiber(fiber_stack, code, status);
                stack.push(fiber)?;
                Ok((stack, ctx))
            })
        },
    ));

    // fiber-result: (fiber -- value)
    // Get the top value from a completed fiber's stack
    dict.register(Tool::native(
        "fiber-result",
        "(fiber:Fiber -- result:Any)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let fiber_val = stack.pop()?;
                let (mut fiber_stack, _, status) = extract_fiber(&fiber_val)?;
                
                if status != FiberStatus::Done {
                    return Err(Error::Runtime(
                        format!("fiber-result requires done fiber, got {}", status.as_str())
                    ));
                }
                
                let result = fiber_stack.pop()
                    .ok_or_else(|| Error::Runtime("fiber has empty stack".into()))?;
                stack.push(result)?;
                Ok((stack, ctx))
            })
        },
    ));

    // is-fiber: (value -- bool)
    // Check if value is a fiber
    dict.register(Tool::native(
        "is-fiber",
        "(value:Any -- bool:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let val = stack.pop()?;
                stack.push(Value::Bool(is_fiber(&val)))?;
                Ok((stack, ctx))
            })
        },
    ));

    // fiber-step: (fiber -- fiber')
    // Execute one operation, return new fiber
    dict.register(Tool::native(
        "fiber-step",
        "(fiber:Fiber -- fiber':Fiber)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let fiber_val = stack.pop()?;
                let (mut fiber_stack, mut code, status) = extract_fiber(&fiber_val)?;
                
                // If already done or error, return unchanged
                if status != FiberStatus::Pending {
                    stack.push(fiber_val)?;
                    return Ok((stack, ctx));
                }
                
                // If no code left, mark as done
                if code.is_empty() {
                    let fiber = make_fiber(fiber_stack, code, FiberStatus::Done);
                    stack.push(fiber)?;
                    return Ok((stack, ctx));
                }
                
                // Execute one operation
                let op = code.remove(0);
                
                match &op {
                    Op::Push(v) => {
                        fiber_stack.push(v.clone());
                        let fiber = make_fiber(fiber_stack, code, FiberStatus::Pending);
                        stack.push(fiber)?;
                        Ok((stack, ctx))
                    }
                    Op::Call(name) => {
                        // For step, we need to execute the tool
                        let dict_guard = ctx.dict.read().await;
                        match dict_guard.get(name) {
                            Ok(tool) => {
                                // Build a temp stack from fiber_stack
                                let temp_stack = Stack::from_values(fiber_stack);
                                
                                // Clone tool body to release lock
                                let tool_body = tool.body.clone();
                                drop(dict_guard);
                                
                                // Execute based on tool body
                                let result = match &tool_body {
                                    ToolBody::Native(native_fn) => {
                                        native_fn.call(temp_stack, ctx.clone()).await
                                    }
                                    ToolBody::Ops(ops) => {
                                        execute(ops, temp_stack, ctx.clone()).await
                                    }
                                };
                                
                                match result {
                                    Ok((new_temp_stack, _new_ctx)) => {
                                        let new_fiber_stack: Vec<Value> = new_temp_stack.into_values();
                                        let fiber = make_fiber(new_fiber_stack, code, FiberStatus::Pending);
                                        stack.push(fiber)?;
                                        Ok((stack, ctx))
                                    }
                                    Err(e) => {
                                        // Capture error in fiber
                                        let error_stack = vec![Value::Error(Box::new(ErrorValue {
                                            code: "FiberError".into(),
                                            message: format!("{}", e),
                                        }))];
                                        let fiber = make_fiber(error_stack, code, FiberStatus::Error);
                                        stack.push(fiber)?;
                                        Ok((stack, ctx))
                                    }
                                }
                            }
                            Err(_) => {
                                drop(dict_guard);
                                let error_stack = vec![Value::Error(Box::new(ErrorValue {
                                    code: "FiberError".into(),
                                    message: format!("unknown tool in fiber: {}", name),
                                }))];
                                let fiber = make_fiber(error_stack, code, FiberStatus::Error);
                                stack.push(fiber)?;
                                Ok((stack, ctx))
                            }
                        }
                    }
                }
            })
        },
    ));

    // fiber-run: (fiber -- fiber')
    // Execute until done or error (max 10000 steps for safety)
    dict.register(Tool::native(
        "fiber-run",
        "(fiber:Fiber -- fiber':Fiber)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let fiber_val = stack.pop()?;
                let (mut fiber_stack, mut code, mut status) = extract_fiber(&fiber_val)?;
                
                const MAX_STEPS: usize = 10000;
                let mut steps = 0;
                
                while status == FiberStatus::Pending && steps < MAX_STEPS {
                    if code.is_empty() {
                        status = FiberStatus::Done;
                        break;
                    }
                    
                    let op = code.remove(0);
                    
                    match &op {
                        Op::Push(v) => {
                            fiber_stack.push(v.clone());
                        }
                        Op::Call(name) => {
                            let dict_guard = ctx.dict.read().await;
                            match dict_guard.get(name) {
                                Ok(tool) => {
                                    let temp_stack = Stack::from_values(fiber_stack);
                                    let tool_body = tool.body.clone();
                                    drop(dict_guard);
                                    
                                    // Execute based on tool body
                                    let result = match &tool_body {
                                        ToolBody::Native(native_fn) => {
                                            native_fn.call(temp_stack, ctx.clone()).await
                                        }
                                        ToolBody::Ops(ops) => {
                                            execute(ops, temp_stack, ctx.clone()).await
                                        }
                                    };
                                    
                                    match result {
                                        Ok((new_stack, _)) => {
                                            fiber_stack = new_stack.into_values();
                                        }
                                        Err(e) => {
                                            fiber_stack = vec![Value::Error(Box::new(ErrorValue {
                                                code: "FiberError".into(),
                                                message: format!("{}", e),
                                            }))];
                                            status = FiberStatus::Error;
                                            break;
                                        }
                                    }
                                }
                                Err(_) => {
                                    drop(dict_guard);
                                    fiber_stack = vec![Value::Error(Box::new(ErrorValue {
                                        code: "FiberError".into(),
                                        message: format!("unknown tool: {}", name),
                                    }))];
                                    status = FiberStatus::Error;
                                    break;
                                }
                            }
                        }
                    }
                    
                    steps += 1;
                }
                
                // Check if we hit max steps
                if steps >= MAX_STEPS && status == FiberStatus::Pending {
                    fiber_stack.push(Value::Error(Box::new(ErrorValue {
                        code: "FiberTimeout".into(),
                        message: format!("fiber exceeded {} steps", MAX_STEPS),
                    })));
                    status = FiberStatus::Error;
                }
                
                let fiber = make_fiber(fiber_stack, code, status);
                stack.push(fiber)?;
                Ok((stack, ctx))
            })
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_extract_fiber() {
        let fiber = make_fiber(
            vec![Value::Int(1), Value::Int(2)],
            vec![Op::call("add")],
            FiberStatus::Pending,
        );
        
        let (stack, code, status) = extract_fiber(&fiber).unwrap();
        assert_eq!(stack, vec![Value::Int(1), Value::Int(2)]);
        assert_eq!(code, vec![Op::call("add")]);
        assert_eq!(status, FiberStatus::Pending);
    }

    #[test]
    fn test_fiber_is_immutable() {
        // Creating a fiber and extracting doesn't modify original
        let fiber1 = make_fiber(
            vec![Value::Int(1)],
            vec![Op::call("dup")],
            FiberStatus::Pending,
        );
        
        let fiber2 = fiber1.clone(); // "fork" is just clone
        
        let (s1, _, _) = extract_fiber(&fiber1).unwrap();
        let (s2, _, _) = extract_fiber(&fiber2).unwrap();
        
        assert_eq!(s1, s2); // Both have same content
    }

    #[test]
    fn test_is_fiber() {
        let fiber = make_fiber(vec![], vec![], FiberStatus::Pending);
        assert!(is_fiber(&fiber));
        assert!(!is_fiber(&Value::Int(42)));
        assert!(!is_fiber(&Value::Quote(vec![])));
    }
}
