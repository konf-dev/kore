#!/bin/bash
# Create a new experiment from template

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATE_DIR="$SCRIPT_DIR/.templates"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

usage() {
    echo "Usage: $0 <experiment-id> <experiment-name>"
    echo ""
    echo "Examples:"
    echo "  $0 E001 agent-runtime"
    echo "  $0 E002 verified-synthesis"
    echo ""
    echo "This will create: experiments/<id>-<name>/"
    exit 1
}

if [[ $# -lt 2 ]]; then
    usage
fi

EXPERIMENT_ID="$1"
EXPERIMENT_NAME="$2"
EXPERIMENT_DIR="$SCRIPT_DIR/${EXPERIMENT_ID}-${EXPERIMENT_NAME}"

# Validate ID format
if [[ ! "$EXPERIMENT_ID" =~ ^E[0-9]{3}$ ]]; then
    echo -e "${RED}Error: Experiment ID must be in format E###${NC}"
    echo "Example: E001, E042, E123"
    exit 1
fi

# Check if already exists
if [[ -d "$EXPERIMENT_DIR" ]]; then
    echo -e "${RED}Error: Experiment directory already exists: $EXPERIMENT_DIR${NC}"
    exit 1
fi

echo -e "${BLUE}Creating experiment: ${EXPERIMENT_ID} - ${EXPERIMENT_NAME}${NC}"

# Create directory structure
mkdir -p "$EXPERIMENT_DIR"/{src,kore,data,results}

# Copy and customize templates
for template in EXPERIMENT.md config.toml run.sh; do
    sed -e "s/{{ID}}/$EXPERIMENT_ID/g" \
        -e "s/{{NAME}}/$EXPERIMENT_NAME/g" \
        "$TEMPLATE_DIR/$template" > "$EXPERIMENT_DIR/$template"
done

# Make run.sh executable
chmod +x "$EXPERIMENT_DIR/run.sh"

# Create placeholder files
touch "$EXPERIMENT_DIR/src/.gitkeep"
touch "$EXPERIMENT_DIR/data/.gitkeep"

cat > "$EXPERIMENT_DIR/kore/main.kore" << 'EOF'
; Main experiment code
; This file contains the primary Kore code for the experiment

; === EXPERIMENT SETUP ===


; === EXPERIMENT CODE ===


; === VERIFICATION ===

EOF

echo -e "${GREEN}✓ Created experiment directory: $EXPERIMENT_DIR${NC}"
echo ""
echo "Next steps:"
echo "  1. Edit $EXPERIMENT_DIR/EXPERIMENT.md to define the experiment"
echo "  2. Edit $EXPERIMENT_DIR/config.toml to set parameters"
echo "  3. Write Kore code in $EXPERIMENT_DIR/kore/"
echo "  4. Implement run_experiment() in $EXPERIMENT_DIR/run.sh"
echo "  5. Run with: $EXPERIMENT_DIR/run.sh"
