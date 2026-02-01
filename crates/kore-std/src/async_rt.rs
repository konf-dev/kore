//! # Async Runtime
//!
//! Concurrent execution primitives.
//!
//! ## Purpose
//!
//! Some operations need to run concurrently:
//! - Multiple HTTP requests in parallel
//! - Timeout wrappers
//! - Background tasks
//!
//! ## Tools
//!
//! | Tool | Effect | Description |
//! |------|--------|-------------|
//! | `spawn` | `(quote -- handle)` | Start a task in background |
//! | `await` | `(handle -- result)` | Wait for a task to complete |
//! | `timeout` | `(ms quote -- result)` | Run with timeout |
//! | `sleep` | `(ms --)` | Pause execution |

use kore::{Context, Stack, Tool, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, timeout as tokio_timeout};
use uuid::Uuid;

/// Task state for spawned tasks.
#[derive(Clone)]
pub enum TaskState {
    /// Task is still running
    Running,
    /// Task completed successfully
    Completed(Vec<Value>),
    /// Task failed with an error
    Failed(String),
}

/// Store for spawned tasks.
#[derive(Default)]
pub struct TaskStore {
    tasks: HashMap<String, TaskState>,
}

impl TaskStore {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    pub fn create(&mut self) -> String {
        let id = Uuid::new_v4().to_string();
        self.tasks.insert(id.clone(), TaskState::Running);
        id
    }

    pub fn complete(&mut self, id: &str, result: Vec<Value>) {
        self.tasks.insert(id.to_string(), TaskState::Completed(result));
    }

    pub fn fail(&mut self, id: &str, error: String) {
        self.tasks.insert(id.to_string(), TaskState::Failed(error));
    }

    pub fn get(&self, id: &str) -> Option<TaskState> {
        self.tasks.get(id).cloned()
    }
}

pub type SharedTaskStore = Arc<RwLock<TaskStore>>;

pub fn new_task_store() -> SharedTaskStore {
    Arc::new(RwLock::new(TaskStore::new()))
}

/// Register async runtime tools into a context.
///
/// # Tools Registered
///
/// - `spawn`: Start a task in background
/// - `await`: Wait for a task to complete
/// - `timeout`: Run with timeout
/// - `sleep`: Pause execution
pub async fn register_async_tools(ctx: &mut Context) {
    let task_store = new_task_store();

    // sleep: (ms --)
    ctx.dict.write().await.register(
        Tool::native("sleep", "(int --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let ms = stack.pop()?.as_int()?;
                if ms < 0 {
                    return Err(kore::Error::Runtime("Sleep duration cannot be negative".into()));
                }

                tokio::time::sleep(Duration::from_millis(ms as u64)).await;
                Ok((stack, ctx))
            })
        }),
    );

    // timeout: (ms quote -- result-or-error)
    ctx.dict.write().await.register(
        Tool::native("timeout", "(int quote -- any)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;
                let ms = stack.pop()?.as_int()?;

                if ms < 0 {
                    return Err(kore::Error::Runtime("Timeout duration cannot be negative".into()));
                }

                let duration = Duration::from_millis(ms as u64);
                let result = tokio_timeout(duration, kore::execute(&quote, stack.clone(), ctx.clone())).await;

                match result {
                    Ok(Ok((new_stack, new_ctx))) => Ok((new_stack, new_ctx)),
                    Ok(Err(e)) => Err(e),
                    Err(_) => {
                        // Timeout elapsed
                        Err(kore::Error::Runtime(format!(
                            "Timeout after {}ms",
                            ms
                        )))
                    }
                }
            })
        }),
    );

    // spawn: (quote -- handle)
    let store = task_store.clone();
    ctx.dict.write().await.register(
        Tool::native("spawn", "(quote -- handle)", move |mut stack: Stack, ctx: Context| {
            let store = store.clone();
            Box::pin(async move {
                let quote = stack.pop()?.into_quote()?;

                // Create task handle
                let task_id = store.write().await.create();

                // Spawn the task
                let store_clone = store.clone();
                let task_id_clone = task_id.clone();
                let ctx_clone = ctx.clone();
                let stack_clone = Stack::new();

                tokio::spawn(async move {
                    match kore::execute(&quote, stack_clone, ctx_clone).await {
                        Ok((result_stack, _)) => {
                            store_clone
                                .write()
                                .await
                                .complete(&task_id_clone, result_stack.values().to_vec());
                        }
                        Err(e) => {
                            store_clone.write().await.fail(&task_id_clone, e.to_string());
                        }
                    }
                });

                // Push handle
                stack.push(Value::Handle(kore::value::Handle {
                    kind: kore::value::HandleKind::Custom("task".into()),
                    id: task_id,
                }))?;

                Ok((stack, ctx))
            })
        }),
    );

    // await: (handle -- ...values)
    let store = task_store.clone();
    ctx.dict.write().await.register(
        Tool::native("await", "(handle -- any)", move |mut stack: Stack, ctx: Context| {
            let store = store.clone();
            Box::pin(async move {
                let handle = stack.pop()?;
                let handle_val = match &handle {
                    Value::Handle(h) => h,
                    _ => {
                        return Err(kore::Error::TypeError {
                            expected: "Handle".into(),
                            got: handle.type_name().into(),
                        });
                    }
                };

                // Poll until complete
                loop {
                    let state = store.read().await.get(&handle_val.id);
                    match state {
                        Some(TaskState::Running) => {
                            // Still running, wait a bit
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                        Some(TaskState::Completed(values)) => {
                            // Push all values onto stack
                            for value in values {
                                stack.push(value)?;
                            }
                            return Ok((stack, ctx));
                        }
                        Some(TaskState::Failed(error)) => {
                            return Err(kore::Error::Runtime(format!("Task failed: {}", error)));
                        }
                        None => {
                            return Err(kore::Error::Runtime(format!(
                                "Task not found: {}",
                                handle_val.id
                            )));
                        }
                    }
                }
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_store_lifecycle() {
        let mut store = TaskStore::new();

        let id = store.create();
        assert!(matches!(store.get(&id), Some(TaskState::Running)));

        store.complete(&id, vec![Value::Int(42)]);
        match store.get(&id) {
            Some(TaskState::Completed(values)) => {
                assert_eq!(values, vec![Value::Int(42)]);
            }
            _ => panic!("Expected Completed state"),
        }
    }

    #[test]
    fn test_task_store_failure() {
        let mut store = TaskStore::new();

        let id = store.create();
        store.fail(&id, "Something went wrong".into());

        match store.get(&id) {
            Some(TaskState::Failed(msg)) => {
                assert_eq!(msg, "Something went wrong");
            }
            _ => panic!("Expected Failed state"),
        }
    }
}
