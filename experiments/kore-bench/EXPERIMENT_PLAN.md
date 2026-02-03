# Kore Benchmark Experiment Plan

> Training an LLM to learn Kore and evaluating against baselines

---

## Overview

```
Goal: Demonstrate that Kore is easier for LLMs to generate correctly than Python

Hypothesis: An LLM trained/prompted on Kore will achieve higher pass@k 
            than the same LLM on equivalent Python tasks

Metrics:
  - Pass@1, Pass@10, Pass@100
  - Syntax error rate  
  - Token efficiency (correct solutions / tokens)
  - Sample efficiency (training samples to reach X% accuracy)
```

### New: Fiber-Guided Program Search (FGPS)

We now have a **Kore-native learning algorithm** that exploits Kore's unique features.
See [FIBER_SEARCH_ALGORITHM.md](FIBER_SEARCH_ALGORITHM.md) for the full design.

**Key Innovation**: Instead of "generate-then-execute", we:
1. Execute incrementally (token by token) using fibers
2. Fork promising branches (`fiber-fork`)
3. Checkpoint successful prefixes
4. Prune invalid paths via static effect analysis
5. Train LLM on **stack-conditioned** next-token prediction

**Files**:
- [fgps.py](fgps.py) - Core FGPS implementation
- [generate_stack_data.py](generate_stack_data.py) - Training data generator

---

## Stage 1: Data Generation

### 1.1 Task Categories

```
KoreEval Benchmark Structure:
├── L1: Stack Basics (50 tasks)
│   ├── arithmetic: "Add two numbers" → `add`
│   ├── stack ops: "Duplicate top" → `dup`
│   └── comparison: "Check if equal" → `eq`
├── L2: Control Flow (50 tasks)  
│   ├── conditionals: "Return max of two" → `... if`
│   ├── loops: "Sum 1 to n" → `... times`
│   └── errors: "Safe divide" → `... try`
├── L3: Data Structures (50 tasks)
│   ├── lists: "Reverse a list" → `... list-reverse`
│   ├── maps: "Get nested value" → `... map-get`
│   └── strings: "Split on comma" → `... "," str-split`
├── L4: Higher-Order (50 tasks)
│   ├── quotations: "Apply function twice" → `[f] call [f] call`
│   ├── combinators: "Keep and transform" → `... keep`
│   └── definitions: "Define factorial" → `[...] "factorial" def`
└── L5: Algorithms (50 tasks)
    ├── sorting: "Bubble sort" → `...`
    ├── search: "Binary search" → `...`
    └── math: "GCD" → `...`
```

### 1.2 Data Generation Strategy

```python
# For each task, generate:
{
    "id": "L1-001",
    "level": 1,
    "category": "arithmetic",
    "description": "Write a Kore program that adds two numbers on the stack.",
    "signature": "(a b -- sum)",  # Stack effect
    "kore_solution": "add",
    "python_equivalent": "def solve(a, b): return a + b",
    "test_cases": [
        {"input": [3, 4], "output": [7]},
        {"input": [0, 0], "output": [0]},
        {"input": [-5, 5], "output": [0]},
    ],
    "difficulty": "trivial"
}
```

### 1.3 Generation Methods

| Method | Tasks | Approach |
|--------|-------|----------|
| **Manual** | L1 (50) | Hand-write canonical examples |
| **Template** | L2-L3 (100) | Fill templates with variations |
| **LLM-assisted** | L4-L5 (100) | GPT-4 generates, human verifies |

### 1.4 Training Data Volume

```
Target: 10,000 training examples
        250 held-out test examples (KoreEval)
        
Distribution:
  L1: 3,000 examples (easy, foundation)
  L2: 3,000 examples (control flow)
  L3: 2,000 examples (data structures)
  L4: 1,500 examples (higher-order)
  L5: 500 examples (algorithms)
```

---

## Stage 2: Training

### 2.1 Model Selection

| Model | Size | VRAM | Use Case |
|-------|------|------|----------|
| Qwen2.5-Coder-0.5B | 0.5B | ~2GB | Fast iteration, RTX 2070 |
| Qwen2.5-Coder-1.5B | 1.5B | ~4GB | Main experiments, RTX 3090 |
| Qwen2.5-Coder-7B | 7B | ~16GB | Best quality, RTX 3090 (4-bit) |
| CodeLlama-7B | 7B | ~16GB | Comparison baseline |

### 2.2 Training Phases

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRAINING CURRICULUM                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Phase 1: Supervised Fine-Tuning (SFT)                          │
│  ─────────────────────────────────────                          │
│  • Data: 10K (description, kore_solution) pairs                 │
│  • Loss: Cross-entropy on Kore tokens                           │
│  • Epochs: 3-5                                                   │
│  • Learning rate: 2e-5 with cosine decay                        │
│  • Batch size: 32 (gradient accumulation)                       │
│  • Expected: 60-70% pass@1 on training distribution             │
│                                                                  │
│  Phase 2: Execution-Based RL (Optional)                         │
│  ───────────────────────────────────────                         │
│  • Start from SFT checkpoint                                    │
│  • Reward: +1 correct, -0.1 syntax error, -0.5 wrong output    │
│  • Algorithm: PPO or REINFORCE with baseline                    │
│  • Samples: 100K episodes                                       │
│  • Expected: +10-15% improvement over SFT                       │
│                                                                  │
│  Phase 3: Rejection Sampling Fine-Tuning (RFT)                  │
│  ─────────────────────────────────────────────                  │
│  • Generate N solutions per problem                             │
│  • Keep only correct ones                                       │
│  • Fine-tune on correct solutions                               │
│  • Iterate                                                       │
│  • Expected: High-quality solutions                             │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 2.3 Training Configuration

```yaml
# config/train_sft.yaml
model:
  name: Qwen/Qwen2.5-Coder-1.5B
  quantization: null  # Full precision for training
  
training:
  method: sft
  epochs: 5
  batch_size: 8
  gradient_accumulation: 4  # Effective batch = 32
  learning_rate: 2e-5
  scheduler: cosine
  warmup_ratio: 0.1
  
lora:
  enabled: true
  r: 32
  alpha: 64
  dropout: 0.05
  target_modules: ["q_proj", "k_proj", "v_proj", "o_proj"]

data:
  train_file: data/kore_train_10k.jsonl
  eval_file: data/kore_eval_500.jsonl
  max_length: 512
  
hardware:
  device: cuda:0  # RTX 3090 Ti
  bf16: true
```

---

## Stage 3: Evaluation

### 3.1 Evaluation Protocol

```
For each test problem:
  1. Generate k=100 candidate solutions
  2. Parse each solution (check syntax)
  3. Execute on test cases
  4. Record: pass@1, pass@10, pass@100
  
Comparison Setup:
  A) Kore-trained model on Kore tasks
  B) Same base model on Python equivalent tasks  
  C) Few-shot (no training) on Kore tasks
  D) Few-shot on Python tasks
```

### 3.2 Metrics

| Metric | Formula | Meaning |
|--------|---------|---------|
| **Pass@k** | $\mathbb{E}[1 - \binom{n-c}{k}/\binom{n}{k}]$ | Probability ≥1 correct in k samples |
| **Syntax Error Rate** | $\text{parse\_errors} / \text{total\_samples}$ | % of unparseable outputs |
| **Exact Match** | $\text{exact\_match} / \text{total}$ | Character-exact solution |
| **Token Efficiency** | $\text{correct} / \text{tokens\_generated}$ | Correctness per token |

### 3.3 Baseline Comparisons

```
┌─────────────────────────────────────────────────────────────────┐
│                    COMPARISON MATRIX                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│                      │ Kore Tasks │ Python Tasks │              │
│  ────────────────────┼────────────┼──────────────┤              │
│  Kore-SFT Model      │   MAIN     │     N/A      │              │
│  Python-SFT Model    │    N/A     │   BASELINE   │              │
│  Base Model Few-shot │   TEST A   │   TEST B     │              │
│  GPT-4 Few-shot      │   TEST C   │   TEST D     │              │
│                                                                  │
│  Key Comparison:                                                │
│    MAIN vs BASELINE = Does Kore help?                           │
│    MAIN vs TEST A = Does training help?                         │
│    TEST A vs TEST B = Is Kore easier zero-shot?                 │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Stage 4: Analysis

### 4.1 Research Questions

1. **RQ1: Learnability** - How many training examples needed to reach X% accuracy?
2. **RQ2: Syntax Simplicity** - What's the syntax error rate vs Python?
3. **RQ3: Generalization** - Does L1-L3 training transfer to L4-L5?
4. **RQ4: Sample Efficiency** - How does pass@k scale with k?
5. **RQ5: Token Efficiency** - Correct solutions per generated token?

### 4.2 Expected Results

| Metric | Kore (expected) | Python (baseline) | Improvement |
|--------|-----------------|-------------------|-------------|
| Pass@1 | 45-55% | 30-40% | +15-20% |
| Pass@10 | 70-80% | 55-65% | +15% |
| Pass@100 | 90-95% | 80-85% | +10% |
| Syntax Error | 5-10% | 15-25% | -50-60% |
| Tokens/Correct | 50-80 | 100-150 | -40-50% |

### 4.3 Ablation Studies

```
Ablations to run:
├── Training data size: 1K, 2K, 5K, 10K
├── Model size: 0.5B, 1.5B, 7B
├── Training method: SFT only, SFT+RL, SFT+RFT
├── Prompt format: minimal, with signature, with examples
└── Difficulty level: L1 only, L1-L3, full
```

---

## Implementation Timeline

### Week 1: Data Generation
- [ ] Define 250 KoreEval tasks (50 per level)
- [ ] Write Python equivalents
- [ ] Generate 10K training examples
- [ ] Implement Kore executor for test cases

### Week 2: Training Infrastructure
- [ ] Set up training pipeline (transformers + PEFT)
- [ ] Implement evaluation harness
- [ ] Run 0.5B pilot on RTX 2070
- [ ] Debug and iterate

### Week 3: Main Experiments
- [ ] Train 1.5B model on RTX 3090
- [ ] Run full evaluation suite
- [ ] Train Python baseline for comparison
- [ ] Collect results

### Week 4: Analysis & Writing
- [ ] Statistical significance tests
- [ ] Generate figures and tables
- [ ] Write results section
- [ ] Ablation studies

---

## File Structure

```
kore-bench/
├── EXPERIMENT_PLAN.md          # This file
├── data/
│   ├── kore_train_10k.jsonl    # Training data
│   ├── kore_eval_250.jsonl     # KoreEval benchmark
│   ├── python_eval_250.jsonl   # Python equivalents
│   └── generate_data.py        # Data generation script
├── src/
│   ├── train_sft.py            # Supervised fine-tuning
│   ├── train_rl.py             # RL fine-tuning
│   ├── evaluate.py             # Evaluation harness
│   ├── kore_executor.py        # Execute Kore programs
│   └── python_executor.py      # Execute Python programs
├── configs/
│   ├── train_sft.yaml          # SFT config
│   ├── train_rl.yaml           # RL config
│   └── eval.yaml               # Eval config
├── results/
│   ├── sft_results.json        # SFT model results
│   ├── baseline_results.json   # Baseline results
│   └── figures/                # Generated figures
└── scripts/
    ├── run_all.sh              # Full pipeline
    ├── run_eval.sh             # Evaluation only
    └── analyze.py              # Result analysis
```

---

## Quick Start

```bash
# 1. Generate data
cd kore-bench
python data/generate_data.py --output data/kore_train_10k.jsonl

# 2. Train model (SFT)
python src/train_sft.py --config configs/train_sft.yaml

# 3. Evaluate
python src/evaluate.py \
    --model checkpoints/kore-sft-1.5b \
    --benchmark data/kore_eval_250.jsonl \
    --output results/sft_results.json

# 4. Compare to baseline
python scripts/analyze.py \
    --kore results/sft_results.json \
    --python results/baseline_results.json \
    --output results/comparison.pdf
```

---

## Hardware Requirements

| Stage | GPU | VRAM | Time |
|-------|-----|------|------|
| Data Generation | None | N/A | 2-4 hours |
| SFT 0.5B | RTX 2070 | 6GB | 2 hours |
| SFT 1.5B | RTX 3090 | 10GB | 4 hours |
| SFT 7B | RTX 3090 | 20GB | 12 hours |
| Evaluation | RTX 3090 | 8GB | 1 hour |

---

## Success Criteria

```
Minimum Success:
  - Pass@1 on Kore > Pass@1 on Python (same model)
  - Syntax error rate Kore < Python
  - At least 50% pass@10 on L1-L3

Strong Success:
  - Pass@1 improvement > 15%
  - Syntax error rate < 10%
  - Generalization to L4-L5

Publication-Ready:
  - Comprehensive ablations
  - Statistical significance (p < 0.05)
  - Comparison to GPT-4 baseline
  - Open-source code and data
```

---

## Risks and Mitigations

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Not enough training data | Medium | LLM-assisted generation, template expansion |
| Kore executor bugs | High | Extensive testing, use Rust Kore as ground truth |
| Model doesn't learn Kore | Low | Start with more examples, curriculum learning |
| No improvement over Python | Medium | Focus on L1-L3 where advantage should be clearest |
| Compute constraints | Low | Use LoRA, smaller models for iteration |

---

*Created: 2026-02-03*
