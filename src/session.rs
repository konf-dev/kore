//! Session — Stateful execution engine for external search/training
//!
//! A Session wraps the functional executor into a stateful object that
//! external algorithms (MCTS, RL, beam search) can drive step-by-step.
//!
//! Key operations:
//! - `step()`: Execute one op, advance program counter
//! - `step_n(n)`: Execute up to n ops
//! - `run()`: Execute until halt or error
//! - `append_ops()`: Incrementally build programs (for `compile_op`)
//! - `snapshot()` / `restore()`: Checkpoint and rewind state
//! - `fork()`: Clone entire session for parallel exploration
//!
//! ## Design
//!
//! The existing executor is functional: `execute(&[Op], Stack, Context) -> (Stack, Context)`.
//! Session manages the iteration externally, calling `execute_op()` one op at a time.
//! No changes to Kore semantics — just host-side state management.
//!
//! ## What gets snapshotted
//!
//! - `pc` (program counter)
//! - `stack` (includes locals[0..7])
//! - `steps` (total ops executed)
//!
//! What does NOT get snapshotted:
//! - Dictionary (shared via Arc, tools persist across snapshots)
//! - Capabilities (immutable after init)
//! - Memory (session-level, behind Arc<RwLock>)

use crate::context::Context;
use crate::error::{Error, Result};
use crate::executor::execute_op;
use crate::op::Op;
use crate::stack::Stack;
use crate::value::Value;

/// Session execution status
#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    /// Ready to execute more ops (pc < ops.len() or can append)
    Running,
    /// Reached end of ops (pc == ops.len())
    Halted,
    /// Hit a runtime error
    Error(String),
}

impl SessionStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, SessionStatus::Running)
    }

    pub fn is_halted(&self) -> bool {
        matches!(self, SessionStatus::Halted)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, SessionStatus::Error(_))
    }
}

/// Snapshot of session state — cheap, owned, cloneable
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Program counter at time of snapshot
    pub pc: usize,
    /// Number of ops at time of snapshot (for truncation on restore)
    pub ops_len: usize,
    /// Stack state (includes locals)
    pub stack: Stack,
    /// Step counter at time of snapshot
    pub steps: usize,
}

/// Stateful execution session
///
/// Wraps the functional executor for step-by-step control.
/// External algorithms drive this to explore program spaces.
pub struct Session {
    /// The program (owned, appendable)
    ops: Vec<Op>,
    /// Program counter — index of next op to execute
    pc: usize,
    /// Current stack state
    stack: Stack,
    /// Execution context (shared dict + caps + resources)
    ctx: Context,
    /// Current status
    status: SessionStatus,
    /// Total ops executed
    steps: usize,
    /// Max steps before forced halt (0 = unlimited)
    max_steps: usize,
}

impl Session {
    /// Create a new session with given program, stack, and context.
    pub fn new(ops: Vec<Op>, stack: Stack, ctx: Context) -> Self {
        let status = if ops.is_empty() {
            SessionStatus::Halted
        } else {
            SessionStatus::Running
        };
        Self {
            ops,
            pc: 0,
            stack,
            ctx,
            status,
            steps: 0,
            max_steps: 0,
        }
    }

    /// Create a session with a step limit.
    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// Execute exactly one op. Returns the new status.
    ///
    /// If status is not Running, returns current status without doing anything.
    pub async fn step(&mut self) -> &SessionStatus {
        if !self.status.is_running() {
            return &self.status;
        }

        // Check step limit
        if self.max_steps > 0 && self.steps >= self.max_steps {
            self.status = SessionStatus::Halted;
            return &self.status;
        }

        // Check if we've reached the end
        if self.pc >= self.ops.len() {
            self.status = SessionStatus::Halted;
            return &self.status;
        }

        // Execute one op
        let op = self.ops[self.pc].clone();
        match execute_op(&op, self.stack.clone(), self.ctx.clone()).await {
            Ok((new_stack, new_ctx)) => {
                self.stack = new_stack;
                self.ctx = new_ctx;
                self.pc += 1;
                self.steps += 1;

                // Check if we just finished
                if self.pc >= self.ops.len() {
                    self.status = SessionStatus::Halted;
                }
            }
            Err(e) => {
                self.status = SessionStatus::Error(e.to_string());
            }
        }

        &self.status
    }

    /// Execute up to n ops. Returns (number executed, status).
    pub async fn step_n(&mut self, n: usize) -> (usize, &SessionStatus) {
        let mut executed = 0;
        for _ in 0..n {
            if !self.status.is_running() {
                break;
            }
            self.step().await;
            if self.status.is_running() || self.status.is_halted() {
                // Only count if the step actually executed (not if it was already stopped)
                if self.status.is_error() {
                    executed += 1; // The error happened during execution
                    break;
                }
                executed += 1;
            }
            if !self.status.is_running() {
                break;
            }
        }
        (executed, &self.status)
    }

    /// Execute until halt or error. Returns final status.
    pub async fn run(&mut self) -> &SessionStatus {
        while self.status.is_running() {
            self.step().await;
        }
        &self.status
    }

    /// Append ops to the program. If halted, resume running.
    ///
    /// This is the key primitive for incremental program building:
    /// the model picks an action, Python compiles it to ops via `Op::parse()`,
    /// then appends them here.
    pub fn append_ops(&mut self, ops: Vec<Op>) {
        if ops.is_empty() {
            return;
        }
        self.ops.extend(ops);
        // If we were halted (reached end of ops), resume
        if self.status.is_halted() {
            self.status = SessionStatus::Running;
        }
    }

    /// Snapshot current state. O(stack_size).
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            pc: self.pc,
            ops_len: self.ops.len(),
            stack: self.stack.clone(),
            steps: self.steps,
        }
    }

    /// Restore from a snapshot. Rewinds pc, stack, step counter, and ops.
    ///
    /// Ops appended after the snapshot are discarded (truncated to snapshot length).
    /// This is correct MCTS semantics: rewinding to a state means discarding
    /// code paths added after the checkpoint.
    /// Status is set to Running if pc < ops.len().
    pub fn restore(&mut self, snap: &Snapshot) {
        self.pc = snap.pc;
        self.ops.truncate(snap.ops_len);
        self.stack = snap.stack.clone();
        self.steps = snap.steps;
        self.status = if self.pc < self.ops.len() {
            SessionStatus::Running
        } else {
            SessionStatus::Halted
        };
    }

    /// Fork: create an independent clone of this session.
    ///
    /// The clone shares the same dictionary (via Arc) but has independent
    /// stack, pc, and ops. Mutations to one do not affect the other.
    pub fn fork(&self) -> Session {
        Session {
            ops: self.ops.clone(),
            pc: self.pc,
            stack: self.stack.clone(),
            ctx: self.ctx.clone(),
            status: self.status.clone(),
            steps: self.steps,
            max_steps: self.max_steps,
        }
    }

    // === Observers ===

    /// Get current stack (read-only).
    pub fn stack(&self) -> &Stack {
        &self.stack
    }

    /// Get current status.
    pub fn status(&self) -> &SessionStatus {
        &self.status
    }

    /// Get program counter.
    pub fn pc(&self) -> usize {
        self.pc
    }

    /// Get total steps executed.
    pub fn steps(&self) -> usize {
        self.steps
    }

    /// Get number of ops in program.
    pub fn ops_len(&self) -> usize {
        self.ops.len()
    }

    /// Get the context (for shared dict access).
    pub fn context(&self) -> &Context {
        &self.ctx
    }

    /// Push a value onto the session's stack (host-side setup).
    pub fn push_value(&mut self, value: Value) -> Result<()> {
        self.stack.push(value)
    }

    /// Push multiple values onto the stack.
    pub fn push_values(&mut self, values: Vec<Value>) -> Result<()> {
        for v in values {
            self.stack.push(v)?;
        }
        Ok(())
    }
}

// === Value JSON serialization ===

/// Convert a Kore Value to a JSON value for the Python controller.
///
/// Round-trippable for: Null, Bool, Int, Float, Text, List, Map, Error.
/// Observation-only for: Quote, Handle, Ext.
pub fn value_to_json(v: &Value) -> serde_json::Value {
    use serde_json::json;
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => json!(*b),
        Value::Int(n) => json!(*n),
        Value::Float(f) => json!(*f),
        Value::Text(s) => json!(s),
        Value::List(items) => {
            let arr: Vec<serde_json::Value> = items.iter().map(value_to_json).collect();
            serde_json::Value::Array(arr)
        }
        Value::Map(m) => {
            let mut obj = serde_json::Map::new();
            obj.insert("__map".to_string(), {
                let inner: serde_json::Map<String, serde_json::Value> = m
                    .iter()
                    .map(|(k, v)| (k.clone(), value_to_json(v)))
                    .collect();
                serde_json::Value::Object(inner)
            });
            serde_json::Value::Object(obj)
        }
        Value::Error(e) => {
            json!({"__error": {"code": e.code, "message": e.message}})
        }
        Value::Quote(ops) => {
            let desc: Vec<String> = ops.iter().map(|op| format!("{}", op)).collect();
            json!({"__quote": desc.join(" ")})
        }
        Value::Handle(h) => {
            json!({"__handle": format!("{:?}:{}", h.kind, h.id)})
        }
        Value::Ext(e) => {
            json!({"__ext": {"kind": e.kind, "data": value_to_json(&e.data)}})
        }
    }
}

/// Convert a JSON value back to a Kore Value.
///
/// Handles the round-trippable types. Tagged objects (`__map`, `__error`)
/// are decoded. Plain objects without tags become Maps.
pub fn json_to_value(j: &serde_json::Value) -> Result<Value> {
    match j {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Err(Error::Runtime(format!("unsupported JSON number: {}", n)))
            }
        }
        serde_json::Value::String(s) => Ok(Value::Text(s.clone())),
        serde_json::Value::Array(arr) => {
            let values: Result<Vec<Value>> = arr.iter().map(json_to_value).collect();
            Ok(Value::List(values?))
        }
        serde_json::Value::Object(obj) => {
            // Check for tagged types
            if let Some(inner) = obj.get("__map") {
                // Map value
                if let serde_json::Value::Object(map_obj) = inner {
                    let mut map = indexmap::IndexMap::new();
                    for (k, v) in map_obj {
                        map.insert(k.clone(), json_to_value(v)?);
                    }
                    Ok(Value::Map(map))
                } else {
                    Err(Error::Runtime("__map must contain an object".into()))
                }
            } else if let Some(inner) = obj.get("__error") {
                // Error value
                let code = inner
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let message = inner
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(Value::Error(Box::new(crate::value::ErrorValue {
                    code,
                    message,
                })))
            } else {
                // Plain object → Map
                let mut map = indexmap::IndexMap::new();
                for (k, v) in obj {
                    map.insert(k.clone(), json_to_value(v)?);
                }
                Ok(Value::Map(map))
            }
        }
    }
}

/// Convert a stack to a JSON array.
pub fn stack_to_json(stack: &Stack) -> serde_json::Value {
    let arr: Vec<serde_json::Value> = stack.values().iter().map(value_to_json).collect();
    serde_json::Value::Array(arr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::register_builtins;

    async fn setup() -> Context {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        ctx
    }

    // === Session tests ===

    #[tokio::test]
    async fn test_session_step() {
        let ctx = setup().await;
        // Program: push 5, push 3, add → 8
        let ops = vec![Op::push(5), Op::push(3), Op::call("add")];
        let mut session = Session::new(ops, Stack::new(), ctx);

        assert!(session.status().is_running());
        assert_eq!(session.steps(), 0);

        // Step 1: push 5
        session.step().await;
        assert!(session.status().is_running());
        assert_eq!(session.stack().depth(), 1);
        assert_eq!(session.stack().values()[0].as_int().unwrap(), 5);

        // Step 2: push 3
        session.step().await;
        assert!(session.status().is_running());
        assert_eq!(session.stack().depth(), 2);

        // Step 3: add
        session.step().await;
        assert!(session.status().is_halted());
        assert_eq!(session.stack().depth(), 1);
        assert_eq!(session.stack().values()[0].as_int().unwrap(), 8);
        assert_eq!(session.steps(), 3);
    }

    #[tokio::test]
    async fn test_session_step_n() {
        let ctx = setup().await;
        let ops = vec![Op::push(1), Op::push(2), Op::push(3), Op::push(4)];
        let mut session = Session::new(ops, Stack::new(), ctx);

        let (executed, status) = session.step_n(2).await;
        assert_eq!(executed, 2);
        assert!(status.is_running());
        assert_eq!(session.stack().depth(), 2);

        let (executed, status) = session.step_n(10).await;
        assert_eq!(executed, 2); // only 2 remaining
        assert!(status.is_halted());
        assert_eq!(session.stack().depth(), 4);
    }

    #[tokio::test]
    async fn test_session_run() {
        let ctx = setup().await;
        let ops = vec![Op::push(10), Op::call("dup"), Op::call("add")];
        let mut session = Session::new(ops, Stack::new(), ctx);

        let status = session.run().await;
        assert!(status.is_halted());
        assert_eq!(session.stack().values()[0].as_int().unwrap(), 20);
        assert_eq!(session.steps(), 3);
    }

    #[tokio::test]
    async fn test_session_error() {
        let ctx = setup().await;
        // add with empty stack → underflow error
        let ops = vec![Op::call("add")];
        let mut session = Session::new(ops, Stack::new(), ctx);

        let status = session.step().await;
        assert!(status.is_error());

        // Further steps should be no-ops
        let status = session.step().await;
        assert!(status.is_error());
    }

    #[tokio::test]
    async fn test_session_max_steps() {
        let ctx = setup().await;
        let ops = vec![
            Op::push(1),
            Op::push(2),
            Op::push(3),
            Op::push(4),
            Op::push(5),
        ];
        let mut session = Session::new(ops, Stack::new(), ctx).with_max_steps(3);

        let status = session.run().await;
        assert!(status.is_halted());
        assert_eq!(session.steps(), 3);
        assert_eq!(session.stack().depth(), 3);
    }

    #[tokio::test]
    async fn test_session_append_ops() {
        let ctx = setup().await;
        let ops = vec![Op::push(5)];
        let mut session = Session::new(ops, Stack::new(), ctx);

        // Run initial program
        session.run().await;
        assert!(session.status().is_halted());
        assert_eq!(session.stack().depth(), 1);

        // Append more ops and resume
        session.append_ops(vec![Op::call("dup"), Op::call("add")]);
        assert!(session.status().is_running());

        session.run().await;
        assert!(session.status().is_halted());
        assert_eq!(session.stack().values()[0].as_int().unwrap(), 10);
    }

    #[tokio::test]
    async fn test_session_empty_program() {
        let ctx = setup().await;
        let session = Session::new(vec![], Stack::new(), ctx);
        assert!(session.status().is_halted());
    }

    // === Snapshot/Restore tests ===

    #[tokio::test]
    async fn test_snapshot_restore() {
        let ctx = setup().await;
        let ops = vec![
            Op::push(1),
            Op::push(2),
            Op::call("add"),
            Op::push(100),
        ];
        let mut session = Session::new(ops, Stack::new(), ctx);

        // Run first 2 ops: stack = [1, 2]
        session.step_n(2).await;
        let snap = session.snapshot();
        assert_eq!(snap.pc, 2);
        assert_eq!(snap.steps, 2);

        // Run the rest: stack = [3, 100]
        session.run().await;
        assert_eq!(session.stack().depth(), 2);
        assert_eq!(session.stack().values()[0].as_int().unwrap(), 3);
        assert_eq!(session.stack().values()[1].as_int().unwrap(), 100);

        // Restore: back to [1, 2]
        session.restore(&snap);
        assert!(session.status().is_running());
        assert_eq!(session.stack().depth(), 2);
        assert_eq!(session.stack().values()[0].as_int().unwrap(), 1);
        assert_eq!(session.stack().values()[1].as_int().unwrap(), 2);
        assert_eq!(session.steps(), 2);
        assert_eq!(session.pc(), 2);

        // Run again from snapshot: same result
        session.run().await;
        assert_eq!(session.stack().values()[0].as_int().unwrap(), 3);
        assert_eq!(session.stack().values()[1].as_int().unwrap(), 100);
    }

    #[tokio::test]
    async fn test_snapshot_with_locals() {
        let ctx = setup().await;
        let ops = vec![
            Op::push(42),
            Op::call("store0"),
            Op::push(99),
            Op::call("store1"),
            // After this point, load0 = 42, load1 = 99
            Op::call("load0"),
            Op::call("load1"),
            Op::call("add"),
        ];
        let mut session = Session::new(ops, Stack::new(), ctx);

        // Run 4 ops: stored 42 in slot0, 99 in slot1, stack empty
        session.step_n(4).await;
        let snap = session.snapshot();

        // Run rest: load0 + load1 + add = 141
        session.run().await;
        assert_eq!(session.stack().values()[0].as_int().unwrap(), 141);

        // Restore and run again
        session.restore(&snap);
        session.run().await;
        assert_eq!(session.stack().values()[0].as_int().unwrap(), 141);
    }

    #[tokio::test]
    async fn test_fork_isolation() {
        let ctx = setup().await;
        let ops = vec![Op::push(1), Op::push(2), Op::push(3)];
        let mut session = Session::new(ops, Stack::new(), ctx);

        // Run 1 op
        session.step().await;

        // Fork
        let mut forked = session.fork();

        // Original runs to completion: stack = [1, 2, 3]
        session.run().await;
        assert_eq!(session.stack().depth(), 3);

        // Forked also runs to completion independently
        forked.run().await;
        assert_eq!(forked.stack().depth(), 3);

        // Verify they have the same result
        assert_eq!(
            session.stack().values()[0].as_int().unwrap(),
            forked.stack().values()[0].as_int().unwrap()
        );
    }

    #[tokio::test]
    async fn test_fork_with_append() {
        let ctx = setup().await;
        let ops = vec![Op::push(5)];
        let mut session = Session::new(ops, Stack::new(), ctx);
        session.run().await;

        // Fork and append different ops to each
        let mut fork_a = session.fork();
        let mut fork_b = session.fork();

        fork_a.append_ops(vec![Op::call("dup"), Op::call("add")]); // 5 dup add = 10
        fork_b.append_ops(vec![Op::push(3), Op::call("add")]); // 5 3 add = 8

        fork_a.run().await;
        fork_b.run().await;

        assert_eq!(fork_a.stack().values()[0].as_int().unwrap(), 10);
        assert_eq!(fork_b.stack().values()[0].as_int().unwrap(), 8);
    }

    #[tokio::test]
    async fn test_push_value() {
        let ctx = setup().await;
        let ops = vec![Op::call("add")];
        let mut session = Session::new(ops, Stack::new(), ctx);

        session.push_value(Value::Int(3)).unwrap();
        session.push_value(Value::Int(7)).unwrap();

        session.run().await;
        assert_eq!(session.stack().values()[0].as_int().unwrap(), 10);
    }

    // === MCTS simulation pattern ===

    #[tokio::test]
    async fn test_mcts_pattern() {
        let ctx = setup().await;
        // Simulate MCTS: try multiple continuations from the same state
        let ops = vec![Op::push(5)];
        let mut session = Session::new(ops, Stack::new(), ctx);
        session.run().await;
        let root_snap = session.snapshot();

        // Branch 1: dup add → 10
        session.restore(&root_snap);
        session.append_ops(Op::parse("dup add").unwrap());
        session.run().await;
        let result_1 = session.stack().values()[0].as_int().unwrap();
        assert_eq!(result_1, 10);

        // Branch 2: 3 add → 8
        session.restore(&root_snap);
        session.append_ops(Op::parse("3 add").unwrap());
        session.run().await;
        let result_2 = session.stack().values()[0].as_int().unwrap();
        assert_eq!(result_2, 8);

        // Branch 3: dup mul → 25
        session.restore(&root_snap);
        session.append_ops(Op::parse("dup mul").unwrap());
        session.run().await;
        let result_3 = session.stack().values()[0].as_int().unwrap();
        assert_eq!(result_3, 25);
    }

    // === JSON serialization tests ===

    #[test]
    fn test_json_null() {
        let v = Value::Null;
        let j = value_to_json(&v);
        assert!(j.is_null());
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, Value::Null);
    }

    #[test]
    fn test_json_bool() {
        let v = Value::Bool(true);
        let j = value_to_json(&v);
        assert_eq!(j, serde_json::json!(true));
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, Value::Bool(true));
    }

    #[test]
    fn test_json_int() {
        let v = Value::Int(42);
        let j = value_to_json(&v);
        assert_eq!(j, serde_json::json!(42));
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, Value::Int(42));
    }

    #[test]
    fn test_json_float() {
        let v = Value::Float(3.14);
        let j = value_to_json(&v);
        let back = json_to_value(&j).unwrap();
        match back {
            Value::Float(f) => assert!((f - 3.14).abs() < 1e-10),
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_json_text() {
        let v = Value::Text("hello world".into());
        let j = value_to_json(&v);
        assert_eq!(j, serde_json::json!("hello world"));
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, Value::Text("hello world".into()));
    }

    #[test]
    fn test_json_list() {
        let v = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        let j = value_to_json(&v);
        assert_eq!(j, serde_json::json!([1, 2, 3]));
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn test_json_nested_list() {
        let v = Value::List(vec![
            Value::List(vec![Value::Int(1), Value::Int(2)]),
            Value::List(vec![Value::Int(3), Value::Int(4)]),
        ]);
        let j = value_to_json(&v);
        assert_eq!(j, serde_json::json!([[1, 2], [3, 4]]));
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn test_json_map() {
        let mut m = indexmap::IndexMap::new();
        m.insert("x".into(), Value::Int(1));
        m.insert("y".into(), Value::Int(2));
        let v = Value::Map(m.clone());
        let j = value_to_json(&v);

        // Should have __map wrapper
        assert!(j.get("__map").is_some());

        let back = json_to_value(&j).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn test_json_error() {
        let v = Value::Error(Box::new(crate::value::ErrorValue {
            code: "DIV_ZERO".into(),
            message: "division by zero".into(),
        }));
        let j = value_to_json(&v);
        assert!(j.get("__error").is_some());
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn test_json_quote_observation() {
        let v = Value::Quote(vec![Op::call("dup"), Op::call("add")]);
        let j = value_to_json(&v);
        // Quote is observation-only
        assert!(j.get("__quote").is_some());
        let desc = j["__quote"].as_str().unwrap();
        assert!(desc.contains("dup"));
        assert!(desc.contains("add"));
    }

    #[test]
    fn test_json_plain_object_becomes_map() {
        // A plain JSON object without __map tag should still parse as Map
        let j = serde_json::json!({"name": "test", "value": 42});
        let v = json_to_value(&j).unwrap();
        match v {
            Value::Map(m) => {
                assert_eq!(m.get("name").unwrap(), &Value::Text("test".into()));
                assert_eq!(m.get("value").unwrap(), &Value::Int(42));
            }
            _ => panic!("Expected Map"),
        }
    }

    #[test]
    fn test_stack_to_json() {
        let stack = Stack::from_values(vec![
            Value::Int(1),
            Value::Text("hello".into()),
            Value::Bool(true),
        ]);
        let j = stack_to_json(&stack);
        assert_eq!(j, serde_json::json!([1, "hello", true]));
    }

    #[test]
    fn test_json_mixed_types_list() {
        let v = Value::List(vec![
            Value::Int(1),
            Value::Text("two".into()),
            Value::Bool(false),
            Value::Null,
            Value::Float(4.5),
        ]);
        let j = value_to_json(&v);
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn test_json_large_int() {
        let v = Value::Int(i64::MAX);
        let j = value_to_json(&v);
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn test_json_negative_int() {
        let v = Value::Int(-42);
        let j = value_to_json(&v);
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn test_json_empty_list() {
        let v = Value::List(vec![]);
        let j = value_to_json(&v);
        assert_eq!(j, serde_json::json!([]));
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn test_json_empty_map() {
        let v = Value::Map(indexmap::IndexMap::new());
        let j = value_to_json(&v);
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, v);
    }
}
