//! Tensor tools - differentiable multi-dimensional arrays
//!
//! Extension tools for tensor operations. All tools follow P1/P2/P3.
//!
//! # Tools (10)
//!
//! | Tool | Effect | Description |
//! |------|--------|-------------|
//! | tensor-from-list | (list -- tensor) | Create tensor from list |
//! | tensor-shape | (tensor -- list) | Get shape as list |
//! | tensor-rank | (tensor -- n) | Get number of dimensions |
//! | tensor-size | (tensor -- n) | Get total element count |
//! | tensor-add | (t1 t2 -- t3) | Element-wise addition |
//! | tensor-mul | (t1 t2 -- t3) | Element-wise multiplication |
//! | tensor-dot | (t1 t2 -- t3) | Dot product / matrix multiply |
//! | tensor-sum | (tensor -- n) | Sum all elements |
//! | tensor-mean | (tensor -- n) | Mean of all elements |
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
