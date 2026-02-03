#!/bin/bash
# Run all small-scale experiments
# Usage: ./run_all.sh [model_name]

set -e

MODEL="${1:-Qwen/Qwen2.5-Coder-3B-Instruct}"
RESULTS_DIR="results/$(date +%Y%m%d_%H%M%S)"

echo "=========================================="
echo "Kore Small-Scale Experiments"
echo "=========================================="
echo "Model: $MODEL"
echo "Results: $RESULTS_DIR"
echo ""

mkdir -p "$RESULTS_DIR"

# Check Kore runtime
cd "$(dirname "$0")/../.."
if [ ! -f "target/release/kore" ]; then
    echo "Building Kore..."
    cargo build --release
fi
cd -

echo ""
echo "=== Experiment 1: Effect-Guided Search ==="
python 01_effect_guided_search.py -m "$MODEL" -n 20 -o "$RESULTS_DIR/exp1_effect_guided.json"

echo ""
echo "=== Experiment 2: Zero-Shot Equivalence ==="
python 02_zero_shot_equivalence.py -m "$MODEL" -n 30 -o "$RESULTS_DIR/exp2_equivalence.json"

echo ""
echo "=== Experiment 3: Minimal Programs ==="
python 03_minimal_programs.py -m "$MODEL" -n 15 -o "$RESULTS_DIR/exp3_minimal.json"

echo ""
echo "=== Experiment 4: Effect Prediction ==="
python 04_effect_prediction.py -m "$MODEL" -n 100 -o "$RESULTS_DIR/exp4_effect.json"

echo ""
echo "=== Experiment 5: Trace Inversion ==="
python 05_trace_inversion.py -m "$MODEL" -n 50 -o "$RESULTS_DIR/exp5_trace.json"

echo ""
echo "=== Experiment 6: Compositional Arithmetic ==="
python 06_compositional.py -m "$MODEL" -o "$RESULTS_DIR/exp6_compositional.json"

echo ""
echo "=========================================="
echo "ALL EXPERIMENTS COMPLETE"
echo "Results saved to: $RESULTS_DIR"
echo "=========================================="

# Generate summary
python -c "
import json
from pathlib import Path

results_dir = Path('$RESULTS_DIR')
print('\n=== SUMMARY ===\n')

for f in sorted(results_dir.glob('*.json')):
    with open(f) as fp:
        data = json.load(fp)
    
    name = f.stem
    if isinstance(data, dict):
        if 'condition_a' in data:
            a = sum(1 for r in data['condition_a'] if r.get('success') or r.get('correct'))
            b = sum(1 for r in data['condition_b'] if r.get('success') or r.get('correct'))
            n = len(data['condition_a'])
            print(f'{name}: A={a}/{n}, B={b}/{n}')
        else:
            print(f'{name}: {len(data)} items')
    elif isinstance(data, list):
        correct = sum(1 for r in data if r.get('correct') or r.get('exact_match'))
        print(f'{name}: {correct}/{len(data)} ({100*correct/len(data):.1f}%)')
"
