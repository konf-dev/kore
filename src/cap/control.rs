//! Control flow tools (2)
//!
//! Native loop tools that can't be easily composed from primitives
//! because they have special stack isolation semantics.
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | times | (n quote -- ) | Execute quote n times with index |
//! | while | (cond body -- ) | Loop while condition true |

use crate::context::{Context, Dictionary};
use crate::executor::execute;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register control flow tools (2)
pub fn register(dict: &mut Dictionary) {
    // times: (n quote -- ) - execute quote n times
    // For each iteration i in 0..n:
    //   - Create fresh stack with just i
    //   - Execute quote
    //   - Push results back to main stack
    // NOTE: Results are MOVED (not cloned) so linear values are safe
    dict.register(Tool::native(
        "times",
        "(n:Int f:Quote -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                let n = stack.pop()?.into_int()?;
                
                for i in 0..n {
                    let mut iter_stack = Stack::new();
                    iter_stack.push(Value::Int(i))?;
                    let (result_stack, _) = execute(&quote, iter_stack, ctx.clone()).await?;
                    // Move results to main stack (linear-safe: no clone needed)
                    for v in result_stack.into_values() {
                        stack.push(v)?;
                    }
                }
                
                Ok((stack, ctx))
            })
        },
    ));

    // while: (cond-quote body-quote -- ) - while cond returns true, execute body
    // The condition quote is evaluated on the current stack.
    // The condition result (bool) is consumed.
    // If true, body is executed on the remaining stack.
    dict.register(Tool::native(
        "while",
        "(cond:Quote body:Quote -- )",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let body = stack.pop()?.into_quote()?;
                let cond = stack.pop()?.into_quote()?;
                
                loop {
                    // Evaluate condition on current stack
                    let (mut cond_stack, _) = execute(&cond, stack, ctx.clone()).await?;
                    
                    // Pop the condition result
                    let should_continue = cond_stack.pop()?.is_truthy();
                    stack = cond_stack;
                    
                    if !should_continue {
                        break;
                    }
                    
                    // Execute body
                    let (new_stack, _) = execute(&body, stack, ctx.clone()).await?;
                    stack = new_stack;
                }
                
                Ok((stack, ctx))
            })
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::core::register_core;
    use crate::executor::execute;
    use crate::op::Op;

    async fn setup() -> Context {
        let mut ctx = Context::new();
        register_core(&mut ctx).await;
        let mut dict = ctx.dict.write().await;
        register(&mut dict);
        drop(dict);
        ctx
    }

    #[tokio::test]
    async fn test_times() {
        let ctx = setup().await;
        // 3 [ dup ] times -> each iteration gets index i, dup pushes i i, so 0 0 1 1 2 2
        let ops = vec![
            Op::push(3),
            Op::quote(vec![Op::call("dup")]),
            Op::call("times"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values().len(), 6);  // 3 iterations * 2 values each
        assert_eq!(result.values()[0].as_int().unwrap(), 0);
        assert_eq!(result.values()[1].as_int().unwrap(), 0);
        assert_eq!(result.values()[2].as_int().unwrap(), 1);
        assert_eq!(result.values()[3].as_int().unwrap(), 1);
        assert_eq!(result.values()[4].as_int().unwrap(), 2);
        assert_eq!(result.values()[5].as_int().unwrap(), 2);
    }

    #[tokio::test]
    async fn test_while() {
        let ctx = setup().await;
        // 0 [ dup 5 lt ] [ 1 add ] while -> 5
        let ops = vec![
            Op::push(0),
            Op::quote(vec![Op::call("dup"), Op::push(5), Op::call("lt")]),
            Op::quote(vec![Op::push(1), Op::call("add")]),
            Op::call("while"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 5);
    }
}
