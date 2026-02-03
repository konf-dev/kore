//! Linear and Affine Type Tools
//!
//! Extension tools for linear and affine type handling. All tools follow P1/P2/P3.
//!
//! # Linear vs Affine
//!
//! - **Linear**: Cannot be duplicated AND cannot be discarded (must be consumed exactly once)
//! - **Affine**: Cannot be duplicated but CAN be discarded (used at most once)
//!
//! # Tools (7)
//!
//! | Tool | Effect | Description |
//! |------|--------|-------------|
//! | linear-new | (value -- linear) | Wrap value as linear (must consume) |
//! | linear-unwrap | (linear -- value) | Consume linear, get inner value |
//! | affine-new | (value -- affine) | Wrap value as affine (at most once) |
//! | affine-unwrap | (affine -- value) | Consume affine, get inner value |
//! | is-linear | (value -- bool) | Check if value is linear |
//! | is-affine | (value -- bool) | Check if value is affine |
//! | linearity | (value -- str) | Get linearity kind: "linear", "affine", or "unrestricted" |
//!
//! # Use Cases
//!
//! - File handles: affine (must close OR let drop)
//! - Database connections: linear (must explicitly close)
//! - Unique tokens: linear (ownership transfer)
//! - Capabilities: affine (can revoke or use)

use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::{ext, Value};

/// Register all linear/affine type tools
pub fn register(dict: &mut Dictionary) {
    // linear-new: (value -- linear)
    // Wrap a value as linear - it MUST be consumed exactly once
    dict.register(Tool::native(
        "linear-new",
        "(value:Any -- linear:Linear)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                
                // Don't wrap already-wrapped values
                if value.is_linear() {
                    return Err(Error::Runtime("Value is already linear".into()));
                }
                if value.is_affine() {
                    return Err(Error::Runtime("Cannot make affine value linear".into()));
                }
                
                let linear = Value::linear(value);
                stack.push(linear)?;
                Ok((stack, ctx))
            })
        },
    ));

    // linear-unwrap: (linear -- value)
    // Consume a linear value, extracting the inner value
    dict.register(Tool::native(
        "linear-unwrap",
        "(linear:Linear -- value:Any)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let linear = stack.pop()?;
                
                let ext = match &linear {
                    Value::Ext(e) if e.kind == ext::LINEAR => e,
                    _ => return Err(Error::TypeError {
                        expected: "Linear".into(),
                        got: linear.type_name().into(),
                    }),
                };
                
                // Extract inner value
                let inner = (*ext.data).clone();
                stack.push(inner)?;
                Ok((stack, ctx))
            })
        },
    ));

    // affine-new: (value -- affine)
    // Wrap a value as affine - can be used at most once (can drop without using)
    dict.register(Tool::native(
        "affine-new",
        "(value:Any -- affine:Affine)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                
                // Don't wrap already-wrapped values
                if value.is_affine() {
                    return Err(Error::Runtime("Value is already affine".into()));
                }
                if value.is_linear() {
                    // Downgrade linear to affine is allowed
                    let ext = value.as_ext()?;
                    let inner = (*ext.data).clone();
                    let affine = Value::affine(inner);
                    stack.push(affine)?;
                    return Ok((stack, ctx));
                }
                
                let affine = Value::affine(value);
                stack.push(affine)?;
                Ok((stack, ctx))
            })
        },
    ));

    // affine-unwrap: (affine -- value)
    // Consume an affine value, extracting the inner value
    dict.register(Tool::native(
        "affine-unwrap",
        "(affine:Affine -- value:Any)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let affine = stack.pop()?;
                
                let ext = match &affine {
                    Value::Ext(e) if e.kind == ext::AFFINE => e,
                    _ => return Err(Error::TypeError {
                        expected: "Affine".into(),
                        got: affine.type_name().into(),
                    }),
                };
                
                // Extract inner value
                let inner = (*ext.data).clone();
                stack.push(inner)?;
                Ok((stack, ctx))
            })
        },
    ));

    // is-linear: (value -- bool)
    // Check if a value is linear
    dict.register(Tool::native(
        "is-linear",
        "(value:Any -- is_linear:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                let is_linear = value.is_linear();
                stack.push(Value::Bool(is_linear))?;
                Ok((stack, ctx))
            })
        },
    ));

    // is-affine: (value -- bool)
    // Check if a value is affine
    dict.register(Tool::native(
        "is-affine",
        "(value:Any -- is_affine:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                let is_affine = value.is_affine();
                stack.push(Value::Bool(is_affine))?;
                Ok((stack, ctx))
            })
        },
    ));

    // linearity: (value -- str)
    // Get the linearity kind of a value
    dict.register(Tool::native(
        "linearity",
        "(value:Any -- kind:Str)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?;
                let kind = if value.is_linear() {
                    "linear"
                } else if value.is_affine() {
                    "affine"
                } else {
                    "unrestricted"
                };
                stack.push(Value::Text(kind.into()))?;
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

    async fn run_with_linear(ops: Vec<Op>) -> crate::error::Result<Vec<Value>> {
        let mut ctx = Context::new();
        // Register core tools (includes stack: dup, drop, swap, etc.)
        crate::core::register_core(&mut ctx).await;
        // Register linear tools
        {
            let mut dict = ctx.dict.write().await;
            register(&mut dict);
        }
        let stack = Stack::new();
        let (result, _) = execute(&ops, stack, ctx).await?;
        Ok(result.values().to_vec())
    }

    async fn run_expect_error(ops: Vec<Op>) -> Error {
        let mut ctx = Context::new();
        crate::core::register_core(&mut ctx).await;
        {
            let mut dict = ctx.dict.write().await;
            register(&mut dict);
        }
        let stack = Stack::new();
        match execute(&ops, stack, ctx).await {
            Err(e) => e,
            Ok(_) => panic!("Expected error but got success"),
        }
    }

    #[tokio::test]
    async fn test_linear_new_unwrap() {
        let result = run_with_linear(vec![
            Op::push(42),
            Op::call("linear-new"),
            Op::call("linear-unwrap"),
        ]).await.unwrap();
        
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_int().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_linear_cannot_dup() {
        let err = run_expect_error(vec![
            Op::push(42),
            Op::call("linear-new"),
            Op::call("dup"), // This should fail
        ]).await;
        
        assert!(matches!(err, Error::LinearDuplicate(_)));
    }

    #[tokio::test]
    async fn test_linear_cannot_drop() {
        let err = run_expect_error(vec![
            Op::push(42),
            Op::call("linear-new"),
            Op::call("drop"), // This should fail
        ]).await;
        
        assert!(matches!(err, Error::LinearDiscard(_)));
    }

    #[tokio::test]
    async fn test_affine_new_unwrap() {
        let result = run_with_linear(vec![
            Op::push(42),
            Op::call("affine-new"),
            Op::call("affine-unwrap"),
        ]).await.unwrap();
        
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_int().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_affine_cannot_dup() {
        let err = run_expect_error(vec![
            Op::push(42),
            Op::call("affine-new"),
            Op::call("dup"), // This should fail
        ]).await;
        
        assert!(matches!(err, Error::LinearDuplicate(_)));
    }

    #[tokio::test]
    async fn test_affine_can_drop() {
        // Affine values CAN be dropped (unlike linear)
        let result = run_with_linear(vec![
            Op::push(42),
            Op::call("affine-new"),
            Op::call("drop"), // This SHOULD succeed
        ]).await.unwrap();
        
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_is_linear() {
        let result = run_with_linear(vec![
            Op::push(42),
            Op::call("linear-new"),
            Op::call("is-linear"),
        ]).await.unwrap();
        
        // Note: is-linear consumes the value, so we need to check differently
        // Actually is-linear pops and pushes bool, but leaves linear on stack first
        // Let me trace: push 42 -> linear-new (pops 42, pushes linear) -> is-linear (pops linear, pushes bool)
        // So stack has just the bool
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_bool().unwrap(), true);
    }

    #[tokio::test]
    async fn test_is_affine() {
        let result = run_with_linear(vec![
            Op::push(42),
            Op::call("affine-new"),
            Op::call("is-affine"),
        ]).await.unwrap();
        
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_bool().unwrap(), true);
    }

    #[tokio::test]
    async fn test_linearity_linear() {
        let result = run_with_linear(vec![
            Op::push(42),
            Op::call("linear-new"),
            Op::call("linearity"),
        ]).await.unwrap();
        
        assert_eq!(result[0].as_text().unwrap(), "linear");
    }

    #[tokio::test]
    async fn test_linearity_affine() {
        let result = run_with_linear(vec![
            Op::push(42),
            Op::call("affine-new"),
            Op::call("linearity"),
        ]).await.unwrap();
        
        assert_eq!(result[0].as_text().unwrap(), "affine");
    }

    #[tokio::test]
    async fn test_linearity_unrestricted() {
        let result = run_with_linear(vec![
            Op::push(42),
            Op::call("linearity"),
        ]).await.unwrap();
        
        assert_eq!(result[0].as_text().unwrap(), "unrestricted");
    }

    #[tokio::test]
    async fn test_over_rejects_linear() {
        let err = run_expect_error(vec![
            Op::push(100),
            Op::call("linear-new"),  // Linear on bottom
            Op::push(200),           // Normal on top
            Op::call("over"),        // Should fail - would duplicate linear
        ]).await;
        
        assert!(matches!(err, Error::LinearDuplicate(_)));
    }

    #[tokio::test]
    async fn test_over_rejects_affine() {
        let err = run_expect_error(vec![
            Op::push(100),
            Op::call("affine-new"),  // Affine on bottom
            Op::push(200),           // Normal on top
            Op::call("over"),        // Should fail - would duplicate affine
        ]).await;
        
        assert!(matches!(err, Error::LinearDuplicate(_)));
    }

    #[tokio::test]
    async fn test_downgrade_linear_to_affine() {
        // Linear can be downgraded to affine (less restrictive)
        let result = run_with_linear(vec![
            Op::push(42),
            Op::call("linear-new"),
            Op::call("affine-new"),  // Downgrade linear to affine
            Op::call("drop"),        // Should now be droppable
        ]).await.unwrap();
        
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_swap_preserves_linearity() {
        // swap should work with linear values (no duplication)
        let result = run_with_linear(vec![
            Op::push(1),
            Op::call("linear-new"),
            Op::push(2),
            Op::call("linear-new"),
            Op::call("swap"),
            Op::call("linear-unwrap"),  // First unwrap
            Op::call("swap"),
            Op::call("linear-unwrap"),  // Second unwrap
        ]).await.unwrap();
        
        // After swap and unwraps, should have: 1, 2 (reversed from original push order)
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].as_int().unwrap(), 1);  // was on top after swap
        assert_eq!(result[1].as_int().unwrap(), 2);  // was second after swap
    }

    #[tokio::test]
    async fn test_rot_preserves_linearity() {
        // rot should work with linear values (no duplication)
        let result = run_with_linear(vec![
            Op::push(1), Op::call("linear-new"),
            Op::push(2), Op::call("linear-new"),
            Op::push(3), Op::call("linear-new"),
            Op::call("rot"),  // 1 2 3 -> 2 3 1
            Op::call("linear-unwrap"),
            Op::call("rot"), Op::call("rot"),  // rotate back to access next
            Op::call("linear-unwrap"),
            Op::call("swap"),
            Op::call("linear-unwrap"),
        ]).await.unwrap();
        
        // All values consumed successfully
        assert_eq!(result.len(), 3);
    }

    // === Handle Affinity Tests ===

    #[tokio::test]
    async fn test_handle_is_affine() {
        use crate::value::{Handle, HandleKind};
        
        let handle = Value::Handle(Handle {
            kind: HandleKind::File,
            id: "test.txt".into(),
        });
        
        assert!(handle.is_affine(), "Handles should be affine");
        assert!(handle.is_non_duplicable(), "Handles should not be duplicable");
        assert!(!handle.is_linear(), "Handles are not fully linear");
    }

    #[tokio::test]
    async fn test_handle_cannot_dup() {
        use crate::value::{Handle, HandleKind};
        
        let mut ctx = Context::new();
        crate::core::register_core(&mut ctx).await;
        
        // Create a handle value directly on stack
        let mut stack = Stack::new();
        stack.push(Value::Handle(Handle {
            kind: HandleKind::File,
            id: "test.txt".into(),
        })).unwrap();
        
        let ops = vec![Op::call("dup")];
        let result = execute(&ops, stack, ctx).await;
        
        assert!(result.is_err(), "dup on handle should fail");
        match result {
            Err(Error::LinearDuplicate(_)) => (), // Expected
            Err(e) => panic!("Expected LinearDuplicate error, got: {:?}", e),
            Ok(_) => panic!("Expected error but got success"),
        }
    }

    #[tokio::test]
    async fn test_handle_can_drop() {
        use crate::value::{Handle, HandleKind};
        
        let mut ctx = Context::new();
        crate::core::register_core(&mut ctx).await;
        
        // Create a handle value directly on stack
        let mut stack = Stack::new();
        stack.push(Value::Handle(Handle {
            kind: HandleKind::Connection,
            id: "db://localhost".into(),
        })).unwrap();
        
        let ops = vec![Op::call("drop")];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        
        assert_eq!(result.depth(), 0, "Handle should be droppable (affine, not linear)");
    }

    #[tokio::test]
    async fn test_handle_linearity_check() {
        use crate::value::{Handle, HandleKind};
        
        let mut ctx = Context::new();
        crate::core::register_core(&mut ctx).await;
        {
            let mut dict = ctx.dict.write().await;
            register(&mut dict);
        }
        
        let mut stack = Stack::new();
        stack.push(Value::Handle(Handle {
            kind: HandleKind::Secret,
            id: "api-key".into(),
        })).unwrap();
        
        let ops = vec![Op::call("linearity")];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        
        assert_eq!(result.values()[0].as_text().unwrap(), "affine");
    }
}
