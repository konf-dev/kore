//! String primitives (13)
//!
//! Operations on Text values that require byte-level access.
//! These cannot be composed from other primitives.
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | str-len | (s -- n) | Length in characters |
//! | str-get | (s n -- c) | Get character at index |
//! | str-slice | (s start end -- s') | Extract substring |
//! | str-concat | (s1 s2 -- s) | Concatenate |
//! | str-split | (s sep -- list) | Split by separator |
//! | str-join | (list sep -- s) | Join with separator |
//! | str-find | (s needle -- n/-1) | Find index of needle |
//! | str-starts | (s prefix -- bool) | Starts with prefix? |
//! | str-ends | (s suffix -- bool) | Ends with suffix? |
//! | str-replace | (s from to -- s') | Replace all occurrences |
//! | str-trim | (s -- s') | Trim whitespace |
//! | char-code | (s -- n) | Get code of first char |
//! | code-char | (n -- s) | Convert code to char |

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register all string primitives
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "str-len",
        "(s -- n)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let s = stack.pop()?.into_text()?;
                stack.push(Value::Int(s.chars().count() as i64))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "str-get",
        "(s n -- c)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let n = stack.pop()?.as_int()? as usize;
                let s = stack.pop()?.into_text()?;
                let c = s.chars().nth(n).ok_or_else(|| {
                    crate::error::Error::Runtime(format!(
                        "str-get: index {} out of bounds for string of length {}",
                        n,
                        s.chars().count()
                    ))
                })?;
                stack.push(Value::Text(c.to_string()))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "str-slice",
        "(s start end -- s')",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let end = stack.pop()?.as_int()? as usize;
                let start = stack.pop()?.as_int()? as usize;
                let s = stack.pop()?.into_text()?;
                let chars: Vec<char> = s.chars().collect();
                let end = end.min(chars.len());
                let start = start.min(end);
                let slice: String = chars[start..end].iter().collect();
                stack.push(Value::Text(slice))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "str-concat",
        "(s1 s2 -- s)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let s2 = stack.pop()?.into_text()?;
                let s1 = stack.pop()?.into_text()?;
                stack.push(Value::Text(format!("{}{}", s1, s2)))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "str-split",
        "(s sep -- list)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let sep = stack.pop()?.into_text()?;
                let s = stack.pop()?.into_text()?;
                let parts: Vec<Value> = s.split(&sep).map(|p| Value::Text(p.to_string())).collect();
                stack.push(Value::List(parts))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "str-join",
        "(list sep -- s)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let sep = stack.pop()?.into_text()?;
                let list = stack.pop()?.into_list()?;
                let mut strings = Vec::with_capacity(list.len());
                for v in &list {
                    strings.push(v.as_text()?.to_string());
                }
                let result = strings.join(&sep);
                stack.push(Value::Text(result))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "str-find",
        "(s needle -- n)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let needle = stack.pop()?.into_text()?;
                let s = stack.pop()?.into_text()?;
                let idx = s.find(&needle).map(|i| i as i64).unwrap_or(-1);
                stack.push(Value::Int(idx))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "str-starts",
        "(s prefix -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let prefix = stack.pop()?.into_text()?;
                let s = stack.pop()?.into_text()?;
                stack.push(Value::Bool(s.starts_with(&prefix)))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "str-ends",
        "(s suffix -- bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let suffix = stack.pop()?.into_text()?;
                let s = stack.pop()?.into_text()?;
                stack.push(Value::Bool(s.ends_with(&suffix)))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "str-replace",
        "(s from to -- s')",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let to = stack.pop()?.into_text()?;
                let from = stack.pop()?.into_text()?;
                let s = stack.pop()?.into_text()?;
                stack.push(Value::Text(s.replace(&from, &to)))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "str-trim",
        "(s -- s')",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let s = stack.pop()?.into_text()?;
                stack.push(Value::Text(s.trim().to_string()))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "char-code",
        "(s -- n)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let s = stack.pop()?.into_text()?;
                let code = s.chars().next().map(|c| c as i64).unwrap_or(0);
                stack.push(Value::Int(code))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "code-char",
        "(n -- s)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let n = stack.pop()?.into_int()?;
                let c = char::from_u32(n as u32).unwrap_or('\0');
                stack.push(Value::Text(c.to_string()))?;
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

    async fn setup() -> Context {
        let mut ctx = Context::new();
        crate::core::register_core(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn test_str_len() {
        let ctx = setup().await;
        let ops = vec![Op::Push(Value::Text("hello".into())), Op::call("str-len")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Int(5));
    }

    #[tokio::test]
    async fn test_str_len_unicode() {
        let ctx = setup().await;
        let ops = vec![Op::Push(Value::Text("héllo".into())), Op::call("str-len")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Int(5));
    }

    #[tokio::test]
    async fn test_str_get() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("hello".into())),
            Op::push(1),
            Op::call("str-get"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Text("e".into()));
    }

    #[tokio::test]
    async fn test_str_slice() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("hello world".into())),
            Op::push(0),
            Op::push(5),
            Op::call("str-slice"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Text("hello".into()));
    }

    #[tokio::test]
    async fn test_str_concat() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("hello".into())),
            Op::Push(Value::Text(" world".into())),
            Op::call("str-concat"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Text("hello world".into()));
    }

    #[tokio::test]
    async fn test_str_split() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("a,b,c".into())),
            Op::Push(Value::Text(",".into())),
            Op::call("str-split"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(
            result.values()[0],
            Value::List(vec![
                Value::Text("a".into()),
                Value::Text("b".into()),
                Value::Text("c".into())
            ])
        );
    }

    #[tokio::test]
    async fn test_str_join() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::List(vec![
                Value::Text("a".into()),
                Value::Text("b".into()),
                Value::Text("c".into()),
            ])),
            Op::Push(Value::Text("-".into())),
            Op::call("str-join"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Text("a-b-c".into()));
    }

    #[tokio::test]
    async fn test_str_find() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("hello world".into())),
            Op::Push(Value::Text("world".into())),
            Op::call("str-find"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Int(6));
    }

    #[tokio::test]
    async fn test_str_find_not_found() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("hello".into())),
            Op::Push(Value::Text("xyz".into())),
            Op::call("str-find"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Int(-1));
    }

    #[tokio::test]
    async fn test_str_starts() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("hello world".into())),
            Op::Push(Value::Text("hello".into())),
            Op::call("str-starts"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Bool(true));
    }

    #[tokio::test]
    async fn test_str_ends() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("hello world".into())),
            Op::Push(Value::Text("world".into())),
            Op::call("str-ends"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Bool(true));
    }

    #[tokio::test]
    async fn test_str_replace() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Text("hello world".into())),
            Op::Push(Value::Text("world".into())),
            Op::Push(Value::Text("rust".into())),
            Op::call("str-replace"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Text("hello rust".into()));
    }
}
