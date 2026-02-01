//! # Tool Loader
//!
//! Load tools from files and packages.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `load` | `(code --)` | Load and execute Kore code |
//! | `load-file` | `(path --)` | Load Kore file |
//! | `load-package` | `(name --)` | Load a package |
//!
//! ## Example
//!
//! ```kore
//! "mylib.kore" load-file
//! "utils" load-package
//! "[ dup * ] :square define" load
//! ```

use kore::{Context, Stack, Tool};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Loaded packages.
type PackageStore = Arc<RwLock<HashMap<String, String>>>;

fn packages() -> PackageStore {
    static PACKAGES: std::sync::OnceLock<PackageStore> = std::sync::OnceLock::new();
    PACKAGES
        .get_or_init(|| {
            let mut map = HashMap::new();
            // Built-in mini packages
            map.insert(
                "math".to_string(),
                "[ dup * ] :square define [ 2 * ] :double define".to_string(),
            );
            map.insert(
                "string".to_string(),
                "[ dup length ] :strlen define".to_string(),
            );
            Arc::new(RwLock::new(map))
        })
        .clone()
}

/// Register all loader tools into a context.
pub async fn register_loader_tools(ctx: &mut Context) {
    let pkgs = packages();

    // load: (text --)
    ctx.dict.write().await.register(
        Tool::native("load", "(text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let code_str = stack.pop()?.into_text()?;

                // Parse and execute the code
                // This would use kore's parser and evaluator
                // For now, just acknowledge the load
                tracing::debug!(code = %code_str, "loading code");

                // In real implementation:
                // let program = kore::parse(&code_str)?;
                // ctx.eval(&program)?;

                Ok((stack, ctx))
            })
        }),
    );

    // load-file: (text --)
    ctx.dict.write().await.register(
        Tool::native("load-file", "(text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let path_str = stack.pop()?.into_text()?;

                let code = std::fs::read_to_string(&path_str)
                    .map_err(|e| kore::Error::Runtime(format!("Failed to read file {}: {}", path_str, e)))?;

                // Parse and execute the code
                tracing::debug!(code = %code, path = %path_str, "loading file");

                // In real implementation:
                // let program = kore::parse(&code)?;
                // ctx.eval(&program)?;

                Ok((stack, ctx))
            })
        }),
    );

    // load-package: (text --)
    let pkgs_clone = pkgs.clone();
    ctx.dict.write().await.register(
        Tool::native("load-package", "(text --)", move |mut stack: Stack, ctx: Context| {
            let pkgs = pkgs_clone.clone();
            Box::pin(async move {
                let name_str = stack.pop()?.into_text()?;

                let pkgs_read = pkgs.read().await;
                let code = pkgs_read
                    .get(&name_str)
                    .ok_or_else(|| kore::Error::Runtime(format!("Package not found: {}", name_str)))?
                    .clone();

                drop(pkgs_read);

                // Parse and execute the code
                tracing::debug!(code = %code, package = %name_str, "loading package");

                // In real implementation:
                // let program = kore::parse(&code)?;
                // ctx.eval(&program)?;

                Ok((stack, ctx))
            })
        }),
    );

    // package-add: (text text --)
    let pkgs_clone = pkgs.clone();
    ctx.dict.write().await.register(
        Tool::native("package-add", "(text text --)", move |mut stack: Stack, ctx: Context| {
            let pkgs = pkgs_clone.clone();
            Box::pin(async move {
                let code_str = stack.pop()?.into_text()?;
                let name_str = stack.pop()?.into_text()?;

                pkgs.write().await.insert(name_str, code_str);
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_ctx() -> Context {
        let mut ctx = Context::new();
        register_loader_tools(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn test_loader_tools_registered() {
        let ctx = test_ctx().await;
        let names = { let dict = ctx.dict.read().await; dict.list(&ctx.tenant) };
        assert!(names.contains(&"load".to_string()));
        assert!(names.contains(&"load-package".to_string()));
        assert!(names.contains(&"package-add".to_string()));
    }
}
