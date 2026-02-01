//! # Quote Introspection
//!
//! Inspect and construct quotes.
//!
//! ## Purpose
//!
//! Quotes are code as data. Agents need to:
//! - Inspect what operations are in a quote
//! - Construct new quotes programmatically
//! - Transform existing quotes
//!
//! ## Tools
//!
//! | Tool | Effect | Description |
//! |------|--------|-------------|
//! | `quote-ops` | `(quote -- list)` | Get operations from a quote as a list |
//! | `ops-quote` | `(list -- quote)` | Construct a quote from a list of ops |
//! | `quote-len` | `(quote -- int)` | Get the number of operations in a quote |

use kore::{Context, Op, Stack, Tool, Value};
use indexmap::IndexMap;

/// Convert an Op to a Value representation.
///
/// Each Op becomes a Map with:
/// - "op": The operation type ("push", "call", "quote", "if")
/// - Additional fields depending on the op type
fn op_to_value(op: &Op) -> Value {
    match op {
        Op::Push(value) => {
            let mut map = IndexMap::new();
            map.insert("op".to_string(), Value::Text("push".to_string()));
            map.insert("value".to_string(), value.clone());
            Value::Map(map)
        }
        Op::Call(name) => {
            let mut map = IndexMap::new();
            map.insert("op".to_string(), Value::Text("call".to_string()));
            map.insert("name".to_string(), Value::Text(name.clone()));
            Value::Map(map)
        }
        Op::Quote(ops) => {
            let mut map = IndexMap::new();
            map.insert("op".to_string(), Value::Text("quote".to_string()));
            let inner_ops: Vec<Value> = ops.iter().map(op_to_value).collect();
            map.insert("ops".to_string(), Value::List(inner_ops));
            Value::Map(map)
        }
        Op::If { then_ops, else_ops } => {
            let mut map = IndexMap::new();
            map.insert("op".to_string(), Value::Text("if".to_string()));
            let then_vals: Vec<Value> = then_ops.iter().map(op_to_value).collect();
            let else_vals: Vec<Value> = else_ops.iter().map(op_to_value).collect();
            map.insert("then".to_string(), Value::List(then_vals));
            map.insert("else".to_string(), Value::List(else_vals));
            Value::Map(map)
        }
    }
}

/// Convert a Value representation back to an Op.
fn value_to_op(value: &Value) -> Result<Op, String> {
    let map = value.as_map().map_err(|_| "Op must be a map")?;

    let op_type = map
        .get("op")
        .ok_or("Op missing 'op' field")?
        .as_text()
        .map_err(|_| "'op' field must be text")?;

    match op_type.as_ref() {
        "push" => {
            let val = map.get("value").ok_or("Push op missing 'value' field")?;
            Ok(Op::Push(val.clone()))
        }
        "call" => {
            let name = map
                .get("name")
                .ok_or("Call op missing 'name' field")?
                .as_text()
                .map_err(|_| "'name' field must be text")?;
            Ok(Op::call(name))
        }
        "quote" => {
            let ops_val = map.get("ops").ok_or("Quote op missing 'ops' field")?;
            let ops_list = ops_val.as_list().map_err(|_| "'ops' field must be list")?;
            let ops: Result<Vec<Op>, String> = ops_list.iter().map(value_to_op).collect();
            Ok(Op::quote(ops?))
        }
        "if" => {
            let then_val = map.get("then").ok_or("If op missing 'then' field")?;
            let else_val = map.get("else").ok_or("If op missing 'else' field")?;
            let then_list = then_val.as_list().map_err(|_| "'then' field must be list")?;
            let else_list = else_val.as_list().map_err(|_| "'else' field must be list")?;
            let then_ops: Result<Vec<Op>, String> = then_list.iter().map(value_to_op).collect();
            let else_ops: Result<Vec<Op>, String> = else_list.iter().map(value_to_op).collect();
            Ok(Op::if_then_else(then_ops?, else_ops?))
        }
        _ => Err(format!("Unknown op type: {}", op_type)),
    }
}

/// Register quote introspection tools into a context.
///
/// # Tools Registered
///
/// - `quote-ops`: Get operations from a quote as a list
/// - `ops-quote`: Construct a quote from a list of ops
/// - `quote-len`: Get the number of operations in a quote
pub async fn register_quote_tools(ctx: &mut Context) {
    // quote-ops: (quote -- list)
    ctx.dict.write().await.register(
        Tool::native("quote-ops", "(quote -- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                let ops: Vec<Value> = quote.iter().map(op_to_value).collect();
                stack.push(Value::List(ops))?;
                Ok((stack, ctx))
            })
        }),
    );

    // ops-quote: (list -- quote)
    ctx.dict.write().await.register(
        Tool::native("ops-quote", "(list -- quote)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let list = stack.pop()?.into_list()?;
                let ops: Result<Vec<Op>, String> = list.iter().map(value_to_op).collect();
                let ops = ops.map_err(kore::Error::Runtime)?;
                stack.push(Value::Quote(ops))?;
                Ok((stack, ctx))
            })
        }),
    );

    // quote-len: (quote -- int)
    ctx.dict.write().await.register(
        Tool::native("quote-len", "(quote -- int)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                stack.push(Value::Int(quote.len() as i64))?;
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_op_to_value_push() {
        let op = Op::push(42);
        let value = op_to_value(&op);

        let map = value.as_map().unwrap();
        assert_eq!(map.get("op").unwrap(), &Value::Text("push".into()));
        assert_eq!(map.get("value").unwrap(), &Value::Int(42));
    }

    #[test]
    fn test_op_to_value_call() {
        let op = Op::call("dup");
        let value = op_to_value(&op);

        let map = value.as_map().unwrap();
        assert_eq!(map.get("op").unwrap(), &Value::Text("call".into()));
        assert_eq!(map.get("name").unwrap(), &Value::Text("dup".into()));
    }

    #[test]
    fn test_roundtrip() {
        let ops = vec![
            Op::push(42),
            Op::call("dup"),
            Op::quote(vec![Op::push(1)]),
            Op::if_then_else(vec![Op::push(2)], vec![Op::push(3)]),
        ];

        for op in ops {
            let value = op_to_value(&op);
            let back = value_to_op(&value).unwrap();
            assert_eq!(op, back);
        }
    }
}
