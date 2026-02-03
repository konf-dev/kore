# Kore-RL: Training LLMs to Generate Kore Programs

## Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           TRAINING LOOP                                      │
│                                                                              │
│  ┌──────────┐    tokens     ┌──────────────────┐    results    ┌──────────┐ │
│  │          │ ────────────► │  Kore Runtime    │ ────────────► │          │ │
│  │   LLM    │               │   (Container)    │               │  Loss    │ │
│  │  Actor   │ ◄──────────── │                  │ ◄──────────── │ Compute  │ │
│  │          │   gradients   │  • Stack trace   │   rewards     │          │ │
│  └──────────┘               │  • Errors        │               └──────────┘ │
│       │                     │  • Final output  │                     │      │
│       │                     └──────────────────┘                     │      │
│       │                            ▲                                 │      │
│       │                            │                                 │      │
│       └────────────────────────────┴─────────────────────────────────┘      │
│                              PPO / GRPO Update                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Model Selection

### Recommended Base Models (by compute budget)

| GPUs | Model | Parameters | Context | Why |
|------|-------|------------|---------|-----|
| 2x A100 | **Qwen2.5-Coder-7B** | 7B | 32K | Best code model at size, fits in 40GB |
| 4x A100 | **DeepSeek-Coder-V2-Lite** | 16B (2.4B active) | 128K | MoE, very efficient |
| 8x A100 | **Qwen2.5-Coder-32B** | 32B | 128K | SOTA open code model |
| 16x A100 | **DeepSeek-Coder-V2** | 236B (21B active) | 128K | Near-frontier performance |

### Our Recommendation: **Qwen2.5-Coder-32B-Instruct**

Reasons:
1. **SOTA on coding benchmarks** (HumanEval: 92.7%, MBPP: 90.2%)
2. **128K context** - can handle full program traces
3. **Instruction-tuned** - good starting point for RL
4. **Efficient** - 32B is trainable on 8x A100 with DeepSpeed ZeRO-3
5. **Open weights** - full control over training

### Alternative: **DeepSeek-Coder-V2-Lite-Instruct** (16B MoE)

If compute is constrained:
- Only 2.4B parameters active per token
- 128K context
- Very fast inference
- Still competitive performance

## Training Framework

### Option 1: **veRL** (Recommended)

From ByteDance/DeepSeek, designed for efficient RL on LLMs:

```python
# veRL advantages:
# - 3-4x faster than TRL
# - Native DeepSpeed/FSDP integration
# - Efficient rollout collection
# - Proven on DeepSeek-R1
```

### Option 2: **OpenRLHF**

More mature, better documented:

```python
# OpenRLHF advantages:
# - Better documentation
# - Ray-based distributed training
# - Supports PPO, DPO, GRPO
# - Active community
```

### Option 3: **Custom GRPO** (Simplest)

Group Relative Policy Optimization (from DeepSeek-R1):

```python
# GRPO advantages:
# - No critic model needed (saves 50% memory)
# - Simpler implementation
# - Works well for code generation
# - We implement from scratch
```

**Our choice: GRPO with custom implementation**

Reasons:
1. No critic model = more memory for larger batch
2. Simpler to debug
3. Perfect for deterministic reward (Kore execution)
4. Can integrate container communication easily

## Container Architecture

### Kore Runtime Service

```dockerfile
# Dockerfile.kore-runtime
FROM rust:1.75-slim as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/kore-runtime /usr/local/bin/
EXPOSE 8080
CMD ["kore-runtime", "--serve", "--port", "8080"]
```

### API Design

```protobuf
// kore_runtime.proto
service KoreRuntime {
  // Execute a complete program
  rpc Execute(ExecuteRequest) returns (ExecuteResponse);
  
  // Execute incrementally (streaming)
  rpc ExecuteStream(stream Token) returns (stream StepResult);
}

message ExecuteRequest {
  string program = 1;
  repeated Value initial_stack = 2;
  uint32 max_steps = 3;
  uint32 timeout_ms = 4;
}

message ExecuteResponse {
  bool success = 1;
  repeated Value final_stack = 2;
  repeated TraceEntry trace = 3;
  optional string error = 4;
  uint32 steps_executed = 5;
  uint32 gas_used = 6;
}

message TraceEntry {
  uint32 step = 1;
  string op = 2;
  string op_type = 3;  // "push" or "call"
  repeated Value stack_before = 4;
  repeated Value stack_after = 5;
  bool success = 6;
  optional string error = 7;
}
```

### Why Container?

1. **Isolation**: Untrusted code can't affect training
2. **Reproducibility**: Exact same environment every time
3. **Scalability**: Run many containers in parallel
4. **Resource limits**: CPU/memory/time caps
5. **True sandbox**: Model learns real capabilities

## Training Curriculum

### Phase 1: Arithmetic (Week 1)
```
Tasks:
- compute(a + b) → push result
- compute(a * b - c) → push result
- compute((a + b) * (c - d))

Vocabulary: integers, add, sub, mul, div, mod, neg
Stack depth: 1-4
```

### Phase 2: Stack Manipulation (Week 2)
```
Tasks:
- duplicate top of stack
- swap and add
- compute with over/rot

Vocabulary: + dup, drop, swap, over, rot, nip
Stack depth: 2-6
```

### Phase 3: Conditionals (Week 3)
```
Tasks:
- max(a, b)
- abs(a)
- sign(a)
- clamp(x, lo, hi)

Vocabulary: + [quote], if, eq, lt, gt, le, ge, and, or, not
```

### Phase 4: Loops (Week 4)
```
Tasks:
- sum(1..n)
- factorial(n)
- fibonacci(n)
- gcd(a, b)

Vocabulary: + times, while, call
```

### Phase 5: Lists (Week 5)
```
Tasks:
- list-sum
- list-max
- list-reverse
- list-filter

Vocabulary: + list operations
```

### Phase 6: Complex Programs (Week 6+)
```
Tasks:
- sorting algorithms
- tree traversal
- graph algorithms
- parsing

Vocabulary: full Kore
```

## Reward Design

### Primary Reward: Execution Correctness

```python
def compute_reward(result: ExecuteResponse, target: Value) -> float:
    if not result.success:
        # Partial credit for how far it got
        return -1.0 + 0.1 * (result.steps_executed / max_steps)
    
    if result.final_stack == [target]:
        return 1.0  # Perfect!
    
    # Partial credit for close values
    if len(result.final_stack) == 1:
        return 0.5 * similarity(result.final_stack[0], target)
    
    return -0.5  # Wrong output
```

### Auxiliary Rewards

```python
def auxiliary_rewards(result: ExecuteResponse, program: str) -> dict:
    return {
        # Shorter is better (Occam's razor)
        "length_penalty": -0.01 * len(program.split()),
        
        # Fewer stack operations = cleaner
        "stack_efficiency": -0.005 * max(0, max_stack_depth(result.trace) - 4),
        
        # No dead code
        "no_dead_ops": -0.1 if has_dead_ops(result.trace) else 0,
        
        # Type consistency (no runtime type errors)
        "type_safety": 0.1 if no_type_errors(result.trace) else 0,
    }
```

## Hardware Setup

### Recommended: 8x A100 80GB

```yaml
# Training configuration
nodes: 1
gpus_per_node: 8
gpu_type: A100-80GB

# Memory breakdown per GPU:
# - Model (32B, bf16): ~64GB
# - Optimizer states (AdamW): ~12GB (with ZeRO-3)
# - Gradients: ~2GB (with ZeRO-3)
# - Activations: ~2GB (with activation checkpointing)
# Total: ~80GB ✓ (fits!)

# With 4x A100 40GB:
# Use DeepSeek-Coder-V2-Lite (16B MoE) instead
```

### Training Parameters

```yaml
# Hyperparameters
learning_rate: 1e-6  # Low for RL fine-tuning
batch_size: 64       # Per GPU, accumulated
gradient_accumulation: 4
max_seq_length: 4096  # Program + trace
warmup_steps: 100
total_steps: 50000

# GRPO specific
num_samples_per_prompt: 8  # Generate 8 programs, pick best
temperature: 0.7
kl_coef: 0.1  # KL divergence penalty

# DeepSpeed ZeRO-3
zero_stage: 3
offload_optimizer: true  # If memory tight
```

## Evaluation Benchmarks

### Code Generation Benchmarks

| Benchmark | Description | Metric |
|-----------|-------------|--------|
| HumanEval | 164 Python functions | pass@1, pass@10 |
| MBPP | 974 Python problems | pass@1, pass@10 |
| HumanEval+ | Harder test cases | pass@1 |
| DS-1000 | Data science tasks | pass@1 |
| SWE-bench | Real GitHub issues | % resolved |
| LiveCodeBench | Continuously updated | pass@1 |

### Kore-Specific Benchmarks

| Benchmark | Description | Metric |
|-----------|-------------|--------|
| KoreEval-Arith | 500 arithmetic problems | accuracy |
| KoreEval-Stack | 200 stack manipulation | accuracy |
| KoreEval-Control | 150 conditionals/loops | accuracy |
| KoreEval-Complex | 100 complex algorithms | accuracy, efficiency |

### AGI Benchmarks

| Benchmark | Description | Metric |
|-----------|-------------|--------|
| ARC-AGI | Abstract reasoning | accuracy |
| MATH | Mathematical reasoning | accuracy |
| GSM8K | Grade school math | accuracy |
| GPQA | Graduate-level science | accuracy |

## Timeline

### Week 1-2: Infrastructure
- [ ] Containerized Kore runtime with gRPC API
- [ ] GRPO training loop implementation
- [ ] Data generation for Phase 1 (arithmetic)
- [ ] Basic evaluation harness

### Week 3-4: Initial Training
- [ ] Train on arithmetic (Phase 1)
- [ ] Train on stack manipulation (Phase 2)
- [ ] Evaluate on KoreEval-Arith

### Week 5-6: Control Flow
- [ ] Train on conditionals (Phase 3)
- [ ] Train on loops (Phase 4)
- [ ] Evaluate on KoreEval-Control

### Week 7-8: Scaling
- [ ] Train on full Kore vocabulary
- [ ] Evaluate on HumanEval, MBPP (translate to Kore)
- [ ] Evaluate on ARC-AGI

### Week 9-12: Advanced
- [ ] Multi-turn training (debugging)
- [ ] Tool use (external capabilities)
- [ ] Full benchmark evaluation

## File Structure

```
kore/experiments/kore-rl/
├── ARCHITECTURE.md          # This file
├── docker/
│   ├── Dockerfile.runtime   # Kore runtime container
│   ├── Dockerfile.train     # Training container
│   └── docker-compose.yml   # Orchestration
├── proto/
│   └── kore_runtime.proto   # gRPC API definition
├── runtime/
│   ├── src/main.rs          # Runtime server
│   └── Cargo.toml
├── training/
│   ├── grpo.py              # GRPO implementation
│   ├── trainer.py           # Main training loop
│   ├── rollout.py           # Parallel rollout collection
│   ├── reward.py            # Reward computation
│   └── config.yaml          # Training config
├── curriculum/
│   ├── phase1_arithmetic.py
│   ├── phase2_stack.py
│   ├── phase3_conditionals.py
│   ├── phase4_loops.py
│   └── phase5_lists.py
├── eval/
│   ├── kore_eval.py         # Kore-specific benchmarks
│   ├── humaneval.py         # HumanEval adapter
│   └── arc_agi.py           # ARC-AGI adapter
└── scripts/
    ├── train.sh             # Main training script
    ├── eval.sh              # Evaluation script
    └── ablations.sh         # Ablation studies
```

## Quick Start

```bash
# 1. Build runtime container
cd kore/experiments/kore-rl
docker build -f docker/Dockerfile.runtime -t kore-runtime .

# 2. Start runtime service
docker run -d -p 8080:8080 --name kore-runtime kore-runtime

# 3. Install training dependencies
pip install torch transformers accelerate deepspeed grpcio

# 4. Run training
python training/trainer.py --config training/config.yaml

# 5. Evaluate
python eval/kore_eval.py --model checkpoints/step_10000
```

## Key Insights

1. **Container isolation is crucial**: The model should only interact with Kore through the sandbox API. No Python fallback, no escape hatches.

2. **GRPO > PPO for code**: No critic model needed when you have deterministic reward from execution.

3. **Curriculum matters**: Start simple. Arithmetic → Stack → Control → Complex.

4. **Incremental execution**: Catch errors early, give partial credit.

5. **Trace as context**: Include execution trace in prompt for debugging iterations.

6. **32B is the sweet spot**: Large enough for strong coding, small enough to train on 8x A100.
