//! # Package Management
//!
//! Install, uninstall, and publish packages.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `pkg-install` | `(name --)` | Install a package |
//! | `pkg-uninstall` | `(name --)` | Uninstall a package |
//! | `pkg-publish` | `(name config --)` | Publish a package |
//! | `pkg-update` | `(name --)` | Update a package |
//!
//! ## Example
//!
//! ```kore
//! "http-utils" pkg-install
//! "http-utils" pkg-uninstall
//! "my-package" { "version": "1.0.0", "code": "..." } pkg-publish
//! ```

use indexmap::IndexMap;
use kore::{Context, Stack, Tool, Value};
#[cfg(test)]
use kore::{execute, Op};
use std::collections::HashMap;
use std::sync::RwLock;

/// Installed packages (shared with registry module via crate::registry::installed()).
/// This is a local fallback if registry module is not available.
static INSTALLED: std::sync::OnceLock<RwLock<HashMap<String, String>>> =
    std::sync::OnceLock::new();

fn installed() -> &'static RwLock<HashMap<String, String>> {
    INSTALLED.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Published packages.
static PUBLISHED: std::sync::OnceLock<RwLock<HashMap<String, Value>>> =
    std::sync::OnceLock::new();

fn published() -> &'static RwLock<HashMap<String, Value>> {
    PUBLISHED.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Register all package tools into a context.
pub async fn register_package_tools(ctx: &mut Context) {
    // pkg-install: (name --)
    ctx.dict.write().await.register(
        Tool::native("pkg-install", "(text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name_str = stack.pop()?.into_text()?;

                // In real implementation, would:
                // 1. Fetch package from registry
                // 2. Resolve dependencies
                // 3. Install dependencies first
                // 4. Install package

                // For now, mock installation
                tracing::info!(package = %name_str, "installing package");

                // Get version from registry (mock)
                let version = "1.0.0".to_string();

                installed()
                    .write()
                    .unwrap()
                    .insert(name_str.to_string(), version.clone());

                tracing::info!(package = %name_str, version = %version, "package installed");

                Ok((stack, ctx))
            })
        }),
    );

    // pkg-uninstall: (name --)
    ctx.dict.write().await.register(
        Tool::native("pkg-uninstall", "(text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name_str = stack.pop()?.into_text()?;

                let was_installed = installed().write().unwrap().remove(&*name_str).is_some();

                if !was_installed {
                    return Err(kore::Error::Runtime(format!(
                        "Package not installed: {}",
                        name_str
                    )));
                }

                tracing::info!(package = %name_str, "package uninstalled");

                Ok((stack, ctx))
            })
        }),
    );

    // pkg-publish: (name config --)
    ctx.dict.write().await.register(
        Tool::native("pkg-publish", "(text map --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let config = stack.pop()?;
                let name_str = stack.pop()?.into_text()?;

                let config_map = match &config {
                    Value::Map(m) => m,
                    _ => return Err(kore::Error::Runtime("Config must be a map".into())),
                };

                let version = config_map
                    .get("version")
                    .and_then(|v| match v {
                        Value::Text(s) => Some(s.clone()),
                        _ => None,
                    })
                    .ok_or_else(|| kore::Error::Runtime("Config must have 'version' field".into()))?;

                // In real implementation, would:
                // 1. Validate package
                // 2. Sign package
                // 3. Upload to registry

                let published_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                let mut package_data = IndexMap::new();
                package_data.insert("name".to_string(), Value::Text(name_str.to_string()));
                package_data.insert("version".to_string(), Value::Text(version.clone()));
                package_data.insert("published_at".to_string(), Value::Int(published_at as i64));
                package_data.insert("config".to_string(), config);

                published()
                    .write()
                    .unwrap()
                    .insert(format!("{}@{}", name_str, version), Value::Map(package_data));

                tracing::info!(package = %name_str, version = %version, "package published");

                Ok((stack, ctx))
            })
        }),
    );

    // pkg-update: (name --)
    ctx.dict.write().await.register(
        Tool::native("pkg-update", "(text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name_str = stack.pop()?.into_text()?;

                // Check if installed
                let is_installed = installed().read().unwrap().contains_key(&*name_str);
                if !is_installed {
                    return Err(kore::Error::Runtime(format!(
                        "Package not installed: {}",
                        name_str
                    )));
                }

                // In real implementation, would:
                // 1. Check for newer version
                // 2. Download new version
                // 3. Replace existing

                // Mock update to new version
                let new_version = "1.1.0".to_string();
                installed()
                    .write()
                    .unwrap()
                    .insert(name_str.to_string(), new_version.clone());

                tracing::info!(package = %name_str, version = %new_version, "package updated");

                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn reset_state() {
        *installed().write().unwrap() = HashMap::new();
        *published().write().unwrap() = HashMap::new();
    }

    #[tokio::test]
    async fn test_package_tools_registered() {
        let mut ctx = Context::new();
        register_package_tools(&mut ctx).await;

        let dict = ctx.dict.read().await;
        let tenant = &ctx.tenant;
        assert!(dict.get("pkg-install", tenant).is_ok());
        assert!(dict.get("pkg-uninstall", tenant).is_ok());
        assert!(dict.get("pkg-publish", tenant).is_ok());
        assert!(dict.get("pkg-update", tenant).is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_pkg_install() {
        reset_state();

        let mut ctx = Context::new();
        register_package_tools(&mut ctx).await;

        let stack = Stack::new();
        let ops = vec![
            Op::push("test-pkg"),
            Op::call("pkg-install"),
        ];

        let _ = execute(&ops, stack, ctx).await.unwrap();
        assert!(installed().read().unwrap().contains_key("test-pkg"));
    }

    #[tokio::test]
    #[serial]
    async fn test_pkg_uninstall() {
        reset_state();

        let mut ctx = Context::new();
        register_package_tools(&mut ctx).await;

        // Install then uninstall
        let stack = Stack::new();
        let ops = vec![
            Op::push("test-pkg"),
            Op::call("pkg-install"),
            Op::push("test-pkg"),
            Op::call("pkg-uninstall"),
        ];

        let _ = execute(&ops, stack, ctx).await.unwrap();
        assert!(!installed().read().unwrap().contains_key("test-pkg"));
    }

    #[tokio::test]
    #[serial]
    async fn test_pkg_uninstall_not_installed() {
        reset_state();

        let mut ctx = Context::new();
        register_package_tools(&mut ctx).await;

        let stack = Stack::new();
        let ops = vec![
            Op::push("nonexistent"),
            Op::call("pkg-uninstall"),
        ];

        let result = execute(&ops, stack, ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_pkg_publish() {
        reset_state();

        let mut ctx = Context::new();
        register_package_tools(&mut ctx).await;

        let mut stack = Stack::new();
        let _ = stack.push(Value::Text("my-package".into()));

        let mut config = IndexMap::new();
        config.insert("version".to_string(), Value::Text("1.0.0".into()));
        config.insert("code".to_string(), Value::Text("[ dup * ] :square define".into()));
        let _ = stack.push(Value::Map(config));

        let ops = vec![Op::call("pkg-publish")];

        let _ = execute(&ops, stack, ctx).await.unwrap();
        assert!(published().read().unwrap().contains_key("my-package@1.0.0"));
    }

    #[tokio::test]
    #[serial]
    async fn test_pkg_update() {
        reset_state();

        let mut ctx = Context::new();
        register_package_tools(&mut ctx).await;

        // Install then update
        let stack = Stack::new();
        let ops = vec![
            Op::push("test-pkg"),
            Op::call("pkg-install"),
            Op::push("test-pkg"),
            Op::call("pkg-update"),
        ];

        let _ = execute(&ops, stack, ctx).await.unwrap();

        let version = installed()
            .read()
            .unwrap()
            .get("test-pkg")
            .cloned()
            .unwrap();
        assert_eq!(version, "1.1.0");
    }
}
