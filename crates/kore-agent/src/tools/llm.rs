//! LLM tools: llm, llm-system
//!
//! STATELESS: These tools have no memory of previous calls.
//! The workflow must provide all context.

use kore::{Context, Stack, Tool, Value};

/// llm: (prompt -- response)
pub fn llm_tool() -> Tool {
    Tool::native("llm", "(prompt:Text -- response:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let prompt = stack.pop()?.as_text()?.to_string();
            
            let response = call_llm("", &prompt).await
                .map_err(|e| kore::Error::Runtime(e))?;
            
            stack.push(Value::Text(response))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Call LLM with a prompt. STATELESS - no memory of previous calls.")
}

/// llm-system: (system user -- response)
pub fn llm_system_tool() -> Tool {
    Tool::native("llm-system", "(system:Text user:Text -- response:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let user = stack.pop()?.as_text()?.to_string();
            let system = stack.pop()?.as_text()?.to_string();
            
            let response = call_llm(&system, &user).await
                .map_err(|e| kore::Error::Runtime(e))?;
            
            stack.push(Value::Text(response))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Call LLM with system and user prompts. STATELESS - no memory of previous calls.")
}

/// Call LLM - auto-detects OpenAI vs Anthropic based on env vars
pub async fn call_llm(system: &str, user: &str) -> Result<String, String> {
    // Check for OpenAI first (including custom base URL)
    if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
        if !api_key.is_empty() {
            let base_url = std::env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com".to_string());
            return call_openai(&base_url, &api_key, system, user).await;
        }
    }
    
    // Fall back to Anthropic
    if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
        if !api_key.is_empty() {
            return call_anthropic(&api_key, system, user).await;
        }
    }
    
    Err("No LLM API key set. Set OPENAI_API_KEY or ANTHROPIC_API_KEY".into())
}

async fn call_openai(base_url: &str, api_key: &str, system: &str, user: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    
    let mut messages = Vec::new();
    if !system.is_empty() {
        messages.push(serde_json::json!({"role": "system", "content": system}));
    }
    messages.push(serde_json::json!({"role": "user", "content": user}));
    
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
    
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "messages": messages
    });
    
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error {}: {}", status, text));
    }
    
    let json: serde_json::Value = response.json().await
        .map_err(|e| e.to_string())?;
    
    json["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("No content in response: {:?}", json))
}

async fn call_anthropic(api_key: &str, system: &str, user: &str) -> Result<String, String> {
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
        return Err(format!("Anthropic API error {}: {}", status, text));
    }
    
    let json: serde_json::Value = response.json().await
        .map_err(|e| e.to_string())?;
    
    json["content"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No text in response".to_string())
}
