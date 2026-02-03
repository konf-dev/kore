#!/bin/bash
# Kore Experiment Manager
# Run experiments with different capability configurations

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
EXPERIMENTS_DIR="$SCRIPT_DIR/experiments"
RESULTS_DIR="$EXPERIMENTS_DIR/results"

# Default configuration - shell disabled
DEFAULT_CAPS="fs,http,llm,spawn"

# Capability presets
declare -A PRESETS=(
    ["minimal"]="fs"                           # File system only
    ["network"]="fs,http"                      # + HTTP
    ["llm"]="fs,http,llm"                      # + LLM calls
    ["spawn"]="fs,http,llm,spawn"              # + Sub-agents (default)
    ["shell"]="fs,http,llm,spawn,shell"        # + Shell access (risky!)
    ["all"]="all"                              # Everything
)

usage() {
    cat << EOF
Kore Experiment Manager

USAGE:
    $0 <command> [options]

COMMANDS:
    run <goal> [preset]     Run an experiment with a goal
    list                    List all experiments
    show <id>               Show experiment details
    compare <id1> <id2>     Compare two experiments
    clean                   Remove old experiments

PRESETS:
    minimal    - File system only (safest)
    network    - + HTTP requests
    llm        - + LLM API calls  
    spawn      - + Sub-agent spawning (default)
    shell      - + Shell commands (risky!)
    all        - Everything enabled

EXAMPLES:
    $0 run "Build a hello world" minimal
    $0 run "Create a calculator" llm
    $0 run "Research and build a web scraper" spawn
    $0 list
    $0 show 2024-01-15_14-30-00_abc123

ENVIRONMENT:
    OPENAI_API_KEY          Required for LLM experiments
    KORE_MODEL              Model to use (default: gpt-4o)
    KORE_MAX_ITERATIONS     Max agent iterations (default: 50)
EOF
}

# Generate experiment ID
gen_id() {
    echo "$(date +%Y-%m-%d_%H-%M-%S)_$(head -c 4 /dev/urandom | xxd -p)"
}

# Run an experiment
run_experiment() {
    local goal="$1"
    local preset="${2:-spawn}"
    
    if [ -z "$goal" ]; then
        echo "Error: Goal is required"
        usage
        exit 1
    fi
    
    local caps="${PRESETS[$preset]}"
    if [ -z "$caps" ]; then
        echo "Error: Unknown preset '$preset'"
        echo "Available presets: ${!PRESETS[@]}"
        exit 1
    fi
    
    local exp_id=$(gen_id)
    local exp_dir="$RESULTS_DIR/$exp_id"
    
    mkdir -p "$exp_dir/logs"
    mkdir -p "$exp_dir/world"
    
    # Save experiment metadata
    cat > "$exp_dir/metadata.json" << EOF
{
    "id": "$exp_id",
    "goal": "$goal",
    "preset": "$preset",
    "capabilities": "$caps",
    "started_at": "$(date -Iseconds)",
    "model": "${KORE_MODEL:-gpt-4o}",
    "max_iterations": ${KORE_MAX_ITERATIONS:-50}
}
EOF
    
    echo "═══════════════════════════════════════════════════════════════"
    echo "  KORE EXPERIMENT: $exp_id"
    echo "═══════════════════════════════════════════════════════════════"
    echo "  Goal:         $goal"
    echo "  Preset:       $preset"
    echo "  Capabilities: $caps"
    echo "  Model:        ${KORE_MODEL:-gpt-4o}"
    echo "  Results:      $exp_dir"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
    
    # Build Docker command with capability restrictions
    local docker_caps=""
    case "$preset" in
        "minimal")
            docker_caps="--cap-drop=ALL --security-opt=no-new-privileges"
            ;;
        "network"|"llm"|"spawn")
            docker_caps="--cap-drop=ALL --security-opt=no-new-privileges"
            ;;
        "shell"|"all")
            docker_caps=""  # Full access
            echo "⚠️  WARNING: Shell access enabled - agent can run arbitrary commands"
            ;;
    esac
    
    # Run the experiment
    docker compose run --rm \
        -e "KORE_GOAL=$goal" \
        -e "KORE_CAPS=$caps" \
        -e "KORE_MODEL=${KORE_MODEL:-gpt-4o}" \
        -e "KORE_MAX_ITERATIONS=${KORE_MAX_ITERATIONS:-50}" \
        -e "OPENAI_API_KEY=${OPENAI_API_KEY}" \
        -v "$exp_dir/world:/world" \
        -v "$exp_dir/logs:/logs" \
        $docker_caps \
        kore-agent 2>&1 | tee "$exp_dir/logs/stdout.log"
    
    local exit_code=${PIPESTATUS[0]}
    
    # Update metadata with results
    local ended_at=$(date -Iseconds)
    cat > "$exp_dir/metadata.json" << EOF
{
    "id": "$exp_id",
    "goal": "$goal",
    "preset": "$preset", 
    "capabilities": "$caps",
    "started_at": "$(jq -r .started_at "$exp_dir/metadata.json")",
    "ended_at": "$ended_at",
    "exit_code": $exit_code,
    "model": "${KORE_MODEL:-gpt-4o}",
    "max_iterations": ${KORE_MAX_ITERATIONS:-50}
}
EOF
    
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    if [ $exit_code -eq 0 ]; then
        echo "  ✅ EXPERIMENT COMPLETED: $exp_id"
    else
        echo "  ❌ EXPERIMENT FAILED: $exp_id (exit code: $exit_code)"
    fi
    echo "  Results: $exp_dir"
    echo "═══════════════════════════════════════════════════════════════"
}

# List experiments
list_experiments() {
    echo "KORE EXPERIMENTS"
    echo "════════════════"
    echo ""
    
    if [ ! -d "$RESULTS_DIR" ]; then
        echo "No experiments found."
        return
    fi
    
    printf "%-30s %-10s %-10s %s\n" "ID" "PRESET" "STATUS" "GOAL"
    printf "%-30s %-10s %-10s %s\n" "──────────────────────────────" "──────────" "──────────" "────────────────────"
    
    for exp_dir in "$RESULTS_DIR"/*; do
        if [ -f "$exp_dir/metadata.json" ]; then
            local id=$(basename "$exp_dir")
            local preset=$(jq -r .preset "$exp_dir/metadata.json")
            local exit_code=$(jq -r '.exit_code // "running"' "$exp_dir/metadata.json")
            local goal=$(jq -r .goal "$exp_dir/metadata.json" | head -c 40)
            
            local status="⏳"
            if [ "$exit_code" = "0" ]; then
                status="✅"
            elif [ "$exit_code" != "running" ] && [ "$exit_code" != "null" ]; then
                status="❌"
            fi
            
            printf "%-30s %-10s %-10s %s\n" "$id" "$preset" "$status" "$goal"
        fi
    done
}

# Show experiment details
show_experiment() {
    local exp_id="$1"
    local exp_dir="$RESULTS_DIR/$exp_id"
    
    if [ ! -d "$exp_dir" ]; then
        echo "Experiment not found: $exp_id"
        exit 1
    fi
    
    echo "EXPERIMENT: $exp_id"
    echo "════════════════════════════════════════════════════════════════"
    echo ""
    
    if [ -f "$exp_dir/metadata.json" ]; then
        cat "$exp_dir/metadata.json" | jq .
    fi
    
    echo ""
    echo "FILES IN WORLD:"
    echo "────────────────"
    ls -la "$exp_dir/world/" 2>/dev/null || echo "(empty)"
    
    echo ""
    echo "LOG FILES:"
    echo "────────────────"
    ls -la "$exp_dir/logs/" 2>/dev/null || echo "(empty)"
    
    if [ -f "$exp_dir/logs/trace.jsonl" ]; then
        echo ""
        echo "TRACE SUMMARY (last 5 steps):"
        echo "────────────────────────────────"
        tail -5 "$exp_dir/logs/trace.jsonl" | jq -c '{kind, message: .message[0:80]}' 2>/dev/null || tail -5 "$exp_dir/logs/trace.jsonl"
    fi
}

# Compare experiments
compare_experiments() {
    local id1="$1"
    local id2="$2"
    
    if [ -z "$id1" ] || [ -z "$id2" ]; then
        echo "Usage: $0 compare <id1> <id2>"
        exit 1
    fi
    
    echo "COMPARING EXPERIMENTS"
    echo "════════════════════════════════════════════════════════════════"
    echo ""
    
    printf "%-20s %-30s %-30s\n" "PROPERTY" "$id1" "$id2"
    printf "%-20s %-30s %-30s\n" "────────────────────" "──────────────────────────────" "──────────────────────────────"
    
    for prop in preset capabilities exit_code model; do
        local val1=$(jq -r ".$prop // \"N/A\"" "$RESULTS_DIR/$id1/metadata.json" 2>/dev/null || echo "N/A")
        local val2=$(jq -r ".$prop // \"N/A\"" "$RESULTS_DIR/$id2/metadata.json" 2>/dev/null || echo "N/A")
        printf "%-20s %-30s %-30s\n" "$prop" "$val1" "$val2"
    done
    
    echo ""
    echo "FILES DIFF:"
    echo "────────────────"
    diff -rq "$RESULTS_DIR/$id1/world" "$RESULTS_DIR/$id2/world" 2>/dev/null || echo "Directories differ or don't exist"
}

# Clean old experiments
clean_experiments() {
    local days="${1:-7}"
    echo "Removing experiments older than $days days..."
    find "$RESULTS_DIR" -maxdepth 1 -type d -mtime "+$days" -exec rm -rf {} \;
    echo "Done."
}

# Main
case "${1:-}" in
    run)
        run_experiment "$2" "$3"
        ;;
    list)
        list_experiments
        ;;
    show)
        show_experiment "$2"
        ;;
    compare)
        compare_experiments "$2" "$3"
        ;;
    clean)
        clean_experiments "${2:-7}"
        ;;
    *)
        usage
        ;;
esac
