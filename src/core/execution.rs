//! Execution primitives (4)
//!
//! - call: (quote -- ...) execute a quote
//! - spawn: (quote caps res -- handle) sandboxed execution
//! - if: (bool then else -- ...) conditional
//! - loop: (body exit -- ...) iteration with escape

use crate::algebra::{CapSet, Res};
use crate::context::{Context, Dictionary};
use crate::executor::execute;
use crate::resources::Resources;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn register(dict: &mut Dictionary) {
    // call: (quote -- ...)
    dict.register(Tool::native(
        "call",
        "(q:Quote -- ...)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                execute(&quote, stack, ctx).await
            })
        },
    ));

    // if: (cond then else -- ...)
    // Condition is evaluated using truthy semantics:
    // - false, 0, 0.0, "", [], {}, null -> falsy
    // - everything else -> truthy
    dict.register(Tool::native(
        "if",
        "(cond:Any then:Quote else:Quote -- ...)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let else_branch = stack.pop()?.into_quote()?;
                let then_branch = stack.pop()?.into_quote()?;
                let cond = stack.pop()?;
                
                let branch = if cond.is_truthy() { then_branch } else { else_branch };
                execute(&branch, stack, ctx).await
            })
        },
    ));

    // loop: (body exit -- ...)
    // Execute body, then exit. If exit leaves true on top, stop.
    // Otherwise repeat.
    dict.register(Tool::native(
        "loop",
        "(body:Quote exit:Quote -- ...)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let exit = stack.pop()?.into_quote()?;
                let body = stack.pop()?.into_quote()?;
                
                loop {
                    // Execute body
                    let (new_stack, _) = execute(&body, stack, ctx.clone()).await?;
                    stack = new_stack;
                    
                    // Execute exit condition
                    let (mut cond_stack, _) = execute(&exit, stack, ctx.clone()).await?;
                    
                    // Pop and check condition
                    let should_exit = cond_stack.pop()?.is_truthy();
                    stack = cond_stack;
                    
                    if should_exit {
                        break;
                    }
                }
                
                Ok((stack, ctx))
            })
        },
    ));

    // spawn: (quote caps ratio -- result-list)
    // Execute in sandboxed context with attenuated capabilities and split resources
    // - child-caps ≤ parent-caps (attenuation)
    // - child-res + parent-remaining = parent-original (conservation)
    dict.register(Tool::native(
        "spawn",
        "(body:Quote caps:List ratio:Float -- result:List)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let ratio = stack.pop()?.as_float()?;
                let caps_list = stack.pop()?.into_list()?;
                let quote = stack.pop()?.into_quote()?;
                
                // Parse requested capabilities
                let caps_str: String = caps_list
                    .iter()
                    .filter_map(|v| v.as_text().ok())
                    .collect::<Vec<_>>()
                    .join(",");
                let requested_caps = CapSet::parse(&caps_str);
                
                // Attenuate: child can't have more caps than parent
                let parent_caps = CapSet::from_capabilities(&ctx.caps);
                let child_caps = parent_caps.attenuate(&requested_caps);
                
                // Get parent resources and split by ratio
                let parent_res = {
                    let res = ctx.resources.read().await;
                    Res::new(
                        res.mem.available(),
                        res.rom.available(),
                        res.compute.available(),
                        res.net.available(),
                    )
                };
                let (child_res, remaining) = parent_res.split(ratio);
                
                // Update parent's resources (they gave some to child)
                {
                    let mut res = ctx.resources.write().await;
                    res.mem.used = res.mem.total.saturating_sub(remaining.mem);
                    res.rom.used = res.rom.total.saturating_sub(remaining.rom);
                    res.compute.used = res.compute.total.saturating_sub(remaining.compute);
                    res.net.used = res.net.total.saturating_sub(remaining.net);
                }
                
                // Create child context
                let child_caps_obj = child_caps.to_capabilities();
                let child_resources = Resources::with_limits(
                    child_res.mem, child_res.rom, 
                    child_res.compute, child_res.net
                );
                let child_ctx = Context {
                    dict: ctx.dict.clone(),
                    caps: Arc::new(child_caps_obj),
                    resources: Arc::new(RwLock::new(child_resources)),
                    memory: crate::memory::Memory::new(),
                    storage: ctx.storage.clone(),
                };
                
                // Execute in child context with fresh stack
                let (result_stack, _) = execute(&quote, Stack::new(), child_ctx).await?;
                
                // Return child's stack as list
                let result: Vec<Value> = result_stack.values().to_vec();
                stack.push(Value::List(result))?;
                
                Ok((stack, ctx))
            })
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core;
    use crate::op::Op;

    async fn setup() -> Context {
        let ctx = Context::trusted();
        {
            let mut dict = ctx.dict.write().await;
            register(&mut dict);
            core::stack::register(&mut dict);
            core::arithmetic::register(&mut dict);
            core::comparison::register(&mut dict);
            core::logic::register(&mut dict);
        }
        ctx
    }

    #[tokio::test]
    async fn test_call() {
        let ctx = setup().await;
        let ops = vec![
            Op::push(5),
            Op::quote(vec![Op::call("dup"), Op::call("add")]),
            Op::call("call"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 10);
    }

    #[tokio::test]
    async fn test_if_true() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Bool(true)),
            Op::quote(vec![Op::push(1)]),
            Op::quote(vec![Op::push(2)]),
            Op::call("if"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_if_false() {
        let ctx = setup().await;
        let ops = vec![
            Op::Push(Value::Bool(false)),
            Op::quote(vec![Op::push(1)]),
            Op::quote(vec![Op::push(2)]),
            Op::call("if"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 2);
    }

    #[tokio::test]
    async fn test_loop() {
        let ctx = setup().await;
        // Count from 0 to 5
        // 0 [1 add] [dup 5 eq] loop -> 5
        let ops = vec![
            Op::push(0),
            Op::quote(vec![Op::push(1), Op::call("add")]),
            Op::quote(vec![Op::call("dup"), Op::push(5), Op::call("eq")]),
            Op::call("loop"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 5);
    }

    #[tokio::test]
    async fn test_spawn_basic() {
        let ctx = setup().await;
        // Spawn with empty caps and 50% resources
        let ops = vec![
            Op::quote(vec![Op::push(42)]),
            Op::Push(Value::List(vec![])),
            Op::Push(Value::Float(0.5)),  // 50% resource split ratio
            Op::call("spawn"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        // spawn returns a list containing child's stack
        let child_stack = result.values()[0].as_list().unwrap();
        assert_eq!(child_stack.len(), 1);
        assert_eq!(child_stack[0].as_int().unwrap(), 42);
    }
}
