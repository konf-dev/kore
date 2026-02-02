# Experiment System Plan

## Current State Analysis

### Philosophy Violations Found

| File | Issue | Principle Violated |
|------|-------|-------------------|
| `main.rs:63-89` | `extract_code()` - regex extraction, comment stripping, multi-line handling | #2 (does multiple things), #3 (hides errors) |
| `main.rs:92-125` | `validate_code()` - preprocesses LLM output, rejects patterns | #2 (multiple responsibilities), #3 (magic) |
| `main.rs:14-60` | Embedded fallback prompt | #3 (implicit), #5 (not minimal) |
| `io.rs:7-24` | `GOAL_DONE`, `GOAL_MESSAGE` global state | All principles - global mutable state |
| `trace.rs:220-246` | `build_llm_context()` - formats context for LLM | #2 (trace shouldn't know about prompts) |
| Agent loop | Parse→Validate→Extract→Execute→CheckGoal→SaveTrace | #1 (not divided), #2 (too many things) |

### What Should Change

The agent is currently **accommodating** the LLM instead of **teaching** it.

**Wrong approach** (current):
```
LLM → messy output → extract_code() → validate_code() → clean code → execute
```

**Right approach** (philosophy):
```
LLM → kore code → parse → execute (errors go to trace, LLM learns)
```

---

## The Clean Architecture

### Three Components (Each Does One Thing)

```
┌─────────────────────────────────────────────────────────────────┐
│                     1. PROMPT (prompt.md)                       │
│  - Defines agent behavior                                       │
│  - Only thing we modify for experiments                         │
│  - Contains: tools list, format rules, philosophy               │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                     2. AGENT (kore-agent)                       │
│  - Reads prompt                                                 │
│  - Runs loop: trace→LLM→parse→execute→trace                    │
│  - No preprocessing, no magic, no global state                  │
│  - Exits when "done" tool called                                │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                  3. EXPERIMENT RUNNER (run.sh)                  │
│  - Creates isolated workspace                                   │
│  - Captures all inputs/outputs                                  │
│  - Runs Docker container                                        │
│  - Saves metadata                                               │
└─────────────────────────────────────────────────────────────────┘
```

---

## Detailed Changes

### 1. main.rs - Simplify Agent Loop

**Remove:**
- `extract_code()` - LLM output is kore code directly
- `validate_code()` - parser errors go to trace
- Embedded prompt - require file
- Global goal state checks

**The new loop (pseudocode):**
```rust
fn main() {
    let config = Config::from_env();  // workspace, logs, prompt, goal, llm
    let prompt = fs::read_to_string(&config.prompt)?;
    let mut trace = Trace::new(&config.logs);
    
    trace.log(Event::Start { goal: &config.goal });
    
    loop {
        // Build context: just goal + recent trace
        let context = format!(
            "GOAL: {}\n\nTRACE:\n{}\n\nYOUR TURN:",
            config.goal,
            trace.recent(10)
        );
        
        // Call LLM
        let response = llm::call(&prompt, &context)?;
        trace.log(Event::LlmResponse { text: &response });
        
        // Parse as kore code (no extraction, no cleaning)
        match Op::parse(&response) {
            Ok(ops) => {
                match execute(&ops, &mut stack, &mut ctx).await {
                    Ok(_) => trace.log(Event::Success { stack: &stack }),
                    Err(e) => trace.log(Event::Error { msg: &e }),
                }
            }
            Err(e) => {
                trace.log(Event::ParseError { msg: &e });
                // Error is in trace, LLM will see it next iteration
            }
        }
    }
}
```

### 2. tools/io.rs - Clean done Tool

**Remove:**
- `GOAL_DONE` static
- `GOAL_MESSAGE` static
- `reset_goal_state()`
- `is_goal_done()`
- `get_goal_message()`

**New done tool:**
```rust
/// done: (message --) - Signal completion and exit
pub fn done_tool() -> Tool {
    Tool::native("done", "(message:Text --)", |mut stack, ctx| {
        Box::pin(async move {
            let message = stack.pop()?.as_text()?.to_string();
            println!("✅ DONE: {}", message);
            std::process::exit(0);
        })
    })
}
```

That's it. One thing: exit.

### 3. trace.rs - Just Log, Don't Format

**Remove:**
- `build_llm_context()` - prompt's job

**Keep:**
- Event logging to stdout
- Event logging to file
- `recent(n)` to get last n events

**New trace (minimal):**
```rust
pub struct Trace {
    log_file: File,
}

impl Trace {
    pub fn new(log_path: &Path) -> Self { ... }
    
    pub fn log(&mut self, event: Event) {
        // 1. Print to stdout (human readable)
        println!("{}", event.display());
        
        // 2. Write to file (JSON lines)
        writeln!(self.log_file, "{}", event.to_json());
    }
    
    pub fn recent(&self, n: usize) -> String {
        // Return last n events as simple text
    }
}
```

### 4. Config - All From Environment

```rust
pub struct Config {
    pub workspace: PathBuf,   // KORE_WORKSPACE (default: /mnt/workspace)
    pub logs: PathBuf,        // KORE_LOGS (default: /mnt/logs)
    pub prompt: PathBuf,      // KORE_PROMPT (required)
    pub goal: String,         // KORE_GOAL (required)
    pub llm_base: String,     // OPENAI_BASE_URL
    pub llm_key: String,      // OPENAI_API_KEY
    pub llm_model: String,    // OPENAI_MODEL
}
```

---

## Experiment System

### Directory Structure

```
experiments/
├── prompts/                    # Prompt versions we test
│   ├── v1-basic.md
│   ├── v2-explicit-args.md
│   └── v3-examples.md
│
├── run.sh                      # Main runner script
│
└── results/                    # Experiment outputs
    └── 2026-02-02_14-30-00_abc123/
        ├── config.json         # All inputs
        ├── prompt.md           # Copy of prompt used
        ├── workspace/          # Agent's sandbox
        │   ├── notes/
        │   └── hello.txt
        └── logs/
            ├── trace.jsonl     # Full event log
            └── stdout.log      # Console output
```

### config.json (Experiment Metadata)

```json
{
  "timestamp": "2026-02-02T14:30:00Z",
  "id": "2026-02-02_14-30-00_abc123",
  
  "inputs": {
    "goal": "Create notes folder and write hello.txt",
    "prompt": "prompts/v2-explicit-args.md",
    "prompt_sha256": "a1b2c3...",
    "model": "qwen2.5-32b",
    "code_version": "git:abc123def",
    "code_dirty": false
  },
  
  "outputs": {
    "exit_code": 0,
    "iterations": 3,
    "duration_seconds": 12.5,
    "final_message": "Created notes with hello.txt",
    "workspace_files": ["notes/", "hello.txt"],
    "errors": []
  }
}
```

### run.sh (Experiment Runner)

```bash
#!/bin/bash
set -euo pipefail

# Usage: ./run.sh <prompt.md> <goal> [model]

PROMPT_FILE="${1:?Usage: ./run.sh <prompt.md> <goal> [model]}"
GOAL="${2:?Usage: ./run.sh <prompt.md> <goal> [model]}"
MODEL="${3:-qwen2.5-32b}"

# Generate experiment ID
TIMESTAMP=$(date +%Y-%m-%d_%H-%M-%S)
CODE_VERSION=$(git rev-parse --short HEAD)
EXPERIMENT_ID="${TIMESTAMP}_${CODE_VERSION}"
EXPERIMENT_DIR="experiments/results/${EXPERIMENT_ID}"

# Create directories
mkdir -p "${EXPERIMENT_DIR}"/{workspace,logs}

# Copy prompt (immutable record)
cp "${PROMPT_FILE}" "${EXPERIMENT_DIR}/prompt.md"

# Save input config
cat > "${EXPERIMENT_DIR}/config.json" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "id": "${EXPERIMENT_ID}",
  "inputs": {
    "goal": "${GOAL}",
    "prompt": "${PROMPT_FILE}",
    "prompt_sha256": "$(sha256sum ${PROMPT_FILE} | cut -d' ' -f1)",
    "model": "${MODEL}",
    "code_version": "$(git rev-parse HEAD)",
    "code_dirty": $(git diff --quiet && echo false || echo true)
  }
}
EOF

echo "═══════════════════════════════════════════════════════"
echo "  Experiment: ${EXPERIMENT_ID}"
echo "  Goal: ${GOAL}"
echo "  Model: ${MODEL}"
echo "  Prompt: ${PROMPT_FILE}"
echo "═══════════════════════════════════════════════════════"

# Run agent in Docker
START_TIME=$(date +%s)

docker run --rm \
  --name "kore-${EXPERIMENT_ID}" \
  -v "${PWD}/${EXPERIMENT_DIR}/workspace:/mnt/workspace" \
  -v "${PWD}/${EXPERIMENT_DIR}/logs:/mnt/logs" \
  -v "${PWD}/${EXPERIMENT_DIR}/prompt.md:/mnt/prompt.md:ro" \
  -e "KORE_WORKSPACE=/mnt/workspace" \
  -e "KORE_LOGS=/mnt/logs" \
  -e "KORE_PROMPT=/mnt/prompt.md" \
  -e "KORE_GOAL=${GOAL}" \
  -e "OPENAI_API_KEY=${OPENAI_API_KEY}" \
  -e "OPENAI_BASE_URL=${OPENAI_BASE_URL}" \
  -e "OPENAI_MODEL=${MODEL}" \
  kore-agent 2>&1 | tee "${EXPERIMENT_DIR}/logs/stdout.log"

EXIT_CODE=${PIPESTATUS[0]}
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# Update config with outputs
ITERATIONS=$(grep -c '"kind":"iteration"' "${EXPERIMENT_DIR}/logs/trace.jsonl" 2>/dev/null || echo 0)
WORKSPACE_FILES=$(ls -1 "${EXPERIMENT_DIR}/workspace" 2>/dev/null | tr '\n' ',' | sed 's/,$//')

# Use jq to update config.json with outputs
jq --arg exit_code "${EXIT_CODE}" \
   --arg iterations "${ITERATIONS}" \
   --arg duration "${DURATION}" \
   --arg files "${WORKSPACE_FILES}" \
   '.outputs = {
     exit_code: ($exit_code | tonumber),
     iterations: ($iterations | tonumber),
     duration_seconds: ($duration | tonumber),
     workspace_files: ($files | split(","))
   }' "${EXPERIMENT_DIR}/config.json" > "${EXPERIMENT_DIR}/config.json.tmp" \
   && mv "${EXPERIMENT_DIR}/config.json.tmp" "${EXPERIMENT_DIR}/config.json"

echo ""
echo "═══════════════════════════════════════════════════════"
echo "  Complete: ${EXPERIMENT_ID}"
echo "  Duration: ${DURATION}s"
echo "  Exit Code: ${EXIT_CODE}"
echo "  Results: ${EXPERIMENT_DIR}"
echo "═══════════════════════════════════════════════════════"
```

### Dockerfile

```dockerfile
FROM rust:1.75-slim AS builder
WORKDIR /build
COPY . .
RUN cargo build --release -p kore-agent

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/kore-agent /usr/local/bin/

# Security: run as non-root
RUN useradd -m -u 1000 kore
USER kore

WORKDIR /mnt/workspace
ENTRYPOINT ["kore-agent"]
```

---

## Final System Overview

### Stakeholders View

#### 1. **Researcher** (runs experiments)
```bash
# Run an experiment
./experiments/run.sh prompts/v3.md "Build a todo app" qwen2.5-32b

# Compare results
ls experiments/results/
diff experiments/results/run1/workspace experiments/results/run2/workspace
```

#### 2. **Prompt Engineer** (modifies prompts)
```markdown
# prompts/v4-stack-order.md

You are Kore. Output ONLY kore code.

## Stack Order
Arguments are pushed LEFT to RIGHT:
- "path" "content" file-write
  ↑ first  ↑ second

## Tools
...
```

#### 3. **Docker Observer** (watches logs)
```bash
# Live tail during experiment
docker logs -f kore-2026-02-02_14-30-00

# Or watch the log file
tail -f experiments/results/*/logs/stdout.log
```

#### 4. **Agent** (inside Docker)
```
Sees only:
  /mnt/workspace/     ← read/write sandbox
  /mnt/logs/          ← write logs here  
  /mnt/prompt.md      ← read-only prompt

Cannot see:
  - Host filesystem
  - Other experiments
  - Source code
```

### Data Flow

```
┌────────────────────────────────────────────────────────────────────┐
│                         EXPERIMENT RUN                              │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  INPUTS (immutable)                                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │
│  │  prompt.md  │  │    goal     │  │    model    │                 │
│  │ (sha256:x)  │  │  (string)   │  │  (string)   │                 │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                 │
│         │                │                │                         │
│         └────────────────┼────────────────┘                         │
│                          ▼                                          │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                     DOCKER CONTAINER                          │ │
│  │  ┌─────────────────────────────────────────────────────────┐  │ │
│  │  │                    kore-agent                           │  │ │
│  │  │                                                         │  │ │
│  │  │   prompt ──→ LLM ──→ response ──→ parse ──→ execute    │  │ │
│  │  │              ▲                              │           │  │ │
│  │  │              │                              ▼           │  │ │
│  │  │              └────────── trace ◄────────────┘           │  │ │
│  │  │                                                         │  │ │
│  │  └─────────────────────────────────────────────────────────┘  │ │
│  │                          │                                     │ │
│  │            ┌─────────────┴─────────────┐                      │ │
│  │            ▼                           ▼                      │ │
│  │  ┌─────────────────┐        ┌─────────────────┐               │ │
│  │  │  /mnt/workspace │        │   /mnt/logs     │               │ │
│  │  │  (agent output) │        │  (trace.jsonl)  │               │ │
│  │  └─────────────────┘        └─────────────────┘               │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                          │                                          │
│  OUTPUTS (captured)      ▼                                          │
│  ┌────────────────────────────────────────────────────────────────┐│
│  │  experiments/results/2026-02-02_14-30-00_abc123/               ││
│  │  ├── config.json      ← inputs + outputs metadata              ││
│  │  ├── prompt.md        ← copy of prompt used                    ││
│  │  ├── workspace/       ← everything agent created               ││
│  │  └── logs/                                                     ││
│  │      ├── trace.jsonl  ← full execution trace                   ││
│  │      └── stdout.log   ← console output                         ││
│  └────────────────────────────────────────────────────────────────┘│
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Order

### Phase 1: Clean Agent (Day 1)
1. Remove `extract_code()` from main.rs
2. Remove `validate_code()` from main.rs  
3. Remove embedded prompt from main.rs
4. Remove global state from io.rs
5. Simplify `done` tool to just exit
6. Simplify trace to just log + recent()
7. Add Config struct for env vars

### Phase 2: Docker Setup (Day 1)
1. Create Dockerfile
2. Build and test locally
3. Verify isolation works

### Phase 3: Experiment Runner (Day 2)
1. Create experiments/ directory structure
2. Write run.sh script
3. Test full flow
4. Create first versioned prompt

### Phase 4: First Experiments (Day 2+)
1. Run same goal with different prompts
2. Compare results
3. Iterate on prompt only

---

## Success Criteria

| Criteria | Measurement |
|----------|-------------|
| Agent code is minimal | < 150 lines in main.rs |
| No magic preprocessing | LLM output goes directly to parser |
| Full isolation | Agent cannot access host filesystem |
| Complete capture | Every experiment fully reproducible |
| Prompt-only iteration | Can improve results by changing prompt.md only |

---

## Philosophy Compliance Checklist

- [x] **Divide**: Agent, Prompt, Runner are separate components
- [x] **Single Purpose**: Each component does one thing
- [x] **Explicit**: All config via env vars, all outputs captured
- [x] **Reuse**: Using existing kore parser, no custom extraction
- [x] **Verify**: Every experiment produces comparable outputs
