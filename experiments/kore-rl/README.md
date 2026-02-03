# Kore-RL: Training LLMs to Generate Kore Programs

Train strong code generation models using reinforcement learning with a sandboxed Kore runtime.

## Architecture

```
┌──────────┐    programs   ┌──────────────────┐    results    ┌──────────┐
│   LLM    │ ────────────► │  Kore Sandbox    │ ────────────► │  Reward  │
│  (GPU)   │               │   (Container)    │               │ Compute  │
└──────────┘               └──────────────────┘               └──────────┘
     │                            │                                 │
     └────────────────────────────┴─────────────────────────────────┘
                              GRPO Update
```

### Sandbox Features

- **Isolated**: Docker container with no host filesystem/localhost access
- **Internet**: Container has internet access for future capabilities
- **Mount-based**: Communication via `/mnt/exchange` directory (faster than HTTP)
- **Auto-restart**: Container restarts on crash, training resumes from checkpoint
- **Full capabilities**: Kore runs with root + all capabilities (configurable per phase)

## Quick Start

### 1. Build kore-train Binary

```bash
# From kore root directory
cargo build --release --bin kore-train

# Test it
echo "3 4 add" | ./target/release/kore-train
# {"success":true,"final_stack":[7],...}
```

### 2. Start the Sandbox (Production)

```bash
cd experiments/kore-rl

# Set exchange directory
export KORE_EXCHANGE_DIR=/tmp/kore-exchange

# Build and start
docker compose -f docker/docker-compose.yml build kore-sandbox
docker compose -f docker/docker-compose.yml up -d kore-sandbox

# Verify
cat $KORE_EXCHANGE_DIR/status.json
# {"ready": true, ...}
```

### 3. Install Training Dependencies

```bash
pip install torch transformers accelerate tqdm pyyaml
```

### 4. Test Pipeline (No GPU Required)

```bash
python test_pipeline.py

# Output:
# Batch execution (500 programs):
#   Rate: 2950 programs/sec
# All pipeline tests passed! ✓
```

### 5. Run Training

```bash
# With sandbox container
python training/train_sandbox.py \
  --model Qwen/Qwen2.5-Coder-7B-Instruct \
  --use-sandbox \
  --exchange-dir /tmp/kore-exchange \
  --phase 1 \
  --batch-size 4

# Or local execution (faster for development)
python training/train_sandbox.py \
  --model Qwen/Qwen2.5-Coder-7B-Instruct \
  --no-sandbox \
  --kore-binary ../../target/release/kore-train \
  --phase 1
```

## Curriculum Learning

Training proceeds through 5 phases, each adding new capabilities:

| Phase | Operations | Example Task |
|-------|------------|--------------|
| 1 | add, sub, mul, div, mod | Compute 3 + 4 * 2 |
| 2 | + dup, swap, over, rot | Square a number using dup |
| 3 | + if, eq, lt, gt | Compute max(a, b) |
| 4 | + times, while | Compute factorial |
| 5 | + list ops (map, fold) | Sum a list |

## System Prompt

The LLM receives a comprehensive Kore documentation as system prompt (~4000 chars):

- Kore philosophy (simplicity, composability, explicitness, determinism)
- How the stack works
- All operations with stack effects
- Examples for each category
- Best practices

See `training/prompts.py` for the full prompt.

## Reward Function

```python
def compute_reward(result, target):
    if not result.success:
        return -1.0 + 0.1 * (steps / 20)  # Partial credit for progress
    
    if result.final_stack == [target]:
        return 1.0  # Perfect!
    
    if close_numeric(result.final_stack[0], target):
        return 0.5 / (1 + diff * 0.1)  # Partial credit
    
    return -0.5  # Wrong answer
```

## Model Recommendations

| Hardware | Model | Notes |
|----------|-------|-------|
| 2x A100 40GB | Qwen2.5-Coder-7B-Instruct | Fast iteration |
| 4x A100 40GB | DeepSeek-Coder-V2-Lite | MoE, good efficiency |
| 8x A100 80GB | Qwen2.5-Coder-32B-Instruct | Best quality |

## Files

```
experiments/kore-rl/
├── docker/
│   ├── Dockerfile.sandbox    # Sandbox container
│   ├── docker-compose.yml    # Container orchestration
│   └── worker.py             # Container worker process
├── training/
│   ├── prompts.py            # System prompt & task generation
│   ├── grpo.py               # GRPO algorithm
│   ├── sandbox_executor.py   # Docker sandbox executor
│   ├── docker_executor.py    # Local/CLI executor
│   ├── train_sandbox.py      # Main training script
│   └── config.yaml           # Configuration
├── eval/
│   └── kore_eval.py          # Evaluation benchmarks
├── test_pipeline.py          # Pipeline tests
└── README.md                 # This file
```

## Performance

Local execution benchmark (RTX 3090):
- ~3000 programs/second (parallel, 32 workers)
- ~520 programs/second (sequential)

## Resume Training

Training automatically saves checkpoints and can resume after crash:

```bash
# Will automatically resume from last checkpoint
python training/train_sandbox.py --resume

# Or start fresh
python training/train_sandbox.py --no-resume
```
