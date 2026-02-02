//! Standard Library Loader
//!
//! Loads .kore files from the stdlib directory and registers them
//! into the tool dictionary. This allows composed tools to be
//! defined in .kore files rather than Rust.
//!
//! # Architecture
//!
//! The stdlib follows a layered approach:
//! 1. **Native Primitives** (~50): Defined in Rust in builtins.rs
//! 2. **Composed Tools**: Defined in .kore files using primitives
//!
//! # File Format
//!
//! .kore files use the standard syntax:
//! ```text
//! # Comments start with #
//! [ body ] "name" def    # Define a tool
//! [ names... ] export    # (optional) List of exports
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use kore::{Context, stdlib};
//!
//! let ctx = Context::new();
//! stdlib::load_prelude(&ctx).await?;
//! ```

use crate::{Context, Op, Stack, error::Result, execute};

/// Embedded stdlib files (compiled into the binary)
mod embedded {
    /// prelude.kore - essential composed tools
    pub const PRELUDE: &str = include_str!("../stdlib/prelude.kore");
    /// list.kore - list manipulation tools
    pub const LIST: &str = include_str!("../stdlib/list.kore");
}

/// Load the prelude (essential composed tools)
///
/// This loads definitions like:
/// - Stack: dupd, nip2, 2dup, 2drop, dip, keep, bi, tri
/// - Boolean: true, false, bool
/// - Arithmetic: inc, dec, zero?, pos?, neg?, even?, odd?
/// - Control: when, unless, times, while
/// - List: first, second, third, last, empty?, singleton
pub async fn load_prelude(ctx: &Context) -> Result<()> {
    load_source(ctx, embedded::PRELUDE).await
}

/// Load the list library
///
/// This loads definitions like:
/// - Construction: range, repeat
/// - Access: len, nth, head, tail, init, take, drop-n
/// - Predicates: null?, elem?
/// - Transformation: reverse, concat, flatten, filter, map, fold
pub async fn load_list(ctx: &Context) -> Result<()> {
    load_source(ctx, embedded::LIST).await
}

/// Load all stdlib modules
pub async fn load_all(ctx: &Context) -> Result<()> {
    load_prelude(ctx).await?;
    load_list(ctx).await?;
    Ok(())
}

/// Load source code into the context
///
/// Parses the source and executes it, which registers
/// any `def` calls into the dictionary.
async fn load_source(ctx: &Context, source: &str) -> Result<()> {
    let ops = Op::parse(source)?;
    let stack = Stack::new();
    execute(&ops, stack, ctx.clone()).await?;
    Ok(())
}

/// Load a .kore file from a path
///
/// # Example
///
/// ```ignore
/// stdlib::load_file(&ctx, "mylib.kore").await?;
/// ```
pub async fn load_file(ctx: &Context, path: &str) -> Result<()> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| crate::error::Error::Runtime(format!("Failed to load {}: {}", path, e)))?;
    load_source(ctx, &source).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::register_builtins;

    async fn setup() -> Context {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn test_load_prelude() {
        let ctx = setup().await;
        load_prelude(&ctx).await.expect("prelude should load");

        // Verify some tools were defined
        let dict = ctx.dict.read().await;
        assert!(dict.get("inc").is_ok(), "inc should be defined");
        assert!(dict.get("dec").is_ok(), "dec should be defined");
        assert!(dict.get("when").is_ok(), "when should be defined");
        assert!(dict.get("first").is_ok(), "first should be defined");
    }

    #[tokio::test]
    async fn test_inc_works() {
        let ctx = setup().await;
        load_prelude(&ctx).await.unwrap();

        // Test: 5 inc → 6
        let ops = Op::parse("5 inc").unwrap();
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 6);
    }

    #[tokio::test]
    async fn test_when_works() {
        let ctx = setup().await;
        load_prelude(&ctx).await.unwrap();

        // Test: true [ 42 ] when → 42
        let ops = Op::parse("true [ 42 ] when").unwrap();
        let (result, _) = execute(&ops, Stack::new(), ctx.clone()).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 42);

        // Test: false [ 42 ] when → (empty)
        let ops = Op::parse("false [ 42 ] when").unwrap();
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(result.values().is_empty());
    }

    #[tokio::test]
    async fn test_first_works() {
        let ctx = setup().await;
        load_prelude(&ctx).await.unwrap();

        // Test: [1 2 3] first → 1
        let ops = vec![
            Op::Push(crate::value::Value::List(vec![
                crate::value::Value::Int(1),
                crate::value::Value::Int(2),
                crate::value::Value::Int(3),
            ])),
            Op::call("first"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_load_list() {
        let ctx = setup().await;
        load_prelude(&ctx).await.unwrap();
        load_list(&ctx).await.expect("list should load");

        // Verify some list tools were defined
        let dict = ctx.dict.read().await;
        assert!(dict.get("range").is_ok(), "range should be defined");
        assert!(dict.get("len").is_ok(), "len should be defined");
        assert!(dict.get("head").is_ok(), "head should be defined");
    }
}
