//! # Tool Resolver
//!
//! Resolve tools across multiple sources.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `resolve` | `(name -- tool-info)` | Resolve a tool by name |
//! | `resolve-all` | `(pattern -- tools)` | Resolve all matching tools |
//! | `sources` | `(-- sources)` | List resolution sources |
//! | `source-add` | `(source --)` | Add a resolution source |
//!
//! ## Example
//!
//! ```kore
//! "http-get" resolve  -- find tool info
//! "http-*" resolve-all  -- find all http tools
//! ```
//!
//! ## Sources
//!
//! - `local`: Local dictionary
//! - `package:<name>`: Loaded package
//! - `mcp:<handle>`: Connected MCP server

use indexmap::IndexMap;
use kore::{Context, Stack, Tool, Value};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Resolution sources.
#[derive(Debug, Clone)]
enum Source {
    Local,
    Package(String),
    Mcp(String),
}

/// Global source list.
type SourceStore = Arc<RwLock<Vec<Source>>>;

fn sources() -> SourceStore {
    static SOURCES: std::sync::OnceLock<SourceStore> = std::sync::OnceLock::new();
    SOURCES
        .get_or_init(|| Arc::new(RwLock::new(vec![Source::Local])))
        .clone()
}

/// Simple glob-like pattern matching.
fn matches_pattern(name: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        return name.starts_with(prefix);
    }

    if pattern.starts_with('*') {
        let suffix = &pattern[1..];
        return name.ends_with(suffix);
    }

    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            return name.starts_with(parts[0]) && name.ends_with(parts[1]);
        }
    }

    name == pattern
}

/// Register all resolver tools into a context.
pub async fn register_resolver_tools(ctx: &mut Context) {
    let srcs = sources();

    // resolve: (text -- map)
    let srcs_clone = srcs.clone();
    ctx.dict.write().await.register(
        Tool::native("resolve", "(text -- map)", move |mut stack: Stack, ctx: Context| {
            let srcs = srcs_clone.clone();
            Box::pin(async move {
                let name_str = stack.pop()?.into_text()?;

                // Check local dictionary first
                let found_local = {
                    let dict = ctx.dict.read().await;
                    if let Ok(tool) = dict.get(&name_str, &ctx.tenant) {
                        let mut info: IndexMap<String, Value> = IndexMap::new();
                        info.insert("name".to_string(), Value::Text(tool.name.clone()));
                        info.insert("doc".to_string(), Value::Text(
                            tool.effect.as_ref().map(|e| e.to_string()).unwrap_or_default()
                        ));
                        info.insert("source".to_string(), Value::Text("local".to_string()));
                        info.insert("found".to_string(), Value::Bool(true));
                        Some(info)
                    } else {
                        None
                    }
                };

                if let Some(info) = found_local {
                    stack.push(Value::Map(info))?;
                    return Ok((stack, ctx));
                }

                // Check other sources
                let srcs_read = srcs.read().await;
                for source in srcs_read.iter() {
                    match source {
                        Source::Package(pkg_name) => {
                            // Would check package for tool
                            tracing::debug!(package = %pkg_name, tool = %name_str, "checking package");
                        }
                        Source::Mcp(handle) => {
                            // Would query MCP server for tool
                            tracing::debug!(mcp = %handle, tool = %name_str, "checking MCP");
                        }
                        Source::Local => {
                            // Already checked
                        }
                    }
                }
                drop(srcs_read);

                // Not found
                let mut info: IndexMap<String, Value> = IndexMap::new();
                info.insert("name".to_string(), Value::Text(name_str));
                info.insert("found".to_string(), Value::Bool(false));
                stack.push(Value::Map(info))?;
                Ok((stack, ctx))
            })
        }),
    );

    // resolve-all: (text -- list)
    ctx.dict.write().await.register(
        Tool::native("resolve-all", "(text -- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let pattern_str = stack.pop()?.into_text()?;

                let results: Vec<Value> = {
                    let mut results_inner: Vec<Value> = Vec::new();
                    let mut seen: HashSet<String> = HashSet::new();

                    // Check local dictionary
                    let dict = ctx.dict.read().await;
                    for name in dict.list(&ctx.tenant) {
                        if matches_pattern(&name, &pattern_str) && !seen.contains(&name) {
                            if let Ok(tool) = dict.get(&name, &ctx.tenant) {
                                let mut info: IndexMap<String, Value> = IndexMap::new();
                                info.insert("name".to_string(), Value::Text(tool.name.clone()));
                                info.insert("doc".to_string(), Value::Text(
                                    tool.effect.as_ref().map(|e| e.to_string()).unwrap_or_default()
                                ));
                                info.insert("source".to_string(), Value::Text("local".to_string()));
                                results_inner.push(Value::Map(info));
                                seen.insert(name);
                            }
                        }
                    }
                    results_inner
                };

                stack.push(Value::List(results))?;
                Ok((stack, ctx))
            })
        }),
    );

    // sources: (-- list)
    let srcs_clone = srcs.clone();
    ctx.dict.write().await.register(
        Tool::native("sources", "(-- list)", move |mut stack: Stack, ctx: Context| {
            let srcs = srcs_clone.clone();
            Box::pin(async move {
                let srcs_read = srcs.read().await;
                let source_list: Vec<Value> = srcs_read
                    .iter()
                    .map(|s| {
                        let mut map: IndexMap<String, Value> = IndexMap::new();
                        match s {
                            Source::Local => {
                                map.insert("type".to_string(), Value::Text("local".to_string()));
                            }
                            Source::Package(name) => {
                                map.insert("type".to_string(), Value::Text("package".to_string()));
                                map.insert("name".to_string(), Value::Text(name.clone()));
                            }
                            Source::Mcp(handle) => {
                                map.insert("type".to_string(), Value::Text("mcp".to_string()));
                                map.insert("handle".to_string(), Value::Text(handle.clone()));
                            }
                        }
                        Value::Map(map)
                    })
                    .collect();

                stack.push(Value::List(source_list))?;
                Ok((stack, ctx))
            })
        }),
    );

    // source-add: (map --)
    let srcs_clone = srcs.clone();
    ctx.dict.write().await.register(
        Tool::native("source-add", "(map --)", move |mut stack: Stack, ctx: Context| {
            let srcs = srcs_clone.clone();
            Box::pin(async move {
                let source = stack.pop()?.into_map()?;

                let source_type = source
                    .get("type")
                    .and_then(|v| if let Value::Text(t) = v { Some(t.as_str()) } else { None })
                    .ok_or_else(|| kore::Error::Runtime("Source must have 'type' field".to_string()))?;

                let new_source = match source_type {
                    "local" => Source::Local,
                    "package" => {
                        let name = source
                            .get("name")
                            .and_then(|v| if let Value::Text(t) = v { Some(t.as_str()) } else { None })
                            .ok_or_else(|| kore::Error::Runtime("Package source must have 'name' field".to_string()))?;
                        Source::Package(name.to_string())
                    }
                    "mcp" => {
                        let handle = source
                            .get("handle")
                            .and_then(|v| if let Value::Text(t) = v { Some(t.as_str()) } else { None })
                            .ok_or_else(|| kore::Error::Runtime("MCP source must have 'handle' field".to_string()))?;
                        Source::Mcp(handle.to_string())
                    }
                    _ => return Err(kore::Error::Runtime(format!("Unknown source type: {}", source_type))),
                };

                srcs.write().await.push(new_source);
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
        register_resolver_tools(&mut ctx).await;
        ctx
    }

    #[test]
    fn test_matches_pattern() {
        assert!(matches_pattern("http-get", "http-*"));
        assert!(matches_pattern("http-post", "http-*"));
        assert!(!matches_pattern("ws-connect", "http-*"));

        assert!(matches_pattern("data-get", "*-get"));
        assert!(!matches_pattern("data-set", "*-get"));

        assert!(matches_pattern("anything", "*"));
        assert!(matches_pattern("exact", "exact"));
    }

    #[tokio::test]
    async fn test_resolver_tools_registered() {
        let ctx = test_ctx().await;
        let names = { let dict = ctx.dict.read().await; dict.list(&ctx.tenant) };
        assert!(names.contains(&"resolve".to_string()));
        assert!(names.contains(&"resolve-all".to_string()));
        assert!(names.contains(&"sources".to_string()));
    }
}
