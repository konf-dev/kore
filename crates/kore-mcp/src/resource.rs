//! # MCP Resources
//!
//! MCP resource access for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `mcp-resources` | `(handle -- resources)` | List available resources |
//! | `mcp-resource` | `(handle uri -- content)` | Read a resource |
//!
//! ## Example
//!
//! ```kore
//! handle mcp-resources  -- list resources
//! handle "file://config.json" mcp-resource  -- read resource
//! ```
//!
//! ## Resources
//!
//! Resources are identified by URIs and can be files, database
//! records, API responses, etc.

use indexmap::IndexMap;
use kore::{Context, Stack, Tool, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock resource store for testing.
type ResourceStore = Arc<RwLock<HashMap<String, String>>>;

fn mock_resources() -> ResourceStore {
    static MOCK_RESOURCES: std::sync::OnceLock<ResourceStore> = std::sync::OnceLock::new();
    MOCK_RESOURCES
        .get_or_init(|| {
            let mut map = HashMap::new();
            map.insert(
                "file://config.json".to_string(),
                r#"{"setting": "value", "debug": true}"#.to_string(),
            );
            map.insert(
                "file://readme.md".to_string(),
                "# Welcome\nThis is a readme file.".to_string(),
            );
            Arc::new(RwLock::new(map))
        })
        .clone()
}

/// Register all MCP resource tools into a context.
pub async fn register_resource_tools(ctx: &mut Context) {
    let resources = mock_resources();

    // mcp-resources: (text -- list)
    let resources_clone = resources.clone();
    ctx.dict.write().await.register(
        Tool::native("mcp-resources", "(text -- list)", move |mut stack: Stack, ctx: Context| {
            let resources = resources_clone.clone();
            Box::pin(async move {
                let _handle = stack.pop()?.into_text()?;
                // In real implementation, would query the MCP server

                let resources_read = resources.read().await;
                let resource_list: Vec<Value> = resources_read
                    .keys()
                    .map(|uri| {
                        let mut map: IndexMap<String, Value> = IndexMap::new();
                        map.insert("uri".to_string(), Value::Text(uri.clone()));
                        map.insert("name".to_string(), Value::Text(uri.split('/').last().unwrap_or(uri).to_string()));
                        map.insert("mimeType".to_string(), Value::Text(
                            if uri.ends_with(".json") { "application/json" } else { "text/plain" }.to_string()
                        ));
                        Value::Map(map)
                    })
                    .collect();

                stack.push(Value::List(resource_list))?;
                Ok((stack, ctx))
            })
        }),
    );

    // mcp-resource: (text text -- map)
    let resources_clone = resources.clone();
    ctx.dict.write().await.register(
        Tool::native("mcp-resource", "(text text -- map)", move |mut stack: Stack, ctx: Context| {
            let resources = resources_clone.clone();
            Box::pin(async move {
                let uri_str = stack.pop()?.into_text()?;
                let _handle = stack.pop()?.into_text()?;

                // In real implementation, would request from MCP server
                let resources_read = resources.read().await;
                let content = resources_read
                    .get(&uri_str)
                    .ok_or_else(|| kore::Error::Runtime(format!("Resource not found: {}", uri_str)))?
                    .clone();

                let mut content_item: IndexMap<String, Value> = IndexMap::new();
                content_item.insert("uri".to_string(), Value::Text(uri_str.clone()));
                content_item.insert("mimeType".to_string(), Value::Text(
                    if uri_str.ends_with(".json") { "application/json" } else { "text/plain" }.to_string()
                ));
                content_item.insert("text".to_string(), Value::Text(content));

                let mut result: IndexMap<String, Value> = IndexMap::new();
                result.insert("contents".to_string(), Value::List(vec![Value::Map(content_item)]));

                stack.push(Value::Map(result))?;
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
        register_resource_tools(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn test_resource_tools_registered() {
        let ctx = test_ctx().await;
        let names = { let dict = ctx.dict.read().await; dict.list(&ctx.tenant) };
        assert!(names.contains(&"mcp-resources".to_string()));
        assert!(names.contains(&"mcp-resource".to_string()));
    }
}
