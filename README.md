# Kore

**The fundamental runtime for agentic AI.**

Kore is a minimal, stack-based programming language designed for composable tool orchestration. It provides exactly what's needed for agents to compose, execute, and reason about tools—nothing more.

## Philosophy

1. **Minimal**: 4 operations, 10 types, 9 built-in tools
2. **Predictable**: No hidden state, no magic, explicit error handling
3. **Secure**: Capability-based access control
4. **Composable**: Tools are the only abstraction

## The Language

### 10 Value Types

| Type | Example | Purpose |
|------|---------|---------|
| Null | `null` | Absence of value |
| Bool | `true`, `false` | Logic |
| Int | `42`, `-7` | Whole numbers |
| Float | `3.14` | Decimals |
| Text | `"hello"` | Strings |
| List | `[1, 2, 3]` | Ordered collections |
| Map | `{"a": 1}` | Key-value pairs |
| Quote | `(add 1)` | Deferred code |
| Handle | `@file:123` | External resources |
| Error | `Error(...)` | Captured failures |

### 4 Operations

| Op | Effect | Description |
|----|--------|-------------|
| Push | `( -- value)` | Put a value on the stack |
| Call | `(... -- ...)` | Look up and run a tool |
| Quote | `( -- quote)` | Capture ops as a value |
| If | `(bool -- )` | Conditional execution |

### 9 Built-in Tools

| Tool | Effect | Purpose |
|------|--------|---------|
| `call` | `(quote -- ...)` | Run a quote |
| `try` | `(quote -- value-or-error)` | Run, capture errors |
| `is-error` | `(value -- bool)` | Check if Error |
| `unwrap` | `(value-or-error -- value)` | Extract or stop |
| `dup` | `(a -- a a)` | Duplicate top |
| `drop` | `(a -- )` | Remove top |
| `swap` | `(a b -- b a)` | Swap top two |
| `over` | `(a b -- a b a)` | Copy second to top |
| `rot` | `(a b c -- b c a)` | Rotate three |

## Components

| Crate | Purpose |
|-------|---------|
| `kore` | Core runtime - parser, executor, types |
| `kore-agent` | Autonomous LLM agent with experiment tracking |
| `kore-workflow` | Concurrent workflow execution |

## Quick Start: Agent

```bash
cd experiments
export OPENAI_API_KEY="your-key"
export OPENAI_BASE_URL="https://your-llm-api"

# Run an experiment
./run.sh prompts/v1-genesis.md "Create a notes folder" qwen2.5-32b

# Results in experiments/results/<timestamp>/
```

See [crates/kore-agent/README.md](crates/kore-agent/README.md) for details.

## Error Handling

Errors are values, not exceptions. This gives agents full visibility:

```
# Try something that might fail
(risky-operation) try

# Check if it failed
dup is-error
(handle-error)
(unwrap continue-with-result)
if
```

## Design for Agentic AI

Kore is designed with AI agents in mind:

- **Visibility**: Agents can inspect what tools are available
- **Control**: Explicit error handling lets agents decide how to recover
- **Simplicity**: Small surface area is easier to learn and reason about
- **Safety**: Capability system prevents unauthorized operations
- **Reproducibility**: Experiment system captures all inputs/outputs

## License

MIT
