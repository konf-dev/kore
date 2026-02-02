#!/bin/bash
# Experiment runner - isolates agent in Docker, captures everything
#
# Usage: ./run.sh <prompt.md> <goal> [model]
#
# Required env vars: OPENAI_API_KEY, OPENAI_BASE_URL

set -euo pipefail

# Parse args
PROMPT_FILE="${1:?Usage: ./run.sh <prompt.md> <goal> [model]}"
GOAL="${2:?Usage: ./run.sh <prompt.md> <goal> [model]}"
MODEL="${3:-qwen2.5-32b}"

# Check required env vars
if [[ -z "${OPENAI_API_KEY:-}" ]]; then
    echo "Error: OPENAI_API_KEY is required"
    exit 1
fi

# Check prompt file exists
if [[ ! -f "${PROMPT_FILE}" ]]; then
    echo "Error: Prompt file not found: ${PROMPT_FILE}"
    exit 1
fi

# Generate experiment ID
TIMESTAMP=$(date +%Y-%m-%d_%H-%M-%S)
CODE_VERSION=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
EXPERIMENT_ID="${TIMESTAMP}_${CODE_VERSION}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
EXPERIMENT_DIR="${SCRIPT_DIR}/results/${EXPERIMENT_ID}"

# Create directories
mkdir -p "${EXPERIMENT_DIR}"/{workspace,logs}

# Copy prompt (immutable record of what was used)
cp "${PROMPT_FILE}" "${EXPERIMENT_DIR}/prompt.md"

# Save input config
PROMPT_HASH=$(sha256sum "${PROMPT_FILE}" | cut -d' ' -f1)
GIT_DIRTY=$(git diff --quiet 2>/dev/null && echo "false" || echo "true")

cat > "${EXPERIMENT_DIR}/config.json" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "id": "${EXPERIMENT_ID}",
  "inputs": {
    "goal": "${GOAL}",
    "prompt_file": "${PROMPT_FILE}",
    "prompt_sha256": "${PROMPT_HASH}",
    "model": "${MODEL}",
    "code_version": "$(git rev-parse HEAD 2>/dev/null || echo 'unknown')",
    "code_dirty": ${GIT_DIRTY}
  },
  "outputs": null
}
EOF

echo "═══════════════════════════════════════════════════════════════════════"
echo "  EXPERIMENT: ${EXPERIMENT_ID}"
echo "  Goal: ${GOAL}"
echo "  Model: ${MODEL}"
echo "  Prompt: ${PROMPT_FILE}"
echo "  Output: ${EXPERIMENT_DIR}"
echo "═══════════════════════════════════════════════════════════════════════"
echo ""

# Run agent in Docker with full sandbox capabilities
START_TIME=$(date +%s)

# Network: bridge (default) - has internet access but isolated from host localhost
# No --network=host means container can't access host's localhost/127.0.0.1
# Agent runs as root inside container, can apt install, npm install, etc.
docker run --rm \
    --name "kore-${EXPERIMENT_ID}" \
    -v "${EXPERIMENT_DIR}/workspace:/workspace" \
    -v "${EXPERIMENT_DIR}/logs:/logs" \
    -v "${EXPERIMENT_DIR}/prompt.md:/prompt.md:ro" \
    -e "KORE_WORKSPACE=/workspace" \
    -e "KORE_LOGS=/logs" \
    -e "KORE_PROMPT=/prompt.md" \
    -e "KORE_GOAL=${GOAL}" \
    -e "OPENAI_API_KEY=${OPENAI_API_KEY}" \
    -e "OPENAI_BASE_URL=${OPENAI_BASE_URL:-https://api.openai.com}" \
    -e "OPENAI_MODEL=${MODEL}" \
    kore-agent 2>&1 | tee "${EXPERIMENT_DIR}/logs/stdout.log"

EXIT_CODE=${PIPESTATUS[0]}
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# Count iterations from trace
ITERATIONS=$(grep -c '"kind":"iteration"' "${EXPERIMENT_DIR}/logs/trace.jsonl" 2>/dev/null || echo 0)

# List workspace files
WORKSPACE_FILES=$(ls -1 "${EXPERIMENT_DIR}/workspace" 2>/dev/null | tr '\n' ',' | sed 's/,$//' || echo "")

# Update config with outputs using a temp file
python3 -c "
import json
import sys

with open('${EXPERIMENT_DIR}/config.json', 'r') as f:
    config = json.load(f)

config['outputs'] = {
    'exit_code': ${EXIT_CODE},
    'iterations': ${ITERATIONS},
    'duration_seconds': ${DURATION},
    'workspace_files': '${WORKSPACE_FILES}'.split(',') if '${WORKSPACE_FILES}' else []
}

with open('${EXPERIMENT_DIR}/config.json', 'w') as f:
    json.dump(config, f, indent=2)
" 2>/dev/null || echo "Note: Could not update config.json with outputs"

echo ""
echo "═══════════════════════════════════════════════════════════════════════"
echo "  COMPLETE: ${EXPERIMENT_ID}"
echo "  Duration: ${DURATION}s"
echo "  Iterations: ${ITERATIONS}"
echo "  Exit Code: ${EXIT_CODE}"
echo "  Results: ${EXPERIMENT_DIR}"
echo "═══════════════════════════════════════════════════════════════════════"
