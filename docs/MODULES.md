# Kore Module Architecture

> **STATUS: VISION DOCUMENT**
> 
> This describes the full planned module architecture. Current implementation:
> - kore-lang: ✅ Implemented
> - kore-agent: ✅ Implemented (simplified)
> - kore-workflow: ⬜ Planned
> - kore-checkpoint: ⬜ Planned
> - kore-authoring: ⬜ Planned
> - kore-genesis: ⬜ Planned

## Design Principles for Modules

1. **Layered Abstraction**: Higher layers depend on lower layers, never the reverse
2. **Single Responsibility**: Each module does exactly one thing
3. **Full Context**: Every operation carries context from input through output
4. **Explicit Dependencies**: No hidden coupling, all dependencies in Cargo.toml
5. **Testable Isolation**: Each module testable without others

---

## Abstraction Layers

```
┌─────────────────────────────────────────────────────────────────────────┐
│  LAYER 5: ORCHESTRATION                                                 │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │ kore-genesis     │ LLM-powered goal decomposition & orchestration   ││
│  └─────────────────────────────────────────────────────────────────────┘│
│                                    │                                     │
│                    depends on: workflow, checkpoint, authoring, ai       │
└────────────────────────────────────┼─────────────────────────────────────┘
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  LAYER 4: CAPABILITIES                                                  │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐   │
│  │kore-workflow │ │kore-checkpoint│ │kore-authoring│ │ kore-ai      │   │
│  │spawn, await  │ │save, restore │ │codegen,verify│ │llm, embed    │   │
│  │send, recv    │ │branch, diff  │ │sandbox,      │ │classify      │   │
│  │emit, observe │ │history       │ │register      │ │              │   │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘   │
│                                    │                                     │
│                    depends on: runtime, primitives, context              │
└────────────────────────────────────┼─────────────────────────────────────┘
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  LAYER 3: DOMAIN EXTENSIONS                                             │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐   │
│  │ kore-net     │ │ kore-db      │ │ kore-crypto  │ │ kore-mcp     │   │
│  │http, ws      │ │sqlite, pg    │ │hash, sign    │ │tool protocol │   │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘   │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                    │
│  │ kore-resolve │ │ kore-flow    │ │ kore-trace   │                    │
│  │schema, glob  │ │conditionals  │ │logging,spans │                    │
│  └──────────────┘ └──────────────┘ └──────────────┘                    │
│                                    │                                     │
│                    depends on: runtime, primitives                       │
└────────────────────────────────────┼─────────────────────────────────────┘
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  LAYER 2: STANDARD LIBRARY                                              │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                    │
│  │ kore-std     │ │kore-introspect│ │kore-registry│                    │
│  │math, text    │ │type checks   │ │tool catalog  │                    │
│  │list, map     │ │tool info     │ │discovery     │                    │
│  └──────────────┘ └──────────────┘ └──────────────┘                    │
│                                    │                                     │
│                    depends on: runtime only                              │
└────────────────────────────────────┼─────────────────────────────────────┘
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  LAYER 1: RUNTIME (kore crate)                                          │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │ value.rs    │ 10 value types with serialization                  │   │
│  │ stack.rs    │ LIFO stack with operations                         │   │
│  │ op.rs       │ 4 operations: Push, Call, Quote, If                │   │
│  │ effect.rs   │ Type system for tool composition                   │   │
│  │ tool.rs     │ Native and composed tool definitions               │   │
│  │ context.rs  │ Execution environment with capabilities            │   │
│  │ executor.rs │ Main execution loop                                │   │
│  │ builtins.rs │ 9 core tools (dup, drop, swap, etc.)               │   │
│  │ error.rs    │ Error types with full context                      │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                    │                                     │
│                    depends on: nothing (foundation)                      │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Layer 1: Runtime (`kore` crate)

The foundation. No dependencies on other kore crates.

### Module Breakdown

```
src/
├── lib.rs          # Public API, re-exports
├── value.rs        # Value enum and type operations
├── stack.rs        # Stack data structure
├── op.rs           # Operation enum (Push, Call, Quote, If)
├── effect.rs       # Effect type and parsing
├── tool.rs         # Tool trait and implementations
├── context.rs      # Context struct with identity, dict, capabilities
├── executor.rs     # execute() function - the main loop
├── builtins.rs     # 9 built-in tools
└── error.rs        # Error types
```

### value.rs

```rust
// Responsibility: Define all value types and their operations
// Depends on: nothing internal

pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    List(Vec<Value>),
    Map(IndexMap<String, Value>),
    Quote(Vec<Op>),
    Handle(Handle),      // For workflow handles
    Error(ErrorValue),
}

// Full context: every value knows its type
impl Value {
    pub fn type_name(&self) -> &'static str;
    pub fn as_int(&self) -> Option<i64>;
    // ... type-safe accessors for each variant
}
```

### stack.rs

```rust
// Responsibility: LIFO stack for passing values between tools
// Depends on: value.rs

pub struct Stack {
    values: Vec<Value>,
}

// Full context: stack operations report what they did
impl Stack {
    pub fn push(&mut self, value: Value);
    pub fn pop(&mut self) -> Result<Value, Error>;  // Error includes stack state
    pub fn peek(&self) -> Option<&Value>;
    pub fn depth(&self) -> usize;
    
    // For debugging/tracing
    pub fn values(&self) -> &[Value];
}
```

### op.rs

```rust
// Responsibility: The 4 fundamental operations
// Depends on: value.rs

pub enum Op {
    Push(Value),           // Push value onto stack
    Call(String),          // Call tool by name
    Quote(Vec<Op>),        // Nested quotation
    If,                    // Conditional execution
}

// Full context: operations carry their intent
impl Op {
    pub fn push<V: Into<Value>>(v: V) -> Self;
    pub fn call(name: &str) -> Self;
    pub fn quote(ops: Vec<Op>) -> Self;
}
```

### effect.rs

```rust
// Responsibility: Type system for tool composition
// Depends on: nothing internal

pub struct Effect {
    pub inputs: Vec<TypedParam>,
    pub outputs: Vec<TypedParam>,
}

pub struct TypedParam {
    pub name: String,
    pub type_name: String,
}

// Full context: effects describe complete I/O contract
impl Effect {
    pub fn parse(s: &str) -> Result<Effect, ParseError>;
    pub fn composes_with(&self, other: &Effect) -> bool;
    pub fn to_string(&self) -> String;
}
```

### tool.rs

```rust
// Responsibility: Define what a tool is
// Depends on: value.rs, op.rs, effect.rs, context.rs, stack.rs

pub struct Tool {
    pub name: String,
    pub effect: Option<Effect>,
    pub implementation: ToolImpl,
}

pub enum ToolImpl {
    Native(NativeFn),
    Composed(Vec<Op>),
}

// Full context: tools carry their entire definition
impl Tool {
    pub fn native(name: &str, effect: Option<Effect>, f: NativeFn) -> Self;
    pub fn composed(name: &str, effect: Option<Effect>, ops: Vec<Op>) -> Self;
}
```

### context.rs

```rust
// Responsibility: Execution environment
// Depends on: tool.rs

pub struct Context {
    pub identity: Identity,
    pub dictionary: Dictionary,
    pub capabilities: Capabilities,
}

pub struct Identity {
    pub tenant_id: TenantId,
    pub workflow_id: Option<WorkflowId>,
    pub parent_id: Option<WorkflowId>,
}

pub struct Dictionary {
    tools: HashMap<String, Tool>,
}

// Full context: context carries all state needed for execution
impl Context {
    pub fn resolve(&self, name: &str) -> Option<&Tool>;
    pub fn register(&mut self, tool: Tool);
}
```

### executor.rs

```rust
// Responsibility: Execute operations
// Depends on: all other modules

pub async fn execute(
    ops: &[Op],
    stack: Stack,
    ctx: Context
) -> Result<(Stack, Context), Error>;

// Full context: errors include:
// - Which op failed
// - Stack state at failure
// - Tool that was being called
// - Full execution trace
```

### error.rs

```rust
// Responsibility: Error types with full context
// Depends on: value.rs, op.rs

#[derive(Debug)]
pub enum Error {
    StackUnderflow {
        tool: String,
        expected: usize,
        actual: usize,
    },
    TypeMismatch {
        tool: String,
        param: String,
        expected: String,
        actual: String,
    },
    ToolNotFound {
        name: String,
        available: Vec<String>,  // Help user find similar
    },
    EffectMismatch {
        tool: String,
        expected: Effect,
        actual: Effect,
    },
    // ... with full context in each variant
}
```

---

## Layer 2: Standard Library

### kore-std

```
kore-std/
└── src/
    ├── lib.rs          # Re-exports all tool groups
    ├── math.rs         # add, sub, mul, div, mod, neg, abs
    ├── text.rs         # concat, split, join, trim, upper, lower
    ├── list.rs         # len, get, set, append, prepend, slice
    ├── map.rs          # keys, values, get, set, has
    ├── compare.rs      # eq, neq, lt, gt, lte, gte
    └── logic.rs        # and, or, not, xor
```

Each module:
- Depends only on `kore`
- Exports a `tools() -> Vec<Tool>` function
- Each tool has explicit Effect

### kore-introspect

```
kore-introspect/
└── src/
    ├── lib.rs
    ├── types.rs        # type, is-null, is-int, is-list, etc.
    ├── tools.rs        # tool-exists, tool-effect, list-tools
    └── stack.rs        # stack-depth, stack-peek
```

Purpose: Runtime reflection on types and tools

### kore-registry

```
kore-registry/
└── src/
    ├── lib.rs
    ├── registry.rs     # Central tool catalog
    ├── discovery.rs    # Find tools by effect, tag, category
    └── metadata.rs     # Tool descriptions, examples, tags
```

Purpose: Tool discovery and organization

---

## Layer 3: Domain Extensions

### kore-net

```
kore-net/
└── src/
    ├── lib.rs
    ├── http.rs         # http-get, http-post, http-request
    ├── websocket.rs    # ws-connect, ws-send, ws-recv, ws-close
    └── url.rs          # url-parse, url-encode, url-decode
```

### kore-db

```
kore-db/
└── src/
    ├── lib.rs
    ├── connection.rs   # db-connect, db-close
    ├── query.rs        # db-query, db-execute
    ├── transaction.rs  # db-begin, db-commit, db-rollback
    └── adapters/
        ├── sqlite.rs
        └── postgres.rs
```

### kore-crypto

```
kore-crypto/
└── src/
    ├── lib.rs
    ├── hash.rs         # sha256, blake3, md5
    ├── sign.rs         # sign, verify (ed25519)
    ├── encrypt.rs      # encrypt, decrypt (aes-gcm)
    └── random.rs       # random-bytes, random-int
```

### kore-mcp

```
kore-mcp/
└── src/
    ├── lib.rs
    ├── protocol.rs     # MCP message types
    ├── server.rs       # Run as MCP server
    ├── client.rs       # Connect to MCP server
    └── bridge.rs       # Bridge external MCP tools into kore
```

### kore-trace

```
kore-trace/
└── src/
    ├── lib.rs
    ├── span.rs         # trace-start, trace-end, trace-event
    ├── logger.rs       # log-debug, log-info, log-warn, log-error
    └── export.rs       # Export to various backends
```

---

## Layer 4: Capabilities (NEW - Phases 1-4)

### kore-workflow

```
kore-workflow/
└── src/
    ├── lib.rs              # Public API
    │
    ├── types/              # Core type definitions
    │   ├── mod.rs
    │   ├── handle.rs       # WorkflowHandle - unique ID for workflow
    │   ├── message.rs      # Message - typed workflow communication
    │   ├── status.rs       # WorkflowStatus - Running, Waiting, Complete, Failed
    │   └── definition.rs   # WorkflowDef - name, effect, body
    │
    ├── runtime/            # Execution infrastructure
    │   ├── mod.rs
    │   ├── runtime.rs      # WorkflowRuntime - manages all workflows
    │   ├── executor.rs     # Execute single workflow
    │   ├── scheduler.rs    # Decide which workflow runs next
    │   └── registry.rs     # Track active workflow handles
    │
    ├── messaging/          # Inter-workflow communication
    │   ├── mod.rs
    │   ├── queue.rs        # MessageQueue - per-workflow inbox
    │   ├── router.rs       # Route messages between workflows
    │   └── patterns.rs     # Request-response, pub-sub patterns
    │
    ├── supervision/        # Fault tolerance (Erlang-inspired)
    │   ├── mod.rs
    │   ├── supervisor.rs   # Supervise child workflows
    │   ├── strategy.rs     # Restart strategies (one-for-one, all-for-one)
    │   └── monitor.rs      # Monitor workflow health
    │
    └── tools/              # User-facing tools
        ├── mod.rs
        ├── spawn.rs        # spawn: (quote -- handle)
        ├── await.rs        # await: (handle -- result)
        ├── send.rs         # send: (handle message --)
        ├── recv.rs         # recv: (-- message), recv-timeout
        ├── self.rs         # self: (-- handle)
        ├── parent.rs       # parent: (-- handle-or-null)
        ├── status.rs       # status: (handle -- status)
        └── cancel.rs       # cancel: (handle --)
```

**Context Flow in Workflow:**
```rust
// Every workflow carries its full context
struct WorkflowState {
    // Identity - WHO am I?
    handle: WorkflowHandle,
    parent: Option<WorkflowHandle>,
    children: Vec<WorkflowHandle>,
    
    // Execution - WHAT am I running?
    definition: WorkflowDef,
    current_op: usize,
    stack: Stack,
    
    // Communication - WHO am I talking to?
    inbox: MessageQueue,
    
    // Context - WHAT have I learned?
    local_context: Context,
    emitted_context: Vec<ContextUpdate>,
    
    // History - WHAT have I done?
    op_trace: Vec<TraceEntry>,
}
```

### kore-checkpoint

```
kore-checkpoint/
└── src/
    ├── lib.rs
    │
    ├── types/
    │   ├── mod.rs
    │   ├── checkpoint.rs   # Checkpoint struct - complete system snapshot
    │   ├── metadata.rs     # Label, timestamp, parent checkpoint
    │   └── diff.rs         # Difference between two checkpoints
    │
    ├── capture/            # State serialization
    │   ├── mod.rs
    │   ├── workflow.rs     # Serialize workflow state
    │   ├── registry.rs     # Serialize tool registry
    │   ├── context.rs      # Serialize context
    │   └── message.rs      # Serialize pending messages
    │
    ├── storage/            # Persistence backends
    │   ├── mod.rs
    │   ├── trait.rs        # CheckpointStore trait
    │   ├── memory.rs       # InMemoryStore - for testing
    │   ├── file.rs         # FileStore - JSON files
    │   └── sqlite.rs       # SQLiteStore - production
    │
    ├── restore/            # State restoration
    │   ├── mod.rs
    │   ├── workflow.rs     # Restore workflow state
    │   ├── registry.rs     # Restore tool registry
    │   └── validate.rs     # Validate checkpoint integrity
    │
    └── tools/
        ├── mod.rs
        ├── checkpoint.rs   # checkpoint: (label --)
        ├── restore.rs      # restore: (label --)
        ├── branch.rs       # branch: (label -- handle)
        ├── history.rs      # history: (-- checkpoints)
        └── diff.rs         # diff: (label1 label2 -- changes)
```

**Context Flow in Checkpoint:**
```rust
// Checkpoint captures EVERYTHING needed to resume
struct Checkpoint {
    // Metadata - WHAT is this checkpoint?
    label: String,
    timestamp: Instant,
    parent_checkpoint: Option<String>,
    
    // System state - WHAT was running?
    workflows: HashMap<WorkflowHandle, WorkflowState>,
    tool_registry: ToolRegistry,
    
    // Relationships - HOW were they connected?
    parent_child: HashMap<WorkflowHandle, Vec<WorkflowHandle>>,
    
    // Messages - WHAT was in flight?
    pending_messages: Vec<(WorkflowHandle, Message)>,
    
    // Context - WHAT was known?
    aggregated_context: Vec<ContextUpdate>,
    
    // Trace - HOW did we get here?
    decision_log: Vec<Decision>,
}
```

### kore-authoring

```
kore-authoring/
└── src/
    ├── lib.rs
    │
    ├── types/
    │   ├── mod.rs
    │   ├── spec.rs         # ToolSpec - what tool should do
    │   ├── test_case.rs    # Input/output test case
    │   └── result.rs       # Verification result
    │
    ├── generation/         # Code generation
    │   ├── mod.rs
    │   ├── prompt.rs       # LLM prompt for codegen
    │   ├── parser.rs       # Parse generated code
    │   └── validator.rs    # Syntax/type validation
    │
    ├── sandbox/            # Isolated execution
    │   ├── mod.rs
    │   ├── environment.rs  # Sandbox environment setup
    │   ├── limits.rs       # Time, memory, recursion limits
    │   ├── capabilities.rs # Restricted capability set
    │   └── executor.rs     # Execute in sandbox
    │
    ├── verification/       # Tool verification
    │   ├── mod.rs
    │   ├── effect.rs       # Verify effect signature
    │   ├── behavior.rs     # Verify against test cases
    │   └── safety.rs       # Check for dangerous operations
    │
    └── tools/
        ├── mod.rs
        ├── codegen.rs      # codegen: (spec -- code)
        ├── sandbox.rs      # sandbox: (code inputs -- outputs)
        ├── verify.rs       # verify: (code spec -- bool)
        ├── register.rs     # register: (name effect code --)
        ├── unregister.rs   # unregister: (name --)
        ├── tool_exists.rs  # tool-exists: (name -- bool)
        ├── tool_effect.rs  # tool-effect: (name -- effect)
        └── list_tools.rs   # list-tools: (-- names)
```

**Context Flow in Authoring:**
```rust
// Authoring pipeline tracks full provenance
struct AuthoringContext {
    // Input - WHAT was requested?
    spec: ToolSpec,
    
    // Generation - HOW was it created?
    llm_provider: String,
    prompt_used: String,
    raw_response: String,
    parsed_code: Vec<Op>,
    
    // Sandbox - HOW was it tested?
    test_cases: Vec<TestCase>,
    test_results: Vec<TestResult>,
    resource_usage: ResourceUsage,
    
    // Verification - WAS it correct?
    effect_verified: bool,
    behavior_verified: bool,
    safety_checked: bool,
    
    // Registration - WHERE did it end up?
    registered_name: Option<String>,
    registration_time: Option<Instant>,
}
```

---

## Layer 5: Orchestration (NEW - Phases 5-6)

### kore-genesis

```
kore-genesis/
└── src/
    ├── lib.rs
    │
    ├── types/
    │   ├── mod.rs
    │   ├── goal.rs         # Goal representation
    │   ├── decision.rs     # Decision and reasoning
    │   ├── action.rs       # Actions genesis can take
    │   └── state.rs        # Genesis workflow state
    │
    ├── llm/                # LLM integration
    │   ├── mod.rs
    │   ├── trait.rs        # LLMProvider trait
    │   ├── claude.rs       # Claude implementation
    │   ├── openai.rs       # OpenAI implementation
    │   └── mock.rs         # Mock for testing
    │
    ├── planning/           # Goal decomposition
    │   ├── mod.rs
    │   ├── decompose.rs    # Break goal into sub-goals
    │   ├── prioritize.rs   # Order sub-goals
    │   └── validate.rs     # Validate decomposition
    │
    ├── orchestration/      # Workflow management
    │   ├── mod.rs
    │   ├── spawner.rs      # Spawn sub-workflows
    │   ├── aggregator.rs   # Aggregate child context
    │   ├── coordinator.rs  # Coordinate parallel work
    │   └── synthesizer.rs  # Combine results
    │
    ├── reflection/         # Learning from execution
    │   ├── mod.rs
    │   ├── analyze.rs      # Analyze what happened
    │   ├── adapt.rs        # Adjust strategy
    │   └── remember.rs     # Store learnings
    │
    ├── prompts/            # System prompts
    │   ├── mod.rs
    │   ├── decompose.rs    # Prompt for goal decomposition
    │   ├── decide.rs       # Prompt for decisions
    │   ├── reflect.rs      # Prompt for reflection
    │   └── authoring.rs    # Prompt for tool authoring
    │
    └── tools/
        ├── mod.rs
        ├── decompose.rs    # decompose: (goal -- sub-goals)
        ├── synthesize.rs   # synthesize: (results -- output)
        ├── decide.rs       # decide: (context options -- choice)
        └── reflect.rs      # reflect: (context -- insights)
```

**Context Flow in Genesis:**
```rust
// Genesis maintains complete situational awareness
struct GenesisState {
    // Goal - WHAT are we trying to achieve?
    original_goal: String,
    sub_goals: Vec<SubGoal>,
    
    // Children - WHO is working on what?
    active_workflows: HashMap<WorkflowHandle, WorkflowMeta>,
    completed_workflows: Vec<(WorkflowHandle, Value)>,
    
    // Context - WHAT do we know?
    aggregated_context: Vec<ContextUpdate>,
    current_findings: Vec<Finding>,
    
    // History - WHAT decisions did we make?
    decision_log: Vec<Decision>,
    checkpoint_history: Vec<String>,
    
    // Learning - WHAT have we learned?
    patterns_discovered: Vec<Pattern>,
    tools_created: Vec<String>,
    
    // Resources - WHAT have we used?
    llm_calls: usize,
    tool_calls: usize,
    elapsed_time: Duration,
}
```

---

## Directory Structure Summary

```
kore/
├── Cargo.toml                      # Workspace root
├── README.md
│
├── docs/
│   ├── ARCHITECTURE.md             # System design
│   ├── PHILOSOPHY.md               # Core principles
│   ├── PLAN.md                     # Implementation plan
│   ├── MODULES.md                  # This document
│   └── API.md                      # Tool reference
│
├── src/                            # LAYER 1: Runtime
│   ├── lib.rs
│   ├── value.rs                    # 10 value types
│   ├── stack.rs                    # LIFO stack
│   ├── op.rs                       # 4 operations
│   ├── effect.rs                   # Type system
│   ├── tool.rs                     # Tool abstraction
│   ├── context.rs                  # Execution environment
│   ├── executor.rs                 # Main loop
│   ├── builtins.rs                 # 9 built-ins
│   └── error.rs                    # Error types
│
├── crates/
│   │
│   │ # LAYER 2: Standard Library
│   ├── kore-std/                   # Math, text, list, map
│   ├── kore-introspect/            # Type checking, tool info
│   ├── kore-registry/              # Tool catalog
│   │
│   │ # LAYER 3: Domain Extensions
│   ├── kore-net/                   # HTTP, WebSocket
│   ├── kore-db/                    # Database access
│   ├── kore-crypto/                # Hashing, signing
│   ├── kore-mcp/                   # MCP protocol
│   ├── kore-resolve/               # Schema, glob
│   ├── kore-flow/                  # Conditionals
│   ├── kore-trace/                 # Logging, tracing
│   │
│   │ # LAYER 4: Capabilities (NEW)
│   ├── kore-workflow/              # Concurrent workflows
│   │   ├── src/
│   │   │   ├── types/              # Handle, Message, Status
│   │   │   ├── runtime/            # Runtime, Scheduler
│   │   │   ├── messaging/          # Queue, Router
│   │   │   ├── supervision/        # Supervisor, Strategy
│   │   │   └── tools/              # spawn, await, send, recv
│   │   └── tests/
│   │
│   ├── kore-checkpoint/            # State persistence
│   │   ├── src/
│   │   │   ├── types/              # Checkpoint, Diff
│   │   │   ├── capture/            # Serialize state
│   │   │   ├── storage/            # Store backends
│   │   │   ├── restore/            # Restore state
│   │   │   └── tools/              # checkpoint, restore, branch
│   │   └── tests/
│   │
│   ├── kore-authoring/             # Tool creation
│   │   ├── src/
│   │   │   ├── types/              # Spec, TestCase
│   │   │   ├── generation/         # LLM codegen
│   │   │   ├── sandbox/            # Isolated execution
│   │   │   ├── verification/       # Effect, behavior checking
│   │   │   └── tools/              # codegen, sandbox, verify
│   │   └── tests/
│   │
│   │ # LAYER 5: Orchestration (NEW)
│   ├── kore-genesis/               # LLM orchestration
│   │   ├── src/
│   │   │   ├── types/              # Goal, Decision, Action
│   │   │   ├── llm/                # LLM providers
│   │   │   ├── planning/           # Decomposition
│   │   │   ├── orchestration/      # Workflow coordination
│   │   │   ├── reflection/         # Learning
│   │   │   ├── prompts/            # System prompts
│   │   │   └── tools/              # decompose, synthesize
│   │   └── tests/
│   │
│   └── kore-ai/                    # LLM utilities (Layer 3, used by Layer 4-5)
│
└── tests/
    ├── integration_tests.rs
    ├── language_semantics.rs
    ├── workflow_tests.rs           # NEW
    ├── checkpoint_tests.rs         # NEW
    ├── authoring_tests.rs          # NEW
    └── genesis_tests.rs            # NEW
```

---

## Context Tracing

Every operation in Kore carries full context. Here's how context flows:

### Input → Processing → Output

```rust
// Example: spawn tool execution

// 1. INPUT CONTEXT
//    - Who called spawn? (workflow handle, or root)
//    - What stack state? (quote on top)
//    - What capabilities? (does caller have SpawnWorkflow?)

// 2. PROCESSING CONTEXT
//    - What quote was popped?
//    - What handle was assigned?
//    - What parent-child relationship created?
//    - What default context for new workflow?

// 3. OUTPUT CONTEXT
//    - What handle was pushed to stack?
//    - What workflow was registered?
//    - What trace entry was logged?

fn spawn_tool(stack: &mut Stack, ctx: &mut Context) -> Result<(), Error> {
    // Capture input context
    let caller = ctx.identity.workflow_id.clone();
    let stack_depth_before = stack.depth();
    
    // Process
    let quote = stack.pop().context("spawn requires quote on stack")?;
    let quote = quote.as_quote().context("spawn requires Quote type")?;
    
    let child_handle = WorkflowHandle::new();
    
    let child_context = Context {
        identity: Identity {
            tenant_id: ctx.identity.tenant_id.clone(),
            workflow_id: Some(child_handle.clone()),
            parent_id: caller.clone(),  // CONTEXT: parent relationship
        },
        ..Default::default()
    };
    
    // Register and spawn
    ctx.runtime.spawn(child_handle.clone(), quote, child_context)?;
    
    // Output
    stack.push(Value::Handle(child_handle.clone()));
    
    // Trace (full context captured)
    ctx.trace.record(TraceEntry::Spawn {
        caller: caller,
        child: child_handle,
        stack_depth_before,
        stack_depth_after: stack.depth(),
    });
    
    Ok(())
}
```

### Error Context Chain

```rust
// Errors always include full context

Error::SpawnFailed {
    // WHO was trying to spawn?
    caller: WorkflowHandle,
    
    // WHAT were they trying to spawn?
    quote: Vec<Op>,
    
    // WHY did it fail?
    reason: SpawnError,
    
    // WHAT was the system state?
    active_workflows: usize,
    memory_usage: usize,
    
    // HOW do we recover?
    suggestion: String,
}
```

---

## Dependency Matrix

Shows which crates depend on which:

|               | kore | std | introspect | workflow | checkpoint | authoring | genesis |
|---------------|:----:|:---:|:----------:|:--------:|:----------:|:---------:|:-------:|
| kore          |  -   |     |            |          |            |           |         |
| kore-std      |  ✓   |  -  |            |          |            |           |         |
| kore-introspect| ✓  |     |     -      |          |            |           |         |
| kore-workflow |  ✓   |     |            |    -     |            |           |         |
| kore-checkpoint| ✓  |     |            |    ✓     |     -      |           |         |
| kore-authoring|  ✓   |     |     ✓      |    ✓     |            |     -     |         |
| kore-genesis  |  ✓   |     |     ✓      |    ✓     |     ✓      |     ✓     |    -    |

**Key observations:**
- `kore` is the foundation, depends on nothing
- `kore-genesis` is the capstone, depends on almost everything
- Layers only depend downward, never upward

---

## Summary

This module architecture ensures:

1. **Clear Abstraction Layers**: 5 layers from runtime to orchestration
2. **Single Responsibility**: Each module/file has one purpose
3. **Full Context**: Every operation carries input→processing→output context
4. **Explicit Dependencies**: Dependency matrix shows all relationships
5. **Testable Isolation**: Each layer testable independently
6. **Directory = Abstraction**: File structure mirrors conceptual structure
