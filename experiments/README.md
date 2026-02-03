# Kore Experiments

Systematic experiments to demonstrate and validate Kore's capabilities.

> **Full Catalog**: See [CATALOG.md](CATALOG.md) for detailed specifications of all 15 experiments.

## Experiment Registry

| ID | Name | Status | Start | End | Result |
|----|------|--------|-------|-----|--------|
| **Tier 1: Foundation** |
| E001 | Agent Runtime | 🔵 Planned | - | - | - |
| E002 | Verified Synthesis | 🔵 Planned | - | - | - |
| E003 | Differentiable Search | 🔵 Planned | - | - | - |
| **Tier 2: Demonstrations** |
| E004 | LLM Agent Executor | 🔵 Planned | - | - | - |
| E005 | Safe Code Sandbox | 🔵 Planned | - | - | - |
| E006 | Pipeline DSL | 🔵 Planned | - | - | - |
| **Tier 3: Novel Research** |
| E007 | End-to-End LLM+Kore | 🔵 Planned | - | - | - |
| E008 | Circuit Minimization | 🔵 Planned | - | - | - |
| E009 | Sorting Network Synthesis | 🔵 Planned | - | - | - |
| **Tier 4: Ambitious** |
| E010 | Small Ramsey Bounds | 🔵 Planned | - | - | - |
| E011 | Integer Sequence Discovery | 🔵 Planned | - | - | - |
| E012 | Knot Simplification | 🔵 Planned | - | - | - |
| **Tier 5: Moonshots** |
| E013 | Busy Beaver Candidates | 🔵 Planned | - | - | - |
| E014 | Novel Matrix Mult (3×3) | 🔵 Planned | - | - | - |
| E015 | Collatz Counter-Example | 🔵 Planned | - | - | - |

### Status Legend

- 🔵 Planned
- 🟡 In Progress
- 🟢 Complete
- 🔴 Failed
- ⚪ Abandoned

## Directory Structure

```
experiments/
├── README.md                 # This file (registry)
├── .templates/               # Templates for new experiments
│   ├── EXPERIMENT.md         # Experiment spec template
│   ├── config.toml           # Config template
│   └── run.sh                # Runner template
├── E001-agent-runtime/       # Each experiment gets own dir
│   ├── EXPERIMENT.md         # Specification & results
│   ├── config.toml           # Configuration
│   ├── src/                  # Rust code (if any)
│   ├── kore/                 # Kore code
│   ├── data/                 # Input/output data
│   ├── results/              # Metrics, logs, artifacts
│   └── run.sh                # Execution script
└── ...
```

## Quick Commands

```bash
# Check status
./experiments/status.sh

# Create new experiment
./experiments/new.sh E004 llm-agent-executor

# Run an experiment
./experiments/E001-agent-runtime/run.sh
```

## Recommended Order

```
Phase 1 (Weeks 1-3):   E001 → E004          Foundation + LLM integration
Phase 2 (Weeks 4-7):   E002 → E003          Verified + Differentiable synthesis  
Phase 3 (Weeks 8-12):  E007 → E009 → E008   End-to-end training + Discovery
Phase 4 (Weeks 13-20): E011 → E010 → E014   Math discovery (ambitious)
Phase 5 (If time):     E013 → E012          Moonshots
```

## Hardware (Available)

| GPU | VRAM | Role |
|-----|------|------|
| RTX 3090 Ti | 24GB | Main training, large batches |
| RTX 2070 Super | 8GB | Verification, parallel search |

## Metrics We Track

1. **Correctness** - Does it produce right results?
2. **Performance** - Latency, throughput, resource usage
3. **Comparison** - How does it compare to baselines?
4. **Reproducibility** - Can others replicate results?

## Publication Targets

| Paper | Experiments | Venue |
|-------|-------------|-------|
| "Safe Execution for LLM Agents" | E001, E004, E005 | AAAI / Industry |
| "Verified Algorithm Synthesis" | E002, E008, E009 | PLDI / POPL |
| "Differentiable Stack Machines" | E003, E007 | NeurIPS / ICML |
| "Gradient-Based Discovery" | E010, E014 | Nature MI |
