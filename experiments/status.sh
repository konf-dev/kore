#!/bin/bash
# Show status of all experiments

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║               KORE EXPERIMENTS STATUS                          ║${NC}"
echo -e "${CYAN}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Count experiments by status
total=0
planned=0
in_progress=0
complete=0
failed=0

for dir in "$SCRIPT_DIR"/E[0-9][0-9][0-9]-*/; do
    if [[ ! -d "$dir" ]]; then
        continue
    fi
    
    ((total++)) || true
    
    exp_file="$dir/EXPERIMENT.md"
    if [[ ! -f "$exp_file" ]]; then
        continue
    fi
    
    # Extract info from EXPERIMENT.md
    name=$(basename "$dir")
    
    # Parse status from the markdown
    if grep -q "🟢 Complete" "$exp_file" 2>/dev/null; then
        status="${GREEN}🟢 Complete${NC}"
        ((complete++)) || true
    elif grep -q "🟡 In Progress" "$exp_file" 2>/dev/null; then
        status="${YELLOW}🟡 In Progress${NC}"
        ((in_progress++)) || true
    elif grep -q "🔴 Failed" "$exp_file" 2>/dev/null; then
        status="${RED}🔴 Failed${NC}"
        ((failed++)) || true
    else
        status="${BLUE}🔵 Planned${NC}"
        ((planned++)) || true
    fi
    
    # Check for results
    results_count=$(find "$dir/results" -maxdepth 1 -type d 2>/dev/null | wc -l)
    ((results_count--)) || true  # Subtract 1 for the results dir itself
    
    # Print experiment line
    printf "  %-35s %b  [%d runs]\n" "$name" "$status" "$results_count"
done

if [[ $total -eq 0 ]]; then
    echo -e "  ${YELLOW}No experiments found.${NC}"
    echo ""
    echo "  Create one with: ./experiments/new.sh E001 my-experiment"
fi

echo ""
echo -e "${CYAN}────────────────────────────────────────────────────────────────${NC}"
echo -e "  Total: $total | ${BLUE}Planned: $planned${NC} | ${YELLOW}In Progress: $in_progress${NC} | ${GREEN}Complete: $complete${NC} | ${RED}Failed: $failed${NC}"
echo ""
