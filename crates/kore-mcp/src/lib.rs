//! # Kore MCP
//!
//! Model Context Protocol (MCP) client for Kore programs.
//!
//! ## Purpose
//!
//! MCP enables agents to access external tools and resources:
//! - Connect to MCP servers
//! - List and call remote tools
//! - Access resources
//!
//! ## Modules
//!
//! | Module | Purpose | Key Tools |
//! |--------|---------|-----------|
//! | [`client`] | MCP client | `mcp-connect`, `mcp-call`, `mcp-list` |
//! | [`resource`] | Resource access | `mcp-resource`, `mcp-resources` |
//!
//! ## Example
//!
//! ```kore
//! "http://localhost:3000/mcp" mcp-connect  -- ( handle )
//! dup mcp-list  -- ( handle tools )
//! swap "tool-name" { "arg": "value" } mcp-call
//! ```

pub mod client;
pub mod resource;

pub use client::register_client_tools;
pub use resource::register_resource_tools;

use kore::Context;

/// Register all MCP tools into a context.
pub async fn register_all(ctx: &mut Context) {
    register_client_tools(ctx).await;
    register_resource_tools(ctx).await;
}
