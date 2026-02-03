# Small-Scale Kore Experiments

Six high-impact, low-compute experiments demonstrating unique capabilities of the Kore language for LLM program synthesis.

## Quick Start

```bash
# From kore root directory
cargo build --release

# Install Python deps
pip install torch transformers accelerate

# Run all experiments (~1-2 hours on RTX 3090)
cd experiments/small-scale
chmod +x run_all.sh
./run_all.sh "Qwen/Qwen2.5-Coder-3B-Instruct"
```

## Experiments

### 1. Effect-Guided Search (~30 min)
**Question**: Does type-directed synthesis beat unconstrained generation?

- Condition A: LLM generates freely, verify after
- Condition B: Filter by effect signature first
- **Hypothesis**: Effect filtering improves success rate

```bash
python 01_effect_guided_search.py -m "Qwen/Qwen2.5-Coder-3B-Instruct" -n 50
```

### 2. Zero-Shot Equivalence (~20 min)
**Question**: Can LLMs detect program equivalence using traces?

- Condition A: Source code only
- Condition B: Source + execution traces  
- Condition C: Traces only (no source!)
- **Hypothesis**: Traces improve accuracy

```bash
python 02_zero_shot_equivalence.py -m "Qwen/Qwen2.5-Coder-3B-Instruct" -n 100
```

### 3. Minimal Programs (~30 min)
**Question**: Can LLMs find minimal-length solutions?

- Compare generated program length to known optimal
- Measures Kolmogorov complexity approximation
- **Hypothesis**: LLMs learn program compression

```bash
python 03_minimal_programs.py -m "Qwen/Qwen2.5-Coder-3B-Instruct"
```

### 4. Effect Prediction (~30 min) ⭐ MOST NOVEL
**Question**: Can LLMs do abstract interpretation?

- LLM predicts effect signature without execution
- Tests understanding of stack semantics
- **Hypothesis**: LLMs learn compositional effects

```bash
python 04_effect_prediction.py -m "Qwen/Qwen2.5-Coder-3B-Instruct" -n 200
```

### 5. Trace Inversion (~40 min)
**Question**: Can LLMs reconstruct programs from traces?

- Novel task: no existing benchmark
- Tests causal reasoning about computation
- **Hypothesis**: LLMs can reverse-engineer programs

```bash
python 05_trace_inversion.py -m "Qwen/Qwen2.5-Coder-3B-Instruct" -n 100
```

### 6. Compositional Arithmetic (~15 min)
**Question**: Can LLMs compose operations they learned separately?

- Train on atoms (add, mul, dup)
- Test on novel compositions
- **Hypothesis**: Zero-shot composition works

```bash
python 06_compositional.py -m "Qwen/Qwen2.5-Coder-3B-Instruct"
```

## System Requirements

- **GPU**: 8GB+ VRAM (RTX 2070+ or equivalent)
  - 3B model: ~7GB VRAM
  - 7B model: ~14GB VRAM (or 8GB with 4-bit)
- **RAM**: 16GB+
- **Time**: ~1-2 hours total for all experiments

## Model Options

| Model | VRAM | Quality |
|-------|------|---------|
| `Qwen/Qwen2.5-Coder-3B-Instruct` | ~7GB | Good |
| `Qwen/Qwen2.5-Coder-7B-Instruct` | ~14GB | Better |
| `deepseek-ai/deepseek-coder-6.7b-instruct` | ~14GB | Good |

## Expected Results

Preliminary estimates (your results may vary):

| Experiment | Expected Accuracy |
|------------|------------------|
| Effect-Guided (improvement) | +15-30% |
| Equivalence (with traces) | 70-85% |
| Minimal Programs (exact) | 40-60% |
| Effect Prediction | 60-80% |
| Trace Inversion | 50-70% |
| Compositional (depth 3-4) | 40-70% |

## Publication Potential

| Experiment | Venue Fit |
|------------|-----------|
| Effect-Guided Search | Workshop (PLDI, ICFP) |
| Zero-Shot Equivalence | Full paper (ICSE, FSE) |
| Minimal Programs | Workshop |
| **Effect Prediction** | **Full paper (NeurIPS, ICLR)** |
| Trace Inversion | Full paper (ACL, EMNLP) |
| Compositional | Workshop (ICLR, NeurIPS) |
