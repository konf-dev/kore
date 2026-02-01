//! # Registry Client
//!
//! Query and browse the Kore package registry.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `pkg-search` | `(query -- packages)` | Search for packages |
//! | `pkg-info` | `(name -- info)` | Get package info |
//! | `pkg-list` | `(-- packages)` | List installed packages |
//! | `registry-url` | `(url --)` | Set registry URL |
//!
//! ## Example
//!
//! ```kore
//! "http" pkg-search  -- find http-related packages
//! "http-utils" pkg-info  -- get package details
//! pkg-list  -- show installed packages
//! ```

use indexmap::IndexMap;
use kore::{Context, Stack, Tool, Value};
use std::collections::HashMap;
use std::sync::RwLock;

/// Mock package registry.
#[derive(Debug, Clone)]
struct PackageInfo {
    name: String,
    version: String,
    description: String,
    dependencies: Vec<String>,
}

/// Registry URL.
static REGISTRY_URL: std::sync::OnceLock<RwLock<String>> = std::sync::OnceLock::new();

fn registry_url() -> &'static RwLock<String> {
    REGISTRY_URL.get_or_init(|| RwLock::new("https://registry.kore.dev".to_string()))
}

/// Mock package database.
static PACKAGES: std::sync::OnceLock<RwLock<HashMap<String, PackageInfo>>> =
    std::sync::OnceLock::new();

fn packages() -> &'static RwLock<HashMap<String, PackageInfo>> {
    PACKAGES.get_or_init(|| {
        let mut map = HashMap::new();
        map.insert(
            "http-utils".to_string(),
            PackageInfo {
                name: "http-utils".to_string(),
                version: "1.0.0".to_string(),
                description: "HTTP utility tools for Kore".to_string(),
                dependencies: vec![],
            },
        );
        map.insert(
            "json-tools".to_string(),
            PackageInfo {
                name: "json-tools".to_string(),
                version: "0.2.0".to_string(),
                description: "JSON manipulation utilities".to_string(),
                dependencies: vec![],
            },
        );
        map.insert(
            "crypto-basics".to_string(),
            PackageInfo {
                name: "crypto-basics".to_string(),
                version: "1.1.0".to_string(),
                description: "Basic cryptographic operations".to_string(),
                dependencies: vec![],
            },
        );
        RwLock::new(map)
    })
}

/// Installed packages.
static INSTALLED: std::sync::OnceLock<RwLock<HashMap<String, String>>> =
    std::sync::OnceLock::new();

pub fn installed() -> &'static RwLock<HashMap<String, String>> {
    INSTALLED.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Register all registry tools into a context.
pub async fn register_registry_tools(ctx: &mut Context) {
    // pkg-search: (query -- packages)
    ctx.dict.write().await.register(
        Tool::native("pkg-search", "(text -- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let query_str = stack.pop()?.into_text()?;

                let pkgs = packages().read().unwrap();
                let results: Vec<Value> = pkgs
                    .values()
                    .filter(|p| {
                        p.name.contains(&*query_str)
                            || p.description.to_lowercase().contains(&query_str.to_lowercase())
                    })
                    .map(|p| {
                        let mut m = IndexMap::new();
                        m.insert("name".to_string(), Value::Text(p.name.clone()));
                        m.insert("version".to_string(), Value::Text(p.version.clone()));
                        m.insert("description".to_string(), Value::Text(p.description.clone()));
                        Value::Map(m)
                    })
                    .collect();

                stack.push(Value::List(results))?;
                Ok((stack, ctx))
            })
        }),
    );

    // pkg-info: (name -- info)
    ctx.dict.write().await.register(
        Tool::native("pkg-info", "(text -- map)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let name_str = stack.pop()?.into_text()?;

                let pkgs = packages().read().unwrap();
                let pkg = pkgs
                    .get(&*name_str)
                    .ok_or_else(|| kore::Error::Runtime(format!("Package not found: {}", name_str)))?;

                let installed_version = installed().read().unwrap().get(&*name_str).cloned();

                let mut info = IndexMap::new();
                info.insert("name".to_string(), Value::Text(pkg.name.clone()));
                info.insert("version".to_string(), Value::Text(pkg.version.clone()));
                info.insert("description".to_string(), Value::Text(pkg.description.clone()));
                info.insert(
                    "dependencies".to_string(),
                    Value::List(pkg.dependencies.iter().map(|d| Value::Text(d.clone())).collect()),
                );
                info.insert("installed".to_string(), Value::Bool(installed_version.is_some()));
                info.insert(
                    "installed_version".to_string(),
                    installed_version.map(Value::Text).unwrap_or(Value::Null),
                );

                stack.push(Value::Map(info))?;
                Ok((stack, ctx))
            })
        }),
    );

    // pkg-list: (-- packages)
    ctx.dict.write().await.register(
        Tool::native("pkg-list", "(-- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let inst = installed().read().unwrap();
                let list: Vec<Value> = inst
                    .iter()
                    .map(|(name, version)| {
                        let mut m = IndexMap::new();
                        m.insert("name".to_string(), Value::Text(name.clone()));
                        m.insert("version".to_string(), Value::Text(version.clone()));
                        Value::Map(m)
                    })
                    .collect();

                stack.push(Value::List(list))?;
                Ok((stack, ctx))
            })
        }),
    );

    // registry-url: (url --)
    ctx.dict.write().await.register(
        Tool::native("registry-url", "(text --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let url_str = stack.pop()?.into_text()?;
                *registry_url().write().unwrap() = url_str;
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use kore::{execute, Op};

    #[tokio::test]
    async fn test_registry_tools_registered() {
        let mut ctx = Context::new();
        register_registry_tools(&mut ctx).await;

        let dict = ctx.dict.read().await;
        let tenant = &ctx.tenant;
        assert!(dict.get("pkg-search", tenant).is_ok());
        assert!(dict.get("pkg-info", tenant).is_ok());
        assert!(dict.get("pkg-list", tenant).is_ok());
        assert!(dict.get("registry-url", tenant).is_ok());
    }

    #[tokio::test]
    async fn test_pkg_search() {
        let mut ctx = Context::new();
        register_registry_tools(&mut ctx).await;

        let stack = Stack::new();
        let ops = vec![
            Op::push("http"),
            Op::call("pkg-search"),
        ];

        let (result_stack, _) = execute(&ops, stack, ctx).await.unwrap();
        let result = result_stack.values().last().unwrap();

        match result {
            Value::List(arr) => {
                assert!(!arr.is_empty());
            }
            _ => panic!("Expected list"),
        }
    }

    #[tokio::test]
    async fn test_pkg_list_empty() {
        // Clear installed for test
        *installed().write().unwrap() = HashMap::new();

        let mut ctx = Context::new();
        register_registry_tools(&mut ctx).await;

        let stack = Stack::new();
        let ops = vec![Op::call("pkg-list")];

        let (result_stack, _) = execute(&ops, stack, ctx).await.unwrap();
        let result = result_stack.values().last().unwrap();

        match result {
            Value::List(arr) => {
                assert!(arr.is_empty());
            }
            _ => panic!("Expected list"),
        }
    }
}
