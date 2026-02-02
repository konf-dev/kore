//! Checkpoint tools: checkpoint, restore, list-checkpoints
//!
//! Save and restore agent state.

use kore::{Context, Stack, Tool, Value};
use crate::tools::checkpoint_path;

/// checkpoint: (name --)
pub fn checkpoint_tool() -> Tool {
    Tool::native("checkpoint", "(name:Text --)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let name = stack.pop()?.as_text()?.to_string();
            
            // Serialize stack values
            let stack_values: Vec<serde_json::Value> = stack.values()
                .iter()
                .map(value_to_json)
                .collect();
            
            let state = serde_json::json!({
                "stack": stack_values,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });
            
            // Save to checkpoint directory
            let cp_path = checkpoint_path(&name);
            tokio::fs::create_dir_all(&cp_path).await
                .map_err(|e| kore::Error::Runtime(format!("checkpoint: {}", e)))?;
            
            tokio::fs::write(
                cp_path.join("state.json"),
                serde_json::to_string_pretty(&state).unwrap()
            ).await
            .map_err(|e| kore::Error::Runtime(format!("checkpoint: {}", e)))?;
            
            Ok((stack, ctx))
        })
    })
    .with_doc("Save current stack state to named checkpoint.")
}

/// restore: (name --)
/// Note: This replaces the current stack with checkpoint state
pub fn restore_tool() -> Tool {
    Tool::native("restore", "(name:Text --)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let name = stack.pop()?.as_text()?.to_string();
            
            let cp_path = checkpoint_path(&name);
            let state_file = cp_path.join("state.json");
            
            let content = tokio::fs::read_to_string(&state_file).await
                .map_err(|e| kore::Error::Runtime(format!("restore '{}': {}", name, e)))?;
            
            let state: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| kore::Error::Runtime(format!("restore '{}': {}", name, e)))?;
            
            // Clear stack and restore values
            while stack.pop().is_ok() {}
            
            if let Some(values) = state["stack"].as_array() {
                for v in values {
                    stack.push(json_to_value(v.clone()))?;
                }
            }
            
            Ok((stack, ctx))
        })
    })
    .with_doc("Restore stack state from named checkpoint.")
}

/// list-checkpoints: (-- names)
pub fn list_checkpoints_tool() -> Tool {
    Tool::native("list-checkpoints", "(-- names:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let cp_base = std::env::var("KORE_CHECKPOINTS")
                .unwrap_or_else(|_| "./checkpoints".to_string());
            
            let mut names = Vec::new();
            
            if let Ok(mut dir) = tokio::fs::read_dir(&cp_base).await {
                while let Ok(Some(entry)) = dir.next_entry().await {
                    if entry.file_type().await.map(|ft| ft.is_dir()).unwrap_or(false) {
                        names.push(Value::Text(entry.file_name().to_string_lossy().to_string()));
                    }
                }
            }
            
            stack.push(Value::List(names))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("List available checkpoint names.")
}

// Helper: Convert Value to JSON
fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Int(i) => serde_json::json!(i),
        Value::Float(f) => serde_json::json!(f),
        Value::Text(s) => serde_json::json!(s),
        Value::Bool(b) => serde_json::json!(b),
        Value::Null => serde_json::Value::Null,
        Value::List(l) => serde_json::json!(l.iter().map(value_to_json).collect::<Vec<_>>()),
        Value::Map(m) => {
            let obj: serde_json::Map<String, serde_json::Value> = m.iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
        Value::Quote(_) => serde_json::json!("<quote>"),
        Value::Handle(h) => serde_json::json!(format!("<handle:{}>", h.id)),
        Value::Error(e) => serde_json::json!({"error": e.message}),
    }
}

// Helper: Convert JSON to Value
fn json_to_value(j: serde_json::Value) -> Value {
    match j {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Int(0)
            }
        }
        serde_json::Value::String(s) => Value::Text(s),
        serde_json::Value::Array(a) => Value::List(a.into_iter().map(json_to_value).collect()),
        serde_json::Value::Object(o) => {
            let map: indexmap::IndexMap<String, Value> = o.into_iter()
                .map(|(k, v)| (k, json_to_value(v)))
                .collect();
            Value::Map(map)
        }
    }
}
