//! # Handle Management
//!
//! Manages lifecycle of external resources (connections, files, streams).
//!
//! ## Purpose
//!
//! External resources need explicit lifecycle management:
//! - Creation: Allocate and track the resource
//! - Access: Retrieve the resource by handle
//! - Closure: Release the resource
//!
//! ## Tools
//!
//! | Tool | Effect | Description |
//! |------|--------|-------------|
//! | `handle-create` | `(kind id -- handle)` | Create a handle for a resource |
//! | `handle-get` | `(handle -- resource)` | Get the resource from a handle |
//! | `handle-close` | `(handle --)` | Close and invalidate a handle |
//! | `handle-valid` | `(handle -- bool)` | Check if handle is still valid |
//! | `handle-kind` | `(handle -- kind)` | Get the kind of a handle |
//! | `handle-id` | `(handle -- id)` | Get the id of a handle |
//!
//! ## Handle Kinds
//!
//! - `connection` - Database, WebSocket, MCP connections
//! - `stream` - Streaming responses (LLM, HTTP)
//! - `file` - File handles
//! - `span` - Tracing spans
//! - `task` - Async task handles

use kore::{Context, Stack, Tool, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Global handle store.
///
/// Maps handle IDs to their resources. Thread-safe via RwLock.
#[derive(Default)]
pub struct HandleStore {
    handles: HashMap<String, HandleEntry>,
}

/// A single handle entry.
struct HandleEntry {
    kind: String,
    resource: Value,
    closed: bool,
}

impl HandleStore {
    /// Create a new empty handle store.
    pub fn new() -> Self {
        Self {
            handles: HashMap::new(),
        }
    }

    /// Create a handle for a resource.
    ///
    /// Returns the handle ID.
    pub fn create(&mut self, kind: String, resource: Value) -> String {
        let id = Uuid::new_v4().to_string();
        self.handles.insert(
            id.clone(),
            HandleEntry {
                kind,
                resource,
                closed: false,
            },
        );
        id
    }

    /// Get a resource by handle ID.
    ///
    /// Returns None if handle doesn't exist or is closed.
    pub fn get(&self, id: &str) -> Option<Value> {
        self.handles
            .get(id)
            .filter(|e| !e.closed)
            .map(|e| e.resource.clone())
    }

    /// Close a handle.
    ///
    /// Returns true if handle existed and was open.
    pub fn close(&mut self, id: &str) -> bool {
        if let Some(entry) = self.handles.get_mut(id) {
            if !entry.closed {
                entry.closed = true;
                return true;
            }
        }
        false
    }

    /// Check if a handle is valid (exists and not closed).
    pub fn is_valid(&self, id: &str) -> bool {
        self.handles.get(id).map(|e| !e.closed).unwrap_or(false)
    }

    /// Get the kind of a handle.
    pub fn kind(&self, id: &str) -> Option<String> {
        self.handles.get(id).map(|e| e.kind.clone())
    }
}

/// Shared handle store type.
pub type SharedHandleStore = Arc<RwLock<HandleStore>>;

/// Create a new shared handle store.
pub fn new_handle_store() -> SharedHandleStore {
    Arc::new(RwLock::new(HandleStore::new()))
}

/// Register handle management tools into a context.
///
/// # Tools Registered
///
/// - `handle-create`: Create a handle for a resource
/// - `handle-get`: Get the resource from a handle
/// - `handle-close`: Close and invalidate a handle
/// - `handle-valid`: Check if handle is still valid
/// - `handle-kind`: Get the kind of a handle
/// - `handle-id`: Get the id from a handle value
pub async fn register_handle_tools(ctx: &mut Context) {
    let store = new_handle_store();

    // handle-create: (kind resource -- handle)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("handle-create", "(text any -- handle)", move |mut stack: Stack, ctx: Context| {
            let store = store_clone.clone();
            Box::pin(async move {
                let resource = stack.pop()?;
                let kind = stack.pop()?.into_text()?;

                let id = store.write().await.create(kind.clone(), resource);

                stack.push(Value::Handle(kore::value::Handle {
                    kind: kore::value::HandleKind::Custom(kind),
                    id,
                }))?;

                Ok((stack, ctx))
            })
        }),
    );

    // handle-get: (handle -- resource)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("handle-get", "(handle -- any)", move |mut stack: Stack, ctx: Context| {
            let store = store_clone.clone();
            Box::pin(async move {
                let handle = stack.pop()?;
                let handle_val = match &handle {
                    Value::Handle(h) => h,
                    _ => return Err(kore::Error::TypeError {
                        expected: "Handle".into(),
                        got: handle.type_name().into(),
                    }),
                };

                let resource = store.read().await.get(&handle_val.id);
                match resource {
                    Some(val) => {
                        stack.push(val)?;
                        Ok((stack, ctx))
                    }
                    None => Err(kore::Error::Runtime(format!(
                        "Handle not found or closed: {}",
                        handle_val.id
                    ))),
                }
            })
        }),
    );

    // handle-close: (handle --)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("handle-close", "(handle --)", move |mut stack: Stack, ctx: Context| {
            let store = store_clone.clone();
            Box::pin(async move {
                let handle = stack.pop()?;
                let handle_val = match &handle {
                    Value::Handle(h) => h,
                    _ => return Err(kore::Error::TypeError {
                        expected: "Handle".into(),
                        got: handle.type_name().into(),
                    }),
                };

                let closed = store.write().await.close(&handle_val.id);
                if !closed {
                    return Err(kore::Error::Runtime(format!(
                        "Handle not found or already closed: {}",
                        handle_val.id
                    )));
                }

                Ok((stack, ctx))
            })
        }),
    );

    // handle-valid: (handle -- bool)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("handle-valid", "(handle -- bool)", move |mut stack: Stack, ctx: Context| {
            let store = store_clone.clone();
            Box::pin(async move {
                let handle = stack.pop()?;
                let handle_val = match &handle {
                    Value::Handle(h) => h,
                    _ => return Err(kore::Error::TypeError {
                        expected: "Handle".into(),
                        got: handle.type_name().into(),
                    }),
                };

                let valid = store.read().await.is_valid(&handle_val.id);
                stack.push(Value::Bool(valid))?;

                Ok((stack, ctx))
            })
        }),
    );

    // handle-kind: (handle -- text)
    let store_clone = store.clone();
    ctx.dict.write().await.register(
        Tool::native("handle-kind", "(handle -- text)", move |mut stack: Stack, ctx: Context| {
            let store = store_clone.clone();
            Box::pin(async move {
                let handle = stack.pop()?;
                let handle_val = match &handle {
                    Value::Handle(h) => h,
                    _ => return Err(kore::Error::TypeError {
                        expected: "Handle".into(),
                        got: handle.type_name().into(),
                    }),
                };

                let kind = store.read().await.kind(&handle_val.id);
                match kind {
                    Some(k) => {
                        stack.push(Value::Text(k))?;
                        Ok((stack, ctx))
                    }
                    None => Err(kore::Error::Runtime(format!(
                        "Handle not found: {}",
                        handle_val.id
                    ))),
                }
            })
        }),
    );

    // handle-id: (handle -- text)
    ctx.dict.write().await.register(
        Tool::native("handle-id", "(handle -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let handle = stack.pop()?;
                let handle_val = match &handle {
                    Value::Handle(h) => h,
                    _ => return Err(kore::Error::TypeError {
                        expected: "Handle".into(),
                        got: handle.type_name().into(),
                    }),
                };

                stack.push(Value::Text(handle_val.id.clone()))?;
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_store_lifecycle() {
        let mut store = HandleStore::new();

        // Create
        let id = store.create("connection".into(), Value::Text("postgres://...".into()));
        assert!(store.is_valid(&id));

        // Get
        let resource = store.get(&id);
        assert!(resource.is_some());
        assert_eq!(resource.unwrap(), Value::Text("postgres://...".into()));

        // Kind
        assert_eq!(store.kind(&id), Some("connection".into()));

        // Close
        assert!(store.close(&id));
        assert!(!store.is_valid(&id));
        assert!(store.get(&id).is_none());

        // Double close fails
        assert!(!store.close(&id));
    }

    #[test]
    fn test_invalid_handle() {
        let store = HandleStore::new();
        assert!(!store.is_valid("nonexistent"));
        assert!(store.get("nonexistent").is_none());
        assert!(store.kind("nonexistent").is_none());
    }
}
