# Experiment E001: Agent Execution Engine

> **Status**: 🔵 Planned  
> **Started**: -  
> **Completed**: -  
> **Author**: Kore Team

## 1. Hypothesis

Kore can serve as a **safe, analyzable runtime** for LLM agents that is:
1. Faster than Docker sandboxes (target: <5ms analysis latency)
2. More granular than process isolation (per-operation capabilities)
3. Provably safe (static effect analysis before execution)

## 2. Background

### Current Approaches

| Approach | Analysis | Granularity | Overhead | Safety |
|----------|----------|-------------|----------|--------|
| Python exec() | ❌ None | ❌ All-or-nothing | ~0ms | ❌ Unsafe |
| Docker sandbox | ❌ None | ❌ Container-level | 200-500ms | 🟡 Medium |
| WASM | ❌ None | 🟡 Import-level | 50-100ms | 🟡 Medium |
| **Kore Runtime** | ✅ Static | ✅ Per-operation | <5ms | ✅ High |

### Why Kore is Different

1. **Pre-flight Analysis**: Know exactly what a program will do before running it
2. **Capability Lattice**: Fine-grained permissions that can only be attenuated
3. **Linear Resources**: Prevent resource leaks and double-use
4. **Effect Algebra**: Compose programs with predictable effects

## 3. Method

### 3.1 What We Build

```
┌────────────────────────────────────────────────────────────────┐
│                    Kore Agent Runtime                          │
├────────────────────────────────────────────────────────────────┤
│  HTTP API (Axum)                                               │
│  ├── POST /analyze  → Static effect analysis                   │
│  ├── POST /execute  → Run with capability bounds               │
│  ├── POST /define   → Define new tools in session              │
│  └── GET  /session  → Get session state                        │
├────────────────────────────────────────────────────────────────┤
│  Capability Profiles                                           │
│  ├── pure       → No IO at all                                 │
│  ├── read-only  → fs-read, http-get (no writes)               │
│  ├── local-io   → fs-*, no network                            │
│  └── full       → Everything (admin only)                      │
├────────────────────────────────────────────────────────────────┤
│  Session Management                                            │
│  ├── Tool definitions persist per-session                      │
│  ├── Capability grants per-session                             │
│  └── Audit log of all executions                               │
└────────────────────────────────────────────────────────────────┘
```

### 3.2 Procedure

#### Phase 1: Core API (Days 1-2)
1. Set up Axum HTTP server in `src/server/`
2. Implement `/analyze` endpoint
3. Implement `/execute` endpoint
4. Basic session management

#### Phase 2: Capability Integration (Days 3-4)
1. Define capability profiles
2. Pre-flight check against granted capabilities
3. Reject execution if caps insufficient

#### Phase 3: Testing & Demo (Days 5-6)
1. Unit tests for all endpoints
2. Integration tests with real agent scenarios
3. Benchmark latency

#### Phase 4: Integration (Day 7)
1. Connect to konf-agents-api
2. Documentation

### 3.3 Baselines

| Baseline | How We Test | Expected |
|----------|-------------|----------|
| Direct Kore CLI | `cargo run -- file.kore` | ~10ms |
| Python subprocess | `subprocess.run(["python", "-c", code])` | ~50ms |
| Docker exec | `docker run --rm python -c code` | ~300ms |
| **Kore HTTP API** | `POST /execute` | ~15ms |

## 4. Success Criteria

| Metric | Target | How Measured |
|--------|--------|--------------|
| Analysis latency | <5ms for 100 ops | Benchmark 1000 requests |
| Execution latency | <20ms for simple programs | Benchmark 1000 requests |
| False negative rate | 0% | Fuzz with dangerous programs |
| False positive rate | <1% | Fuzz with safe programs |
| Throughput | >100 req/s | Load test |

## 5. Implementation

### 5.1 Files to Create

```
kore/
├── src/
│   └── server/
│       ├── mod.rs          # Module exports
│       ├── app.rs          # Axum app setup
│       ├── handlers.rs     # Route handlers
│       ├── session.rs      # Session state
│       ├── profiles.rs     # Capability profiles
│       └── error.rs        # Error types
├── src/bin/
│   └── kore-server.rs      # Server binary
└── experiments/E001-agent-runtime/
    ├── kore/
    │   ├── demo_tools.kore     # Example agent tools
    │   ├── test_safe.kore      # Safe programs
    │   └── test_unsafe.kore    # Unsafe programs (should reject)
    ├── data/
    │   └── benchmark_programs.json
    └── results/
```

### 5.2 API Specification

```yaml
# POST /analyze
Request:
  code: string          # Kore code to analyze
Response:
  effect:
    consumes: int       # Stack items consumed
    produces: int       # Stack items produced
  io_effects: string[]  # ["fs", "net", "io", ...]
  pure: bool            # No IO effects?
  safe: bool            # Valid program?
  required_caps: string[] # Capabilities needed

# POST /execute  
Request:
  code: string          # Kore code to run
  caps: string[]        # Granted capabilities
  session_id?: string   # Optional session
Response:
  result: Value[]       # Stack after execution
  trace?: string[]      # Execution trace (if debug)
  error?: string        # Error message if failed

# POST /define
Request:
  name: string          # Tool name
  code: string          # Tool body (quote)
  session_id: string    # Session to define in
Response:
  success: bool
  effect: Effect        # Inferred effect
```

### 5.3 Dependencies

- [x] Kore version: 2.0 (linear types, effects, optimizer)
- [ ] Axum: HTTP framework
- [ ] Tokio: Async runtime
- [ ] Serde: Serialization

## 6. Demo Scenarios

### Scenario 1: Safe Computation

```kore
; Agent wants to compute something
; This should be allowed with "pure" capability

[ 1 2 3 ] [ dup mul ] map    ; => [1 4 9]
0 [ add ] fold               ; => 14
```

Analysis result: `{ io_effects: [], pure: true, required_caps: [] }`

### Scenario 2: File Reading

```kore
; Agent wants to read a config file
; This should require "fs" capability

"config.json" fs-read json-parse
"api_key" map-get
```

Analysis result: `{ io_effects: ["fs"], pure: false, required_caps: ["fs"] }`

If agent has `caps: ["fs"]` → Execute
If agent has `caps: ["pure"]` → Reject

### Scenario 3: Multi-Agent Collaboration

```kore
; Agent A (has: net, fs-write) fetches data
[ "https://api.example.com/data" http-get
  json-parse
  "cache.json" fs-write
] "fetch-data" def

; Agent B (has: fs-read, pure) processes data
[ "cache.json" fs-read json-parse
  [ "value" map-get 0 gt ] filter
  list-len
] "count-positive" def

; Agent C (has: io) reports
[ "Found " print count-positive print " positive values" println
] "report" def
; ERROR: Agent C cannot call count-positive (needs fs)
```

## 7. Changelog

| Date | Change |
|------|--------|
| 2026-02-03 | Created experiment specification |
