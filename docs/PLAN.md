# Kore Development Plan

> **STATUS: VISION DOCUMENT**
> 
> This is the original development plan. Current work has taken a simpler path.
> For current state, see [../README.md](../README.md) and [../crates/kore-agent/README.md](../crates/kore-agent/README.md)

## Project Timeline

**Duration**: 7 weeks  
**Goal**: Self-evolving autonomous agent substrate

---

## Phase Overview

| Phase | Weeks | Focus | Key Deliverables |
|-------|-------|-------|------------------|
| 1 | 1-2 | Workflow Primitives | spawn, await, send, recv |
| 2 | 3 | Context Emission | emit, observe, context flow |
| 3 | 4 | Checkpointing | checkpoint, restore, branch |
| 4 | 5 | Tool Authoring | codegen, sandbox, verify, register |
| 5 | 6 | Genesis Workflow | LLM integration, orchestration |
| 6 | 7 | Integration | Testing, docs, hardening |

---

## Phase 1: Workflow Primitives (Weeks 1-2)

### Goal
Enable concurrent workflows that compose like regular tools.

### Week 1: Foundation

#### Day 1-2: Core Types
- [ ] Create `crates/kore-workflow/` crate
- [ ] Define `WorkflowHandle` type
- [ ] Define `Message` type  
- [ ] Define `WorkflowStatus` enum
- [ ] Add `WorkflowHandle` to kore `Value` enum (as Handle variant)
- [ ] Tests: Type creation and serialization

#### Day 3-4: Workflow Runtime
- [ ] Implement `WorkflowRuntime` struct
- [ ] Workflow spawning logic
- [ ] Workflow execution loop
- [ ] Handle registry (track running workflows)
- [ ] Tests: Spawn and execute simple workflow

#### Day 5: spawn Tool
- [ ] Implement `spawn` tool: `(quote -- handle)`
- [ ] Async workflow execution
- [ ] Return handle immediately
- [ ] Tests: Spawn returns valid handle

### Week 2: Communication

#### Day 1-2: await Tool
- [ ] Implement `await` tool: `(handle -- result)`
- [ ] Block until workflow completes
- [ ] Return workflow result or error
- [ ] Tests: Spawn then await

#### Day 3-4: Messaging
- [ ] Implement message queues per workflow
- [ ] `send` tool: `(handle message --)`
- [ ] `recv` tool: `(-- message)` (blocking)
- [ ] `recv-timeout` tool: `(ms -- message-or-null)`
- [ ] Tests: Send/receive patterns

#### Day 5: Utility Tools
- [ ] `self` tool: `(-- handle)`
- [ ] `parent` tool: `(-- handle-or-null)`
- [ ] `status` tool: `(handle -- status)`
- [ ] `cancel` tool: `(handle --)`
- [ ] Integration tests

### Phase 1 Deliverables
- [ ] `kore-workflow` crate
- [ ] 8 new tools: spawn, await, send, recv, recv-timeout, self, parent, status, cancel
- [ ] 20+ unit tests
- [ ] 5+ integration tests
- [ ] Documentation for all tools

### Phase 1 Success Criteria
```rust
// This test must pass:
#[tokio::test]
async fn spawn_await_basic() {
    let code = r#"
        (recv dup add) spawn  # workflow that doubles input
        21 swap send          # send 21
        await                 # wait for result
    "#;
    assert_eq!(run(code).await, vec![Value::Int(42)]);
}
```

---

## Phase 2: Context Emission (Week 3)

### Goal
Enable upward context flow for hierarchical aggregation.

### Day 1-2: Context Types
- [ ] Define `ContextUpdate` struct
- [ ] Define `UpdateType` enum (Progress, Finding, Error, etc.)
- [ ] Add context buffer to workflow
- [ ] Tests: Context type creation

### Day 3: emit Tool
- [ ] Implement `emit` tool: `(context --)`
- [ ] Send context update to parent
- [ ] Buffer in parent's context store
- [ ] Tests: Emit to parent

### Day 4: observe Tool
- [ ] Implement `observe` tool: `(handles -- contexts)`
- [ ] Collect context from child workflows
- [ ] Return as list of context objects
- [ ] Tests: Observe children

### Day 5: Context Management
- [ ] `context` tool: `(-- context)`
- [ ] `context-get` tool: `(key -- value)`
- [ ] `context-set` tool: `(key value --)`
- [ ] Integration tests for context flow

### Phase 2 Deliverables
- [ ] Context emission system
- [ ] 5 new tools: emit, observe, context, context-get, context-set
- [ ] 15+ unit tests
- [ ] Documentation

### Phase 2 Success Criteria
```rust
#[tokio::test]
async fn context_flows_up() {
    let code = r#"
        # Spawn child that emits context
        (
            { "status": "working" } emit
            42
        ) spawn
        
        # Wait a bit, then observe
        100 sleep
        1 list observe  # observe one child
        
        # Check context received
    "#;
    // Context should contain status: working
}
```

---

## Phase 3: Checkpointing (Week 4)

### Goal
Enable save/restore for exploration and backtracking.

### Day 1: Checkpoint Types
- [ ] Define `Checkpoint` struct
- [ ] Define `CheckpointStore` trait
- [ ] Implement `InMemoryStore`
- [ ] Tests: Store operations

### Day 2: checkpoint Tool
- [ ] Implement `checkpoint` tool: `(label --)`
- [ ] Serialize all workflow states
- [ ] Serialize tool registry
- [ ] Save to store
- [ ] Tests: Create checkpoint

### Day 3: restore Tool
- [ ] Implement `restore` tool: `(label --)`
- [ ] Load checkpoint from store
- [ ] Restore all workflow states
- [ ] Restore tool registry
- [ ] Tests: Restore works

### Day 4: branch Tool
- [ ] Implement `branch` tool: `(label -- handle)`
- [ ] Fork from checkpoint
- [ ] Create new workflow tree
- [ ] Return handle to branched genesis
- [ ] Tests: Branch and explore

### Day 5: Utility & Storage
- [ ] `history` tool: `(-- checkpoints)`
- [ ] `diff` tool: `(label1 label2 -- changes)`
- [ ] Implement `FileStore`
- [ ] Integration tests

### Phase 3 Deliverables
- [ ] `kore-checkpoint` crate
- [ ] 5 new tools: checkpoint, restore, branch, history, diff
- [ ] 2 store implementations (Memory, File)
- [ ] 20+ tests

### Phase 3 Success Criteria
```rust
#[tokio::test]
async fn checkpoint_restore_works() {
    let code = r#"
        42 "my-value" context-set
        "before-change" checkpoint
        
        100 "my-value" context-set
        "my-value" context-get  # 100
        
        "before-change" restore
        "my-value" context-get  # 42 (restored!)
    "#;
    assert_eq!(run(code).await, vec![Value::Int(42)]);
}
```

---

## Phase 4: Tool Authoring (Week 5)

### Goal
Enable dynamic tool creation and verification.

### Day 1: Sandbox
- [ ] Create `crates/kore-authoring/` crate
- [ ] Implement `Sandbox` struct
- [ ] Isolated execution environment
- [ ] Resource limits (time, memory)
- [ ] Tests: Sandbox execution

### Day 2: codegen Tool
- [ ] Implement `codegen` tool: `(spec -- code)`
- [ ] Integrate with LLM for code generation
- [ ] Return generated kore code as quote
- [ ] Tests: Generate simple tools

### Day 3: sandbox Tool
- [ ] Implement `sandbox` tool: `(code inputs -- outputs)`
- [ ] Execute code in sandbox
- [ ] Return results or error
- [ ] Tests: Sandboxed execution

### Day 4: verify Tool
- [ ] Implement `verify` tool: `(code spec -- bool)`
- [ ] Check code meets specification
- [ ] Run test cases
- [ ] Verify effect signature
- [ ] Tests: Verification logic

### Day 5: Registry Tools
- [ ] `register` tool: `(name effect code --)`
- [ ] `unregister` tool: `(name --)`
- [ ] `tool-exists` tool: `(name -- bool)`
- [ ] `tool-effect` tool: `(name -- effect)`
- [ ] `list-tools` tool: `(-- names)`
- [ ] Integration tests

### Phase 4 Deliverables
- [ ] `kore-authoring` crate
- [ ] Sandbox implementation
- [ ] 8 new tools
- [ ] LLM integration for codegen
- [ ] 25+ tests

### Phase 4 Success Criteria
```rust
#[tokio::test]
async fn author_new_tool() {
    let code = r#"
        # Generate a doubling tool
        "A tool that doubles an integer" codegen
        
        # Test in sandbox
        dup [21] sandbox
        # Should return [42]
        
        # Verify it works
        "(n:Int -- doubled:Int)" verify
        
        # Register if verified
        ("double" swap) if-true register
        
        # Use the new tool!
        21 double
    "#;
    assert_eq!(run(code).await, vec![Value::Int(42)]);
}
```

---

## Phase 5: Genesis Workflow (Week 6)

### Goal
LLM-powered orchestration of the entire system.

### Day 1: Genesis Structure
- [ ] Create `crates/kore-genesis/` crate
- [ ] Define `GenesisWorkflow` struct
- [ ] Define `Decision` and `Action` types
- [ ] Basic genesis loop

### Day 2: LLM Integration
- [ ] Define `LLMProvider` trait
- [ ] Implement for Claude/GPT
- [ ] Genesis system prompt
- [ ] Decision parsing

### Day 3: Goal Decomposition
- [ ] `decompose` tool: `(goal -- sub-goals)`
- [ ] LLM-powered goal breakdown
- [ ] Sub-goal validation
- [ ] Tests: Decomposition

### Day 4: Orchestration
- [ ] `synthesize` tool: `(results -- output)`
- [ ] `decide` tool: `(context options -- choice)`
- [ ] Genesis decision loop
- [ ] Child workflow management

### Day 5: Integration
- [ ] Connect all phases
- [ ] End-to-end genesis workflow
- [ ] Mock LLM tests
- [ ] Real LLM tests (manual)

### Phase 5 Deliverables
- [ ] `kore-genesis` crate
- [ ] Genesis workflow implementation
- [ ] LLM provider abstraction
- [ ] 4 new tools: decompose, synthesize, decide, reflect
- [ ] 20+ tests

### Phase 5 Success Criteria
```rust
#[tokio::test]
async fn genesis_decomposes_goal() {
    let genesis = GenesisWorkflow::new(mock_llm());
    
    let result = genesis.run("Calculate the factorial of 5").await;
    
    // Genesis should have:
    // 1. Decomposed the goal
    // 2. Either found factorial tool or composed one
    // 3. Executed and returned 120
    assert_eq!(result, Value::Int(120));
}
```

---

## Phase 6: Integration & Hardening (Week 7)

### Goal
Production readiness.

### Day 1: Error Handling
- [ ] Comprehensive error types
- [ ] Error context and traces
- [ ] Recovery strategies
- [ ] Tests: Error paths

### Day 2: Logging & Tracing
- [ ] Structured logging throughout
- [ ] Trace spans for workflows
- [ ] Log aggregation
- [ ] Tests: Log verification

### Day 3: Metrics
- [ ] Prometheus metrics
- [ ] Workflow metrics
- [ ] Tool metrics
- [ ] Dashboard setup

### Day 4: Documentation
- [ ] API documentation (rustdoc)
- [ ] Tutorial document
- [ ] Example programs
- [ ] README updates

### Day 5: CLI & Polish
- [ ] CLI for running kore scripts
- [ ] REPL for interactive use
- [ ] Final integration tests
- [ ] Performance benchmarks

### Phase 6 Deliverables
- [ ] Complete error handling
- [ ] Full observability
- [ ] CLI/REPL
- [ ] Documentation
- [ ] Benchmarks

---

## Testing Strategy

### Unit Tests (Per Phase)
Each phase targets 15-25 unit tests covering:
- Normal operation
- Edge cases  
- Error conditions
- Property-based tests

### Integration Tests
After each phase:
- Cross-component tests
- Workflow composition tests
- Error propagation tests

### System Tests (Phase 6)
- End-to-end scenarios
- Multi-workflow hierarchies
- Checkpoint/restore cycles
- Tool authoring flows

### Performance Tests
- Workflow spawn overhead
- Message passing throughput
- Checkpoint size/speed
- Large-scale concurrent workflows

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| LLM integration complexity | Abstract behind trait, mock for tests |
| Checkpoint size explosion | Incremental checkpoints, compression |
| Message queue bottlenecks | Bounded queues, backpressure |
| Sandbox escapes | Process isolation, capability restrictions |
| Workflow deadlocks | Timeout on all blocking operations |

---

## Dependencies Between Phases

```
Phase 1 (Workflow) ──┐
                     ├──> Phase 5 (Genesis)
Phase 2 (Context) ───┤
                     │
Phase 3 (Checkpoint)─┤
                     │
Phase 4 (Authoring)──┘
                          │
                          ▼
                    Phase 6 (Integration)
```

- Phases 1-4 can partially parallelize
- Phase 5 requires Phases 1-4
- Phase 6 requires Phase 5

---

## Weekly Checkpoints

### Week 1 Checkpoint
- [ ] `kore-workflow` crate exists
- [ ] `spawn` tool works
- [ ] Basic workflow execution

### Week 2 Checkpoint
- [ ] All workflow tools complete
- [ ] Send/receive messaging works
- [ ] Phase 1 tests pass

### Week 3 Checkpoint
- [ ] Context emission works
- [ ] Parent can observe children
- [ ] Phase 2 tests pass

### Week 4 Checkpoint
- [ ] Checkpoint/restore works
- [ ] Branch exploration works
- [ ] Phase 3 tests pass

### Week 5 Checkpoint
- [ ] Tool authoring works
- [ ] Sandbox verification works
- [ ] Phase 4 tests pass

### Week 6 Checkpoint
- [ ] Genesis workflow runs
- [ ] End-to-end test passes
- [ ] Phase 5 tests pass

### Week 7 Checkpoint
- [ ] All error handling complete
- [ ] Documentation complete
- [ ] CLI/REPL usable
- [ ] Ready for production use

---

## Definition of Done

The project is complete when:

1. **Functional**: All tools work as specified
2. **Tested**: 100+ tests passing
3. **Documented**: Full API docs, tutorial, examples
4. **Observable**: Logging, metrics, tracing
5. **Usable**: CLI/REPL for interactive use
6. **Demonstrable**: Can run genesis with real goal

---

## Getting Started

To begin Phase 1:

```bash
# Create workflow crate
cd /home/bert/Work/orgs/konf-dev/kore
mkdir -p crates/kore-workflow/src

# Initialize crate
cd crates/kore-workflow
cargo init --lib

# Add to workspace in root Cargo.toml
# Add dependencies
```

First task: Define `WorkflowHandle` and `Message` types.

**Ready to start?**
