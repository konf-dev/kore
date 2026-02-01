//! # LLM Tools
//!
//! Language model interaction for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `llm-chat` | `(messages config -- response)` | Chat completion |
//! | `llm-complete` | `(prompt config -- text)` | Text completion |
//! | `ai-config` | `(config --)` | Configure AI provider |
//!
//! ## Example
//!
//! ```kore
//! { "provider": "openai", "api_key": "sk-...", "model": "gpt-4" } ai-config
//!
//! [
//!   { "role": "system", "content": "You are a helpful assistant." },
//!   { "role": "user", "content": "Hello!" }
//! ] {} llm-chat
//! -- { "role": "assistant", "content": "Hello! How can I help?" }
//! ```
//!
//! ## Providers
//!
//! - `openai`: OpenAI API (GPT models)
//! - `anthropic`: Anthropic API (Claude models)
//! - `mock`: Mock provider for testing

use indexmap::IndexMap;
use kore::{Context, Stack, Tool, Value};
use reqwest::Client;
use serde_json::json;
use std::sync::{Arc, RwLock};

/// AI provider configuration.
#[derive(Debug, Clone, Default)]
pub struct AiConfig {
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
}

/// Global AI configuration.
static AI_CONFIG: std::sync::OnceLock<Arc<RwLock<AiConfig>>> = std::sync::OnceLock::new();

pub fn config() -> Arc<RwLock<AiConfig>> {
    AI_CONFIG
        .get_or_init(|| Arc::new(RwLock::new(AiConfig::default())))
        .clone()
}

/// Shared HTTP client.
static HTTP_CLIENT: std::sync::OnceLock<Client> = std::sync::OnceLock::new();

fn client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// Register all LLM tools into a context.
pub async fn register_llm_tools(ctx: &mut Context) {
    // ai-config: (map --)
    ctx.dict.write().await.register(
        Tool::native("ai-config", "(map --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let cfg_map = stack.pop()?.into_map()?;

                let provider = cfg_map
                    .get("provider")
                    .and_then(|v| v.text_opt())
                    .unwrap_or("mock")
                    .to_string();

                let api_key = cfg_map
                    .get("api_key")
                    .and_then(|v| v.text_opt())
                    .unwrap_or("")
                    .to_string();

                let model = cfg_map
                    .get("model")
                    .and_then(|v| v.text_opt())
                    .unwrap_or("")
                    .to_string();

                let base_url = cfg_map
                    .get("base_url")
                    .and_then(|v| v.text_opt())
                    .map(|s| s.to_string());

                *config().write().unwrap() = AiConfig {
                    provider,
                    api_key,
                    model,
                    base_url,
                };

                Ok((stack, ctx))
            })
        }),
    );

    // llm-chat: (list map -- map)
    ctx.dict.write().await.register(
        Tool::native("llm-chat", "(list map -- map)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let options = stack.pop()?.into_map()?;
                let messages = stack.pop()?.into_list()?;

                let cfg = config().read().unwrap().clone();

                let response = match cfg.provider.as_str() {
                    "openai" => openai_chat(&cfg, &messages, &options).await,
                    "anthropic" => anthropic_chat(&cfg, &messages, &options).await,
                    "mock" | "" => mock_chat(&messages),
                    _ => Err(kore::Error::Runtime(format!("Unknown provider: {}", cfg.provider))),
                }?;

                stack.push(response)?;
                Ok((stack, ctx))
            })
        }),
    );

    // llm-complete: (text map -- text)
    ctx.dict.write().await.register(
        Tool::native("llm-complete", "(text map -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let options = stack.pop()?.into_map()?;
                let prompt = stack.pop()?.into_text()?;

                let cfg = config().read().unwrap().clone();

                // Convert prompt to messages format for chat models
                let mut msg = IndexMap::new();
                msg.insert("role".to_string(), Value::Text("user".to_string()));
                msg.insert("content".to_string(), Value::Text(prompt));
                let messages = vec![Value::Map(msg)];

                let response = match cfg.provider.as_str() {
                    "openai" => {
                        let result = openai_chat(&cfg, &messages, &options).await?;
                        extract_content(&result)
                    }
                    "anthropic" => {
                        let result = anthropic_chat(&cfg, &messages, &options).await?;
                        extract_content(&result)
                    }
                    "mock" | "" => {
                        let result = mock_chat(&messages)?;
                        extract_content(&result)
                    }
                    _ => Err(kore::Error::Runtime(format!("Unknown provider: {}", cfg.provider))),
                }?;

                stack.push(Value::Text(response))?;
                Ok((stack, ctx))
            })
        }),
    );
}

/// Extract content from a response map.
fn extract_content(response: &Value) -> Result<String, kore::Error> {
    match response {
        Value::Map(m) => {
            m.get("content")
                .and_then(|v| v.text_opt())
                .map(|s| s.to_string())
                .ok_or_else(|| kore::Error::Runtime("Missing content in response".to_string()))
        }
        _ => Err(kore::Error::Runtime("Response is not a map".to_string())),
    }
}

/// Convert Value messages to serde_json for API calls.
fn messages_to_json(messages: &[Value]) -> serde_json::Value {
    let arr: Vec<serde_json::Value> = messages
        .iter()
        .filter_map(|v| {
            if let Value::Map(m) = v {
                let role = m.get("role").and_then(|v| v.text_opt()).unwrap_or("user");
                let content = m.get("content").and_then(|v| v.text_opt()).unwrap_or("");
                Some(json!({
                    "role": role,
                    "content": content
                }))
            } else {
                None
            }
        })
        .collect();
    serde_json::Value::Array(arr)
}

/// Convert IndexMap options to serde_json.
fn options_to_json(options: &IndexMap<String, Value>) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for (k, v) in options {
        obj.insert(k.clone(), value_to_json(v));
    }
    serde_json::Value::Object(obj)
}

/// Convert a Value to serde_json::Value.
fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::Text(s) => serde_json::Value::String(s.clone()),
        Value::List(arr) => serde_json::Value::Array(arr.iter().map(value_to_json).collect()),
        Value::Map(m) => {
            let obj: serde_json::Map<String, serde_json::Value> = m
                .iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
        _ => serde_json::Value::Null,
    }
}

/// Convert serde_json::Value to a Value map.
fn json_to_value_map(j: &serde_json::Value) -> Value {
    match j {
        serde_json::Value::Object(obj) => {
            let mut m = IndexMap::new();
            for (k, v) in obj {
                m.insert(k.clone(), json_to_value(v));
            }
            Value::Map(m)
        }
        _ => Value::Null,
    }
}

/// Convert serde_json::Value to Value.
fn json_to_value(j: &serde_json::Value) -> Value {
    match j {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::Text(s.clone()),
        serde_json::Value::Array(arr) => Value::List(arr.iter().map(json_to_value).collect()),
        serde_json::Value::Object(obj) => {
            let mut m = IndexMap::new();
            for (k, v) in obj {
                m.insert(k.clone(), json_to_value(v));
            }
            Value::Map(m)
        }
    }
}

async fn openai_chat(
    cfg: &AiConfig,
    messages: &[Value],
    options: &IndexMap<String, Value>,
) -> Result<Value, kore::Error> {
    let base_url = cfg
        .base_url
        .clone()
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

    let model = if cfg.model.is_empty() {
        "gpt-4"
    } else {
        &cfg.model
    };

    let messages_json = messages_to_json(messages);
    let options_json = options_to_json(options);

    let mut body = json!({
        "model": model,
        "messages": messages_json
    });

    // Merge options
    if let Some(obj) = options_json.as_object() {
        for (k, v) in obj {
            body[k] = v.clone();
        }
    }

    let response = client()
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", cfg.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| kore::Error::Runtime(format!("OpenAI request failed: {}", e)))?;

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| kore::Error::Runtime(format!("Failed to parse response: {}", e)))?;

    if let Some(error) = response_json.get("error") {
        return Err(kore::Error::Runtime(format!("OpenAI error: {}", error)));
    }

    let message = &response_json["choices"][0]["message"];
    Ok(json_to_value_map(message))
}

async fn anthropic_chat(
    cfg: &AiConfig,
    messages: &[Value],
    options: &IndexMap<String, Value>,
) -> Result<Value, kore::Error> {
    let base_url = cfg
        .base_url
        .clone()
        .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string());

    let model = if cfg.model.is_empty() {
        "claude-3-sonnet-20240229"
    } else {
        &cfg.model
    };

    // Convert messages to Anthropic format
    let mut system_content = String::new();
    let mut anthropic_messages: Vec<serde_json::Value> = Vec::new();

    for msg in messages {
        if let Value::Map(m) = msg {
            let role = m.get("role").and_then(|v| v.text_opt()).unwrap_or("user");
            let content = m.get("content").and_then(|v| v.text_opt()).unwrap_or("");

            if role == "system" {
                system_content = content.to_string();
            } else {
                anthropic_messages.push(json!({
                    "role": role,
                    "content": content
                }));
            }
        }
    }

    let options_json = options_to_json(options);

    let mut body = json!({
        "model": model,
        "messages": anthropic_messages,
        "max_tokens": 4096
    });

    if !system_content.is_empty() {
        body["system"] = json!(system_content);
    }

    // Merge options
    if let Some(obj) = options_json.as_object() {
        for (k, v) in obj {
            body[k] = v.clone();
        }
    }

    let response = client()
        .post(format!("{}/messages", base_url))
        .header("x-api-key", &cfg.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| kore::Error::Runtime(format!("Anthropic request failed: {}", e)))?;

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| kore::Error::Runtime(format!("Failed to parse response: {}", e)))?;

    if let Some(error) = response_json.get("error") {
        return Err(kore::Error::Runtime(format!("Anthropic error: {}", error)));
    }

    let content = response_json["content"][0]["text"]
        .as_str()
        .unwrap_or("");

    let mut result = IndexMap::new();
    result.insert("role".to_string(), Value::Text("assistant".to_string()));
    result.insert("content".to_string(), Value::Text(content.to_string()));
    Ok(Value::Map(result))
}

fn mock_chat(messages: &[Value]) -> Result<Value, kore::Error> {
    // Return a mock response based on the last message
    let last_content = messages
        .last()
        .and_then(|v| {
            if let Value::Map(m) = v {
                m.get("content").and_then(|c| c.text_opt())
            } else {
                None
            }
        })
        .unwrap_or("Hello");

    let mut result = IndexMap::new();
    result.insert("role".to_string(), Value::Text("assistant".to_string()));
    result.insert(
        "content".to_string(),
        Value::Text(format!("Mock response to: {}", last_content)),
    );
    Ok(Value::Map(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kore::tool::ToolBody;

    /// Helper to execute a tool by name from context
    async fn exec_tool(
        name: &str,
        stack: Stack,
        ctx: &Context,
    ) -> kore::Result<(Stack, Context)> {
        let dict = ctx.dict.read().await;
        let tool = dict.get(name, &ctx.tenant)?;
        drop(dict);

        match &tool.body {
            ToolBody::Native(native_fn) => native_fn.call(stack, ctx.clone()).await,
            ToolBody::Ops(ops) => kore::execute(ops, stack, ctx.clone()).await,
        }
    }

    #[tokio::test]
    async fn test_ai_config() {
        let mut ctx = Context::new();
        register_llm_tools(&mut ctx).await;

        // Reset config
        *config().write().unwrap() = AiConfig::default();

        let mut cfg_map = IndexMap::new();
        cfg_map.insert("provider".to_string(), Value::Text("mock".to_string()));
        cfg_map.insert("api_key".to_string(), Value::Text("test-key".to_string()));
        cfg_map.insert("model".to_string(), Value::Text("test-model".to_string()));

        let mut stack = Stack::new();
        stack.push(Value::Map(cfg_map)).unwrap();

        let (_, _) = exec_tool("ai-config", stack, &ctx).await.unwrap();

        let binding = config();
        let cfg = binding.read().unwrap();
        assert_eq!(cfg.provider, "mock");
        assert_eq!(cfg.api_key, "test-key");
        assert_eq!(cfg.model, "test-model");
    }

    #[tokio::test]
    async fn test_mock_llm_chat() {
        let mut ctx = Context::new();
        register_llm_tools(&mut ctx).await;

        // Configure mock provider
        *config().write().unwrap() = AiConfig {
            provider: "mock".to_string(),
            ..Default::default()
        };

        // Build messages
        let mut msg = IndexMap::new();
        msg.insert("role".to_string(), Value::Text("user".to_string()));
        msg.insert("content".to_string(), Value::Text("Hello!".to_string()));

        let mut stack = Stack::new();
        stack.push(Value::List(vec![Value::Map(msg)])).unwrap();
        stack.push(Value::Map(IndexMap::new())).unwrap();

        let (mut stack, _) = exec_tool("llm-chat", stack, &ctx).await.unwrap();

        let result = stack.pop().unwrap();
        if let Value::Map(m) = result {
            assert_eq!(m.get("role").and_then(|v| v.text_opt()), Some("assistant"));
            assert!(m.get("content").and_then(|v| v.text_opt()).unwrap().contains("Mock response"));
        } else {
            panic!("Expected Map result");
        }
    }

    #[tokio::test]
    async fn test_mock_llm_complete() {
        let mut ctx = Context::new();
        register_llm_tools(&mut ctx).await;

        // Configure mock provider
        *config().write().unwrap() = AiConfig {
            provider: "mock".to_string(),
            ..Default::default()
        };

        let mut stack = Stack::new();
        stack.push(Value::Text("What is 2+2?".to_string())).unwrap();
        stack.push(Value::Map(IndexMap::new())).unwrap();

        let (mut stack, _) = exec_tool("llm-complete", stack, &ctx).await.unwrap();

        let result = stack.pop().unwrap();
        assert!(matches!(result, Value::Text(_)));
    }
}
