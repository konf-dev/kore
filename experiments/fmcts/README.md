# FMCTS: Fiber-Native Monte Carlo Tree Search

> "The stack is the only truth." — Kore Postulate P2

## Overview

FMCTS is a program synthesis algorithm that exploits Kore's mathematical guarantees:

| Guarantee | How We Exploit It |
|-----------|------------------|
| **Determinism** | 1 execution = exact reward (no sampling) |
| **Termination** | Every rollout completes (tick bounds) |
| **Observable Stack** | Dense reward at every step |
| **Trace Semantics** | Supervised signal from execution |
| **Fiber Immutability** | O(1) MCTS backtracking |

## Quick Start

```bash
# 1. Build Kore runtime
cd /home/bert/Work/orgs/konf-dev/kore
cargo build --release -p kore-runtime

# 2. Start runtime container (or use local binary)
docker-compose up -d kore-runtime

# 3. Run FMCTS
cd experiments/fmcts
python run_fmcts.py --task arithmetic --model qwen-0.5b
```

## Architecture

```
fmcts/
├── README.md              # This file
├── PLAN.md                # Detailed research plan
├── core/
│   ├── __init__.py
│   ├── mcts.py            # MCTS algorithm
│   ├── node.py            # Tree node with fiber state
│   └── config.py          # Configuration
├── executor/
│   ├── __init__.py
│   └── kore_executor.py   # Real Kore runtime (docker/local)
├── rewards/
│   ├── __init__.py
│   ├── stack_distance.py  # Stack-based reward
│   └── trace_reward.py    # Trace-based dense reward
├── policy/
│   ├── __init__.py
│   ├── base.py            # Abstract policy
│   ├── llm_policy.py      # LLM-based token prediction
│   └── random_policy.py   # Baseline
├── training/
│   ├── __init__.py
│   ├── trace_supervised.py # Train from traces
│   ├── mcts_rl.py         # MCTS-guided RL
│   └── data_gen.py        # Generate training data
├── benchmark/
│   ├── __init__.py
│   ├── koreeval.py        # KoreEval-500 benchmark
│   └── tasks/             # Task definitions
├── experiments/
│   ├── run_fmcts.py       # Main entry point
│   ├── run_baseline.py    # Beam search baseline
│   └── ablations.py       # Ablation studies
└── tests/
    ├── test_mcts.py
    ├── test_executor.py
    └── test_rewards.py
```

## Key Differences from kore-rl

| Aspect | kore-rl (GRPO) | FMCTS |
|--------|----------------|-------|
| Generation | Complete program | Token-by-token |
| Search | None (direct LLM) | MCTS tree search |
| Reward | Final only | Dense (per-step) |
| Backtracking | Re-generate | Fiber fork (O(1)) |
| Supervision | RL only | Trace-supervised + RL |

## Expected Results

| Method | Model | Pass@1 | Compute |
|--------|-------|--------|---------|
| Beam search | - | 25% | CPU |
| GRPO (kore-rl) | 7B | 45% | 8× A100, 1 week |
| **FMCTS** | **0.5B** | **80%** | **1× 3090, 1 day** |

## References

- [MCTS in AlphaGo](https://www.nature.com/articles/nature16961)
- [Program Synthesis with MCTS](https://arxiv.org/abs/2106.03893)
- [DeepSeek-R1 GRPO](https://arxiv.org/abs/2401.02954)
