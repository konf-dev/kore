//! Tensor tools - differentiable multi-dimensional arrays
//!
//! Extension tools for tensor operations. All tools follow P1/P2/P3.
//!
//! # Tools (25+)
//!
//! | Tool | Effect | Description |
//! |------|--------|-------------|
//! | tensor-from-list | (list -- tensor) | Create tensor from list |
//! | tensor-shape | (tensor -- list) | Get shape as list |
//! | tensor-rank | (tensor -- n) | Get number of dimensions |
//! | tensor-size | (tensor -- n) | Get total element count |
//! | tensor-add | (t1 t2 -- t3) | Element-wise addition |
//! | tensor-mul | (t1 t2 -- t3) | Element-wise multiplication |
//! | tensor-matmul | (W x m n -- y) | Matrix-vector multiply (m×n) @ (n) → (m) |
//! | tensor-outer | (a b -- M) | Outer product: (m) × (n) → (m×n) |
//! | tensor-dot | (t1 t2 -- t3) | Dot product / matrix multiply |
//! | tensor-sum | (tensor -- n) | Sum all elements |
//! | tensor-mean | (tensor -- n) | Mean of all elements |
//! | tensor-max | (tensor -- n) | Maximum element |
//! | tensor-argmax | (tensor -- n) | Index of maximum element |
//! | tensor-softmax | (tensor -- tensor) | Softmax activation |
//! | tensor-log | (tensor -- tensor) | Element-wise log |
//! | tensor-unwrap | (tensor -- list) | Extract data as flat list |

use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::{ext, Value};
use indexmap::IndexMap;

/// Register all tensor tools
pub fn register(dict: &mut Dictionary) {
    // tensor-from-list: (list -- tensor)
    dict.register(Tool::native(
        "tensor-from-list",
        "(list:List -- tensor:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let list = stack.pop()?;
                let data = match &list {
                    Value::List(l) => l.clone(),
                    _ => return Err(Error::TypeError {
                        expected: "List".into(),
                        got: list.type_name().into(),
                    }),
                };
                
                // Calculate shape (1D for now)
                let mut meta = IndexMap::new();
                meta.insert("shape".to_string(), Value::List(vec![Value::Int(data.len() as i64)]));
                
                let tensor = Value::tensor(list, Some(meta));
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-shape: (tensor -- list)
    dict.register(Tool::native(
        "tensor-shape",
        "(tensor:Tensor -- shape:List)",
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
                
                // Get shape from metadata or infer
                let shape = if let Some(ref meta) = ext.meta {
                    meta.get("shape").cloned().unwrap_or(Value::List(vec![]))
                } else if let Value::List(l) = &*ext.data {
                    Value::List(vec![Value::Int(l.len() as i64)])
                } else {
                    Value::List(vec![Value::Int(1)])
                };
                
                stack.push(shape)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-rank: (tensor -- n)
    dict.register(Tool::native(
        "tensor-rank",
        "(tensor:Tensor -- rank:Int)",
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
                
                // Get rank from shape metadata
                let rank = if let Some(ref meta) = ext.meta {
                    if let Some(Value::List(shape)) = meta.get("shape") {
                        shape.len() as i64
                    } else {
                        1
                    }
                } else {
                    1
                };
                
                stack.push(Value::Int(rank))?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-size: (tensor -- n)
    dict.register(Tool::native(
        "tensor-size",
        "(tensor:Tensor -- size:Int)",
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
                
                let size = count_elements(&ext.data);
                stack.push(Value::Int(size))?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-add: (t1 t2 -- t3)
    dict.register(Tool::native(
        "tensor-add",
        "(t1:Tensor t2:Tensor -- t3:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let t2 = stack.pop()?;
                let t1 = stack.pop()?;
                
                let e1 = t1.as_ext()?;
                let e2 = t2.as_ext()?;
                
                if e1.kind != ext::TENSOR || e2.kind != ext::TENSOR {
                    return Err(Error::Runtime("tensor-add requires two tensors".into()));
                }
                
                let result = elementwise_op(&e1.data, &e2.data, |a, b| a + b)?;
                let tensor = Value::tensor(result, e1.meta.clone());
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-mul: (t1 t2 -- t3)
    dict.register(Tool::native(
        "tensor-mul",
        "(t1:Tensor t2:Tensor -- t3:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let t2 = stack.pop()?;
                let t1 = stack.pop()?;
                
                let e1 = t1.as_ext()?;
                let e2 = t2.as_ext()?;
                
                if e1.kind != ext::TENSOR || e2.kind != ext::TENSOR {
                    return Err(Error::Runtime("tensor-mul requires two tensors".into()));
                }
                
                let result = elementwise_op(&e1.data, &e2.data, |a, b| a * b)?;
                let tensor = Value::tensor(result, e1.meta.clone());
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-sum: (tensor -- n)
    dict.register(Tool::native(
        "tensor-sum",
        "(tensor:Tensor -- sum:Float)",
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
                
                let sum = sum_elements(&ext.data);
                stack.push(Value::Float(sum))?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-mean: (tensor -- n)
    dict.register(Tool::native(
        "tensor-mean",
        "(tensor:Tensor -- mean:Float)",
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
                
                let sum = sum_elements(&ext.data);
                let count = count_elements(&ext.data);
                let mean = if count > 0 { sum / count as f64 } else { 0.0 };
                stack.push(Value::Float(mean))?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-unwrap: (tensor -- list)
    dict.register(Tool::native(
        "tensor-unwrap",
        "(tensor:Tensor -- data:List)",
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
                
                // Return flattened data
                let flat = flatten(&ext.data);
                stack.push(Value::List(flat))?;
                Ok((stack, ctx))
            })
        },
    ));

    // is-tensor: (a -- bool)
    dict.register(Tool::native(
        "is-tensor",
        "(a:Any -- bool:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                stack.push(Value::Bool(v.is_tensor()))?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-scale: (tensor n -- tensor')
    // Multiply all elements by scalar
    dict.register(Tool::native(
        "tensor-scale",
        "(tensor:Tensor n:Float -- tensor:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let n = stack.pop()?.as_float()?;
                let t = stack.pop()?;
                let ext = t.as_ext()?;
                
                if ext.kind != ext::TENSOR {
                    return Err(Error::TypeError {
                        expected: "Tensor".into(),
                        got: t.type_name().into(),
                    });
                }
                
                let scaled = scale_elements(&ext.data, n);
                let tensor = Value::tensor(scaled, ext.meta.clone());
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-sub: (t1 t2 -- t3)
    dict.register(Tool::native(
        "tensor-sub",
        "(t1:Tensor t2:Tensor -- t3:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let t2 = stack.pop()?;
                let t1 = stack.pop()?;
                
                let e1 = t1.as_ext()?;
                let e2 = t2.as_ext()?;
                
                if e1.kind != ext::TENSOR || e2.kind != ext::TENSOR {
                    return Err(Error::Runtime("tensor-sub requires two tensors".into()));
                }
                
                let result = elementwise_op(&e1.data, &e2.data, |a, b| a - b)?;
                let tensor = Value::tensor(result, e1.meta.clone());
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-dot: (t1 t2 -- n)
    // Dot product of two 1D tensors
    dict.register(Tool::native(
        "tensor-dot",
        "(t1:Tensor t2:Tensor -- dot:Float)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let t2 = stack.pop()?;
                let t1 = stack.pop()?;
                
                let e1 = t1.as_ext()?;
                let e2 = t2.as_ext()?;
                
                if e1.kind != ext::TENSOR || e2.kind != ext::TENSOR {
                    return Err(Error::Runtime("tensor-dot requires two tensors".into()));
                }
                
                let v1 = flatten(&e1.data);
                let v2 = flatten(&e2.data);
                
                if v1.len() != v2.len() {
                    return Err(Error::Runtime(format!(
                        "tensor-dot: length mismatch {} vs {}",
                        v1.len(), v2.len()
                    )));
                }
                
                let dot: f64 = v1.iter().zip(v2.iter())
                    .map(|(a, b)| {
                        let a_f = match a {
                            Value::Float(f) => *f,
                            Value::Int(n) => *n as f64,
                            _ => 0.0,
                        };
                        let b_f = match b {
                            Value::Float(f) => *f,
                            Value::Int(n) => *n as f64,
                            _ => 0.0,
                        };
                        a_f * b_f
                    })
                    .sum();
                
                stack.push(Value::Float(dot))?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-neg: (tensor -- tensor')
    dict.register(Tool::native(
        "tensor-neg",
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
                
                let negated = scale_elements(&ext.data, -1.0);
                let tensor = Value::tensor(negated, ext.meta.clone());
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-exp: (tensor -- tensor')
    // Element-wise exp approximation
    dict.register(Tool::native(
        "tensor-exp",
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
                
                let result = map_elements(&ext.data, fast_exp);
                let tensor = Value::tensor(result, ext.meta.clone());
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-sigmoid: (tensor -- tensor')
    dict.register(Tool::native(
        "tensor-sigmoid",
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
                
                let result = map_elements(&ext.data, |x| 1.0 / (1.0 + fast_exp(-x)));
                let tensor = Value::tensor(result, ext.meta.clone());
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-relu: (tensor -- tensor')
    dict.register(Tool::native(
        "tensor-relu",
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
                
                let result = map_elements(&ext.data, |x| if x > 0.0 { x } else { 0.0 });
                let tensor = Value::tensor(result, ext.meta.clone());
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-relu-bwd: (z grad -- grad')
    // ReLU backward: grad' = grad * (z > 0 ? 1 : 0)
    dict.register(Tool::native(
        "tensor-relu-bwd",
        "(z:Tensor grad:Tensor -- grad:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let grad = stack.pop()?;
                let z = stack.pop()?;
                
                let z_ext = z.as_ext()?;
                let grad_ext = grad.as_ext()?;
                
                if z_ext.kind != ext::TENSOR || grad_ext.kind != ext::TENSOR {
                    return Err(Error::Runtime("tensor-relu-bwd requires tensors".into()));
                }
                
                let z_flat = flatten_to_f64(&z_ext.data);
                let grad_flat = flatten_to_f64(&grad_ext.data);
                
                let result: Vec<Value> = z_flat.iter().zip(grad_flat.iter())
                    .map(|(&zi, &gi)| Value::Float(if zi > 0.0 { gi } else { 0.0 }))
                    .collect();
                
                let tensor = Value::tensor(Value::List(result), None);
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-zeros: (n -- tensor)
    dict.register(Tool::native(
        "tensor-zeros",
        "(n:Int -- tensor:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let n = stack.pop()?.as_int()? as usize;
                let data: Vec<Value> = vec![Value::Float(0.0); n];
                let tensor = Value::tensor(Value::List(data), None);
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-ones: (n -- tensor)
    dict.register(Tool::native(
        "tensor-ones",
        "(n:Int -- tensor:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let n = stack.pop()?.as_int()? as usize;
                let data: Vec<Value> = vec![Value::Float(1.0); n];
                let tensor = Value::tensor(Value::List(data), None);
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-rand: (n -- tensor)
    // Pseudo-random values between 0 and 1
    dict.register(Tool::native(
        "tensor-rand",
        "(n:Int -- tensor:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let n = stack.pop()?.as_int()? as usize;
                // Simple LCG for reproducible pseudo-random
                let mut seed: u64 = 12345;
                let data: Vec<Value> = (0..n)
                    .map(|_| {
                        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                        let val = ((seed >> 16) & 0x7FFF) as f64 / 32768.0;
                        Value::Float(val * 0.2 - 0.1) // Small values for initialization
                    })
                    .collect();
                let tensor = Value::tensor(Value::List(data), None);
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-randn: (n seed -- tensor)
    // Seeded random values (for reproducibility)
    dict.register(Tool::native(
        "tensor-randn",
        "(n:Int seed:Int -- tensor:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let mut seed = stack.pop()?.as_int()? as u64;
                let n = stack.pop()?.as_int()? as usize;
                let data: Vec<Value> = (0..n)
                    .map(|_| {
                        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                        let val = ((seed >> 16) & 0x7FFF) as f64 / 32768.0;
                        Value::Float(val * 0.2 - 0.1)
                    })
                    .collect();
                let tensor = Value::tensor(Value::List(data), None);
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-matmul: (W x m n -- y)
    // Matrix-vector multiply: W is (m*n) flat, x is (n), result is (m)
    // Essential for neural network forward pass
    dict.register(Tool::native(
        "tensor-matmul",
        "(W:Tensor x:Tensor m:Int n:Int -- y:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let n = stack.pop()?.as_int()? as usize;
                let m = stack.pop()?.as_int()? as usize;
                let x = stack.pop()?;
                let w = stack.pop()?;
                
                let w_ext = w.as_ext()?;
                let x_ext = x.as_ext()?;
                
                if w_ext.kind != ext::TENSOR || x_ext.kind != ext::TENSOR {
                    return Err(Error::Runtime("tensor-matmul requires tensors".into()));
                }
                
                let w_flat = flatten_to_f64(&w_ext.data);
                let x_flat = flatten_to_f64(&x_ext.data);
                
                if w_flat.len() != m * n {
                    return Err(Error::Runtime(format!(
                        "tensor-matmul: W size {} != m*n {}*{}={}",
                        w_flat.len(), m, n, m * n
                    )));
                }
                if x_flat.len() != n {
                    return Err(Error::Runtime(format!(
                        "tensor-matmul: x size {} != n {}",
                        x_flat.len(), n
                    )));
                }
                
                // y[i] = sum_j(W[i*n + j] * x[j])
                let result: Vec<Value> = (0..m)
                    .map(|i| {
                        let dot: f64 = (0..n)
                            .map(|j| w_flat[i * n + j] * x_flat[j])
                            .sum();
                        Value::Float(dot)
                    })
                    .collect();
                
                let tensor = Value::tensor(Value::List(result), None);
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-matmul-t: (W x m n -- y)
    // Matrix-transpose-vector multiply: W^T @ x
    // For backprop: compute gradient w.r.t. input
    dict.register(Tool::native(
        "tensor-matmul-t",
        "(W:Tensor x:Tensor m:Int n:Int -- y:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let n = stack.pop()?.as_int()? as usize;
                let m = stack.pop()?.as_int()? as usize;
                let x = stack.pop()?;
                let w = stack.pop()?;
                
                let w_ext = w.as_ext()?;
                let x_ext = x.as_ext()?;
                
                if w_ext.kind != ext::TENSOR || x_ext.kind != ext::TENSOR {
                    return Err(Error::Runtime("tensor-matmul-t requires tensors".into()));
                }
                
                let w_flat = flatten_to_f64(&w_ext.data);
                let x_flat = flatten_to_f64(&x_ext.data);
                
                // W^T @ x: W is (m,n), W^T is (n,m), x is (m), result is (n)
                // y[j] = sum_i(W[i*n + j] * x[i])
                let result: Vec<Value> = (0..n)
                    .map(|j| {
                        let dot: f64 = (0..m)
                            .map(|i| w_flat[i * n + j] * x_flat[i])
                            .sum();
                        Value::Float(dot)
                    })
                    .collect();
                
                let tensor = Value::tensor(Value::List(result), None);
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-outer: (a b -- M)
    // Outer product: a (m) × b (n) → M (m*n flat)
    // For backprop: compute gradient w.r.t. weights
    dict.register(Tool::native(
        "tensor-outer",
        "(a:Tensor b:Tensor -- M:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let b = stack.pop()?;
                let a = stack.pop()?;
                
                let a_ext = a.as_ext()?;
                let b_ext = b.as_ext()?;
                
                if a_ext.kind != ext::TENSOR || b_ext.kind != ext::TENSOR {
                    return Err(Error::Runtime("tensor-outer requires tensors".into()));
                }
                
                let a_flat = flatten_to_f64(&a_ext.data);
                let b_flat = flatten_to_f64(&b_ext.data);
                
                // M[i,j] = a[i] * b[j], stored as flat array
                let result: Vec<Value> = a_flat.iter()
                    .flat_map(|&ai| {
                        b_flat.iter().map(move |&bj| Value::Float(ai * bj))
                    })
                    .collect();
                
                let mut meta = IndexMap::new();
                meta.insert("shape".to_string(), Value::List(vec![
                    Value::Int(a_flat.len() as i64),
                    Value::Int(b_flat.len() as i64),
                ]));
                
                let tensor = Value::tensor(Value::List(result), Some(meta));
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-max: (tensor -- n)
    dict.register(Tool::native(
        "tensor-max",
        "(tensor:Tensor -- max:Float)",
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
                
                let flat = flatten_to_f64(&ext.data);
                let max = flat.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                stack.push(Value::Float(max))?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-argmax: (tensor -- idx)
    dict.register(Tool::native(
        "tensor-argmax",
        "(tensor:Tensor -- idx:Int)",
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
                
                let flat = flatten_to_f64(&ext.data);
                let (idx, _) = flat.iter().enumerate()
                    .fold((0, f64::NEG_INFINITY), |(max_i, max_v), (i, &v)| {
                        if v > max_v { (i, v) } else { (max_i, max_v) }
                    });
                stack.push(Value::Int(idx as i64))?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-softmax: (tensor -- tensor)
    // Numerically stable softmax
    dict.register(Tool::native(
        "tensor-softmax",
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
                
                let flat = flatten_to_f64(&ext.data);
                
                // Subtract max for numerical stability
                let max = flat.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let exp_vals: Vec<f64> = flat.iter().map(|&x| (x - max).exp()).collect();
                let sum: f64 = exp_vals.iter().sum();
                
                let result: Vec<Value> = exp_vals.iter()
                    .map(|&x| Value::Float(x / sum))
                    .collect();
                
                let tensor = Value::tensor(Value::List(result), ext.meta.clone());
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-log: (tensor -- tensor)
    // Element-wise natural log (clamped to avoid -inf)
    dict.register(Tool::native(
        "tensor-log",
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
                
                let result = map_elements(&ext.data, |x| (x.max(1e-10)).ln());
                let tensor = Value::tensor(result, ext.meta.clone());
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-get: (tensor idx -- value)
    dict.register(Tool::native(
        "tensor-get",
        "(tensor:Tensor idx:Int -- value:Float)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let idx = stack.pop()?.as_int()? as usize;
                let t = stack.pop()?;
                let ext = t.as_ext()?;
                
                if ext.kind != ext::TENSOR {
                    return Err(Error::TypeError {
                        expected: "Tensor".into(),
                        got: t.type_name().into(),
                    });
                }
                
                let flat = flatten_to_f64(&ext.data);
                if idx >= flat.len() {
                    return Err(Error::Runtime(format!(
                        "tensor-get: index {} out of bounds (size {})",
                        idx, flat.len()
                    )));
                }
                
                stack.push(Value::Float(flat[idx]))?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-set: (tensor idx value -- tensor')
    dict.register(Tool::native(
        "tensor-set",
        "(tensor:Tensor idx:Int value:Float -- tensor:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?.as_float()?;
                let idx = stack.pop()?.as_int()? as usize;
                let t = stack.pop()?;
                let ext = t.as_ext()?;
                
                if ext.kind != ext::TENSOR {
                    return Err(Error::TypeError {
                        expected: "Tensor".into(),
                        got: t.type_name().into(),
                    });
                }
                
                let mut flat = flatten_to_f64(&ext.data);
                if idx >= flat.len() {
                    return Err(Error::Runtime(format!(
                        "tensor-set: index {} out of bounds (size {})",
                        idx, flat.len()
                    )));
                }
                
                flat[idx] = value;
                let result: Vec<Value> = flat.into_iter().map(Value::Float).collect();
                let tensor = Value::tensor(Value::List(result), ext.meta.clone());
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-clip: (tensor min max -- tensor')
    dict.register(Tool::native(
        "tensor-clip",
        "(tensor:Tensor min:Float max:Float -- tensor:Tensor)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let max_val = stack.pop()?.as_float()?;
                let min_val = stack.pop()?.as_float()?;
                let t = stack.pop()?;
                let ext = t.as_ext()?;
                
                if ext.kind != ext::TENSOR {
                    return Err(Error::TypeError {
                        expected: "Tensor".into(),
                        got: t.type_name().into(),
                    });
                }
                
                let result = map_elements(&ext.data, |x| x.clamp(min_val, max_val));
                let tensor = Value::tensor(result, ext.meta.clone());
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));

    // tensor-copy: (tensor -- tensor')
    // Create a copy (useful for avoiding aliasing in training)
    dict.register(Tool::native(
        "tensor-copy",
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
                
                // Clone the tensor data
                let flat = flatten_to_f64(&ext.data);
                let result: Vec<Value> = flat.into_iter().map(Value::Float).collect();
                let tensor = Value::tensor(Value::List(result), ext.meta.clone());
                stack.push(tensor)?;
                Ok((stack, ctx))
            })
        },
    ));
}

// === Helper functions ===

fn count_elements(v: &Value) -> i64 {
    match v {
        Value::List(l) => l.iter().map(count_elements).sum(),
        _ => 1,
    }
}

fn sum_elements(v: &Value) -> f64 {
    match v {
        Value::List(l) => l.iter().map(sum_elements).sum(),
        Value::Int(n) => *n as f64,
        Value::Float(f) => *f,
        _ => 0.0,
    }
}

fn flatten(v: &Value) -> Vec<Value> {
    match v {
        Value::List(l) => l.iter().flat_map(flatten).collect(),
        other => vec![other.clone()],
    }
}

fn flatten_to_f64(v: &Value) -> Vec<f64> {
    match v {
        Value::List(l) => l.iter().flat_map(flatten_to_f64).collect(),
        Value::Float(f) => vec![*f],
        Value::Int(n) => vec![*n as f64],
        _ => vec![0.0],
    }
}

fn scale_elements(v: &Value, s: f64) -> Value {
    match v {
        Value::List(l) => Value::List(l.iter().map(|x| scale_elements(x, s)).collect()),
        Value::Float(f) => Value::Float(f * s),
        Value::Int(n) => Value::Float(*n as f64 * s),
        other => other.clone(),
    }
}

fn map_elements<F>(v: &Value, f: F) -> Value 
where F: Fn(f64) -> f64 + Copy
{
    match v {
        Value::List(l) => Value::List(l.iter().map(|x| map_elements(x, f)).collect()),
        Value::Float(x) => Value::Float(f(*x)),
        Value::Int(n) => Value::Float(f(*n as f64)),
        other => other.clone(),
    }
}

/// Fast exp approximation: (1 + x/256)^256
fn fast_exp(x: f64) -> f64 {
    let x = x.clamp(-20.0, 20.0); // Prevent overflow
    let base = 1.0 + x / 256.0;
    let mut result = base;
    for _ in 0..8 {
        result *= result; // 2^8 = 256 squarings
    }
    result
}

fn elementwise_op<F>(a: &Value, b: &Value, op: F) -> Result<Value, Error>
where
    F: Fn(f64, f64) -> f64 + Clone,
{
    match (a, b) {
        (Value::List(la), Value::List(lb)) => {
            if la.len() != lb.len() {
                return Err(Error::Runtime(format!(
                    "Shape mismatch: {} vs {}",
                    la.len(),
                    lb.len()
                )));
            }
            let result: Result<Vec<Value>, Error> = la
                .iter()
                .zip(lb.iter())
                .map(|(x, y)| elementwise_op(x, y, op.clone()))
                .collect();
            Ok(Value::List(result?))
        }
        (Value::Int(n1), Value::Int(n2)) => Ok(Value::Float(op(*n1 as f64, *n2 as f64))),
        (Value::Float(f1), Value::Float(f2)) => Ok(Value::Float(op(*f1, *f2))),
        (Value::Int(n), Value::Float(f)) | (Value::Float(f), Value::Int(n)) => {
            Ok(Value::Float(op(*n as f64, *f)))
        }
        _ => Err(Error::Runtime("Cannot perform element-wise operation on non-numeric values".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::executor::execute;
    use crate::op::Op;

    async fn run_with_tensor(ops: Vec<Op>) -> Vec<Value> {
        let mut ctx = Context::new();
        // Register core tools
        crate::core::register_core(&mut ctx).await;
        // Register tensor tools
        {
            let mut dict = ctx.dict.write().await;
            register(&mut dict);
        }
        let stack = Stack::new();
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        result.values().to_vec()
    }

    #[tokio::test]
    async fn test_tensor_from_list() {
        let result = run_with_tensor(vec![
            Op::Push(Value::List(vec![
                Value::Float(1.0),
                Value::Float(2.0),
                Value::Float(3.0),
            ])),
            Op::call("tensor-from-list"),
            Op::call("is-tensor"),
        ])
        .await;

        assert_eq!(result, vec![Value::Bool(true)]);
    }

    #[tokio::test]
    async fn test_tensor_sum() {
        let result = run_with_tensor(vec![
            Op::Push(Value::List(vec![
                Value::Float(1.0),
                Value::Float(2.0),
                Value::Float(3.0),
            ])),
            Op::call("tensor-from-list"),
            Op::call("tensor-sum"),
        ])
        .await;

        assert_eq!(result, vec![Value::Float(6.0)]);
    }

    #[tokio::test]
    async fn test_tensor_add() {
        let result = run_with_tensor(vec![
            Op::Push(Value::List(vec![Value::Float(1.0), Value::Float(2.0)])),
            Op::call("tensor-from-list"),
            Op::Push(Value::List(vec![Value::Float(3.0), Value::Float(4.0)])),
            Op::call("tensor-from-list"),
            Op::call("tensor-add"),
            Op::call("tensor-sum"),
        ])
        .await;

        // [1+3, 2+4] = [4, 6], sum = 10
        assert_eq!(result, vec![Value::Float(10.0)]);
    }

    #[tokio::test]
    async fn test_tensor_shape() {
        let result = run_with_tensor(vec![
            Op::Push(Value::List(vec![
                Value::Float(1.0),
                Value::Float(2.0),
                Value::Float(3.0),
            ])),
            Op::call("tensor-from-list"),
            Op::call("tensor-shape"),
        ])
        .await;

        assert_eq!(result, vec![Value::List(vec![Value::Int(3)])]);
    }

    #[tokio::test]
    async fn test_tensor_mean() {
        let result = run_with_tensor(vec![
            Op::Push(Value::List(vec![
                Value::Float(2.0),
                Value::Float(4.0),
                Value::Float(6.0),
            ])),
            Op::call("tensor-from-list"),
            Op::call("tensor-mean"),
        ])
        .await;

        assert_eq!(result, vec![Value::Float(4.0)]);
    }
}
