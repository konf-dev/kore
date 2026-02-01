//! Core tools for the autonomous agent
//!
//! Minimal set needed to bootstrap an LLM-driven agent:
//! - env-get: Read environment variables (API keys)
//! - llm: Call an LLM API
//! - http-post: Make HTTP requests
//! - parse: Parse kore code from text
//! - print: Output to user

use kore::{Context, Stack, Tool, Value, Op};
use std::collections::HashMap;

/// Create env-get tool: (key -- value)
pub fn env_get_tool() -> Tool {
    Tool::native("env-get", "(key:Text -- value:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let val = stack.pop()?;
            let key = val.as_text()?;
            let value = std::env::var(key).unwrap_or_default();
            stack.push(Value::Text(value))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Read an environment variable. Returns empty string if not set.")
}

/// Create print tool: (value --)
pub fn print_tool() -> Tool {
    Tool::native("print", "(value:Any --)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?;
            println!("{}", format_value(&value));
            Ok((stack, ctx))
        })
    })
    .with_doc("Print a value to stdout.")
}

/// Create print-stack tool: (--)
pub fn print_stack_tool() -> Tool {
    Tool::native("print-stack", "(--)", |stack: Stack, ctx: Context| {
        Box::pin(async move {
            println!("Stack: {:?}", stack.values());
            Ok((stack, ctx))
        })
    })
    .with_doc("Print the current stack for debugging.")
}

/// Create llm tool: (prompt -- response)
/// Uses Anthropic Claude API by default
pub fn llm_tool() -> Tool {
    Tool::native("llm", "(prompt:Text -- response:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let val = stack.pop()?;
            let prompt = val.as_text()?.to_string();
            
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| kore::Error::Runtime("ANTHROPIC_API_KEY not set".into()))?;
            
            let response = call_claude(&api_key, &prompt).await
                .map_err(|e| kore::Error::Runtime(e))?;
            
            stack.push(Value::Text(response))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Call Claude API with a prompt. Returns the response text.")
}

/// Create llm-system tool: (system user -- response)
/// Allows setting system prompt separately
pub fn llm_system_tool() -> Tool {
    Tool::native("llm-system", "(system:Text user:Text -- response:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let user_val = stack.pop()?;
            let system_val = stack.pop()?;
            let user = user_val.as_text()?.to_string();
            let system = system_val.as_text()?.to_string();
            
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| kore::Error::Runtime("ANTHROPIC_API_KEY not set".into()))?;
            
            let response = call_claude_with_system(&api_key, &system, &user).await
                .map_err(|e| kore::Error::Runtime(e))?;
            
            stack.push(Value::Text(response))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Call Claude API with system and user prompts.")
}

/// Create http-post tool: (url body headers -- response)
pub fn http_post_tool() -> Tool {
    Tool::native("http-post", "(url:Text body:Text headers:Map -- response:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let headers_val = stack.pop()?;
            let body_val = stack.pop()?;
            let url_val = stack.pop()?;
            
            let url = url_val.as_text()?.to_string();
            let body = body_val.as_text()?.to_string();
            
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

/// Create http-get tool: (url -- response)
pub fn http_get_tool() -> Tool {
    Tool::native("http-get", "(url:Text -- response:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let url_val = stack.pop()?;
            let url = url_val.as_text()?.to_string();
            
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

/// Create parse tool: (text -- quote)
/// Parses kore code from text into executable quote
pub fn parse_tool() -> Tool {
    Tool::native("parse", "(code:Text -- quote:Quote)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let code_val = stack.pop()?;
            let code = code_val.as_text()?.to_string();
            
            let ops = Op::parse(&code)
                .map_err(|e| kore::Error::Runtime(format!("Parse error: {}", e)))?;
            
            stack.push(Value::Quote(ops))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Parse kore code text into an executable quote.")
}

/// Create read-line tool: (-- text)
pub fn read_line_tool() -> Tool {
    Tool::native("read-line", "(-- input:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)
                .map_err(|e| kore::Error::Runtime(e.to_string()))?;
            stack.push(Value::Text(input.trim().to_string()))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Read a line from stdin.")
}

/// Get all agent tools
pub fn all_tools() -> Vec<Tool> {
    vec![
        env_get_tool(),
        print_tool(),
        print_stack_tool(),
        llm_tool(),
        llm_system_tool(),
        http_get_tool(),
        http_post_tool(),
        parse_tool(),
        read_line_tool(),
    ]
}

/// Register all agent tools into a context
pub async fn register_all(ctx: &mut Context) {
    let mut dict = ctx.dict.write().await;
    for tool in all_tools() {
        dict.register(tool);
    }
}

// === Internal helpers ===

fn format_value(value: &Value) -> String {
    match value {
        Value::Text(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::List(l) => format!("{:?}", l),
        Value::Map(m) => serde_json::to_string(m).unwrap_or_else(|_| format!("{:?}", m)),
        Value::Quote(ops) => format!("({})", ops.iter().map(|o| format!("{:?}", o)).collect::<Vec<_>>().join(" ")),
        Value::Handle(h) => format!("<handle:{}>", h.id),
        Value::Error(e) => format!("Error[{}]: {}", e.code, e.message),
    }
}

async fn call_claude(api_key: &str, prompt: &str) -> Result<String, String> {
    call_claude_with_system(api_key, "", prompt).await
}

async fn call_claude_with_system(api_key: &str, system: &str, user: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    
    let mut body = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 4096,
        "messages": [
            {"role": "user", "content": user}
        ]
    });
    
    if !system.is_empty() {
        body["system"] = serde_json::Value::String(system.to_string());
    }
    
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, text));
    }
    
    let json: serde_json::Value = response.json().await
        .map_err(|e| e.to_string())?;
    
    // Extract text from response
    json["content"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No text in response".to_string())
}
