//! Map primitives (7)
//!
//! Operations on Map values that require internal access.
//! These cannot be composed from other primitives.
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | map-get | (map key -- val) | Get value by key (clones - rejects linear) |
//! | map-take | (map key -- map' val) | Take value by key (moves - linear-safe) |
//! | map-set | (map key val -- map') | Set key-value pair |
//! | map-del | (map key -- map') | Delete key |
//! | map-has | (map key -- bool) | Check if key exists |
//! | map-keys | (map -- list) | Get all keys |
//! | map-vals | (map -- list) | Get all values (clones - rejects linear) |

use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register all map primitives
pub fn register(dict: &mut Dictionary) {
    // map-get clones - CANNOT be used if value is linear/affine
    // Use map-take instead for linear values
    dict.register(Tool::native(
        "map-get",
        "(map key -- val)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                let map = stack.pop()?.into_map()?;
                match map.get(&key) {
                    Some(val) => {
                        // Check if value is non-duplicable (would be cloned here)
                        if val.is_non_duplicable() {
                            return Err(Error::LinearDuplicate(format!(
                                "map-get would clone non-duplicable value at key '{}'. Use map-take instead.",
                                key
                            )));
                        }
                        stack.push(val.clone())?;
                    }
                    None => {
                        return Err(Error::Runtime(format!("map-get: key '{}' not found", key)))
                    }
                }
                Ok((stack, ctx))
            })
        },
    ));

    // map-take: MOVES value out of map (safe for linear/affine values)
    dict.register(Tool::native(
        "map-take",
        "(map key -- map' val)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                let mut map = stack.pop()?.into_map()?;
                match map.shift_remove(&key) {
                    Some(val) => {
                        stack.push(Value::Map(map))?;
                        stack.push(val)?;
                    }
                    None => {
                        return Err(Error::Runtime(format!("map-take: key '{}' not found", key)))
                    }
                }
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "map-set",
        "(map key val -- map')",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let val = stack.pop()?;
                let key = stack.pop()?.into_text()?;
                let mut map = stack.pop()?.into_map()?;
                map.insert(key, val);
                stack.push(Value::Map(map))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "map-del",
        "(map key -- map')",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                let mut map = stack.pop()?.into_map()?;
                map.shift_remove(&key);
                stack.push(Value::Map(map))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "map-has",
        "(map key -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let key = stack.pop()?.into_text()?;
                let map = stack.pop()?.into_map()?;
                stack.push(Value::Bool(map.contains_key(&key)))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "map-keys",
        "(map -- list)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let map = stack.pop()?.into_map()?;
                let keys: Vec<Value> = map.keys().map(|k| Value::Text(k.clone())).collect();
                stack.push(Value::List(keys))?;
                Ok((stack, ctx))
            })
        },
    ));

    // map-vals clones ALL values - CANNOT be used if any value is linear/affine
    dict.register(Tool::native(
        "map-vals",
        "(map -- list)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let map = stack.pop()?.into_map()?;
                // Check if any value is non-duplicable
                for (key, val) in map.iter() {
                    if val.is_non_duplicable() {
                        return Err(Error::LinearDuplicate(format!(
                            "map-vals would clone non-duplicable value at key '{}'",
                            key
                        )));
                    }
                }
                let vals: Vec<Value> = map.values().cloned().collect();
                stack.push(Value::List(vals))?;
                Ok((stack, ctx))
            })
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::execute;
    use crate::op::Op;
    use indexmap::IndexMap;

    async fn setup() -> Context {
        let mut ctx = Context::new();
        crate::core::register_core(&mut ctx).await;
        ctx
    }

    fn make_map() -> Value {
        let mut map = IndexMap::new();
        map.insert("a".to_string(), Value::Int(1));
        map.insert("b".to_string(), Value::Int(2));
        Value::Map(map)
    }

    #[tokio::test]
    async fn test_map_get() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(make_map()),
            Op::Push(Value::Text("a".into())),
            Op::call("map-get"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Int(1));
    }

    #[tokio::test]
    async fn test_map_set() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(make_map()),
            Op::Push(Value::Text("c".into())),
            Op::push(3),
            Op::call("map-set"),
            Op::Push(Value::Text("c".into())),
            Op::call("map-get"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Int(3));
    }

    #[tokio::test]
    async fn test_map_del() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(make_map()),
            Op::Push(Value::Text("a".into())),
            Op::call("map-del"),
            Op::Push(Value::Text("a".into())),
            Op::call("map-has"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Bool(false));
    }

    #[tokio::test]
    async fn test_map_has() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(make_map()),
            Op::Push(Value::Text("a".into())),
            Op::call("map-has"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Bool(true));
    }

    #[tokio::test]
    async fn test_map_keys() {
        let ctx = setup().await;
        let ops = vec![Op::Push(make_map()), Op::call("map-keys")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(
            result.values()[0],
            Value::List(vec![Value::Text("a".into()), Value::Text("b".into())])
        );
    }

    #[tokio::test]
    async fn test_map_vals() {
        let ctx = setup().await;
        let ops = vec![Op::Push(make_map()), Op::call("map-vals")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(
            result.values()[0],
            Value::List(vec![Value::Int(1), Value::Int(2)])
        );
    }
}
