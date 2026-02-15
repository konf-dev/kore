//! Local variable slots (16 tools)
//!
//! 8 store tools and 8 load tools for scratch variable storage.
//! These give the model fast named registers without complex stack juggling.
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | store0 | (a -- ) | Pop TOS into slot 0 |
//! | load0 | ( -- a) | Push slot 0 value (Null if unset) |
//! | ... | ... | ... |
//! | store7 | (a -- ) | Pop TOS into slot 7 |
//! | load7 | ( -- a) | Push slot 7 value (Null if unset) |
//!
//! ## Scoping
//!
//! Locals live on the Stack, so they follow stack lifetime:
//! - Shared across `call`, `loop`, `while`, `if`, `dip` (same stack)
//! - Fresh in `spawn`, `times`, `map`/`filter`/`each` (new stack)
//! - Saved/restored by `try`/`catch` (stack checkpoint)

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;

pub fn register(dict: &mut Dictionary) {
    for slot in 0..8u8 {
        // storeN: (a -- ) — pop TOS into local slot N
        let store_name = format!("store{}", slot);
        let store_sig = format!("(a -- )  store into slot {}", slot);
        dict.register(Tool::native(
            store_name,
            &store_sig,
            move |mut stack: Stack, ctx: Context| {
                Box::pin(async move {
                    let val = stack.pop()?;
                    stack.store_local(slot as usize, val)?;
                    Ok((stack, ctx))
                })
            },
        ));

        // loadN: ( -- a) — push local slot N value (Null if unset)
        let load_name = format!("load{}", slot);
        let load_sig = format!("( -- a)  load from slot {}", slot);
        dict.register(Tool::native(
            load_name,
            &load_sig,
            move |mut stack: Stack, ctx: Context| {
                Box::pin(async move {
                    let val = stack.load_local(slot as usize)?;
                    stack.push(val)?;
                    Ok((stack, ctx))
                })
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core;
    use crate::executor::execute;
    use crate::op::Op;

    async fn setup() -> Context {
        let ctx = Context::new();
        {
            let mut dict = ctx.dict.write().await;
            register(&mut dict);
            core::stack::register(&mut dict);
            core::arithmetic::register(&mut dict);
            core::comparison::register(&mut dict);
            core::execution::register(&mut dict);
        }
        ctx
    }

    #[tokio::test]
    async fn test_store_load_roundtrip() {
        let ctx = setup().await;
        // 42 store0 load0  →  42
        let ops = vec![
            Op::push(42),
            Op::call("store0"),
            Op::call("load0"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_unset_slot_returns_null() {
        let ctx = setup().await;
        // load3 (never stored)  →  Null
        let ops = vec![Op::call("load3")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert!(result.values()[0].is_null());
    }

    #[tokio::test]
    async fn test_slot_isolation() {
        let ctx = setup().await;
        // 10 store0  20 store1  load0 load1  →  10 20
        let ops = vec![
            Op::push(10),
            Op::call("store0"),
            Op::push(20),
            Op::call("store1"),
            Op::call("load0"),
            Op::call("load1"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 2);
        assert_eq!(result.values()[0].as_int().unwrap(), 10);
        assert_eq!(result.values()[1].as_int().unwrap(), 20);
    }

    #[tokio::test]
    async fn test_overwrite_slot() {
        let ctx = setup().await;
        // 10 store0  20 store0  load0  →  20 (overwritten)
        let ops = vec![
            Op::push(10),
            Op::call("store0"),
            Op::push(20),
            Op::call("store0"),
            Op::call("load0"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 20);
    }

    #[tokio::test]
    async fn test_all_eight_slots() {
        let ctx = setup().await;
        // Store 0..7 into slots 0..7, then load them all back
        let mut ops = Vec::new();
        for i in 0..8 {
            ops.push(Op::push(i as i64));
            ops.push(Op::call(format!("store{}", i)));
        }
        for i in 0..8 {
            ops.push(Op::call(format!("load{}", i)));
        }
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 8);
        for i in 0..8 {
            assert_eq!(result.values()[i].as_int().unwrap(), i as i64);
        }
    }

    #[tokio::test]
    async fn test_locals_persist_across_call() {
        let ctx = setup().await;
        // 42 store0  [load0] call  →  42  (locals shared through call)
        let ops = vec![
            Op::push(42),
            Op::call("store0"),
            Op::quote(vec![Op::call("load0")]),
            Op::call("call"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_locals_with_different_types() {
        let ctx = setup().await;
        // Store different types: int, text, bool
        let ops = vec![
            Op::push(42),
            Op::call("store0"),
            Op::push("hello"),
            Op::call("store1"),
            Op::push(true),
            Op::call("store2"),
            Op::call("load0"),
            Op::call("load1"),
            Op::call("load2"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 3);
        assert_eq!(result.values()[0].as_int().unwrap(), 42);
        assert_eq!(result.values()[1].as_text().unwrap(), "hello");
        assert_eq!(result.values()[2].as_bool().unwrap(), true);
    }

    #[tokio::test]
    async fn test_store_empty_stack_errors() {
        let ctx = setup().await;
        // store0 with empty stack  →  StackUnderflow
        let ops = vec![Op::call("store0")];
        let result = execute(&ops, Stack::new(), ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_locals_swap_pattern() {
        let ctx = setup().await;
        // Classic "save and restore" pattern:
        // 10 20 store0 store1 load0 load1  →  20 10  (swapped via locals)
        let ops = vec![
            Op::push(10),
            Op::push(20),
            Op::call("store0"),  // slot0 = 20
            Op::call("store1"),  // slot1 = 10
            Op::call("load0"),   // push 20
            Op::call("load1"),   // push 10
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.depth(), 2);
        assert_eq!(result.values()[0].as_int().unwrap(), 20);
        assert_eq!(result.values()[1].as_int().unwrap(), 10);
    }
}
