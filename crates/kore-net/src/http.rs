//! # HTTP Client
//!
//! HTTP request tools for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `http-get` | `(url -- response)` | GET request |
//! | `http-post` | `(url body -- response)` | POST request with JSON body |
//! | `http-request` | `(config -- response)` | Full HTTP request from config |
//!
//! ## Example
//!
//! ```kore
//! "https://api.example.com/users" http-get
//! -- response on stack: { "status": 200, "headers": {...}, "body": {...} }
//!
//! "https://api.example.com/users" { "name": "Alice" } http-post
//! ```
//!
//! ## Response Format
//!
//! All HTTP tools return a Map:
//! - `status`: HTTP status code (Int)
//! - `headers`: Map of header name to value
//! - `body`: Parsed JSON or text string

use indexmap::IndexMap;
use kore::{Context, Stack, Tool, Value};
use reqwest::{header::HeaderMap, Client, Method};
use std::sync::OnceLock;

/// Shared HTTP client.
static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

fn client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// Convert headers to Value::Map.
fn headers_to_value(headers: &HeaderMap) -> Value {
    let mut map = IndexMap::new();
    for (key, value) in headers {
        if let Ok(v) = value.to_str() {
            map.insert(key.as_str().to_string(), Value::Text(v.to_string()));
        }
    }
    Value::Map(map)
}

/// Build response Value from reqwest response.
async fn response_to_value(response: reqwest::Response) -> kore::Result<Value> {
    let status = response.status().as_u16() as i64;
    let headers = headers_to_value(response.headers());

    let body_text = response
        .text()
        .await
        .map_err(|e| kore::Error::Runtime(format!("Failed to read response body: {}", e)))?;

    // Try to parse as JSON, fall back to string
    let body: Value = serde_json::from_str(&body_text)
        .unwrap_or_else(|_| Value::Text(body_text));

    let mut result = IndexMap::new();
    result.insert("status".to_string(), Value::Int(status));
    result.insert("headers".to_string(), headers);
    result.insert("body".to_string(), body);

    Ok(Value::Map(result))
}

/// Register all HTTP tools into a context.
pub async fn register_http_tools(ctx: &mut Context) {
    // http-get: (url -- response)
    ctx.dict.write().await.register(
        Tool::native("http-get", "(text -- map)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let url = stack.pop()?.into_text()?;

                let response = client()
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| kore::Error::Runtime(format!("HTTP GET failed: {}", e)))?;

                let result = response_to_value(response).await?;
                stack.push(result)?;

                Ok((stack, ctx))
            })
        }).with_doc("Perform an HTTP GET request. Returns map with status, headers, body."),
    );

    // http-post: (url body -- response)
    ctx.dict.write().await.register(
        Tool::native("http-post", "(text any -- map)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let body = stack.pop()?;
                let url = stack.pop()?.into_text()?;

                // Convert body to JSON
                let body_json = serde_json::to_value(&body)
                    .map_err(|e| kore::Error::Runtime(format!("Failed to serialize body: {}", e)))?;

                let response = client()
                    .post(&url)
                    .json(&body_json)
                    .send()
                    .await
                    .map_err(|e| kore::Error::Runtime(format!("HTTP POST failed: {}", e)))?;

                let result = response_to_value(response).await?;
                stack.push(result)?;

                Ok((stack, ctx))
            })
        }).with_doc("Perform an HTTP POST request with JSON body."),
    );

    // http-request: (config -- response)
    ctx.dict.write().await.register(
        Tool::native("http-request", "(map -- map)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let config = stack.pop()?.into_map()?;

                // Extract method (default GET)
                let method_str = config
                    .get("method")
                    .map(|v| v.clone().into_text().unwrap_or_else(|_| "GET".to_string()))
                    .unwrap_or_else(|| "GET".to_string());

                let method = match method_str.to_uppercase().as_str() {
                    "GET" => Method::GET,
                    "POST" => Method::POST,
                    "PUT" => Method::PUT,
                    "DELETE" => Method::DELETE,
                    "PATCH" => Method::PATCH,
                    "HEAD" => Method::HEAD,
                    "OPTIONS" => Method::OPTIONS,
                    _ => return Err(kore::Error::Runtime(format!("Unknown HTTP method: {}", method_str))),
                };

                // Extract URL (required)
                let url = config
                    .get("url")
                    .ok_or_else(|| kore::Error::Runtime("Config must have 'url' field".into()))?
                    .clone()
                    .into_text()?;

                // Build request
                let mut request = client().request(method, &url);

                // Add headers if present
                if let Some(headers_val) = config.get("headers") {
                    if let Ok(headers) = headers_val.clone().into_map() {
                        for (key, value) in headers {
                            if let Ok(v) = value.into_text() {
                                request = request.header(&key, v);
                            }
                        }
                    }
                }

                // Add body if present
                if let Some(body_val) = config.get("body") {
                    let body_json = serde_json::to_value(body_val)
                        .map_err(|e| kore::Error::Runtime(format!("Failed to serialize body: {}", e)))?;
                    request = request.json(&body_json);
                }

                let response = request
                    .send()
                    .await
                    .map_err(|e| kore::Error::Runtime(format!("HTTP request failed: {}", e)))?;

                let result = response_to_value(response).await?;
                stack.push(result)?;

                Ok((stack, ctx))
            })
        }).with_doc("Perform a full HTTP request from config object with method, url, headers, body."),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_tools_registered() {
        let mut ctx = Context::new();
        register_http_tools(&mut ctx).await;

        let names = {
            let dict = ctx.dict.read().await;
            dict.list(&ctx.tenant)
        };

        assert!(names.contains(&"http-get".to_string()));
        assert!(names.contains(&"http-post".to_string()));
        assert!(names.contains(&"http-request".to_string()));
    }
}
