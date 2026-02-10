# LLM Code Generation for Kore

Teaching a language model to write verified Kore programs, using the compiler as an oracle.

## What this is

A fine-tuning pipeline (SFT → GRPO) that trains DeepSeek-R1-Distill-Qwen-14B to generate Kore programs from natural language descriptions. Every generated program is verified by `korec` — the compiler either accepts it (type-checked, proof-checked, runs correctly) or rejects it. No manual labeling needed.

## Results

**SFT model (final):** 476/478 curriculum tasks correct (**99.6%**), evaluated on 16 curriculum levels.

**GRPO model on held-out demo tasks:** 27/29 correct (**93.1%**) on tasks phrased differently from training.

Per-level breakdown (SFT-v3, 478 tasks):

| Level | Category | Correct | Total | Rate |
|-------|----------|---------|-------|------|
| 0 | Push literals | 19 | 19 | 100% |
| 1 | Arithmetic | 41 | 41 | 100% |
| 2 | Stack manipulation | 44 | 44 | 100% |
| 3 | Comparisons & logic | 43 | 43 | 100% |
| 4 | Local variables | 46 | 46 | 100% |
| 5 | Control flow | 50 | 50 | 100% |
| 6 | Quotations & higher-order | 27 | 27 | 100% |
| 7 | Bitwise operations | 28 | 28 | 100% |
| 8 | Pairs & data structures | 17 | 17 | 100% |
| 9 | Lists & higher-order ops | 22 | 22 | 100% |
| 10 | Error handling | 17 | 17 | 100% |
| 11 | String operations | 24 | 24 | 100% |
| 12 | Type conversion | 18 | 18 | 100% |
| 13 | Float math | 29 | 29 | 100% |
| 17 | Maps | 13 | 13 | 100% |
| 19 | Function definitions | 38 | 40 | 95% |

The two failures are both at level 19 (function definitions with recursion), where the model generates correct logic but occasionally gets the recursive base case wrong.

### Demo transcript (selected)

The GRPO model generating programs for tasks it wasn't directly trained on:

```
Task: Compute (4 + 5) * 3
Kore: 4 5 + 3 *                    ✓ → 27

Task: Cube of 3
Kore: 3 dup dup * *                ✓ → 27

Task: a=2, b=3, compute b-a
Kore: 2 ->a 3 ->b b a -            ✓ → 1

Task: If 1 > 9 then 1 else 0
Kore: 1 9 > if 1 else 0 end        ✓ → 0

Task: 0 + 1 three times
Kore: 0 3 [ 1 + ] times            ✓ → 3

Task: Apply square to 6
Kore: 6 [ dup * ] apply            ✓ → 36

Task: Make a pair of 10 and 20, get second
Kore: 10 20 pair second            ✓ → 20
```

## Why Kore is good for this

Most LLM code generation works by generating Python/JavaScript and running it, hoping it doesn't crash. Kore has properties that make the generate-verify loop much tighter:

1. **Compiler as oracle.** `korec` tells you before execution whether a program is type-safe and stack-balanced. This is a free reward signal — no test cases needed for basic correctness.

2. **Small token vocabulary.** ~140 tokens total (vs. ~50,000 for Python). The model's action space is tiny, which means faster convergence and less hallucination.

3. **Deterministic semantics.** Same program → same result, always. No side effects, no imports, no environment dependencies. Evaluation is pure.

4. **Fast verification.** `korec serve` evaluates ~50,000 programs/second via a persistent stdin/stdout pipe. No process creation overhead. This matters for RL training where you need thousands of evaluations per batch.

5. **Curriculum maps to capability levels.** Kore's vocabulary is organized into capability levels (literals → arithmetic → stack ops → control flow → data structures → ...). This maps directly to curriculum learning — teach simple tokens first, unlock harder ones.

## How it works

### Stage 1: Data preparation

`data_prep.py` converts the curriculum (478 tasks across 16 levels) into SFT training data:
- Each task gets a prompt with available tokens, rules, and level-specific hints
- The completion includes a synthesized reasoning chain + the hint solution
- Output: `data/sft_train.jsonl` (2,317 examples with augmentation)

### Stage 2: Supervised fine-tuning (SFT)

`train_sft.py` fine-tunes DeepSeek-R1-Distill-Qwen-14B with QLoRA:
- LoRA rank 32, alpha 32, dropout 0.05
- 3 epochs, batch size 2, gradient accumulation 4, lr 2e-4
- Unsloth for 4-bit quantized training
- Training time: ~1h50m on RTX 3090 Ti

This teaches the model Kore syntax and basic program structure.

### Stage 3: GRPO reinforcement learning

`train_grpo.py` runs Group Relative Policy Optimization:
- Generates 4 candidate programs per prompt
- Each is evaluated by `korec serve` (compile + run + verify)
- Dense reward from `scorer.py` (partial credit for compiling, running, type-matching, closeness)
- Training time: ~52 min on RTX 3090 Ti

GRPO was expected to improve robustness over SFT, but in practice the SFT model had already converged on most tasks (reward_std ≈ 0 on most batches). The GRPO stage may have been unnecessary for this curriculum size.

### Verification loop

The key component is `experiment_runner.py` → `ServeRunner`, which manages a persistent `korec serve` process:

```
Python ──stdin──▶ korec serve ──stdout──▶ Python
         (source)              (JSON result)
```

One process handles all evaluations. Measured throughput: ~50,000 programs/sec. This means reward computation is never the bottleneck — the LLM's generation speed is.

### Scoring

`scorer.py` provides dense rewards (not just correct/incorrect):

| Stage | Reward | Condition |
|-------|--------|-----------|
| Compiles | +0.05 | Syntactically valid |
| Type-checks | +0.10 | Passes proof checker |
| Runs | +0.05 | No runtime error |
| Type match | +0.05 | Output type matches expected |
| Closeness | 0–0.30 | Numeric proximity to expected |
| Correct | +1.00 | Exact match |
| Elegance | 0–0.20 | Bonus for shorter programs |
| Penalty | -0.10 | Doesn't compile |

This gives gradient signal even from failed programs, which is critical for RL.

## File structure

```
curriculum.py        — 19-level task generator (478+ tasks)
data_prep.py         — Converts tasks to SFT/GRPO training data
train_sft.py         — Stage 1: QLoRA SFT with Unsloth
train_grpo.py        — Stage 2: GRPO with korec serve rewards
eval_model.py        — Evaluate a model on all curriculum levels
demo.py              — Interactive demo: watch the model write Kore
experiment_runner.py — korec serve pipe interface (~50K evals/sec)
kore_env.py          — Vocabulary, korec interface, environment
scorer.py            — Dense reward function
mutator.py           — Random program mutation (for evolutionary baseline)
baseline_train.py    — Evolutionary search baseline (no LLM)
stdlib_grower.py     — Self-extending stdlib (model writes verified functions)
data/
  sft_train.jsonl    — 2,317 SFT training examples
  grpo_prompts.jsonl — 500 GRPO prompts
```

## How to reproduce

### Prerequisites

- GPU with ≥24GB VRAM (RTX 3090 Ti or better)
- `korec` binary built: `cd <kore-root> && cargo build --release`
- Python 3.10+

### Install dependencies

```bash
pip install unsloth trl transformers datasets torch
```

### Run training

```bash
# Stage 1: SFT (~1h50m on 3090 Ti)
python train_sft.py --output-dir checkpoints/sft

# Stage 2: GRPO (~52min on 3090 Ti)
python train_grpo.py --adapter checkpoints/sft --output-dir checkpoints/grpo

# Evaluate
python eval_model.py --adapter checkpoints/grpo

# Interactive demo
python demo.py checkpoints/grpo
```

### Run without GPU (evolutionary baseline only)

```bash
python baseline_train.py --max-level 5 --episodes 5000
```

This uses mutation + selection + `korec serve` — no neural network. Solves levels 0–5 in ~45 seconds. Proves the eval pipeline works end-to-end.

## Limitations

1. **No held-out test set.** The 99.6% accuracy is evaluated on the same curriculum used for training. The 93.1% demo result is on differently-phrased tasks, which is a better signal but still not a proper held-out benchmark.

2. **GRPO may have been unnecessary.** The SFT model already achieved 99.6% on the curriculum. GRPO reward_std was ~0 on most batches, meaning all 4 candidates per prompt were already correct. The GRPO stage confirmed robustness but likely didn't improve the model.

3. **Curriculum is finite.** 478 tasks across 16 levels is small. The model memorizes solutions rather than learning general Kore programming. Extending to open-ended generation (e.g., "write a Kore program that sorts a list") is future work.

4. **No generalization testing.** We haven't tested the model on tasks outside the curriculum categories. Performance on truly novel tasks is unknown.

5. **Single model.** Only tested with DeepSeek-R1-Distill-Qwen-14B. Smaller models may struggle with the longer reasoning chains; larger models may converge faster.

## What this demonstrates

Despite the limitations, this experiment validates a concrete pipeline:

- An LLM can learn to generate programs in a novel stack-based language
- The compiler's proof checker provides a free verification oracle (no manual test cases)
- `korec serve` makes the eval loop fast enough for RL (50K evals/sec)
- Dense reward shaping (partial credit for compilation, type-checking, closeness) gives gradient signal from every attempt
- The capability-level vocabulary provides a natural curriculum structure

The key insight: **Kore's proof checker is a reward function.** In most code generation settings, you need either test cases (expensive to write) or execution-based evaluation (slow, unreliable). Kore's type system and stack-effect checker give you correctness signal at compile time, for free, at 50K programs/second.
