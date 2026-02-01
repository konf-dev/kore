# Context Flow in Kore

## Core Principle

> **Every operation must carry full context from input through everything it called.**

This document defines how context flows through the system, ensuring:
- Complete traceability
- Debuggability
- Reproducibility
- Error diagnosis

---

## What is Context?

Context is the complete set of information needed to understand:
1. **WHO** initiated an operation
2. **WHAT** was being done
3. **WHY** it was done (intent)
4. **WHERE** it happened (location in system)
5. **WHEN** it happened (ordering)
6. **HOW** it was done (implementation details)
7. **WHAT ELSE** was called (children/dependencies)

---

## Context Types

### 1. Identity Context

WHO is performing the operation?

```rust
struct IdentityContext {
    // Root identity
    tenant_id: TenantId,
    
    // Workflow identity (if in workflow)
    workflow_id: Option<WorkflowId>,
    workflow_name: Option<String>,
    
    // Hierarchy
    parent_id: Option<WorkflowId>,
    root_id: WorkflowId,  // Genesis or top-level workflow
    
    // Depth in hierarchy
    depth: usize,
    
    // Lineage (full path from root)
    lineage: Vec<WorkflowId>,
}
```

### 2. Execution Context

WHAT is being executed?

```rust
struct ExecutionContext {
    // Current operation
    current_op: Op,
    op_index: usize,
    
    // Stack state
    stack_depth: usize,
    stack_types: Vec<&'static str>,  // Types on stack
    
    // Program being run
    program: Vec<Op>,
    program_name: Option<String>,
    
    // Caller info
    call_site: Option<CallSite>,
}

struct CallSite {
    caller_tool: String,
    caller_op_index: usize,
    caller_program: String,
}
```

### 3. Capability Context

WHAT is allowed?

```rust
struct CapabilityContext {
    // Granted capabilities
    capabilities: HashSet<Capability>,
    
    // Where capabilities came from
    capability_origin: HashMap<Capability, CapabilityOrigin>,
    
    // Restricted tools
    blocked_tools: HashSet<String>,
    
    // Resource limits
    limits: ResourceLimits,
}

struct ResourceLimits {
    max_stack_depth: usize,
    max_recursion: usize,
    max_execution_time: Duration,
    max_memory: usize,
}
```

### 4. Communication Context

WHO am I talking to?

```rust
struct CommunicationContext {
    // Parent relationship
    parent: Option<WorkflowHandle>,
    
    // Children
    children: Vec<WorkflowHandle>,
    
    // Messages pending
    inbox_size: usize,
    outbox_size: usize,
    
    // Message history (recent)
    recent_messages: VecDeque<MessageSummary>,
}
```

### 5. Trace Context

WHAT happened?

```rust
struct TraceContext {
    // Trace ID (spans entire operation tree)
    trace_id: TraceId,
    
    // Span ID (this specific operation)
    span_id: SpanId,
    
    // Parent span
    parent_span_id: Option<SpanId>,
    
    // Timing
    start_time: Instant,
    
    // Events recorded
    events: Vec<TraceEvent>,
}
```

---

## Full Context Structure

All context types combined:

```rust
pub struct Context {
    // WHO
    pub identity: IdentityContext,
    
    // WHAT
    pub execution: ExecutionContext,
    
    // ALLOWED
    pub capabilities: CapabilityContext,
    
    // COMMUNICATION
    pub communication: CommunicationContext,
    
    // TRACE
    pub trace: TraceContext,
    
    // USER DATA
    pub dictionary: Dictionary,
    pub local_data: HashMap<String, Value>,
    
    // EMITTED (for parent to see)
    pub emitted: Vec<ContextUpdate>,
}
```

---

## Context Flow Patterns

### Pattern 1: Tool Call

When a tool is called, context flows:

```
┌─────────────────────────────────────────────────────────────────┐
│                         TOOL CALL FLOW                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Caller                  Tool                    Callees        │
│    │                      │                         │           │
│    │  [1] Call with ctx   │                         │           │
│    │ ────────────────────>│                         │           │
│    │                      │                         │           │
│    │                      │ [2] Record call site    │           │
│    │                      │ [3] Push trace span     │           │
│    │                      │                         │           │
│    │                      │ [4] Call sub-tools      │           │
│    │                      │ ───────────────────────>│           │
│    │                      │                         │           │
│    │                      │ [5] Receive results     │           │
│    │                      │ <───────────────────────│           │
│    │                      │                         │           │
│    │                      │ [6] Pop trace span      │           │
│    │                      │ [7] Record result       │           │
│    │                      │                         │           │
│    │ [8] Receive result   │                         │           │
│    │ <────────────────────│                         │           │
│    │                      │                         │           │
│    │ Context now includes:                          │           │
│    │ - What tool was called                         │           │
│    │ - What it called internally                    │           │
│    │ - How long it took                             │           │
│    │ - What it returned                             │           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Pattern 2: Workflow Spawn

When a workflow is spawned:

```
┌─────────────────────────────────────────────────────────────────┐
│                       SPAWN FLOW                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Parent                 Runtime              Child              │
│    │                      │                    │                │
│    │ [1] spawn(quote)     │                    │                │
│    │ ────────────────────>│                    │                │
│    │                      │                    │                │
│    │                      │ [2] Create child identity:          │
│    │                      │     - new workflow_id              │
│    │                      │     - parent_id = caller           │
│    │                      │     - depth = parent.depth + 1     │
│    │                      │     - lineage = parent.lineage + [id]│
│    │                      │                    │                │
│    │                      │ [3] Inherit capabilities:           │
│    │                      │     - subset of parent              │
│    │                      │     - same tenant                   │
│    │                      │                    │                │
│    │                      │ [4] Create communication:           │
│    │                      │     - link parent <-> child         │
│    │                      │     - create inbox                  │
│    │                      │                    │                │
│    │                      │ [5] Create trace:                   │
│    │                      │     - same trace_id                 │
│    │                      │     - new span_id                   │
│    │                      │     - parent_span = parent.span     │
│    │                      │                    │                │
│    │                      │ ───────────────────>│ [6] Start    │
│    │                      │                    │                │
│    │ <────────────────────│                    │                │
│    │ [7] Receive handle   │                    │                │
│    │                      │                    │                │
│    │ Context now includes:                     │                │
│    │ - New child in children list              │                │
│    │ - Updated communication context           │                │
│    │ - Trace span for spawn operation          │                │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Pattern 3: Context Emission

When a child emits context to parent:

```
┌─────────────────────────────────────────────────────────────────┐
│                       EMIT FLOW                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Child                  Runtime              Parent             │
│    │                      │                    │                │
│    │ [1] emit(update)     │                    │                │
│    │ ────────────────────>│                    │                │
│    │                      │                    │                │
│    │                      │ [2] Enrich update:                  │
│    │                      │     - source = child.id             │
│    │                      │     - timestamp = now               │
│    │                      │     - depth = child.depth           │
│    │                      │                    │                │
│    │                      │ [3] Route to parent                 │
│    │                      │ ───────────────────>│               │
│    │                      │                    │                │
│    │                      │                    │ [4] Buffer in  │
│    │                      │                    │     parent's   │
│    │                      │                    │     context    │
│    │                      │                    │                │
│    │                      │                    │ [5] Available  │
│    │                      │                    │     via observe│
│    │                      │                    │                │
│    │ Context update includes:                  │                │
│    │ - WHO emitted (child id)                  │                │
│    │ - WHAT was emitted (update data)          │                │
│    │ - WHEN (timestamp)                        │                │
│    │ - WHERE in hierarchy (depth, lineage)     │                │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Pattern 4: Error Propagation

When an error occurs:

```
┌─────────────────────────────────────────────────────────────────┐
│                       ERROR FLOW                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Deep Call             Mid Call              Top Call           │
│    │                      │                    │                │
│    │ [1] Error occurs     │                    │                │
│    │                      │                    │                │
│    │ [2] Create error with context:            │                │
│    │     - What operation failed               │                │
│    │     - Stack state at failure              │                │
│    │     - Current execution context           │                │
│    │                      │                    │                │
│    │ [3] Propagate up     │                    │                │
│    │ ────────────────────>│                    │                │
│    │                      │                    │                │
│    │                      │ [4] Add to chain:                   │
│    │                      │     - What was I doing?             │
│    │                      │     - My context at call            │
│    │                      │                    │                │
│    │                      │ [5] Propagate up   │                │
│    │                      │ ───────────────────>│               │
│    │                      │                    │                │
│    │                      │                    │ [6] Add to     │
│    │                      │                    │     chain      │
│    │                      │                    │                │
│    │ Final error includes:                     │                │
│    │ - Original error message                  │                │
│    │ - Full call chain with context            │                │
│    │ - Stack state at each level               │                │
│    │ - What each caller was trying to do       │                │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Implementation Guidelines

### Rule 1: Context Enters Every Function

```rust
// GOOD: Context is first parameter
fn spawn_tool(ctx: &mut Context, stack: &mut Stack) -> Result<(), Error> {
    // Can access full context
}

// BAD: No context access
fn spawn_tool(stack: &mut Stack) -> Result<(), Error> {
    // Can't record trace, check capabilities, etc.
}
```

### Rule 2: Context Updates Before Operations

```rust
fn some_tool(ctx: &mut Context, stack: &mut Stack) -> Result<(), Error> {
    // 1. Record entry to this operation
    let span = ctx.trace.enter("some_tool");
    
    // 2. Record stack state
    span.record("stack_depth", stack.depth());
    span.record("stack_types", stack.type_signature());
    
    // 3. Do the operation
    let result = do_work(ctx, stack)?;
    
    // 4. Record exit
    span.record("result", &result);
    ctx.trace.exit(span);
    
    Ok(())
}
```

### Rule 3: Child Context Derived From Parent

```rust
fn spawn_child(parent_ctx: &Context, child_code: &[Op]) -> Context {
    Context {
        // Identity derived from parent
        identity: IdentityContext {
            tenant_id: parent_ctx.identity.tenant_id.clone(),
            workflow_id: Some(WorkflowId::new()),
            parent_id: parent_ctx.identity.workflow_id.clone(),
            depth: parent_ctx.identity.depth + 1,
            lineage: {
                let mut l = parent_ctx.identity.lineage.clone();
                l.push(parent_ctx.identity.workflow_id.unwrap());
                l
            },
            ..Default::default()
        },
        
        // Capabilities inherited (subset of parent)
        capabilities: parent_ctx.capabilities.for_child(),
        
        // Trace linked to parent
        trace: TraceContext {
            trace_id: parent_ctx.trace.trace_id.clone(),  // Same trace
            span_id: SpanId::new(),                        // New span
            parent_span_id: Some(parent_ctx.trace.span_id.clone()),
            ..Default::default()
        },
        
        // Communication linked
        communication: CommunicationContext {
            parent: parent_ctx.identity.workflow_id.clone(),
            ..Default::default()
        },
        
        // Fresh execution context
        execution: ExecutionContext::new(child_code),
        
        // Empty local data (child starts fresh)
        dictionary: Dictionary::default(),
        local_data: HashMap::new(),
        emitted: Vec::new(),
    }
}
```

### Rule 4: Errors Include Full Context

```rust
#[derive(Debug)]
pub struct Error {
    // What happened
    pub kind: ErrorKind,
    pub message: String,
    
    // Context at failure
    pub context: ErrorContext,
    
    // Chain of callers
    pub chain: Vec<ErrorFrame>,
}

pub struct ErrorContext {
    // WHO
    pub workflow_id: Option<WorkflowId>,
    
    // WHAT
    pub operation: String,
    pub op_index: usize,
    
    // STACK
    pub stack_depth: usize,
    pub stack_snapshot: Vec<Value>,  // Top N values
    
    // TRACE
    pub trace_id: TraceId,
    pub span_id: SpanId,
}

pub struct ErrorFrame {
    pub tool: String,
    pub context: ErrorContext,
}

// Usage
fn pop_required(stack: &mut Stack, ctx: &Context, tool: &str) -> Result<Value, Error> {
    stack.pop().map_err(|_| Error {
        kind: ErrorKind::StackUnderflow,
        message: format!("{} requires a value on stack", tool),
        context: ErrorContext {
            workflow_id: ctx.identity.workflow_id.clone(),
            operation: tool.to_string(),
            op_index: ctx.execution.op_index,
            stack_depth: stack.depth(),
            stack_snapshot: stack.peek_n(3),  // Top 3 values
            trace_id: ctx.trace.trace_id.clone(),
            span_id: ctx.trace.span_id.clone(),
        },
        chain: Vec::new(),  // Caller will add their frame
    })
}
```

### Rule 5: Trace Spans For Every Tool

```rust
// Macro for consistent tracing
macro_rules! trace_tool {
    ($ctx:expr, $tool:expr, $body:expr) => {{
        let span = $ctx.trace.enter($tool);
        span.record("stack_before", $ctx.stack_signature());
        
        let result = $body;
        
        match &result {
            Ok(_) => span.record("status", "ok"),
            Err(e) => span.record("error", e.message.as_str()),
        }
        
        span.record("stack_after", $ctx.stack_signature());
        $ctx.trace.exit(span);
        
        result
    }};
}

// Usage
fn add_tool(ctx: &mut Context, stack: &mut Stack) -> Result<(), Error> {
    trace_tool!(ctx, "add", {
        let b = pop_required(stack, ctx, "add")?;
        let a = pop_required(stack, ctx, "add")?;
        
        let result = match (a, b) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a + b),
            // ...
        };
        
        stack.push(result);
        Ok(())
    })
}
```

---

## Context Serialization

For checkpointing, context must be serializable:

```rust
#[derive(Serialize, Deserialize)]
pub struct SerializableContext {
    // Identity - fully serializable
    pub identity: IdentityContext,
    
    // Execution - serializable except for native code
    pub execution: SerializableExecutionContext,
    
    // Capabilities - serializable
    pub capabilities: CapabilityContext,
    
    // Communication - handles mapped to IDs
    pub communication: SerializableCommunicationContext,
    
    // Trace - optionally included
    pub trace: Option<TraceContext>,
    
    // User data
    pub dictionary: SerializableDictionary,
    pub local_data: HashMap<String, Value>,
    pub emitted: Vec<ContextUpdate>,
}

impl Context {
    pub fn serialize(&self) -> Result<SerializableContext, Error> {
        // Convert handles to IDs
        // Exclude non-serializable native tools
        // Optionally include trace
    }
    
    pub fn deserialize(data: SerializableContext, runtime: &Runtime) -> Result<Self, Error> {
        // Restore handles from IDs
        // Restore native tools from registry
        // Restore trace context
    }
}
```

---

## Observing Context

Tools for inspecting context:

```rust
// Get current context as value
fn context_tool(ctx: &Context) -> Value {
    Value::Map(indexmap! {
        "workflow_id".into() => ctx.identity.workflow_id.map(Value::from).unwrap_or(Value::Null),
        "parent_id".into() => ctx.identity.parent_id.map(Value::from).unwrap_or(Value::Null),
        "depth".into() => Value::Int(ctx.identity.depth as i64),
        "capabilities".into() => Value::List(
            ctx.capabilities.capabilities.iter()
                .map(|c| Value::Text(c.to_string()))
                .collect()
        ),
        "stack_depth".into() => Value::Int(ctx.execution.stack_depth as i64),
        "children_count".into() => Value::Int(ctx.communication.children.len() as i64),
    })
}

// Get specific context value
fn context_get(key: &str, ctx: &Context) -> Option<Value> {
    match key {
        "workflow_id" => ctx.identity.workflow_id.map(Value::from),
        "parent_id" => ctx.identity.parent_id.map(Value::from),
        "depth" => Some(Value::Int(ctx.identity.depth as i64)),
        "trace_id" => Some(Value::Text(ctx.trace.trace_id.to_string())),
        _ => ctx.local_data.get(key).cloned(),
    }
}
```

---

## Summary

Context flow ensures:

| Property | How |
|----------|-----|
| **Traceability** | Every operation records entry/exit in trace |
| **Debuggability** | Errors include full context chain |
| **Reproducibility** | Context is serializable for checkpoints |
| **Security** | Capabilities flow from parent to child |
| **Hierarchy** | Identity tracks full lineage |
| **Communication** | Links to parent/children maintained |

**Golden Rules:**
1. Context enters every function
2. Context updates before operations
3. Child context derives from parent
4. Errors include full context
5. Trace spans for every tool
