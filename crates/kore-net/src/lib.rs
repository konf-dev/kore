//! # Kore Network
//!
//! Network tools for Kore programs.
//!
//! ## Purpose
//!
//! Agents need to communicate with external services:
//! - HTTP APIs (REST, GraphQL)
//! - WebSocket connections (real-time data)
//! - DNS resolution (service discovery)
//!
//! ## Modules
//!
//! | Module | Purpose | Key Tools |
//! |--------|---------|-----------|
//! | [`http`] | HTTP client | `http-get`, `http-post`, `http-request` |
//! | [`websocket`] | WebSocket client | `ws-connect`, `ws-send`, `ws-recv`, `ws-close` |
//! | [`dns`] | DNS resolution | `dns-lookup`, `dns-reverse` |
//!
//! ## Capabilities
//!
//! All network operations require capabilities:
//! - `net:http` for HTTP requests
//! - `net:ws` for WebSocket connections
//! - `net:dns` for DNS operations

pub mod dns;
pub mod http;
pub mod websocket;

pub use dns::register_dns_tools;
pub use http::register_http_tools;
pub use websocket::register_websocket_tools;

use kore::Context;

/// Register all network tools into a context.
pub async fn register_all(ctx: &mut Context) {
    register_http_tools(ctx).await;
    register_websocket_tools(ctx).await;
    register_dns_tools(ctx).await;
}
