#!/bin/bash
# Kore-RL Training Script
# =======================
#
# Prerequisites:
# 1. Docker with BuildKit
# 2. NVIDIA GPU with drivers
# 3. Python 3.10+ with PyTorch
#
# Usage:
#   ./scripts/train.sh [phase] [model]
#
# Examples:
#   ./scripts/train.sh 1 qwen-7b     # Phase 1 with Qwen 7B
#   ./scripts/train.sh 2 qwen-32b    # Phase 2 with Qwen 32B

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

PHASE="${1:-1}"
MODEL="${2:-qwen-7b}"

# Model mapping
case "$MODEL" in
    qwen-7b)
        MODEL_NAME="Qwen/Qwen2.5-Coder-7B-Instruct"
        ;;
    qwen-32b)
        MODEL_NAME="Qwen/Qwen2.5-Coder-32B-Instruct"
        ;;
    deepseek-lite)
        MODEL_NAME="deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct"
        ;;
    deepseek)
        MODEL_NAME="deepseek-ai/DeepSeek-Coder-V2-Instruct"
        ;;
    *)
        MODEL_NAME="$MODEL"
        ;;
esac

echo "=========================================="
echo "Kore-RL Training"
echo "=========================================="
echo "Phase: $PHASE"
echo "Model: $MODEL_NAME"
echo "Project: $PROJECT_DIR"
echo "=========================================="

# Step 1: Start Kore runtime containers
echo ""
echo "Starting Kore runtime containers..."
cd "$PROJECT_DIR/docker"

# Build and start
docker compose up -d --build

# Wait for health
echo "Waiting for runtime to be healthy..."
for i in {1..30}; do
    if curl -s http://localhost:8080/health > /dev/null 2>&1; then
        echo "Runtime is healthy!"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "ERROR: Runtime failed to start"
        docker compose logs
        exit 1
    fi
    sleep 1
done

# Quick test
echo ""
echo "Testing runtime..."
RESULT=$(curl -s -X POST http://localhost:8080/execute \
    -H "Content-Type: application/json" \
    -d '{"program": "3 4 add"}')
echo "Test: 3 4 add → $RESULT"

# Step 2: Install Python dependencies
echo ""
echo "Setting up Python environment..."
cd "$PROJECT_DIR"

if [ ! -d ".venv" ]; then
    python3 -m venv .venv
fi

source .venv/bin/activate

pip install --quiet --upgrade pip
pip install --quiet torch transformers accelerate
pip install --quiet httpx pyyaml tqdm

# Step 3: Run training
echo ""
echo "Starting training..."
echo ""

cd "$PROJECT_DIR/training"

python trainer.py \
    --model "$MODEL_NAME" \
    --runtime-url "http://localhost:8080" \
    --output-dir "../checkpoints" \
    --phase "$PHASE" \
    --batch-size 4 \
    --num-samples 8 \
    --max-steps 10000

echo ""
echo "Training complete!"
echo "Checkpoints saved to: $PROJECT_DIR/checkpoints"
