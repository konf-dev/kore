# Kore Experiments

Systematic experiments to demonstrate and validate Kore's capabilities.

## Experiment Registry

| ID | Name | Status | Start | End | Result |
|----|------|--------|-------|-----|--------|
| E001 | Agent Execution Engine | 🔵 Planned | - | - | - |
| E002 | Verified Algorithm Synthesis | 🔵 Planned | - | - | - |
| E003 | Differentiable Program Search | 🔵 Planned | - | - | - |

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
# Create new experiment
./experiments/new.sh E004 "my-experiment-name"

# Run an experiment
./experiments/E001-agent-runtime/run.sh

# Check status of all experiments
./experiments/status.sh
```

## Metrics We Track

1. **Correctness** - Does it produce right results?
2. **Performance** - Latency, throughput, resource usage
3. **Comparison** - How does it compare to baselines?
4. **Reproducibility** - Can others replicate results?

## Publication Targets

- E001: Blog post + integration with konf-agents-api
- E002: arXiv paper "Verified Algorithm Synthesis with Linear Resources"
- E003: arXiv paper "Differentiable Stack Machines for Program Synthesis"
