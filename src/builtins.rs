//! Built-in Tools Registration
//!
//! This module provides `register_builtins` which registers ALL tools
//! needed for a fully functional Kore environment:
//!
//! 1. **Core Primitives (75)** - Pure computational primitives
//! 2. **Capability Tools (67)** - OS/network/storage/tensor access
//!
//! ## Architecture
//!
//! ```text
//! register_builtins()
//!     ├── register_core()     # 75 pure primitives
//!     │   ├── execution (4)   # call, spawn, if, loop
//!     │   ├── definition (2)  # def, words
//!     │   ├── error (3)       # try, fail, is-error
//!     │   ├── stack (7)       # dup, drop, swap, rot, over, dip, depth
//!     │   ├── arithmetic (6)  # add, sub, mul, div, mod, neg
//!     │   ├── comparison (2)  # eq, lt
//!     │   ├── logic (3)       # and, or, not
//!     │   ├── data (3)        # list, unlist, map-new
//!     │   ├── string (13)     # str-len, str-get, str-trim, char-code, ...
//!     │   ├── list (8)        # list-len, list-get, ...
//!     │   ├── map (6)         # map-get, map-set, ...
//!     │   ├── type (14)       # type-of, is-int, is-list, unwrap, ...
//!     │   └── combinators (4) # map, filter, fold, each
//!     │
//!     └── register_cap()      # 67 capability tools
//!         ├── algebra (11)    # cap-leq, cap-meet, res-split, ...
//!         ├── control (2)     # times, while
//!         ├── introspect (3)  # meta, meta!, defined?
//!         ├── fs (7)          # fs-read, fs-write, ...
//!         ├── http (3)        # http-get, http-post, ...
//!         ├── io (4)          # print, println, ...
//!         ├── process (5)     # exec, pid, cwd, ...
//!         ├── time (2)        # now, sleep
//!         ├── env (2)         # env-get, env-set
//!         ├── json (2)        # json-parse, json-encode
//!         ├── trace (4)       # trace-on, trace, ...
//!         ├── mem (5)         # mem-set, mem-get, ...
//!         ├── rom (5)         # rom-set, rom-get, ...
//!         └── tensor (12)     # tensor-new, tensor-matmul, tensor-relu, ...
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! let mut ctx = Context::new();
//! register_builtins(&mut ctx).await;
//! // Now ctx has all 142 tools available
//! ```
//!
//! For minimal environments, use `register_core` alone.

use crate::cap::register_cap;
use crate::context::Context;
use crate::core::register_core;

/// Total count of all built-in tools
pub const BUILTINS_COUNT: usize = 130; // 75 core + 55 cap

/// Register all built-in tools
///
/// This is the standard way to initialize a Kore context.
/// For minimal/sandboxed environments, use `register_core` instead.
pub async fn register_builtins(ctx: &mut Context) {
    // Register core primitives (pure computation)
    register_core(ctx).await;
    
    // Register capability tools (OS/network/storage access)
    register_cap(ctx).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{execute, Op, Stack, Value};

    #[tokio::test]
    async fn test_all_builtins_registered() {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        
        let dict = ctx.dict.read().await;
        
        // Check some core primitives
        assert!(dict.get("add").is_ok(), "add should be registered");
        assert!(dict.get("dup").is_ok(), "dup should be registered");
        assert!(dict.get("if").is_ok(), "if should be registered");
        assert!(dict.get("map").is_ok(), "map should be registered");
        
        // Check some capability tools
        assert!(dict.get("print").is_ok(), "print should be registered");
        assert!(dict.get("now").is_ok(), "now should be registered");
        assert!(dict.get("json-parse").is_ok(), "json-parse should be registered");
    }

    #[tokio::test]
    async fn test_core_arithmetic() {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        
        let ops = vec![Op::push(2), Op::push(3), Op::call("add")];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0], Value::Int(5));
    }

    #[tokio::test]
    async fn test_combinators() {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        
        // [1 2 3] [ 1 add ] map → [2 3 4]
        let ops = vec![
            Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
            Op::Push(Value::Quote(vec![Op::push(1), Op::call("add")])),
            Op::call("map"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(
            result.values()[0],
            Value::List(vec![Value::Int(2), Value::Int(3), Value::Int(4)])
        );
    }
}
