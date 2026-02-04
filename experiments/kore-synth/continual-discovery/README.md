# Continual Mathematical Discovery

A Docker-containerized Kore system that discovers, composes, and persists mathematical operations.

## What It Does

Discovers mathematical operations in 5 phases:

1. **Primitives**: Basic operations (succ, pred, double, square, negate, absolute)
2. **Compositions**: Combine existing tools (quadruple = double ∘ double)
3. **Pattern Discovery**: Synthesize from examples (triangular numbers, power-of-2)
4. **Verification**: Test all discoveries
5. **Meta-tools**: Tools that generate tools (make-adder, make-multiplier)

All discoveries are **persisted to disk** and loaded on subsequent runs.

## Quick Start

```bash
# Run discovery (first time - discovers 16 tools)
./run.sh

# Run again (loads existing knowledge)
./run.sh

# Interactive shell
docker compose run --rm kore-shell
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Docker Container                                           │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Kore Runtime                                          │  │
│  │  • Discovers operations                               │  │
│  │  • Verifies correctness                               │  │
│  │  • Serializes to source code                          │  │
│  └───────────────────────────────────────────────────────┘  │
│                           ↓                                 │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ /mnt/knowledge.kore  (persisted volume)               │  │
│  │  - Pure Kore source code                              │  │
│  │  - Executable definitions                             │  │
│  │  - Loadable across sessions                           │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                           ↕
          Host: ./mnt/knowledge.kore (mounted)
```

## Knowledge Base Format

All discoveries are saved as pure Kore source:

```kore
# successor: n → n+1
[ 1 add ] "succ" def

# square: n → n²
[ dup mul ] "square" def

# meta: n → [n add]
[ to-text " add" str-concat "[ " swap str-concat " ]" str-concat 
  text-to-quote ] "make-adder" def
```

## Continual Learning

Each run:
1. Tries to load `/mnt/knowledge.kore`
2. If found: uses existing tools, skips rediscovery
3. If not found: discovers from scratch
4. Saves updated knowledge back to disk

This enables **incremental knowledge building** across sessions!

## Files

- `discovery.kore` - Main discovery script
- `Dockerfile` - Container definition
- `docker-compose.yml` - Service configuration
- `run.sh` - Convenience script
- `mnt/` - Persisted knowledge directory

## Next Steps

To extend this:
1. Add more discovery phases (recursion, higher-order functions)
2. Implement genetic/evolutionary search
3. Scale to domain-specific operations (matrix, tensor, etc.)
4. Add inter-tool composition (discover by combining learned tools)

See `../matrix-discovery/` for matrix multiplication algorithm discovery framework.
