# Kore Agent Architecture

## Vision

An autonomous agent system where **nothing is special**. Agents, supervisors, memory, checkpointing - all are workflows composed from stateless tools.

---

## Core Invariants

### 1. Every Tool Is Stateless

```
tool: (inputs -- outputs)
```

- No internal memory
- No hidden state
- Same inputs → same behavior
- Side effects go to explicit external places (files, network, messages)

### 2. LLM Has No Memory

The `llm` tool is a pure function:

```
llm-system: (system:Text user:Text -- response:Text)
```

It doesn't know:
- Previous calls
- Current goal
- Other agents
- File system state

**The workflow provides all context. Every time.**

### 3. Intelligence Emerges From Composition

Complex behavior = simple tools + smart workflow patterns

```
agent = loop + recv + think + execute + checkpoint
supervisor = agent that monitors other agents
memory = files + context building
```

### 4. Everything Is Recoverable

Checkpoints save complete state. Any failure can be recovered.

---

## Tool Inventory

### Layer 1: Stack Operations (kore built-in)
```
dup     (a -- a a)
drop    (a --)
swap    (a b -- b a)
over    (a b -- a b a)
rot     (a b c -- b c a)
```

### Layer 2: Control Flow (kore built-in)
```
call    (quote --)              Execute quote
if      (bool then else --)     Conditional
loop    (quote --)              Loop until false on stack
try     (quote -- result)       Execute, catch errors
```

### Layer 3: Data (kore built-in)
```
eq      (a b -- bool)
lt      (a b -- bool)
gt      (a b -- bool)
add     (a b -- sum)
sub     (a b -- diff)
mul     (a b -- prod)
div     (a b -- quot)
concat  (a b -- ab)
```

### Layer 4: I/O (kore-agent)
```
print       (value --)
read-line   (-- text)
env-get     (key -- value)
```

### Layer 5: LLM (kore-agent) - STATELESS
```
llm         (prompt -- response)
llm-system  (system user -- response)
```

### Layer 6: Network (kore-agent)
```
http-get    (url -- response:Map)
http-post   (url body headers -- response:Map)
```

### Layer 7: File System (NEW)
```
file-read       (path -- content)
file-write      (path content --)
file-append     (path content --)
file-exists     (path -- bool)
file-delete     (path --)
dir-list        (path -- entries:List)
dir-create      (path --)
dir-delete      (path --)
```

### Layer 8: Process (NEW)
```
shell       (cmd -- {stdout, stderr, code})
shell-dir   (cmd dir -- {stdout, stderr, code})
```

### Layer 9: Workflow (kore-workflow)
```
spawn       (quote -- handle)
await       (handle -- result)
send        (handle msg --)
recv        (-- msg)
recv-timeout (ms -- msg|null)
self        (-- handle)
parent      (-- handle)
children    (-- handles:List)
status      (handle -- {state, error})
```

### Layer 10: Checkpointing (NEW)
```
checkpoint      (name --)
restore         (name --)
list-checkpoints (-- names:List)
```

### Layer 11: Introspection (NEW)
```
list-tools  (-- names:List)
tool-help   (name -- {effect, doc})
define      (name effect body --)
```

### Layer 12: Context Helpers (NEW)
```
now         (-- timestamp:Int)
uuid        (-- id:Text)
json-parse  (text -- value)
json-format (value -- text)
```

---

## Implementation Plan

### Phase 1: File System Tools

Add to `kore-agent/src/tools.rs`:

```rust
file_read_tool()      // Read file content
file_write_tool()     // Write file content  
file_append_tool()    // Append to file
file_exists_tool()    // Check file exists
file_delete_tool()    // Delete file
dir_list_tool()       // List directory contents
dir_create_tool()     // Create directory
dir_delete_tool()     // Delete directory
```

All operate on `/workspace` (sandbox root).

### Phase 2: Shell Execution

```rust
shell_tool()          // Run command, return {stdout, stderr, code}
shell_dir_tool()      // Run in specific directory
```

Security: Commands run inside Docker sandbox. No escaping.

### Phase 3: Workflow Integration

Wire kore-workflow tools into kore-agent:

```rust
spawn_tool()          // From kore-workflow
await_tool()          // From kore-workflow
send_tool()           // From kore-workflow
recv_tool()           // From kore-workflow
recv_timeout_tool()   // NEW - non-blocking receive
self_tool()           // From kore-workflow
parent_tool()         // From kore-workflow
children_tool()       // NEW - list child handles
status_tool()         // NEW - get workflow status
```

### Phase 4: Checkpointing

```rust
checkpoint_tool()     // Save state to disk
restore_tool()        // Load state (terminates current, resumes from checkpoint)
list_checkpoints_tool()
```

Checkpoint format:
```
/checkpoints/<workflow-id>/<name>/
  state.json          # Stack + context serialization
  metadata.json       # Timestamp, parent, etc.
```

### Phase 5: Introspection

```rust
list_tools_tool()     // Get all tool names
tool_help_tool()      // Get tool effect + doc
define_tool()         // Register new composed tool
```

### Phase 6: Context Helpers

```rust
now_tool()            // Current timestamp
uuid_tool()           // Generate unique ID
json_parse_tool()     // Text → Value
json_format_tool()    // Value → Text
```

### Phase 7: Agent Entry Point

Refactor `kore-agent/src/main.rs`:

1. Load genesis system prompt
2. Build initial context
3. Call LLM with full context
4. Parse response for kore code
5. Execute kore code
6. Checkpoint
7. Loop

### Phase 8: Docker Sandbox

Finalize `Dockerfile` and `run-agent.sh`:
- Read-only root
- Writable /workspace and /checkpoints
- Network access for APIs
- No host access

---

## File Structure After Implementation

```
kore/
├── crates/
│   ├── kore/                    # Core runtime (unchanged)
│   ├── kore-workflow/           # Workflow engine (unchanged)
│   └── kore-agent/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs          # Entry point + agent loop
│           └── tools/
│               ├── mod.rs       # Tool registration
│               ├── io.rs        # print, read-line, env-get
│               ├── llm.rs       # llm, llm-system
│               ├── network.rs   # http-get, http-post
│               ├── fs.rs        # file-*, dir-*
│               ├── shell.rs     # shell, shell-dir
│               ├── workflow.rs  # spawn, await, send, recv, etc.
│               ├── checkpoint.rs # checkpoint, restore
│               ├── introspection.rs # list-tools, define
│               └── helpers.rs   # now, uuid, json-*
├── Dockerfile
├── run-agent.sh
├── genesis-prompt.md            # System prompt for genesis
└── docs/
    ├── PHILOSOPHY.md
    └── AGENT_ARCHITECTURE.md    # This file
```

---

## Genesis Prompt Structure

```markdown
# You are Kore

An autonomous agent built on a stack-based runtime.

## Philosophy (MUST FOLLOW)

1. DIVIDE tasks into smallest pieces
2. Each piece does ONE thing
3. Clear inputs/outputs, explicit side effects
4. REUSE existing tools before creating new
5. VERIFY at every step

## Your Capabilities

### Stack Operations
[list all with effects]

### Control Flow
[list all with effects]

### I/O
[list all with effects]

### LLM (STATELESS - YOU HAVE NO MEMORY)
- llm: (prompt -- response)
- llm-system: (system user -- response)

YOU MUST BUILD CONTEXT EXPLICITLY EVERY CALL.
Store important information in files.
Load relevant context before each decision.

### File System
[list all with effects]

### Shell
[list all with effects]

### Workflows
[list all with effects]

### Checkpoints
[list all with effects]

## Syntax
[kore syntax reference]

## Your Task

You are autonomous. You receive a goal.

Your loop:
1. Load relevant context from files
2. Decide next action
3. Output kore code to execute
4. Results shown to you
5. Checkpoint
6. Repeat until goal complete

## Current Goal

{goal}

## Current State

Stack: {stack}
Files in /workspace: {files}
Recent actions: {recent}

## What is your next action?

Output ONLY a kore code block to execute.
```

---

## Detailed Implementation Steps

### Step 1: Reorganize tools.rs into modules

Current: One big `tools.rs`
Target: `tools/` directory with focused modules

```
tools/
├── mod.rs       # pub mod declarations + register_all()
├── io.rs        # print, read-line, env-get
├── llm.rs       # llm, llm-system (move from tools.rs)
├── network.rs   # http-get, http-post
├── math.rs      # add, sub, mul, div
├── fs.rs        # NEW: file and directory operations
├── shell.rs     # NEW: command execution
├── workflow.rs  # NEW: spawn, await, send, recv, etc.
├── checkpoint.rs # NEW: state management
├── introspection.rs # NEW: tool discovery
└── helpers.rs   # NEW: utilities
```

### Step 2: Implement fs.rs

```rust
// file-read: (path:Text -- content:Text)
pub fn file_read_tool() -> Tool {
    Tool::native("file-read", "(path:Text -- content:Text)", |mut stack, ctx| {
        Box::pin(async move {
            let path = stack.pop()?.as_text()?.to_string();
            let full_path = workspace_path(&path);
            let content = tokio::fs::read_to_string(&full_path).await
                .map_err(|e| Error::Runtime(format!("file-read: {}", e)))?;
            stack.push(Value::Text(content))?;
            Ok((stack, ctx))
        })
    })
}

// file-write: (path:Text content:Text --)
pub fn file_write_tool() -> Tool {
    Tool::native("file-write", "(path:Text content:Text --)", |mut stack, ctx| {
        Box::pin(async move {
            let content = stack.pop()?.as_text()?.to_string();
            let path = stack.pop()?.as_text()?.to_string();
            let full_path = workspace_path(&path);
            
            // Ensure parent directory exists
            if let Some(parent) = full_path.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            
            tokio::fs::write(&full_path, content).await
                .map_err(|e| Error::Runtime(format!("file-write: {}", e)))?;
            Ok((stack, ctx))
        })
    })
}

// Similar for: file-append, file-exists, file-delete, dir-list, dir-create, dir-delete

fn workspace_path(path: &str) -> PathBuf {
    // All paths relative to /workspace (or ./workspace in dev)
    let base = std::env::var("KORE_WORKSPACE").unwrap_or_else(|_| "./workspace".to_string());
    PathBuf::from(base).join(path.trim_start_matches('/'))
}
```

### Step 3: Implement shell.rs

```rust
// shell: (cmd:Text -- result:Map)
pub fn shell_tool() -> Tool {
    Tool::native("shell", "(cmd:Text -- result:Map)", |mut stack, ctx| {
        Box::pin(async move {
            let cmd = stack.pop()?.as_text()?.to_string();
            
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .current_dir(workspace_path(""))
                .output()
                .await
                .map_err(|e| Error::Runtime(format!("shell: {}", e)))?;
            
            let result = Value::Map(indexmap::indexmap! {
                "stdout".to_string() => Value::Text(String::from_utf8_lossy(&output.stdout).to_string()),
                "stderr".to_string() => Value::Text(String::from_utf8_lossy(&output.stderr).to_string()),
                "code".to_string() => Value::Int(output.status.code().unwrap_or(-1) as i64),
            });
            
            stack.push(result)?;
            Ok((stack, ctx))
        })
    })
}
```

### Step 4: Implement workflow.rs

Wire kore-workflow into kore-agent. This requires:
1. Workflow runtime running in background
2. Tools that interact with runtime

```rust
// spawn: (quote:Quote -- handle:Handle)
// Uses kore_workflow::Runtime to spawn

// This is complex - needs runtime handle passed through context
// Option: Store runtime handle in Context as special value
// Or: Global runtime (simpler for now)
```

### Step 5: Implement checkpoint.rs

```rust
// checkpoint: (name:Text --)
pub fn checkpoint_tool() -> Tool {
    Tool::native("checkpoint", "(name:Text --)", |mut stack, ctx| {
        Box::pin(async move {
            let name = stack.pop()?.as_text()?.to_string();
            
            // Serialize stack
            let stack_json = serde_json::to_string(stack.values())
                .map_err(|e| Error::Runtime(format!("checkpoint serialize: {}", e)))?;
            
            // Save to checkpoint directory
            let checkpoint_dir = checkpoint_path(&name);
            tokio::fs::create_dir_all(&checkpoint_dir).await.ok();
            
            tokio::fs::write(
                checkpoint_dir.join("state.json"),
                stack_json
            ).await.map_err(|e| Error::Runtime(format!("checkpoint write: {}", e)))?;
            
            // Save metadata
            let metadata = serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "name": name,
            });
            tokio::fs::write(
                checkpoint_dir.join("metadata.json"),
                metadata.to_string()
            ).await.ok();
            
            Ok((stack, ctx))
        })
    })
}

fn checkpoint_path(name: &str) -> PathBuf {
    let base = std::env::var("KORE_CHECKPOINTS").unwrap_or_else(|_| "./checkpoints".to_string());
    PathBuf::from(base).join(name)
}
```

### Step 6: Implement introspection.rs

```rust
// list-tools: (-- names:List)
pub fn list_tools_tool() -> Tool {
    Tool::native("list-tools", "(-- names:List)", |mut stack, ctx| {
        Box::pin(async move {
            let names: Vec<Value> = ctx.tools()
                .keys()
                .map(|k| Value::Text(k.clone()))
                .collect();
            stack.push(Value::List(names))?;
            Ok((stack, ctx))
        })
    })
}

// tool-help: (name:Text -- info:Map)
pub fn tool_help_tool() -> Tool {
    Tool::native("tool-help", "(name:Text -- info:Map)", |mut stack, ctx| {
        Box::pin(async move {
            let name = stack.pop()?.as_text()?.to_string();
            
            if let Some(tool) = ctx.get_tool(&name) {
                let info = Value::Map(indexmap::indexmap! {
                    "name".to_string() => Value::Text(name),
                    "effect".to_string() => Value::Text(tool.effect().to_string()),
                    "doc".to_string() => Value::Text(tool.doc().unwrap_or("").to_string()),
                });
                stack.push(info)?;
            } else {
                return Err(Error::Runtime(format!("Tool not found: {}", name)));
            }
            
            Ok((stack, ctx))
        })
    })
}
```

### Step 7: Implement helpers.rs

```rust
// now: (-- timestamp:Int)
pub fn now_tool() -> Tool {
    Tool::native("now", "(-- timestamp:Int)", |mut stack, ctx| {
        Box::pin(async move {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            stack.push(Value::Int(ts))?;
            Ok((stack, ctx))
        })
    })
}

// uuid: (-- id:Text)
pub fn uuid_tool() -> Tool {
    Tool::native("uuid", "(-- id:Text)", |mut stack, ctx| {
        Box::pin(async move {
            let id = uuid::Uuid::new_v4().to_string();
            stack.push(Value::Text(id))?;
            Ok((stack, ctx))
        })
    })
}

// json-parse: (text:Text -- value:Any)
pub fn json_parse_tool() -> Tool {
    Tool::native("json-parse", "(text:Text -- value:Any)", |mut stack, ctx| {
        Box::pin(async move {
            let text = stack.pop()?.as_text()?.to_string();
            let value: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| Error::Runtime(format!("json-parse: {}", e)))?;
            stack.push(json_to_value(value))?;
            Ok((stack, ctx))
        })
    })
}

// json-format: (value:Any -- text:Text)
pub fn json_format_tool() -> Tool {
    Tool::native("json-format", "(value:Any -- text:Text)", |mut stack, ctx| {
        Box::pin(async move {
            let value = stack.pop()?;
            let json = value_to_json(&value);
            let text = serde_json::to_string_pretty(&json)
                .map_err(|e| Error::Runtime(format!("json-format: {}", e)))?;
            stack.push(Value::Text(text))?;
            Ok((stack, ctx))
        })
    })
}
```

### Step 8: Refactor main.rs

```rust
// New agent loop structure

#[tokio::main]
async fn main() {
    // 1. Initialize
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    tools::register_all(&mut ctx).await;
    
    // 2. Setup workspace
    setup_workspace().await;
    
    // 3. Load or create genesis state
    let mut stack = Stack::new();
    
    // 4. Get goal from args or env
    let goal = std::env::var("KORE_GOAL")
        .unwrap_or_else(|_| "Introduce yourself and await instructions.".to_string());
    
    // 5. Run agent loop
    agent_loop(&mut stack, &mut ctx, &goal).await;
}

async fn agent_loop(stack: &mut Stack, ctx: &mut Context, goal: &str) {
    let mut iteration = 0;
    
    loop {
        iteration += 1;
        
        // 1. Build context for LLM
        let context = build_llm_context(stack, goal, iteration).await;
        
        // 2. Call LLM
        let system = load_genesis_prompt();
        let response = call_llm(&system, &context).await;
        
        match response {
            Ok(text) => {
                // 3. Extract and execute kore code
                if let Some(code) = extract_kore_code(&text) {
                    println!("Executing: {}", code);
                    match execute_code(&code, stack.clone(), ctx.clone()).await {
                        Ok((new_stack, new_ctx)) => {
                            *stack = new_stack;
                            *ctx = new_ctx;
                        }
                        Err(e) => {
                            println!("❌ Execution error: {}", e);
                        }
                    }
                } else {
                    println!("Agent: {}", text);
                }
                
                // 4. Auto-checkpoint
                let checkpoint_name = format!("auto-{}", iteration);
                save_checkpoint(&checkpoint_name, stack).await;
            }
            Err(e) => {
                println!("❌ LLM error: {}", e);
            }
        }
        
        // 5. Check for quit signal
        // (In full implementation, this would check messages)
    }
}

async fn build_llm_context(stack: &Stack, goal: &str, iteration: i32) -> String {
    let mut ctx = String::new();
    
    ctx.push_str(&format!("## Current Goal\n{}\n\n", goal));
    ctx.push_str(&format!("## Iteration\n{}\n\n", iteration));
    ctx.push_str(&format!("## Stack\n{:?}\n\n", stack.values()));
    
    // List workspace files
    if let Ok(entries) = tokio::fs::read_dir(workspace_path("")).await {
        ctx.push_str("## Workspace Files\n");
        let mut entries = entries;
        while let Ok(Some(entry)) = entries.next_entry().await {
            ctx.push_str(&format!("- {}\n", entry.file_name().to_string_lossy()));
        }
        ctx.push_str("\n");
    }
    
    // Load recent actions if exists
    if let Ok(recent) = tokio::fs::read_to_string(workspace_path("recent-actions.txt")).await {
        let lines: Vec<&str> = recent.lines().rev().take(10).collect();
        ctx.push_str("## Recent Actions (last 10)\n");
        for line in lines.iter().rev() {
            ctx.push_str(&format!("{}\n", line));
        }
        ctx.push_str("\n");
    }
    
    ctx.push_str("## What is your next action?\nOutput a kore code block to execute.\n");
    
    ctx
}

fn extract_kore_code(response: &str) -> Option<String> {
    // Look for ```kore ... ``` or ``` ... ```
    let re = regex::Regex::new(r"```(?:kore)?\s*\n([\s\S]*?)\n```").ok()?;
    re.captures(response).map(|c| c[1].to_string())
}
```

---

## Dependencies to Add

### kore-agent/Cargo.toml

```toml
[dependencies]
kore = { path = "../kore" }
kore-workflow = { path = "../kore-workflow" }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
indexmap = { version = "2", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
chrono = "0.4"
regex = "1"
```

---

## Execution Order

1. **Create tools module structure** - Split tools.rs into modules
2. **Implement fs.rs** - File operations
3. **Implement shell.rs** - Command execution
4. **Implement helpers.rs** - Utilities (now, uuid, json)
5. **Implement introspection.rs** - Tool discovery
6. **Implement checkpoint.rs** - State management
7. **Wire workflow tools** - spawn, await, send, recv
8. **Refactor main.rs** - Agent loop with context building
9. **Create genesis-prompt.md** - Full system prompt
10. **Test locally** - Run without Docker
11. **Finalize Docker** - Sandbox setup
12. **Test in Docker** - Full sandbox run

---

## Success Metrics

1. **File operations work** - Can read/write files in workspace
2. **Shell works** - Can execute commands, get output
3. **Context building works** - LLM receives proper context each call
4. **Checkpointing works** - Can save and restore state
5. **Agent loops** - Continuous execution with checkpoints
6. **Recovery works** - Can resume from checkpoint after restart
7. **Tools discoverable** - `list-tools` shows all available tools
