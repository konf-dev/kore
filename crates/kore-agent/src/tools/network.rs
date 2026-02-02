//! Network tools: http-get, http-post

use kore::{Context, Stack, Tool, Value};
use std::collections::HashMap;

/// http-get: (url -- response)
pub fn http_get_tool() -> Tool {
    Tool::native("http-get", "(url:Text -- response:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let url = stack.pop()?.as_text()?.to_string();
            
            let client = reqwest::Client::new();
            let response = client.get(&url).send().await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            let status = response.status().as_u16() as i64;
            let body = response.text().await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            let result = Value::Map(indexmap::indexmap! {
                "status".into() => Value::Int(status),
                "body".into() => Value::Text(body),
            });
            
            stack.push(result)?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Make an HTTP GET request. Returns {status, body}.")
}

/// http-post: (url body headers -- response)
pub fn http_post_tool() -> Tool {
    Tool::native("http-post", "(url:Text body:Text headers:Map -- response:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let headers_val = stack.pop()?;
            let body = stack.pop()?.as_text()?.to_string();
            let url = stack.pop()?.as_text()?.to_string();
            
            let headers: HashMap<String, String> = match headers_val {
                Value::Map(m) => {
                    m.into_iter()
                        .filter_map(|(k, v)| v.as_text().ok().map(|v| (k, v.to_string())))
                        .collect()
                }
                _ => HashMap::new(),
            };
            
            let client = reqwest::Client::new();
            let mut req = client.post(&url).body(body);
            
            for (k, v) in headers {
                req = req.header(&k, &v);
            }
            
            let response = req.send().await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            let status = response.status().as_u16() as i64;
            let body = response.text().await
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            
            let result = Value::Map(indexmap::indexmap! {
                "status".into() => Value::Int(status),
                "body".into() => Value::Text(body),
            });
            
            stack.push(result)?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Make an HTTP POST request. Returns {status, body}.")
}
