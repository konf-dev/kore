# Kore: Autonomous Agent Substrate Architecture

## Vision

**Kore** is a minimal, stack-based runtime designed to be the substrate for autonomous, self-evolving AI agents. The goal is to create a system where:

1. A single prompt interaction initiates a "living, breathing" collection of tools
2. Genesis workflows decompose complex goals into hierarchical sub-workflows
3. Workflows are composable tools that run concurrently
4. The system can extend itself by authoring new tools
5. Checkpoints enable exploration, backtracking, and learning from alternative paths

---

## Related Documents

| Document | Purpose |
|----------|---------|
| [PHILOSOPHY.md](PHILOSOPHY.md) | The five principles guiding all design decisions |
| [MODULES.md](MODULES.md) | Detailed module breakdown and abstraction layers |
| [CONTEXT_FLOW.md](CONTEXT_FLOW.md) | How context flows from input through execution |
| [PLAN.md](PLAN.md) | Week-by-week implementation checklist |

---

## Literature Foundation

### Key Research Informing This Architecture

| Paper | Key Insight | Application to Kore |
|-------|-------------|---------------------|
| **ReAct** (Yao et al., 2022) | Interleave reasoning traces with actions | Workflows emit context upward for LLM decision-making |
| **Tree of Thoughts** (Yao et al., 2023) | Explore multiple reasoning paths, backtrack when needed | Checkpoint system enables branching and exploration |
| **Generative Agents** (Park et al., 2023) | Memory, reflection, and planning architecture | Context aggregation and reflection in genesis workflow |
| **LLM Autonomous Agents Survey** (Wang et al., 2023) | Unified framework: Profile, Memory, Planning, Action | Maps to: Effect signatures, Context, Genesis LLM, Tool execution |
| **More Agents Is All You Need** (Li et al., 2024) | Performance scales with agent count via sampling/voting | Parallel workflows with result synthesis |
| **Actor Model** (Hewitt, 1973) | Message-passing concurrency, no shared state | Workflow isolation, async messaging, capability-based |
| **Erlang/OTP** (Ericsson) | Supervision trees, fault tolerance, behaviors | Workflow supervision, restart strategies, standard behaviors |

### Core Principles from Literature

1. **Actor Model Fundamentals**
   - Each actor (workflow) can: send messages, create new actors, designate next behavior
   - No shared state - only message passing
   - Inherent concurrency with async communication
   - Addresses are capabilities (security through locality)

2. **Erlang/OTP Supervision**
   - Workers do actual computation
   - Supervisors monitor and restart workers
   - Hierarchical supervision trees for fault tolerance
   - Behaviors formalize common patterns (gen_server, gen_statem, supervisor)

3. **LLM Agent Architecture**
   - Planning: Decompose goals into sub-goals
   - Memory: Short-term (context) and long-term (checkpoints)
   - Action: Tool execution with defined effects
   - Reflection: Learn from results, adjust plans

---

## Philosophy: The Five Principles

Every component in Kore embodies these principles:

### 1. Divide Tasks Into Smallest Pieces Possible
- Each tool does exactly one thing
- Workflows are compositions of atomic tools
- Complex behavior emerges from simple building blocks
- No tool should have hidden sub-behaviors

### 2. Each Piece Does One Thing Only
- Single responsibility at every level
- Clear, testable, replaceable units
- If a tool needs to do two things, split it
- "Do one thing well" (Unix philosophy)

### 3. Clear Input/Output Description and Defined Side Effects
- Every tool has an Effect signature: `(inputs -- outputs)`
- Side effects are explicit capabilities, not hidden
- Type system enables static composition checking
- No surprises - behavior is predictable

### 4. See What Tools Are Available and Reuse Them
- Global tool registry with discoverability
- Introspection tools to explore capabilities
- Prefer composition over creation
- New tools only when truly needed

### 5. Minimal, Elegant, High Quality - Check and Verify at Each Iteration
- Test at every step
- Verify before proceeding
- Quality over quantity
- Simple solutions preferred

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              GENESIS LAYER                              │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │                     Genesis Workflow (LLM)                        │  │
│  │  • Receives initial prompt                                        │  │
│  │  • Decomposes into sub-goals                                      │  │
│  │  • Spawns child workflows                                         │  │
│  │  • Aggregates context from children                               │  │
│  │  • Makes high-level decisions                                     │  │
│  │  • Can checkpoint/restore entire system                           │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                    │                                     │
│              ┌─────────────────────┼─────────────────────┐               │
│              ▼                     ▼                     ▼               │
│  ┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐        │
│  │ Workflow A      │   │ Workflow B      │   │ Workflow C      │        │
│  │ (Research)      │   │ (Implement)     │   │ (Test)          │        │
│  │                 │   │                 │   │                 │        │
│  │ effect:         │   │ effect:         │   │ effect:         │        │
│  │ (goal -- facts) │   │ (spec -- code)  │   │ (code -- report)│        │
│  │                 │   │                 │   │                 │        │
│  │ Can spawn:      │   │ Can spawn:      │   │ Can spawn:      │        │
│  │ • Sub-workflows │   │ • Sub-workflows │   │ • Sub-workflows │        │
│  │ • Use LLM       │   │ • Use LLM       │   │ • Use LLM       │        │
│  └────────┬────────┘   └────────┬────────┘   └────────┬────────┘        │
│           │                     │                     │                  │
│           └─────────────────────┴─────────────────────┘                  │
│                                 │                                        │
│                    Context Flows UP (emit)                               │
│                    Commands Flow DOWN (spawn, send)                      │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                            WORKFLOW LAYER                                │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │  Workflow = Tool + Async Execution + Context Emission               ││
│  │                                                                      ││
│  │  Properties:                                                         ││
│  │  • Has an effect signature (inputs → outputs)                       ││
│  │  • Runs concurrently with other workflows                           ││
│  │  • Isolated context (no shared mutable state)                       ││
│  │  • Can emit context upward to parent                                ││
│  │  • Can receive messages from parent/siblings                        ││
│  │  • Is supervised (can be restarted on failure)                      ││
│  └─────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                              TOOL LAYER                                  │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐   │
│  │ Core (9)     │ │ Std          │ │ Net          │ │ AI           │   │
│  │ dup, drop    │ │ add, mul     │ │ http-get     │ │ llm-call     │   │
│  │ swap, over   │ │ concat, len  │ │ http-post    │ │ embed        │   │
│  │ rot, call    │ │ split, join  │ │ ws-connect   │ │ classify     │   │
│  │ try, unwrap  │ │ map, filter  │ │ ws-send      │ │              │   │
│  │ is-error     │ │              │ │              │ │              │   │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘   │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐   │
│  │ OS           │ │ Workflow     │ │ Authoring    │ │ Checkpoint   │   │
│  │ exec, fs-*   │ │ spawn, await │ │ codegen      │ │ checkpoint   │   │
│  │ env-get      │ │ send, recv   │ │ sandbox      │ │ restore      │   │
│  │ env-set      │ │ emit, observe│ │ verify       │ │ branch       │   │
│  │              │ │              │ │ register     │ │ history      │   │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                           CHECKPOINT LAYER                               │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │  Timeline: [S0] ──→ [S1] ──→ [S2] ──→ [S3] ──→ [S4]                ││
│  │                               │                                      ││
│  │             Branch: [S2] ──→ [S2'] ──→ [S3'] (alternative path)     ││
│  │                                                                      ││
│  │  Each checkpoint captures:                                           ││
│  │  • All workflow states                                               ││
│  │  • Tool registry state                                               ││
│  │  • Context/memory                                                    ││
│  │  • Parent-child relationships                                        ││
│  │  • Message queues                                                    ││
│  └─────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                              KORE RUNTIME                                │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │  • 4 Operations: Push, Call, Quote, If                              ││
│  │  • 10 Value Types: Null, Bool, Int, Float, Text, List, Map,        ││
│  │                    Quote, Handle, Error                             ││
│  │  • Effect System: Type-safe tool composition                        ││
│  │  • Stack-based execution (easy to checkpoint)                       ││
│  │  • Capability-based security                                        ││
│  └─────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Core Concepts

### 1. Workflow as Tool

A **Workflow** is a Tool with additional properties:

```rust
struct Workflow {
    // Tool properties
    name: String,
    effect: Effect,           // (inputs -- outputs)
    body: Vec<Op>,            // The kore code
    
    // Workflow properties  
    handle: WorkflowHandle,   // Unique identifier
    parent: Option<WorkflowHandle>,
    children: Vec<WorkflowHandle>,
    
    // Execution state
    stack: Stack,
    context: Context,
    status: WorkflowStatus,   // Running, Waiting, Complete, Failed
    
    // Communication
    inbox: MessageQueue,
    outbox: Vec<(WorkflowHandle, Message)>,
    
    // Context emission
    emitted_context: Vec<ContextUpdate>,
}
```

**Key Insight**: Because workflows ARE tools, they compose naturally. A workflow can call another workflow just like any other tool.

### 2. Genesis Workflow

The Genesis Workflow is a special workflow that:

1. **Receives the initial prompt** from the user
2. **Has LLM capabilities** for reasoning and planning
3. **Decomposes goals** into sub-goals
4. **Spawns child workflows** to handle sub-goals
5. **Aggregates context** from all children
6. **Makes decisions** based on aggregated context
7. **Can checkpoint/restore** the entire system

```
Genesis Prompt Structure:
─────────────────────────
You are the Genesis Workflow of a Kore autonomous system.

PHILOSOPHY (apply to every decision):
1. Divide tasks into smallest pieces possible
2. Each piece does one thing only  
3. Clear input/output, defined side effects
4. Reuse existing tools when possible
5. Verify quality at each step

AVAILABLE TOOLS:
[List of registered tools with effects]

CURRENT CONTEXT:
[Aggregated context from child workflows]

GOAL:
[User's original prompt]

SUB-GOALS IN PROGRESS:
[Status of spawned workflows]

DECISION REQUIRED:
[What needs to be decided]
```

### 3. Context Flow

Context flows **upward** through the hierarchy:

```
Child Workflow                 Parent Workflow
     │                              │
     │ emit(context_update)         │
     │ ─────────────────────────────>│
     │                              │
     │                              │ Aggregates context
     │                              │ from all children
     │                              │
     │                              │ emit(summary)
     │                              │ ───────────────> Grandparent
```

Context includes:
- Progress updates
- Findings/results
- Errors/issues
- Resource usage
- Learned patterns

### 4. Tool Authoring

The system can extend itself by authoring new tools:

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   codegen    │────>│   sandbox    │────>│   verify     │
│              │     │              │     │              │
│ (spec --     │     │ (code tests  │     │ (code spec   │
│  code)       │     │  -- results) │     │  -- bool)    │
└──────────────┘     └──────────────┘     └──────────────┘
                                                │
                                                ▼
                                         ┌──────────────┐
                                         │   register   │
                                         │              │
                                         │ (name code   │
                                         │  effect --)  │
                                         └──────────────┘
```

**Authoring Process**:
1. **codegen**: LLM generates tool code from specification
2. **sandbox**: Run tool in isolated environment with test cases
3. **verify**: Check that tool meets specification
4. **register**: Add to tool library if verified

### 5. Checkpoint System

Checkpoints enable exploration and backtracking:

```rust
struct Checkpoint {
    label: String,
    timestamp: Instant,
    
    // System state
    workflows: HashMap<WorkflowHandle, WorkflowState>,
    tool_registry: ToolRegistry,
    
    // Relationship graph
    parent_child: HashMap<WorkflowHandle, Vec<WorkflowHandle>>,
    
    // Message state
    pending_messages: Vec<(WorkflowHandle, Message)>,
    
    // Metadata
    context_summary: String,
    decision_log: Vec<Decision>,
}
```

**Operations**:
- `checkpoint label` - Save current state
- `restore label` - Return to saved state
- `branch label` - Fork from checkpoint (returns new handle)
- `history` - List all checkpoints
- `diff label1 label2` - Compare states

---

## Implementation Plan

### Phase 1: Workflow Primitives (Week 1-2)

**Goal**: Enable concurrent workflows that compose like tools

#### New Crate: `kore-workflow`

```rust
// Core types
pub struct WorkflowHandle(Uuid);
pub struct Message(Value);

pub enum WorkflowStatus {
    Running,
    Waiting,      // Waiting for message or child
    Complete(Value),
    Failed(Error),
}

// Workflow definition
pub struct WorkflowDef {
    pub name: String,
    pub effect: Effect,
    pub body: Vec<Op>,
}
```

#### New Tools

| Tool | Effect | Description |
|------|--------|-------------|
| `spawn` | `(quote -- handle)` | Start new workflow from quote |
| `await` | `(handle -- result)` | Wait for workflow completion |
| `send` | `(handle message --)` | Send message to workflow |
| `recv` | `(-- message)` | Receive next message (blocks) |
| `recv-timeout` | `(duration -- message-or-null)` | Receive with timeout |
| `self` | `(-- handle)` | Get current workflow handle |
| `parent` | `(-- handle-or-null)` | Get parent workflow handle |

#### Tests for Phase 1

```rust
#[tokio::test]
async fn spawn_and_await() {
    // spawn a workflow that doubles a number
    let code = r#"
        (dup add) spawn    # spawn workflow
        42 swap send       # send input
        await              # wait for result
    "#;
    assert_eq!(run(code).await, vec![Value::Int(84)]);
}

#[tokio::test]
async fn parallel_workflows() {
    // spawn two workflows, await both
    let code = r#"
        (dup add) spawn
        (dup mul) spawn
        # Stack: [handle1, handle2]
        5 over send        # send 5 to handle2
        5 rot send         # send 5 to handle1
        await swap await   # await both
        # Stack: [10, 25]
    "#;
    // Results: 5+5=10, 5*5=25
}
```

### Phase 2: Context Emission (Week 3)

**Goal**: Enable upward context flow for aggregation

#### New Tools

| Tool | Effect | Description |
|------|--------|-------------|
| `emit` | `(context --)` | Emit context to parent |
| `observe` | `(handles -- contexts)` | Get contexts from children |
| `context` | `(-- context)` | Get current context object |
| `context-get` | `(key -- value)` | Get value from context |
| `context-set` | `(key value --)` | Set value in context |

#### Context Structure

```rust
pub struct ContextUpdate {
    pub source: WorkflowHandle,
    pub timestamp: Instant,
    pub update_type: UpdateType,
    pub data: Value,
}

pub enum UpdateType {
    Progress { percent: f64, message: String },
    Finding { category: String, content: Value },
    Error { severity: Severity, message: String },
    Resource { kind: String, amount: i64 },
    Custom(String),
}
```

### Phase 3: Checkpointing (Week 4)

**Goal**: Enable save/restore for exploration

#### New Tools

| Tool | Effect | Description |
|------|--------|-------------|
| `checkpoint` | `(label --)` | Save current system state |
| `restore` | `(label --)` | Restore to checkpoint |
| `branch` | `(label -- handle)` | Fork from checkpoint |
| `history` | `(-- checkpoints)` | List checkpoints |
| `diff` | `(label1 label2 -- changes)` | Compare states |

#### Storage

```rust
pub trait CheckpointStore {
    async fn save(&self, label: &str, checkpoint: Checkpoint) -> Result<()>;
    async fn load(&self, label: &str) -> Result<Checkpoint>;
    async fn list(&self) -> Result<Vec<CheckpointMeta>>;
    async fn delete(&self, label: &str) -> Result<()>;
}

// Implementations:
// - InMemoryStore (testing)
// - FileStore (simple persistence)
// - SQLiteStore (production)
```

### Phase 4: Tool Authoring (Week 5)

**Goal**: Enable dynamic tool creation

#### New Tools

| Tool | Effect | Description |
|------|--------|-------------|
| `codegen` | `(spec -- code)` | Generate tool code |
| `sandbox` | `(code inputs -- outputs)` | Test in isolation |
| `verify` | `(code spec -- bool)` | Verify against spec |
| `register` | `(name effect code --)` | Add to registry |
| `unregister` | `(name --)` | Remove from registry |
| `tool-exists` | `(name -- bool)` | Check if tool exists |
| `tool-effect` | `(name -- effect)` | Get tool's effect |
| `list-tools` | `(-- names)` | List all tools |

#### Sandbox Architecture

```rust
pub struct Sandbox {
    // Isolated execution environment
    context: Context,  // Fresh context with limited capabilities
    timeout: Duration,
    memory_limit: usize,
    
    // What the sandboxed tool can access
    allowed_tools: HashSet<String>,
    allowed_capabilities: HashSet<Capability>,
}

impl Sandbox {
    pub async fn execute(&self, code: &[Op], inputs: Vec<Value>) 
        -> Result<Vec<Value>>;
}
```

### Phase 5: Genesis Workflow (Week 6)

**Goal**: LLM-powered orchestration

#### Genesis Architecture

```rust
pub struct GenesisWorkflow {
    // LLM interface
    llm: Box<dyn LLMProvider>,
    
    // System prompt
    system_prompt: String,
    
    // State
    goal: String,
    context_buffer: Vec<ContextUpdate>,
    decision_history: Vec<Decision>,
    
    // Children
    active_workflows: HashMap<WorkflowHandle, WorkflowMeta>,
}

pub struct Decision {
    timestamp: Instant,
    context_summary: String,
    options_considered: Vec<String>,
    chosen_action: Action,
    reasoning: String,
}

pub enum Action {
    SpawnWorkflow { name: String, goal: String },
    SendMessage { target: WorkflowHandle, message: Value },
    Checkpoint { label: String },
    Restore { label: String },
    AuthorTool { spec: ToolSpec },
    Complete { result: Value },
}
```

### Phase 6: Integration & Hardening (Week 7)

**Goal**: Production readiness

#### Tasks

1. **Error handling**: Comprehensive error types, recovery strategies
2. **Logging**: Structured logging with tracing
3. **Metrics**: Workflow counts, execution times, resource usage
4. **Testing**: Property-based tests, chaos testing
5. **Documentation**: API docs, tutorials, examples
6. **CLI**: Interactive shell, script execution
7. **Benchmarks**: Performance characterization

---

## Tool Reference

### Phase 1: Workflow Tools

```
spawn      (quote -- handle)           Start concurrent workflow
await      (handle -- result)          Wait for completion
send       (handle message --)         Send message to workflow
recv       (-- message)                Receive message (blocks)
recv-timeout (ms -- message-or-null)   Receive with timeout
self       (-- handle)                 Current workflow handle
parent     (-- handle-or-null)         Parent workflow handle
status     (handle -- status)          Get workflow status
cancel     (handle --)                 Cancel workflow
```

### Phase 2: Context Tools

```
emit       (context --)                Emit to parent
observe    (handles -- contexts)       Get child contexts
context    (-- context)                Get current context
context-get (key -- value)             Get from context
context-set (key value --)             Set in context
```

### Phase 3: Checkpoint Tools

```
checkpoint (label --)                  Save state
restore    (label --)                  Restore state
branch     (label -- handle)           Fork from checkpoint
history    (-- checkpoints)            List checkpoints
diff       (label1 label2 -- changes)  Compare states
```

### Phase 4: Authoring Tools

```
codegen    (spec -- code)              Generate tool code
sandbox    (code inputs -- outputs)    Test in isolation
verify     (code spec -- bool)         Verify tool
register   (name effect code --)       Add to registry
unregister (name --)                   Remove from registry
tool-exists (name -- bool)             Check existence
tool-effect (name -- effect)           Get effect
list-tools (-- names)                  List all tools
```

### Phase 5: Genesis Tools

```
decompose  (goal -- sub-goals)         Break down task
synthesize (results -- output)         Combine results
decide     (context options -- choice) LLM decision
reflect    (context -- insights)       Generate insights
```

---

## Genesis Prompt Template

```markdown
# Genesis Workflow System Prompt

You are the Genesis Workflow of a Kore autonomous agent system.

## Your Philosophy (Apply to EVERY decision)

1. **DIVIDE** tasks into the smallest possible pieces
2. **SINGLE PURPOSE** - each piece does exactly one thing
3. **EXPLICIT** - clear inputs, outputs, and side effects
4. **REUSE** - check existing tools before creating new ones
5. **VERIFY** - test and validate at every step

## Your Capabilities

You can:
- **spawn** new workflows to handle sub-goals
- **await** results from workflows
- **send/recv** messages to/from workflows
- **emit** context upward (if you have a parent)
- **observe** context from child workflows
- **checkpoint** the system state
- **restore** to a previous checkpoint
- **branch** to explore alternatives
- **codegen** new tools when needed
- **register** verified tools

## Available Tools

{tool_list}

## Current State

**Goal**: {goal}

**Active Workflows**:
{workflow_status}

**Recent Context**:
{context_summary}

**Checkpoint History**:
{checkpoints}

## Decision Required

{decision_prompt}

## Response Format

Think step by step:
1. What is the current situation?
2. What are the options?
3. Which option best follows the philosophy?
4. What specific action should be taken?

Then provide your action in this format:
```json
{
  "reasoning": "...",
  "action": "spawn|send|checkpoint|restore|codegen|complete",
  "params": { ... }
}
```
```

---

## Example: Building an OS

**User Prompt**: "Build an operating system with real-time scheduling, memory protection, and a microkernel architecture. Research existing approaches, implement a proof of concept, and verify it meets safety requirements."

**Genesis Decomposition**:

```
Goal: Build OS with RT scheduling, memory protection, microkernel
│
├── Workflow: Research
│   ├── Sub: Research RT scheduling algorithms
│   ├── Sub: Research memory protection techniques
│   └── Sub: Research microkernel designs
│
├── Workflow: Design
│   ├── Sub: Design kernel architecture
│   ├── Sub: Design scheduler interface
│   └── Sub: Design IPC mechanism
│
├── Workflow: Implement
│   ├── Sub: Implement bootloader
│   ├── Sub: Implement memory manager
│   ├── Sub: Implement scheduler
│   └── Sub: Implement IPC
│
├── Workflow: Test
│   ├── Sub: Unit tests
│   ├── Sub: Integration tests
│   └── Sub: Safety verification
│
└── Workflow: Document
    ├── Sub: API documentation
    └── Sub: User guide
```

**Context Flow Example**:

```
Research/RT-Scheduling ──emit──> Research ──emit──> Genesis
    │                              │                  │
    │ "Found: EDF optimal for     │ "Research        │ "Research 60%
    │  uniprocessor, O(log n)"    │  findings:       │  complete.
    │                              │  3 algorithms    │  Proceeding
    │                              │  identified"     │  to design."
```

**Checkpoint Usage**:

```
Genesis:
1. checkpoint "after-research"
2. spawn Design workflow
3. ... design progresses ...
4. Design fails validation
5. restore "after-research"  
6. spawn Design with different approach
7. ... alternative succeeds ...
```

---

## Security Model

### Capability-Based Access

```rust
pub enum Capability {
    // Tool categories
    CoreTools,        // dup, drop, swap, etc.
    MathTools,        // add, mul, etc.
    TextTools,        // concat, split, etc.
    
    // System access
    NetworkAccess,    // http-*, ws-*
    FileSystem,       // fs-*
    ProcessExec,      // exec
    Environment,      // env-get, env-set
    
    // Workflow control
    SpawnWorkflow,
    SendMessage,
    
    // Meta capabilities
    RegisterTool,     // Can add new tools
    Checkpoint,       // Can save/restore state
    Introspect,       // Can examine system state
}
```

### Sandbox Restrictions

```rust
pub struct SandboxPolicy {
    // Time limit
    max_execution_time: Duration,
    
    // Memory limit
    max_memory: usize,
    
    // Stack limit
    max_stack_depth: usize,
    
    // Recursion limit
    max_recursion: usize,
    
    // Allowed capabilities
    capabilities: HashSet<Capability>,
    
    // Allowed tools (whitelist)
    allowed_tools: Option<HashSet<String>>,
    
    // Blocked tools (blacklist)  
    blocked_tools: HashSet<String>,
}
```

---

## Testing Strategy

### Unit Tests (Per Tool)
- Each tool has comprehensive unit tests
- Test normal operation, edge cases, error conditions
- Property-based tests for composition

### Integration Tests
- Workflow spawn/await cycles
- Message passing patterns
- Context emission and aggregation
- Checkpoint/restore cycles

### System Tests
- Full genesis workflow with mock LLM
- Multi-level workflow hierarchies
- Concurrent workflow interactions
- Failure and recovery scenarios

### Chaos Tests
- Random workflow failures
- Message drops
- Timeout conditions
- Resource exhaustion

---

## Metrics & Observability

```rust
// Workflow metrics
workflow_spawn_total: Counter,
workflow_complete_total: Counter,
workflow_failed_total: Counter,
workflow_duration_seconds: Histogram,
workflow_active: Gauge,

// Message metrics
message_sent_total: Counter,
message_received_total: Counter,
message_queue_depth: Gauge,

// Checkpoint metrics
checkpoint_created_total: Counter,
checkpoint_restored_total: Counter,
checkpoint_size_bytes: Histogram,

// Tool metrics
tool_call_total: Counter,
tool_call_duration_seconds: Histogram,
tool_error_total: Counter,
```

---

## File Structure

```
kore/
├── Cargo.toml
├── README.md
├── docs/
│   ├── ARCHITECTURE.md          # This document
│   ├── PHILOSOPHY.md            # Core principles
│   ├── TUTORIAL.md              # Getting started
│   └── API.md                   # Tool reference
├── src/                         # Core runtime
│   ├── lib.rs
│   ├── value.rs
│   ├── op.rs
│   ├── stack.rs
│   ├── effect.rs
│   ├── tool.rs
│   ├── context.rs
│   ├── executor.rs
│   └── builtins.rs
├── crates/
│   ├── kore-std/               # Standard library
│   ├── kore-workflow/          # Workflow primitives (NEW)
│   ├── kore-checkpoint/        # Checkpointing (NEW)
│   ├── kore-authoring/         # Tool authoring (NEW)
│   ├── kore-genesis/           # Genesis workflow (NEW)
│   ├── kore-net/               # Networking
│   ├── kore-ai/                # LLM integration
│   ├── kore-trace/             # Observability
│   └── ...
└── tests/
    ├── integration_tests.rs
    ├── language_semantics.rs
    ├── workflow_tests.rs        # NEW
    └── genesis_tests.rs         # NEW
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] Can spawn workflow from quote
- [ ] Can await workflow result
- [ ] Can send/receive messages
- [ ] Workflows run concurrently
- [ ] 10+ tests passing

### Phase 2 Complete When:
- [ ] Context emission works
- [ ] Parent can observe children
- [ ] Context aggregation works
- [ ] 10+ tests passing

### Phase 3 Complete When:
- [ ] Can checkpoint system state
- [ ] Can restore to checkpoint
- [ ] Can branch from checkpoint
- [ ] 10+ tests passing

### Phase 4 Complete When:
- [ ] Can generate tool code
- [ ] Can sandbox test tools
- [ ] Can verify tools
- [ ] Can register new tools
- [ ] 10+ tests passing

### Phase 5 Complete When:
- [ ] Genesis workflow runs
- [ ] Can decompose goals
- [ ] Can spawn child workflows
- [ ] Can aggregate context
- [ ] End-to-end test passes

### Phase 6 Complete When:
- [ ] All error paths tested
- [ ] Logging comprehensive
- [ ] Metrics exposed
- [ ] Documentation complete
- [ ] CLI usable

---

## Next Steps

1. Review and approve this architecture
2. Begin Phase 1 implementation
3. Create `kore-workflow` crate
4. Implement spawn/await primitives
5. Write tests for workflow composition

**Ready to proceed with Phase 1?**
