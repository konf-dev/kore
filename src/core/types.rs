//! Type primitives (16)
//!
//! Type inspection and conversion operations.
//! These require runtime type information.
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | type-of | (a -- type) | Get type name as text |
//! | to-int | (a -- n) | Convert to integer |
//! | to-float | (a -- f) | Convert to float |
//! | to-text | (a -- s) | Convert to text |
//! | to-bool | (a -- b) | Convert to boolean |
//! | is-null | (a -- bool) | Check if null |
//! | is-bool | (a -- bool) | Check if boolean |
//! | is-int | (a -- bool) | Check if integer |
//! | is-float | (a -- bool) | Check if float |
//! | is-text | (a -- bool) | Check if text |
//! | is-list | (a -- bool) | Check if list |
//! | is-map | (a -- bool) | Check if map |
//! | is-quote | (a -- bool) | Check if quote |
//! | unwrap | (a -- a) | Extract value or fail if error |
//! | quote-to-text | (q -- s) | Serialize quote to Kore source |
//! | text-to-quote | (s -- q) | Parse Kore source to quote |

use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::op::Op;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register all type primitives
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "type-of",
        "(a -- type)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                let type_name = match v {
                    Value::Null => "null",
                    Value::Bool(_) => "bool",
                    Value::Int(_) => "int",
                    Value::Float(_) => "float",
                    Value::Text(_) => "text",
                    Value::List(_) => "list",
                    Value::Map(_) => "map",
                    Value::Quote(_) => "quote",
                    Value::Handle(_) => "handle",
                    Value::Error(_) => "error",
                    Value::Ext(_) => "ext",
                };
                stack.push(Value::Text(type_name.to_string()))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "to-int",
        "(a -- n)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                let n = match v {
                    Value::Int(n) => n,
                    Value::Float(f) => f as i64,
                    Value::Bool(b) => if b { 1 } else { 0 },
                    Value::Text(s) => s.parse::<i64>().map_err(|_| {
                        Error::Runtime(format!("to-int: cannot parse '{}' as integer", s))
                    })?,
                    _ => {
                        return Err(Error::Runtime(format!(
                            "to-int: cannot convert {:?} to integer",
                            v
                        )))
                    }
                };
                stack.push(Value::Int(n))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "to-float",
        "(a -- f)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                let f = match v {
                    Value::Float(f) => f,
                    Value::Int(n) => n as f64,
                    Value::Bool(b) => if b { 1.0 } else { 0.0 },
                    Value::Text(s) => s.parse::<f64>().map_err(|_| {
                        Error::Runtime(format!("to-float: cannot parse '{}' as float", s))
                    })?,
                    _ => {
                        return Err(Error::Runtime(format!(
                            "to-float: cannot convert {:?} to float",
                            v
                        )))
                    }
                };
                stack.push(Value::Float(f))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "to-text",
        "(a -- s)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                let s = match v {
                    Value::Text(s) => s,
                    Value::Int(n) => n.to_string(),
                    Value::Float(f) => f.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => "null".to_string(),
                    Value::List(l) => format!("{:?}", l),
                    Value::Map(m) => format!("{:?}", m),
                    Value::Quote(q) => format!(
                        "[{}]",
                        q.iter()
                            .map(|op| format!("{:?}", op))
                            .collect::<Vec<_>>()
                            .join(" ")
                    ),
                    Value::Handle(h) => format!("@{:?}", h),
                    Value::Error(e) => format!("Error({})", e.message),
                    Value::Ext(e) => format!("<ext:{}:{}>", e.kind, e.data),
                };
                stack.push(Value::Text(s))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "to-bool",
        "(a -- b)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                let b = match v {
                    Value::Bool(b) => b,
                    Value::Int(n) => n != 0,
                    Value::Float(f) => f != 0.0,
                    Value::Text(s) => !s.is_empty(),
                    Value::List(l) => !l.is_empty(),
                    Value::Map(m) => !m.is_empty(),
                    Value::Null => false,
                    Value::Quote(q) => !q.is_empty(),
                    Value::Handle(_) => true,
                    Value::Error(_) => false,
                    Value::Ext(_) => true, // Extensions are truthy
                };
                stack.push(Value::Bool(b))?;
                Ok((stack, ctx))
            })
        },
    ));

    // Type checking tools
    dict.register(Tool::native(
        "is-null",
        "(a -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                stack.push(Value::Bool(matches!(v, Value::Null)))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "is-bool",
        "(a -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                stack.push(Value::Bool(matches!(v, Value::Bool(_))))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "is-int",
        "(a -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                stack.push(Value::Bool(matches!(v, Value::Int(_))))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "is-float",
        "(a -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                stack.push(Value::Bool(matches!(v, Value::Float(_))))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "is-text",
        "(a -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                stack.push(Value::Bool(matches!(v, Value::Text(_))))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "is-list",
        "(a -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                stack.push(Value::Bool(matches!(v, Value::List(_))))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "is-map",
        "(a -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                stack.push(Value::Bool(matches!(v, Value::Map(_))))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "is-quote",
        "(a -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                stack.push(Value::Bool(matches!(v, Value::Quote(_))))?;
                Ok((stack, ctx))
            })
        },
    ));

    // unwrap: extract value, or fail if error
    dict.register(Tool::native(
        "unwrap",
        "(a -- a)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                match v {
                    Value::Error(e) => Err(Error::Runtime(e.message)),
                    other => {
                        stack.push(other)?;
                        Ok((stack, ctx))
                    }
                }
            })
        },
    ));

    // quote-to-text: serialize quote to Kore source
    dict.register(Tool::native(
        "quote-to-text",
        "(q -- s)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                match v {
                    Value::Quote(ops) => {
                        let source = ops_to_source(&ops);
                        stack.push(Value::Text(format!("[ {} ]", source)))?;
                        Ok((stack, ctx))
                    }
                    _ => Err(Error::TypeError {
                        expected: "quote".into(),
                        got: v.type_name().into(),
                    }),
                }
            })
        },
    ));

    // text-to-quote: parse Kore source to quote
    dict.register(Tool::native(
        "text-to-quote",
        "(s -- q)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let text = stack.pop()?.into_text()?;
                let ops = Op::parse(&text).map_err(|e| {
                    Error::Runtime(format!("text-to-quote: {}", e))
                })?;
                // If parsed as a single quote, unwrap it
                if ops.len() == 1 {
                    if let Op::Push(Value::Quote(inner)) = &ops[0] {
                        stack.push(Value::Quote(inner.clone()))?;
                        return Ok((stack, ctx));
                    }
                }
                // Otherwise wrap the whole thing as a quote
                stack.push(Value::Quote(ops))?;
                Ok((stack, ctx))
            })
        },
    ));
}

/// Convert ops to Kore source code
fn ops_to_source(ops: &[Op]) -> String {
    ops.iter()
        .map(|op| op_to_source(op))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Convert a single op to Kore source
fn op_to_source(op: &Op) -> String {
    match op {
        Op::Call(name) => name.clone(),
        Op::Push(value) => value_to_source(value),
    }
}

/// Convert a value to Kore source
fn value_to_source(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => if *b { "true" } else { "false" }.into(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => {
            let s = f.to_string();
            if s.contains('.') { s } else { format!("{}.0", s) }
        }
        Value::Text(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        Value::List(items) => {
            let inner: Vec<_> = items.iter().map(value_to_source).collect();
            format!("[{}]", inner.join(" "))
        }
        Value::Map(m) => {
            let pairs: Vec<_> = m.iter()
                .map(|(k, v)| format!("{}: {}", k, value_to_source(v)))
                .collect();
            format!("{{{}}}", pairs.join(" "))
        }
        Value::Quote(ops) => format!("[ {} ]", ops_to_source(ops)),
        Value::Handle(h) => format!("<handle:{}>", h.id),
        Value::Error(e) => format!("<error:{}>", e.message),
        Value::Ext(e) => format!("<ext:{}:{}>", e.kind, e.data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::execute;
    use crate::op::Op;

    async fn setup() -> Context {
        let mut ctx = Context::new();
        crate::core::register_core(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn test_type_of() {
        let ctx = setup().await;
        let ops = vec![Op::push(42), Op::call("type-of")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Text("int".to_string()));
    }

    #[tokio::test]
    async fn test_to_int_from_float() {
        let ctx = setup().await;
        let ops = vec![Op::Push(Value::Float(3.7)), Op::call("to-int")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Int(3));
    }

    #[tokio::test]
    async fn test_to_int_from_text() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("42".to_string())),
            Op::call("to-int"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Int(42));
    }

    #[tokio::test]
    async fn test_to_text_from_int() {
        let ctx = setup().await;
        let ops = vec![Op::push(123), Op::call("to-text")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Text("123".to_string()));
    }

    #[tokio::test]
    async fn test_to_bool_truthy() {
        let ctx = setup().await;
        let ops = vec![Op::push(1), Op::call("to-bool")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Bool(true));
    }

    #[tokio::test]
    async fn test_to_bool_falsy() {
        let ctx = setup().await;
        let ops = vec![Op::push(0), Op::call("to-bool")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Bool(false));
    }
}
