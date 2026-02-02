//! Helper tools: now, uuid, json-parse, json-format, get

use kore::{Context, Stack, Tool, Value};

/// get: (map key -- value)
pub fn get_tool() -> Tool {
    Tool::native("get", "(map:Map key:Text -- value:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let key = stack.pop()?.as_text()?.to_string();
            let map = stack.pop()?;
            
            match map {
                Value::Map(m) => {
                    let value = m.get(&key).cloned().unwrap_or(Value::Null);
                    stack.push(value)?;
                }
                _ => {
                    return Err(kore::Error::Runtime(format!("get: expected Map, got {:?}", map)));
                }
            }
            Ok((stack, ctx))
        })
    })
    .with_doc("Get a value from a map by key. Returns null if key not found.")
}

/// now: (-- timestamp)
pub fn now_tool() -> Tool {
    Tool::native("now", "(-- timestamp:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            stack.push(Value::Int(ts))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Get current Unix timestamp in milliseconds.")
}

/// uuid: (-- id)
pub fn uuid_tool() -> Tool {
    Tool::native("uuid", "(-- id:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let id = uuid::Uuid::new_v4().to_string();
            stack.push(Value::Text(id))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Generate a random UUID v4.")
}

/// json-parse: (text -- value)
pub fn json_parse_tool() -> Tool {
    Tool::native("json-parse", "(text:Text -- value:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let text = stack.pop()?.as_text()?.to_string();
            
            let json: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| kore::Error::Runtime(format!("json-parse: {}", e)))?;
            
            stack.push(json_to_value(json))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Parse JSON text into a value.")
}

/// json-format: (value -- text)
pub fn json_format_tool() -> Tool {
    Tool::native("json-format", "(value:Any -- text:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?;
            let json = value_to_json(&value);
            
            let text = serde_json::to_string_pretty(&json)
                .map_err(|e| kore::Error::Runtime(format!("json-format: {}", e)))?;
            
            stack.push(Value::Text(text))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Format value as pretty JSON text.")
}

// Helper: Convert Value to JSON
pub fn value_to_json(v: &Value) -> serde_json::Value {
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
pub fn json_to_value(j: serde_json::Value) -> Value {
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
