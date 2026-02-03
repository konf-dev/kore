# Kore RL Training Experiments

## Quick Start

```bash
# 1. Clone and enter
git clone <repo> && cd kore

# 2. Build Kore runtime
cargo build --release --bin kore-train

# 3. Install Python dependencies
cd training
pip install -r requirements.txt

# 4. Run supervised experiment
python run_supervised.py --model qwen2.5-coder-7b --phases 1-3

# 5. Run unsupervised experiment  
python run_unsupervised.py --model qwen2.5-coder-7b --steps 10000
```

## Requirements

### System
- Linux (tested on Ubuntu 22.04)
- CUDA 12.x + GPU with 24GB+ VRAM (for 7B model)
- Rust 1.75+ (for building Kore)
- Python 3.10+

### Python Packages
See `requirements.txt` for full list. Key dependencies:
- `transformers>=4.40.0` - Model loading
- `torch>=2.2.0` - Backend
- `vllm>=0.4.0` - Fast inference (optional but recommended)
- `accelerate>=0.28.0` - Multi-GPU
- `peft>=0.10.0` - LoRA fine-tuning
- `trl>=0.8.0` - RL training utilities

### Models
Tested with:
- `Qwen/Qwen2.5-Coder-7B-Instruct` (recommended)
- `deepseek-ai/deepseek-coder-7b-instruct-v1.5`
- `codellama/CodeLlama-7b-Instruct-hf`

## Experiment Overview

### Supervised Curriculum (`run_supervised.py`)
Progressive training through 6 phases:
1. Stack & Arithmetic
2. Control Flow
3. Data Structures
4. Introspection
5. I/O & Capabilities
6. Advanced Patterns

Reward: correctness + effect matching + capability minimality

### Unsupervised Exploration (`run_unsupervised.py`)
Self-play with intrinsic motivation:
- Effect novelty
- Self-play composition
- Equivalence discovery
- Linear resource games
- Capability descent

Reward: novelty + self-consistency + algebraic discovery

## Configuration

Edit `config.yaml` or pass command-line args:

```yaml
# Model settings
model:
  name: "Qwen/Qwen2.5-Coder-7B-Instruct"
  device: "cuda"
  quantization: null  # "4bit" or "8bit" for memory
  
# Training settings  
training:
  batch_size: 8
  learning_rate: 1e-5
  epochs: 10
  gradient_accumulation: 4
  
# Kore runtime
kore:
  binary: "../target/release/kore-train"
  timeout: 5.0
  max_trace_steps: 100
```

## Output Structure

```
logs/
├── supervised/
│   ├── phase_1/
│   │   ├── checkpoints/
│   │   ├── metrics.json
│   │   └── samples.jsonl
│   └── ...
└── unsupervised/
    ├── checkpoints/
    ├── discoveries.jsonl
    ├── effect_coverage.json
    └── equivalences.json
```

## What LLM Sees

The LLM interacts with Kore through structured prompts containing:

1. **Execution Result**: Final stack state
2. **Execution Trace**: Step-by-step stack changes
3. **Effect Analysis**: (consumes, produces, io_effects)
4. **Error Messages**: When programs fail

The LLM does NOT see:
- Rust source code
- Container filesystem
- Network access (initially)

## Troubleshooting

### Out of Memory
```bash
# Use quantization
python run_supervised.py --quantization 4bit

# Reduce batch size
python run_supervised.py --batch-size 4
```

### Kore binary not found
```bash
# Rebuild
cargo build --release --bin kore-train

# Check path in config.yaml
```

### CUDA errors
```bash
# Check GPU
nvidia-smi

# Set specific GPU
CUDA_VISIBLE_DEVICES=0 python run_supervised.py
```
