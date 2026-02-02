# Kore Agent

An autonomous LLM-driven agent that runs kore code to achieve goals.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     PROMPT (prompt.md)                          │
│  - Defines agent behavior                                       │
│  - Only thing we iterate on                                     │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                     AGENT (kore-agent)                          │
│  - Simple loop: trace → LLM → parse → execute                  │
│  - No preprocessing, no magic                                   │
│  - Exits when "done" tool called                                │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                  EXPERIMENT RUNNER (run.sh)                     │
│  - Creates isolated Docker workspace                            │
│  - Captures all inputs/outputs                                  │
│  - Saves metadata for comparison                                │
└─────────────────────────────────────────────────────────────────┘
```

## Quick Start

### Run an Experiment

```bash
cd experiments
export OPENAI_API_KEY="your-key"
export OPENAI_BASE_URL="https://your-llm-api"

./run.sh prompts/v1-genesis.md "Create a notes folder" qwen2.5-32b
```

### Results Structure

```
experiments/results/2026-02-02_01-49-56_abc123/
├── config.json       # Inputs + outputs metadata
├── prompt.md         # Copy of prompt used
├── workspace/        # Agent's sandbox (what it created)
└── logs/
    ├── trace.jsonl   # Full execution trace
    └── stdout.log    # Console output
```

## The Agent Loop

```rust
loop {
    // 1. Build context: goal + recent trace
    let context = format!("GOAL: {}\nTRACE:\n{}", goal, trace.recent(10));
    
    // 2. Call LLM with master prompt
    let response = llm::call(&prompt, &context);
    
    // 3. Parse response as kore code (no extraction, no cleaning)
    match Op::parse(&response) {
        Ok(ops) => execute(&ops),  // Success → trace
        Err(e) => trace.error(e),  // Error → trace (LLM learns)
    }
}
```

**Key principle**: No magic preprocessing. LLM output goes directly to parser. Errors go to trace, LLM sees them and learns.

## Configuration

All via environment variables:

| Variable | Required | Description |
|----------|----------|-------------|
| `KORE_PROMPT` | Yes | Path to prompt file |
| `KORE_GOAL` | Yes | Goal for the agent |
| `OPENAI_API_KEY` | Yes | LLM API key |
| `OPENAI_BASE_URL` | No | LLM API URL (default: OpenAI) |
| `OPENAI_MODEL` | No | Model name (default: gpt-4o) |
| `KORE_WORKSPACE` | No | Workspace dir (default: ./workspace) |
| `KORE_LOGS` | No | Logs dir (default: ./logs) |

## Available Tools

| Tool | Signature | Description |
|------|-----------|-------------|
| `print` | `(value --)` | Print to stdout |
| `done` | `(message --)` | Signal completion and EXIT |
| `dir-list` | `(path -- files)` | List directory |
| `dir-create` | `(path --)` | Create directory |
| `file-read` | `(path -- content)` | Read file |
| `file-write` | `(path content --)` | Write file |
| `file-exists` | `(path -- bool)` | Check file exists |
| `shell` | `(cmd -- output)` | Run shell command |
| `http-get` | `(url -- response)` | HTTP GET |
| `llm` | `(prompt -- response)` | Call LLM |
| `list-tools` | `(-- names)` | List all tools |

## Philosophy Compliance

The agent follows [PHILOSOPHY.md](../docs/PHILOSOPHY.md):

1. **DIVIDE** - One action per iteration
2. **SINGLE PURPOSE** - Each tool does one thing
3. **EXPLICIT** - All config via env vars, all outputs captured
4. **REUSE** - Uses existing kore parser, no custom extraction
5. **VERIFY** - Errors go to trace, LLM sees and corrects

## Docker Isolation

The agent runs in a Docker container:
- Mounts only workspace and logs directories
- Runs as non-root user
- Cannot access host filesystem
- Each experiment is isolated

## Iterating on the Prompt

The **only thing to modify** is the prompt file. Example iteration:

```bash
# v1 - basic
./run.sh prompts/v1-genesis.md "Create todo app" qwen2.5-32b

# v2 - added explicit examples
./run.sh prompts/v2-examples.md "Create todo app" qwen2.5-32b

# Compare results
diff results/run1/workspace results/run2/workspace
```
