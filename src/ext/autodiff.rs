//! Automatic Differentiation for Kore Tensors
//!
//! Implements reverse-mode autodiff (backpropagation) following the postulates:
//! - P1: Everything is a tool (requires-grad, backward, grad-of are tools)
//! - P2: Tools transform stacks (gradient context is a stack value)
//! - P3: Composition is concatenation (no special syntax)
//!
//! # Design
//!
//! Each tensor with requires_grad=true carries:
//! - id: unique identifier for the tensor
//! - grad_fn: the operation that created it (Add, Mul, etc.)
//! - inputs: list of input tensor IDs
//! - saved_data: cached input data for backward pass
//!
//! The computation graph is implicit in the tensor metadata chain.
//! `backward` traverses this chain to compute gradients.
//!
//! # Example
//!
//! ```kore
//! [1.0 2.0 3.0] tensor-from-list requires-grad "x" def
//! [0.5 0.5 0.5] tensor-from-list requires-grad "w" def
//! x w tensor-mul tensor-sum  # loss = sum(x * w)
//! backward                    # compute gradients (returns gradient map)
//! x swap grad-get             # get dx
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use indexmap::IndexMap;

use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::{ext, ExtValue, Value};

/// Unique ID generator for tensors (for graph tracking)
static TENSOR_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a unique tensor ID
pub fn new_tensor_id() -> u64 {
    TENSOR_ID_COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Extension kind for GradContext
pub const GRAD_CONTEXT: u8 = 10;

/// Extension kind for grad_fn info stored in tensor
pub const GRAD_FN_INFO: u8 = 11;

/// Get tensor ID from metadata, or return 0 if not tracked
pub fn get_tensor_id(meta: &Option<IndexMap<String, Value>>) -> u64 {
    meta.as_ref()
        .and_then(|m| m.get("id"))
        .and_then(|v| match v {
            Value::Int(id) => Some(*id as u64),
            _ => None,
        })
        .unwrap_or(0)
}

/// Check if tensor requires gradient
pub fn requires_grad_check(meta: &Option<IndexMap<String, Value>>) -> bool {
    meta.as_ref()
        .and_then(|m| m.get("requires_grad"))
        .map(|v| matches!(v, Value::Bool(true) | Value::Int(1)))
        .unwrap_or(false)
}

/// Flatten tensor data to Vec<f64>
fn flatten_to_f64(v: &Value) -> Vec<f64> {
    match v {
        Value::List(l) => l.iter().flat_map(flatten_to_f64).collect(),
        Value::Float(f) => vec![*f],
        Value::Int(n) => vec![*n as f64],
        _ => vec![0.0],
    }
}

/// Convert Vec<f64> to Value::List
fn f64_to_value_list(data: &[f64]) -> Value {
    Value::List(data.iter().map(|&x| Value::Float(x)).collect())
}

/// Create a tensor with gradient info
pub fn tensor_with_grad(
    data: Value,
    shape: Option<IndexMap<String, Value>>,
    grad_fn: &str,
    input_ids: Vec<u64>,
    saved_tensors: Vec<Vec<f64>>,
) -> Value {
    tensor_with_grad_and_inputs(data, shape, grad_fn, input_ids, saved_tensors, vec![])
}

/// Create a tensor with gradient info and input metadata for graph traversal
pub fn tensor_with_grad_and_inputs(
    data: Value,
    shape: Option<IndexMap<String, Value>>,
    grad_fn: &str,
    input_ids: Vec<u64>,
    saved_tensors: Vec<Vec<f64>>,
    input_metas: Vec<Option<IndexMap<String, Value>>>,
) -> Value {
    let id = new_tensor_id();
    let mut meta = shape.unwrap_or_default();
    meta.insert("id".to_string(), Value::Int(id as i64));
    meta.insert("requires_grad".to_string(), Value::Bool(true));
    meta.insert("grad_fn".to_string(), Value::Text(grad_fn.to_string()));
    
    // Store input IDs
    let input_ids_val: Vec<Value> = input_ids.iter().map(|&id| Value::Int(id as i64)).collect();
    meta.insert("input_ids".to_string(), Value::List(input_ids_val));
    
    // Store saved tensors for backward (flattened)
    let saved: Vec<Value> = saved_tensors.iter()
        .map(|t| f64_to_value_list(t))
        .collect();
    meta.insert("saved_tensors".to_string(), Value::List(saved));
    
    // Store input metadata for graph traversal during backward
    let input_metas_val: Vec<Value> = input_metas.iter()
        .map(|m| m.as_ref().map(|im| Value::Map(im.clone())).unwrap_or(Value::Null))
        .collect();
    meta.insert("input_metas".to_string(), Value::List(input_metas_val));
    
    Value::tensor(data, Some(meta))
}

/// Gradient computation functions
pub mod backward_fns {

    pub fn add_backward(upstream: &[f64], _saved: &[Vec<f64>]) -> Vec<Vec<f64>> {
        // d/dx(x+y) = 1, d/dy(x+y) = 1
        vec![upstream.to_vec(), upstream.to_vec()]
    }

    pub fn sub_backward(upstream: &[f64], _saved: &[Vec<f64>]) -> Vec<Vec<f64>> {
        // d/dx(x-y) = 1, d/dy(x-y) = -1
        let neg: Vec<f64> = upstream.iter().map(|g| -g).collect();
        vec![upstream.to_vec(), neg]
    }

    pub fn mul_backward(upstream: &[f64], saved: &[Vec<f64>]) -> Vec<Vec<f64>> {
        // d/dx(x*y) = y * upstream, d/dy(x*y) = x * upstream
        let x_data = &saved[0];
        let y_data = &saved[1];
        let dx: Vec<f64> = upstream.iter().zip(y_data.iter())
            .map(|(g, y)| g * y).collect();
        let dy: Vec<f64> = upstream.iter().zip(x_data.iter())
            .map(|(g, x)| g * x).collect();
        vec![dx, dy]
    }

    pub fn sum_backward(upstream: &[f64], saved: &[Vec<f64>]) -> Vec<Vec<f64>> {
        // d/dx(sum(x)) = ones * upstream (broadcast)
        let scalar = upstream.get(0).copied().unwrap_or(1.0);
        let size = saved[0].len();
        vec![vec![scalar; size]]
    }

    pub fn relu_backward(upstream: &[f64], saved: &[Vec<f64>]) -> Vec<Vec<f64>> {
        // d/dx(relu(x)) = upstream * (x > 0 ? 1 : 0)
        let x_data = &saved[0];
        let dx: Vec<f64> = upstream.iter().zip(x_data.iter())
            .map(|(g, x)| if *x > 0.0 { *g } else { 0.0 }).collect();
        vec![dx]
    }

    pub fn sigmoid_backward(upstream: &[f64], saved: &[Vec<f64>]) -> Vec<Vec<f64>> {
        // d/dx(σ(x)) = σ(x) * (1 - σ(x)) * upstream
        // saved[0] is the output (sigmoid values)
        let output = &saved[0];
        let dx: Vec<f64> = upstream.iter().zip(output.iter())
            .map(|(g, s)| g * s * (1.0 - s)).collect();
        vec![dx]
    }

    pub fn softmax_backward(upstream: &[f64], saved: &[Vec<f64>]) -> Vec<Vec<f64>> {
        // Jacobian-vector product for softmax
        let output = &saved[0];
        let n = output.len();
        let mut dx = vec![0.0; n];
        
        // J_ij = s_i * (δ_ij - s_j)
        for i in 0..n {
            for j in 0..n {
                let jacobian_ij = if i == j {
                    output[i] * (1.0 - output[i])
                } else {
                    -output[i] * output[j]
                };
                dx[i] += jacobian_ij * upstream[j];
            }
        }
        vec![dx]
    }

    pub fn log_backward(upstream: &[f64], saved: &[Vec<f64>]) -> Vec<Vec<f64>> {
        // d/dx(log(x)) = upstream / x
        let x_data = &saved[0];
        let dx: Vec<f64> = upstream.iter().zip(x_data.iter())
            .map(|(g, x)| g / x.max(1e-10)).collect();
        vec![dx]
    }

    pub fn exp_backward(upstream: &[f64], saved: &[Vec<f64>]) -> Vec<Vec<f64>> {
        // d/dx(exp(x)) = exp(x) * upstream
        // saved[0] is the output (exp values)
        let output = &saved[0];
        let dx: Vec<f64> = upstream.iter().zip(output.iter())
            .map(|(g, e)| g * e).collect();
        vec![dx]
    }

    pub fn neg_backward(upstream: &[f64], _saved: &[Vec<f64>]) -> Vec<Vec<f64>> {
        // d/dx(-x) = -upstream
        let dx: Vec<f64> = upstream.iter().map(|g| -g).collect();
        vec![dx]
    }

    pub fn scale_backward(upstream: &[f64], saved: &[Vec<f64>]) -> Vec<Vec<f64>> {
        // d/dx(c*x) = c * upstream
        // saved[0] contains [scalar]
        let scalar = saved[0].get(0).copied().unwrap_or(1.0);
        let dx: Vec<f64> = upstream.iter().map(|g| g * scalar).collect();
        vec![dx]
    }

    pub fn matmul_backward(upstream: &[f64], saved: &[Vec<f64>]) -> Vec<Vec<f64>> {
        // y = W @ x where W is (m, n) and x is (n,)
        // saved: [w_data, x_data, m, n]
        let w_data = &saved[0];
        let x_data = &saved[1];
        let m = saved[2][0] as usize;
        let n = saved[3][0] as usize;

        // dL/dx = W^T @ dL/dy
        let mut dx = vec![0.0; n];
        for j in 0..n {
            for i in 0..m {
                dx[j] += w_data[i * n + j] * upstream[i];
            }
        }

        // dL/dW = dL/dy ⊗ x (flattened outer product)
        let mut dw = vec![0.0; m * n];
        for i in 0..m {
            for j in 0..n {
                dw[i * n + j] = upstream[i] * x_data[j];
            }
        }

        vec![dw, dx]
    }
}

/// Perform backward pass on a computation graph
/// Reserved for future use when we implement full graph traversal
#[allow(dead_code)]
fn backward_pass(
    loss: &ExtValue,
    grads: &mut HashMap<u64, Vec<f64>>,
    tensors: &HashMap<u64, Arc<ExtValue>>,
) {
    let loss_id = get_tensor_id(&loss.meta);
    let loss_data = flatten_to_f64(&loss.data);
    
    // Initialize loss gradient to 1.0
    grads.insert(loss_id, vec![1.0; loss_data.len().max(1)]);
    
    // Collect all nodes in reverse topological order (by ID, since IDs are sequential)
    let mut nodes: Vec<u64> = tensors.keys().cloned().collect();
    nodes.sort_by(|a, b| b.cmp(a)); // Reverse order
    
    for node_id in nodes {
        let upstream = match grads.get(&node_id) {
            Some(g) => g.clone(),
            None => continue,
        };
        
        let tensor = match tensors.get(&node_id) {
            Some(t) => t,
            None => continue,
        };
        
        // Get grad_fn and inputs
        let grad_fn = tensor.meta.as_ref()
            .and_then(|m| m.get("grad_fn"))
            .and_then(|v| match v {
                Value::Text(s) => Some(s.as_str()),
                _ => None,
            });
        
        let input_ids: Vec<u64> = tensor.meta.as_ref()
            .and_then(|m| m.get("input_ids"))
            .and_then(|v| match v {
                Value::List(l) => Some(l.iter().filter_map(|v| match v {
                    Value::Int(i) => Some(*i as u64),
                    _ => None,
                }).collect()),
                _ => None,
            })
            .unwrap_or_default();
        
        let saved_tensors: Vec<Vec<f64>> = tensor.meta.as_ref()
            .and_then(|m| m.get("saved_tensors"))
            .and_then(|v| match v {
                Value::List(l) => Some(l.iter().map(flatten_to_f64).collect()),
                _ => None,
            })
            .unwrap_or_default();
        
        // Compute input gradients based on grad_fn
        let input_grads: Vec<Vec<f64>> = match grad_fn {
            Some("Add") => backward_fns::add_backward(&upstream, &saved_tensors),
            Some("Sub") => backward_fns::sub_backward(&upstream, &saved_tensors),
            Some("Mul") => backward_fns::mul_backward(&upstream, &saved_tensors),
            Some("Sum") => backward_fns::sum_backward(&upstream, &saved_tensors),
            Some("Relu") => backward_fns::relu_backward(&upstream, &saved_tensors),
            Some("Sigmoid") => backward_fns::sigmoid_backward(&upstream, &saved_tensors),
            Some("Softmax") => backward_fns::softmax_backward(&upstream, &saved_tensors),
            Some("Log") => backward_fns::log_backward(&upstream, &saved_tensors),
            Some("Exp") => backward_fns::exp_backward(&upstream, &saved_tensors),
            Some("Neg") => backward_fns::neg_backward(&upstream, &saved_tensors),
            Some("Scale") => backward_fns::scale_backward(&upstream, &saved_tensors),
            Some("MatMul") => backward_fns::matmul_backward(&upstream, &saved_tensors),
            _ => continue, // Leaf node or unknown
        };
        
        // Accumulate gradients for inputs
        for (i, input_id) in input_ids.iter().enumerate() {
            if let Some(input_grad) = input_grads.get(i) {
                grads.entry(*input_id)
                    .and_modify(|g| {
                        for (j, val) in input_grad.iter().enumerate() {
                            if j < g.len() {
                                g[j] += val;
                            }
                        }
                    })
                    .or_insert_with(|| input_grad.clone());
            }
        }
    }
}

/// Process a single tensor's gradient info and compute input gradients (RECURSIVE)
fn process_grad_backward(
    meta: &IndexMap<String, Value>,
    upstream: &[f64],
    grads: &mut HashMap<u64, Vec<f64>>,
) {
    // Get grad_fn
    let grad_fn = meta.get("grad_fn")
        .and_then(|v| match v {
            Value::Text(s) => Some(s.as_str()),
            _ => None,
        });
    
    // Get input IDs
    let input_ids: Vec<u64> = meta.get("input_ids")
        .and_then(|v| match v {
            Value::List(l) => Some(l.iter().filter_map(|v| match v {
                Value::Int(i) => Some(*i as u64),
                _ => None,
            }).collect()),
            _ => None,
        })
        .unwrap_or_default();
    
    // Get saved tensors
    let saved_tensors: Vec<Vec<f64>> = meta.get("saved_tensors")
        .and_then(|v| match v {
            Value::List(l) => Some(l.iter().map(flatten_to_f64).collect()),
            _ => None,
        })
        .unwrap_or_default();
    
    // Get input metadata for recursive traversal
    let input_metas: Vec<Option<IndexMap<String, Value>>> = meta.get("input_metas")
        .and_then(|v| match v {
            Value::List(l) => Some(l.iter().map(|v| match v {
                Value::Map(m) => Some(m.clone()),
                _ => None,
            }).collect()),
            _ => None,
        })
        .unwrap_or_default();
    
    // Compute input gradients based on grad_fn
    let input_grads: Vec<Vec<f64>> = match grad_fn {
        Some("Add") => backward_fns::add_backward(upstream, &saved_tensors),
        Some("Sub") => backward_fns::sub_backward(upstream, &saved_tensors),
        Some("Mul") => backward_fns::mul_backward(upstream, &saved_tensors),
        Some("Sum") => backward_fns::sum_backward(upstream, &saved_tensors),
        Some("Relu") => backward_fns::relu_backward(upstream, &saved_tensors),
        Some("Sigmoid") => backward_fns::sigmoid_backward(upstream, &saved_tensors),
        Some("Softmax") => backward_fns::softmax_backward(upstream, &saved_tensors),
        Some("Log") => backward_fns::log_backward(upstream, &saved_tensors),
        Some("Exp") => backward_fns::exp_backward(upstream, &saved_tensors),
        Some("Neg") => backward_fns::neg_backward(upstream, &saved_tensors),
        Some("Scale") => backward_fns::scale_backward(upstream, &saved_tensors),
        Some("MatMul") => backward_fns::matmul_backward(upstream, &saved_tensors),
        Some("Leaf") | None => return, // Leaf node - no upstream gradients
        Some(_) => return, // Unknown grad_fn
    };
    
    // Accumulate gradients for inputs and recursively process
    for (i, input_id) in input_ids.iter().enumerate() {
        if *input_id == 0 {
            continue;
        }
        if let Some(input_grad) = input_grads.get(i) {
            // Accumulate gradient
            grads.entry(*input_id)
                .and_modify(|g| {
                    for (j, val) in input_grad.iter().enumerate() {
                        if j < g.len() {
                            g[j] += val;
                        }
                    }
                })
                .or_insert_with(|| input_grad.clone());
            
            // Recursively process input's gradient if it has metadata
            if let Some(Some(input_meta)) = input_metas.get(i) {
                let input_upstream = grads.get(input_id).cloned().unwrap_or_default();
                process_grad_backward(input_meta, &input_upstream, grads);
            }
        }
    }
}

/// Register autodiff tools
pub fn register_autodiff(dict: &mut Dictionary) {
    // requires-grad: (tensor -- tensor)
    // Mark tensor for gradient tracking
    dict.register(Tool::native(
        "requires-grad",
        "(tensor:Tensor -- tensor:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let t = stack.pop()?;
                let ext = t.as_ext()?;

                if ext.kind != ext::TENSOR {
                    return Err(Error::TypeError {
                        expected: "Tensor".into(),
                        got: t.type_name().into(),
                    });
                }

                // Clone metadata and add gradient tracking info
                let mut meta = ext.meta.clone().unwrap_or_default();
                let id = new_tensor_id();
                meta.insert("requires_grad".to_string(), Value::Bool(true));
                meta.insert("id".to_string(), Value::Int(id as i64));
                // Leaf tensor has no grad_fn
                meta.insert("grad_fn".to_string(), Value::Text("Leaf".to_string()));
                meta.insert("input_ids".to_string(), Value::List(vec![]));
                meta.insert("saved_tensors".to_string(), Value::List(vec![]));

                let tensor = Value::tensor((*ext.data).clone(), Some(meta));
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // detach: (tensor -- tensor)
    // Remove tensor from computation graph
    dict.register(Tool::native(
        "detach",
        "(tensor:Tensor -- tensor:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let t = stack.pop()?;
                let ext = t.as_ext()?;

                if ext.kind != ext::TENSOR {
                    return Err(Error::TypeError {
                        expected: "Tensor".into(),
                        got: t.type_name().into(),
                    });
                }

                // Remove gradient tracking
                let mut meta = ext.meta.clone().unwrap_or_default();
                meta.shift_remove("requires_grad");
                meta.shift_remove("id");
                meta.shift_remove("grad_fn");
                meta.shift_remove("input_ids");
                meta.shift_remove("saved_tensors");

                let tensor = Value::tensor((*ext.data).clone(), Some(meta));
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // backward: (loss:Tensor -- grads:Map)
    // Compute gradients for all tensors in the computation graph
    // Returns a map from tensor ID to gradient tensor
    dict.register(Tool::native(
        "backward",
        "(loss:Tensor -- grads:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let loss = stack.pop()?;

                let loss_ext = loss.as_ext()?;
                if loss_ext.kind != ext::TENSOR {
                    return Err(Error::TypeError {
                        expected: "Tensor".into(),
                        got: loss.type_name().into(),
                    });
                }

                // Verify loss requires grad
                if !requires_grad_check(&loss_ext.meta) {
                    return Err(Error::Runtime(
                        "backward: loss tensor must have requires_grad=true".into()
                    ));
                }

                // Compute gradients by traversing the computation graph embedded in tensor metadata
                let mut grads: HashMap<u64, Vec<f64>> = HashMap::new();
                
                // Initialize loss gradient to 1.0
                let loss_id = get_tensor_id(&loss_ext.meta);
                let loss_data = flatten_to_f64(&loss_ext.data);
                grads.insert(loss_id, vec![1.0; loss_data.len().max(1)]);
                
                // Process the loss tensor's gradient info
                if let Some(meta) = &loss_ext.meta {
                    process_grad_backward(meta, &grads.get(&loss_id).unwrap().clone(), &mut grads);
                }

                // Build result map
                let mut result_map = IndexMap::new();
                for (id, grad) in &grads {
                    result_map.insert(
                        id.to_string(),
                        Value::tensor(f64_to_value_list(grad), None),
                    );
                }

                stack.push(Value::Map(result_map))?;
                Ok((stack, ctx))
            })
        },
    ));

    // grad-get: (tensor:Tensor grads:Map -- grad:Tensor)
    // Get gradient of a tensor from the gradient map
    dict.register(Tool::native(
        "grad-get",
        "(tensor:Tensor grads:Map -- grad:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let grads = stack.pop()?;
                let tensor = stack.pop()?;

                let tensor_ext = tensor.as_ext()?;
                if tensor_ext.kind != ext::TENSOR {
                    return Err(Error::TypeError {
                        expected: "Tensor".into(),
                        got: tensor.type_name().into(),
                    });
                }

                let grads_map = match grads {
                    Value::Map(m) => m,
                    _ => return Err(Error::TypeError {
                        expected: "Map".into(),
                        got: grads.type_name().into(),
                    }),
                };

                // Get tensor ID
                let tensor_id = get_tensor_id(&tensor_ext.meta);
                
                // Look up gradient in map
                if let Some(grad) = grads_map.get(&tensor_id.to_string()) {
                    stack.push(grad.clone())?;
                    return Ok((stack, ctx));
                }

                // No gradient found - return zeros
                let size = flatten_to_f64(&tensor_ext.data).len();
                let zeros = Value::List(vec![Value::Float(0.0); size]);
                let grad_tensor = Value::tensor(zeros, None);
                stack.push(grad_tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // zero-grad: (tensor -- tensor)
    // Reset gradient tracking (clear saved tensors)
    dict.register(Tool::native(
        "zero-grad",
        "(tensor:Tensor -- tensor:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let t = stack.pop()?;
                let ext = t.as_ext()?;

                if ext.kind != ext::TENSOR {
                    return Err(Error::TypeError {
                        expected: "Tensor".into(),
                        got: t.type_name().into(),
                    });
                }

                // Keep requires_grad and id, but clear saved tensors
                let mut meta = ext.meta.clone().unwrap_or_default();
                meta.insert("saved_tensors".to_string(), Value::List(vec![]));

                let tensor = Value::tensor((*ext.data).clone(), Some(meta));
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::executor::execute;
    use crate::op::Op;

    async fn run_autodiff(ops: Vec<Op>) -> Vec<Value> {
        let mut ctx = Context::new();
        crate::core::register_core(&mut ctx).await;
        {
            let mut dict = ctx.dict.write().await;
            crate::ext::tensor::register(&mut dict);
            register_autodiff(&mut dict);
        }
        let stack = Stack::new();
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        result.values().to_vec()
    }

    #[tokio::test]
    async fn test_requires_grad() {
        let result = run_autodiff(vec![
            Op::Push(Value::List(vec![
                Value::Float(1.0),
                Value::Float(2.0),
                Value::Float(3.0),
            ])),
            Op::call("tensor-from-list"),
            Op::call("requires-grad"),
            Op::call("tensor-shape"),
        ])
        .await;

        assert_eq!(result, vec![Value::List(vec![Value::Int(3)])]);
    }

    #[tokio::test]
    async fn test_detach() {
        let result = run_autodiff(vec![
            Op::Push(Value::List(vec![Value::Float(1.0), Value::Float(2.0)])),
            Op::call("tensor-from-list"),
            Op::call("requires-grad"),
            Op::call("detach"),
            Op::call("tensor-sum"),
        ])
        .await;

        assert_eq!(result, vec![Value::Float(3.0)]);
    }

    #[tokio::test]
    async fn test_backward_basic() {
        // Test that backward produces a gradient map
        let result = run_autodiff(vec![
            Op::Push(Value::List(vec![Value::Float(1.0), Value::Float(2.0)])),
            Op::call("tensor-from-list"),
            Op::call("requires-grad"),
            Op::call("backward"),
            Op::call("type-of"),
        ])
        .await;

        // type-of returns lowercase "map"
        assert_eq!(result, vec![Value::Text("map".to_string())]);
    }

    #[tokio::test]
    async fn test_grad_mul_simple() {
        // Test gradient computation for x * y where x = [1,2,3], y = [4,5,6]
        // z = x * y = [4, 10, 18], loss = sum(z) = 32
        // We test that backward returns a map with gradients
        let result = run_autodiff(vec![
            // Create x with requires_grad
            Op::Push(Value::List(vec![Value::Float(1.0), Value::Float(2.0), Value::Float(3.0)])),
            Op::call("tensor-from-list"),
            Op::call("requires-grad"),
            // Create y with requires_grad  
            Op::Push(Value::List(vec![Value::Float(4.0), Value::Float(5.0), Value::Float(6.0)])),
            Op::call("tensor-from-list"),
            Op::call("requires-grad"),
            // Compute z = x * y
            Op::call("tensor-mul"),
            // Sum to get scalar loss
            Op::call("tensor-sum"),
            // Backward (returns grads map)
            Op::call("backward"),
            Op::call("type-of"),
        ])
        .await;

        // Should return a map
        assert_eq!(result, vec![Value::Text("map".to_string())]);
    }

    #[tokio::test]
    async fn test_backward_fns_add() {
        // Test add backward function directly
        let upstream = vec![1.0, 1.0, 1.0];
        let saved = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let grads = backward_fns::add_backward(&upstream, &saved);
        
        assert_eq!(grads.len(), 2);
        assert_eq!(grads[0], vec![1.0, 1.0, 1.0]); // d/dx(x+y) = 1
        assert_eq!(grads[1], vec![1.0, 1.0, 1.0]); // d/dy(x+y) = 1
    }

    #[tokio::test]
    async fn test_backward_fns_mul() {
        // Test mul backward function directly
        // z = x * y, dL/dz = [1, 1, 1]
        // dL/dx = y * dL/dz = [4, 5, 6]
        // dL/dy = x * dL/dz = [1, 2, 3]
        let upstream = vec![1.0, 1.0, 1.0];
        let saved = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]; // x, y
        let grads = backward_fns::mul_backward(&upstream, &saved);
        
        assert_eq!(grads.len(), 2);
        assert_eq!(grads[0], vec![4.0, 5.0, 6.0]); // d/dx(x*y) = y
        assert_eq!(grads[1], vec![1.0, 2.0, 3.0]); // d/dy(x*y) = x
    }

    #[tokio::test]
    async fn test_backward_fns_sum() {
        // Test sum backward function directly
        // loss = sum(x), dL/d(loss) = 1
        // dL/dx = ones_like(x)
        let upstream = vec![1.0];
        let saved = vec![vec![1.0, 2.0, 3.0]]; // x
        let grads = backward_fns::sum_backward(&upstream, &saved);
        
        assert_eq!(grads.len(), 1);
        assert_eq!(grads[0], vec![1.0, 1.0, 1.0]); // d/dx(sum(x)) = 1
    }

    #[tokio::test]
    async fn test_backward_fns_relu() {
        // Test relu backward function directly
        // y = relu(x), dL/dy = [1, 1, 1]
        // dL/dx = dL/dy * (x > 0 ? 1 : 0)
        let upstream = vec![1.0, 1.0, 1.0];
        let saved = vec![vec![-1.0, 0.0, 2.0]]; // x (negative, zero, positive)
        let grads = backward_fns::relu_backward(&upstream, &saved);
        
        assert_eq!(grads.len(), 1);
        assert_eq!(grads[0], vec![0.0, 0.0, 1.0]); // Only positive passes gradient
    }

    #[tokio::test]
    async fn test_backward_fns_sigmoid() {
        // Test sigmoid backward function directly
        // y = σ(x), dL/dy = [1, 1]
        // dL/dx = σ(x) * (1 - σ(x)) * dL/dy
        let upstream = vec![1.0, 1.0];
        let output = vec![0.5, 0.7]; // sigmoid values
        let saved = vec![output.clone()];
        let grads = backward_fns::sigmoid_backward(&upstream, &saved);
        
        assert_eq!(grads.len(), 1);
        // σ(x) * (1 - σ(x)) = 0.5 * 0.5 = 0.25, 0.7 * 0.3 = 0.21
        assert!((grads[0][0] - 0.25).abs() < 1e-10);
        assert!((grads[0][1] - 0.21).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_backward_fns_log() {
        // Test log backward function directly
        // y = log(x), dL/dy = [1, 1, 1]
        // dL/dx = 1/x
        let upstream = vec![1.0, 1.0, 1.0];
        let saved = vec![vec![1.0, 2.0, 4.0]]; // x values
        let grads = backward_fns::log_backward(&upstream, &saved);
        
        assert_eq!(grads.len(), 1);
        assert_eq!(grads[0], vec![1.0, 0.5, 0.25]); // 1/x
    }

    #[tokio::test]
    async fn test_backward_fns_exp() {
        // Test exp backward function directly
        // y = exp(x), dL/dy = [1, 1]
        // dL/dx = exp(x) * dL/dy
        let upstream = vec![1.0, 1.0];
        let output = vec![2.718281828, 7.389056099]; // exp(1) and exp(2) approximately
        let saved = vec![output.clone()];
        let grads = backward_fns::exp_backward(&upstream, &saved);
        
        assert_eq!(grads.len(), 1);
        // d/dx(exp(x)) = exp(x)
        assert!((grads[0][0] - 2.718281828).abs() < 1e-6);
        assert!((grads[0][1] - 7.389056099).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_backward_fns_matmul() {
        // Test matmul backward function directly
        // y = W @ x where W is (2, 3) and x is (3,)
        // W = [[1,2,3], [4,5,6]] (flattened: [1,2,3,4,5,6])
        // x = [1, 1, 1]
        // dL/dy = [1, 1] (gradient coming from loss)
        
        let upstream = vec![1.0, 1.0]; // dL/dy
        let w_data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // W flattened
        let x_data = vec![1.0, 1.0, 1.0]; // x
        let m = 2.0;
        let n = 3.0;
        let saved = vec![w_data, x_data.clone(), vec![m], vec![n]];
        
        let grads = backward_fns::matmul_backward(&upstream, &saved);
        
        assert_eq!(grads.len(), 2);
        
        // dL/dW = dL/dy ⊗ x (outer product)
        // [[1*1, 1*1, 1*1], [1*1, 1*1, 1*1]] = [[1,1,1], [1,1,1]] = [1,1,1,1,1,1]
        assert_eq!(grads[0], vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
        
        // dL/dx = W^T @ dL/dy
        // W^T = [[1,4], [2,5], [3,6]]
        // W^T @ [1, 1] = [1+4, 2+5, 3+6] = [5, 7, 9]
        assert_eq!(grads[1], vec![5.0, 7.0, 9.0]);
    }

    #[tokio::test]
    async fn test_backward_fns_softmax() {
        // Test softmax backward function directly
        // For softmax, we test a simple case
        let upstream = vec![1.0, 0.0, 0.0]; // gradient only on first element
        let output = vec![0.5, 0.3, 0.2]; // softmax output
        let saved = vec![output.clone()];
        
        let grads = backward_fns::softmax_backward(&upstream, &saved);
        
        assert_eq!(grads.len(), 1);
        // The Jacobian-vector product for softmax is complex
        // Just verify the shape is correct
        assert_eq!(grads[0].len(), 3);
    }
}
