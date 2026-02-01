//! # MCP Client
//!
//! MCP server connection and tool invocation.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `mcp-connect` | `(url -- handle)` | Connect to MCP server |
//! | `mcp-list` | `(handle -- tools)` | List available tools |
//! | `mcp-call` | `(handle name args -- result)` | Call a tool |
//! | `mcp-close` | `(handle --)` | Close connection |
//!
//! ## Example
//!
//! ```kore
//! "http://localhost:3000/mcp" mcp-connect
//! dup mcp-list  -- get available tools
//! swap "echo" { "message": "hello" } mcp-call
//! ```
//!
//! ## Protocol
//!
//! Uses JSON-RPC over HTTP for MCP communication.

use indexmap::IndexMap;
use kore::{Context, Stack, Tool, Value};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// MCP connection state.
#[derive(Debug, Clone)]
#[allow(dead_code)] // url reserved for reconnection and debugging
struct McpConnection {
    url: String,
    tools: Vec<McpToolInfo>,
}

/// MCP tool info.
#[derive(Debug, Clone)]
struct McpToolInfo {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

/// Global MCP connection store.
type ConnectionStore = Arc<RwLock<HashMap<String, McpConnection>>>;

fn connections() -> ConnectionStore {
    static MCP_CONNECTIONS: std::sync::OnceLock<ConnectionStore> = std::sync::OnceLock::new();
    MCP_CONNECTIONS
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
        .clone()
}

/// Register all MCP client tools into a context.
pub async fn register_client_tools(ctx: &mut Context) {
    let conns = connections();

    // mcp-connect: (text -- text)
    let conns_clone = conns.clone();
    ctx.dict.write().await.register(
        Tool::native("mcp-connect", "(text -- text)", move |mut stack: Stack, ctx: Context| {
            let conns = conns_clone.clone();
            Box::pin(async move {
                let url_str = stack.pop()?.into_text()?;
                let handle_id = Uuid::new_v4().to_string();

                // In real implementation, would:
                // 1. Send initialize request
                // 2. Get server capabilities
                // 3. List tools

                // For now, create mock connection
                let conn = McpConnection {
                    url: url_str.clone(),
                    tools: vec![
                        McpToolInfo {
                            name: "echo".to_string(),
                            description: "Echo a message back".to_string(),
                            input_schema: json!({"type": "object", "properties": {"message": {"type": "string"}}}),
                        },
                        McpToolInfo {
                            name: "time".to_string(),
                            description: "Get current time".to_string(),
                            input_schema: json!({"type": "object", "properties": {}}),
                        },
                    ],
                };

                conns.write().await.insert(handle_id.clone(), conn);
                stack.push(Value::Text(handle_id))?;
                Ok((stack, ctx))
            })
        }),
    );

    // mcp-list: (text -- list)
    let conns_clone = conns.clone();
    ctx.dict.write().await.register(
        Tool::native("mcp-list", "(text -- list)", move |mut stack: Stack, ctx: Context| {
            let conns = conns_clone.clone();
            Box::pin(async move {
                let handle_str = stack.pop()?.into_text()?;

                let conns_read = conns.read().await;
                let conn = conns_read
                    .get(&handle_str)
                    .ok_or_else(|| kore::Error::Runtime(format!("MCP connection not found: {}", handle_str)))?;

                let tools: Vec<Value> = conn
                    .tools
                    .iter()
                    .map(|t| {
                        let mut map: IndexMap<String, Value> = IndexMap::new();
                        map.insert("name".to_string(), Value::Text(t.name.clone()));
                        map.insert("description".to_string(), Value::Text(t.description.clone()));
                        map.insert("inputSchema".to_string(), Value::Text(t.input_schema.to_string()));
                        Value::Map(map)
                    })
                    .collect();

                stack.push(Value::List(tools))?;
                Ok((stack, ctx))
            })
        }),
    );

    // mcp-call: (text text map -- map)
    let conns_clone = conns.clone();
    ctx.dict.write().await.register(
        Tool::native("mcp-call", "(text text map -- map)", move |mut stack: Stack, ctx: Context| {
            let conns = conns_clone.clone();
            Box::pin(async move {
                let args = stack.pop()?.into_map()?;
                let name_str = stack.pop()?.into_text()?;
                let handle_str = stack.pop()?.into_text()?;

                // Verify connection exists
                {
                    let conns_read = conns.read().await;
                    if !conns_read.contains_key(&handle_str) {
                        return Err(kore::Error::Runtime(format!("MCP connection not found: {}", handle_str)));
                    }
                }

                // Mock tool call - in real implementation would send JSON-RPC request
                let result: IndexMap<String, Value> = match name_str.as_str() {
                    "echo" => {
                        let message = args.get("message")
                            .and_then(|v| if let Value::Text(t) = v { Some(t.as_str()) } else { None })
                            .unwrap_or("");
                        let mut content_item: IndexMap<String, Value> = IndexMap::new();
                        content_item.insert("type".to_string(), Value::Text("text".to_string()));
                        content_item.insert("text".to_string(), Value::Text(message.to_string()));
                        let mut result: IndexMap<String, Value> = IndexMap::new();
                        result.insert("content".to_string(), Value::List(vec![Value::Map(content_item)]));
                        result
                    }
                    "time" => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let mut content_item: IndexMap<String, Value> = IndexMap::new();
                        content_item.insert("type".to_string(), Value::Text("text".to_string()));
                        content_item.insert("text".to_string(), Value::Text(format!("Current time: {}", now)));
                        let mut result: IndexMap<String, Value> = IndexMap::new();
                        result.insert("content".to_string(), Value::List(vec![Value::Map(content_item)]));
                        result
                    }
                    _ => return Err(kore::Error::Runtime(format!("Unknown tool: {}", name_str))),
                };

                stack.push(Value::Map(result))?;
                Ok((stack, ctx))
            })
        }),
    );

    // mcp-close: (text --)
    let conns_clone = conns.clone();
    ctx.dict.write().await.register(
        Tool::native("mcp-close", "(text --)", move |mut stack: Stack, ctx: Context| {
            let conns = conns_clone.clone();
            Box::pin(async move {
                let handle_str = stack.pop()?.into_text()?;
                conns.write().await.remove(&handle_str);
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_ctx() -> Context {
        // Clear connections for each test
        *connections().write().await = HashMap::new();

        let mut ctx = Context::new();
        register_client_tools(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn test_client_tools_registered() {
        let ctx = test_ctx().await;
        let names = { let dict = ctx.dict.read().await; dict.list(&ctx.tenant) };
        assert!(names.contains(&"mcp-connect".to_string()));
        assert!(names.contains(&"mcp-list".to_string()));
        assert!(names.contains(&"mcp-call".to_string()));
        assert!(names.contains(&"mcp-close".to_string()));
    }
}
