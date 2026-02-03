//! HTTP Capability Tools
//!
//! Tools for HTTP requests. Requires net:* capability.
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | http-get | (url -- response) | GET request |
//! | http-post | (url body headers -- response) | POST request |
//! | http-request | (method url body headers -- response) | Generic request |
//!
//! Response is a Map with: status (Int), body (Text), headers (Map)

use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;
use indexmap::IndexMap;

/// Register HTTP tools (3)
pub fn register(dict: &mut Dictionary) {
    dict.register(Tool::native(
        "http-get",
        "(url -- response)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let url = stack.pop()?.into_text()?;
                
                let client = reqwest::Client::new();
                let response = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| crate::error::Error::io(format!("http-get '{}': {}", url, e)))?;
                
                let status = response.status().as_u16() as i64;
                let headers = response.headers().clone();
                let body = response
                    .text()
                    .await
                    .map_err(|e| crate::error::Error::io(format!("http-get body: {}", e)))?;
                
                stack.push(build_response(status, body, &headers))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "http-post",
        "(url body headers -- response)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let headers_map = stack.pop()?.into_map()?;
                let body = stack.pop()?.into_text()?;
                let url = stack.pop()?.into_text()?;
                
                let client = reqwest::Client::new();
                let mut request = client.post(&url).body(body);
                
                for (k, v) in headers_map.iter() {
                    if let Value::Text(v_str) = v {
                        request = request.header(k.as_str(), v_str.as_str());
                    }
                }
                
                let response = request
                    .send()
                    .await
                    .map_err(|e| crate::error::Error::io(format!("http-post '{}': {}", url, e)))?;
                
                let status = response.status().as_u16() as i64;
                let headers = response.headers().clone();
                let body = response
                    .text()
                    .await
                    .map_err(|e| crate::error::Error::io(format!("http-post body: {}", e)))?;
                
                stack.push(build_response(status, body, &headers))?;
                Ok((stack, ctx))
            })
        },
    ));

    dict.register(Tool::native(
        "http-request",
        "(method url body headers -- response)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let headers_map = stack.pop()?.into_map()?;
                let body = stack.pop()?.into_text()?;
                let url = stack.pop()?.into_text()?;
                let method = stack.pop()?.into_text()?;
                
                let method_enum = match method.to_uppercase().as_str() {
                    "GET" => reqwest::Method::GET,
                    "POST" => reqwest::Method::POST,
                    "PUT" => reqwest::Method::PUT,
                    "DELETE" => reqwest::Method::DELETE,
                    "PATCH" => reqwest::Method::PATCH,
                    "HEAD" => reqwest::Method::HEAD,
                    "OPTIONS" => reqwest::Method::OPTIONS,
                    _ => {
                        return Err(crate::error::Error::Runtime(format!(
                            "http-request: unknown method '{}'. Valid: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS",
                            method
                        )))
                    }
                };
                
                let client = reqwest::Client::new();
                let mut request = client.request(method_enum, &url).body(body);
                
                for (k, v) in headers_map.iter() {
                    if let Value::Text(v_str) = v {
                        request = request.header(k.as_str(), v_str.as_str());
                    }
                }
                
                let response = request
                    .send()
                    .await
                    .map_err(|e| crate::error::Error::io(format!("http-request '{}': {}", url, e)))?;
                
                let status = response.status().as_u16() as i64;
                let headers = response.headers().clone();
                let body = response
                    .text()
                    .await
                    .map_err(|e| crate::error::Error::io(format!("http-request body: {}", e)))?;
                
                stack.push(build_response(status, body, &headers))?;
                Ok((stack, ctx))
            })
        },
    ));
}

/// Build a response map from HTTP response components
fn build_response(status: i64, body: String, headers: &reqwest::header::HeaderMap) -> Value {
    let mut result = IndexMap::new();
    result.insert("status".to_string(), Value::Int(status));
    result.insert("body".to_string(), Value::Text(body));
    
    let mut header_map = IndexMap::new();
    for (k, v) in headers.iter() {
        if let Ok(v_str) = v.to_str() {
            header_map.insert(k.to_string(), Value::Text(v_str.to_string()));
        }
    }
    result.insert("headers".to_string(), Value::Map(header_map));
    
    Value::Map(result)
}
