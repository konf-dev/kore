#!/bin/bash
# Kore vs Baseline Benchmark
# Compare Kore-only agent against standard shell-based agent

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BENCHMARK_DIR="$SCRIPT_DIR/benchmarks"
RESULTS_DIR="$BENCHMARK_DIR/results"

# Load environment variables
if [ -f "$SCRIPT_DIR/.env" ]; then
    set -a
    source "$SCRIPT_DIR/.env"
    set +a
fi

mkdir -p "$RESULTS_DIR"

# Benchmark tasks - various difficulty levels
declare -a TASKS=(
    "easy:Create a file called hello.txt containing 'Hello World'"
    "easy:List all files in the current directory and save to files.txt"
    "medium:Write a Python script that calculates fibonacci(20) and save the result"
    "medium:Fetch https://httpbin.org/json and extract the 'slideshow' title"
    "medium:Create a simple TODO list manager with add/list/remove functions"
    "hard:Build a web scraper that gets the top 5 headlines from Hacker News"
    "hard:Create a markdown documentation generator that reads Python files"
    "hard:Build a simple key-value store with get/set/delete operations"
)

usage() {
    cat << EOF
Kore vs Baseline Benchmark

USAGE:
    $0 <command> [options]

COMMANDS:
    run [task_filter]    Run benchmarks (optional: easy|medium|hard)
    results              Show benchmark results
    compare <id>         Compare a specific benchmark run
    tasks                List available tasks

EXAMPLES:
    $0 run               # Run all benchmarks
    $0 run easy          # Run only easy tasks
    $0 run medium        # Run only medium tasks
    $0 results           # Show comparison table
    $0 tasks             # List all tasks

ENVIRONMENT:
    OPENAI_API_KEY       Required
    KORE_MODEL           Model to use (default: gpt-4o)
    BENCHMARK_TIMEOUT    Timeout per task in seconds (default: 300)
EOF
}

# Generate benchmark ID
gen_benchmark_id() {
    echo "$(date +%Y-%m-%d_%H-%M-%S)"
}

# Run a single task with Kore agent
run_kore_agent() {
    local task="$1"
    local work_dir="$2"
    local log_file="$3"
    local timeout="${BENCHMARK_TIMEOUT:-90}"
    
    echo "  [KORE] Starting..." >&2
    
    local start_time=$(date +%s.%N)
    
    # Note: --rm removes container after run, -v mounts workspace
    # KORE_CAPS uses format: fs:read:/path,fs:write:/path,net:connect:*,exec,spawn
    # KORE_MAX_ITERATIONS kept low for benchmarking (agent completes early)
    # Use -T to disable pseudo-TTY (prevents hang on non-interactive)
    timeout --signal=KILL "$timeout" docker compose -f "$SCRIPT_DIR/docker-compose.yml" run --rm -T \
        -e "KORE_GOAL=$task" \
        -e "KORE_CAPS=all" \
        -e "KORE_MODEL=${KORE_MODEL:-gpt-4o}" \
        -e "KORE_MAX_ITERATIONS=5" \
        -e "OPENAI_API_KEY=${OPENAI_API_KEY}" \
        -e "OPENAI_BASE_URL=${OPENAI_BASE_URL}" \
        --volume "$work_dir:/world" \
        kore-agent > "$log_file" 2>&1 || true
    
    local end_time=$(date +%s.%N)
    local duration=$(echo "$end_time - $start_time" | bc)
    
    echo "$duration"
}

# Run a single task with Baseline agent (shell access)
run_baseline_agent() {
    local task="$1"
    local work_dir="$2"
    local log_file="$3"
    local timeout="${BENCHMARK_TIMEOUT:-90}"
    
    echo "  [BASELINE] Starting..." >&2
    
    local start_time=$(date +%s.%N)
    
    # Use -T to disable pseudo-TTY (prevents hang on non-interactive)
    timeout --signal=KILL "$timeout" docker compose -f "$SCRIPT_DIR/docker-compose.yml" run --rm -T \
        -e "AGENT_GOAL=$task" \
        -e "OPENAI_API_KEY=${OPENAI_API_KEY}" \
        -e "OPENAI_BASE_URL=${OPENAI_BASE_URL}" \
        -e "AGENT_MODEL=${KORE_MODEL:-gpt-4o}" \
        -e "AGENT_MAX_ITERATIONS=5" \
        --volume "$work_dir:/workspace" \
        baseline-agent > "$log_file" 2>&1 || true
    
    local end_time=$(date +%s.%N)
    local duration=$(echo "$end_time - $start_time" | bc)
    
    echo "$duration"
}

# Evaluate task completion
evaluate_task() {
    local task_type="$1"
    local task="$2"
    local work_dir="$3"
    
    # Simple heuristics for evaluation
    case "$task" in
        *"hello.txt"*)
            [ -f "$work_dir/hello.txt" ] && grep -q "Hello" "$work_dir/hello.txt" && echo "PASS" || echo "FAIL"
            ;;
        *"files.txt"*)
            [ -f "$work_dir/files.txt" ] && echo "PASS" || echo "FAIL"
            ;;
        *"fibonacci"*)
            # Check if any file contains 6765 (fib(20))
            grep -r "6765" "$work_dir" >/dev/null 2>&1 && echo "PASS" || echo "FAIL"
            ;;
        *"httpbin"*|*"slideshow"*)
            # Check if slideshow title was extracted
            grep -ri "slide\|sample" "$work_dir" >/dev/null 2>&1 && echo "PASS" || echo "FAIL"
            ;;
        *"TODO"*|*"todo"*)
            # Check if any todo-related files exist
            ls "$work_dir"/*.{py,js,kore,txt} >/dev/null 2>&1 && echo "PASS" || echo "FAIL"
            ;;
        *"Hacker News"*|*"headlines"*)
            # Check if headlines were saved
            find "$work_dir" -type f -exec grep -l "." {} \; | head -1 | grep -q . && echo "PASS" || echo "FAIL"
            ;;
        *)
            # Default: check if any files were created
            [ "$(ls -A "$work_dir" 2>/dev/null)" ] && echo "PARTIAL" || echo "FAIL"
            ;;
    esac
}

# Run benchmarks
run_benchmarks() {
    local filter="${1:-}"
    local benchmark_id=$(gen_benchmark_id)
    local benchmark_dir="$RESULTS_DIR/$benchmark_id"
    
    mkdir -p "$benchmark_dir"
    
    echo "═══════════════════════════════════════════════════════════════════════"
    echo "  KORE vs BASELINE BENCHMARK"
    echo "  ID: $benchmark_id"
    echo "  Model: ${KORE_MODEL:-gpt-4o}"
    echo "  Timeout: ${BENCHMARK_TIMEOUT:-300}s per task"
    echo "═══════════════════════════════════════════════════════════════════════"
    echo ""
    
    # Results CSV
    echo "task_id,difficulty,task,kore_time,kore_result,baseline_time,baseline_result" > "$benchmark_dir/results.csv"
    
    local task_num=0
    for task_entry in "${TASKS[@]}"; do
        local difficulty="${task_entry%%:*}"
        local task="${task_entry#*:}"
        
        # Apply filter
        if [ -n "$filter" ] && [ "$difficulty" != "$filter" ]; then
            continue
        fi
        
        task_num=$((task_num + 1))
        local task_id="task_$(printf '%02d' $task_num)"
        
        echo "───────────────────────────────────────────────────────────────────────"
        echo "TASK $task_num [$difficulty]: $task"
        echo "───────────────────────────────────────────────────────────────────────"
        
        # Prepare directories (world-writable for container access)
        local kore_dir="$benchmark_dir/$task_id/kore"
        local baseline_dir="$benchmark_dir/$task_id/baseline"
        mkdir -p "$kore_dir" "$baseline_dir"
        chmod 777 "$kore_dir" "$baseline_dir"
        
        # Run Kore agent
        local kore_time=$(run_kore_agent "$task" "$kore_dir" "$benchmark_dir/$task_id/kore.log")
        local kore_result=$(evaluate_task "$difficulty" "$task" "$kore_dir")
        echo "  [KORE] Time: ${kore_time}s, Result: $kore_result"
        
        # Run Baseline agent
        local baseline_time=$(run_baseline_agent "$task" "$baseline_dir" "$benchmark_dir/$task_id/baseline.log")
        local baseline_result=$(evaluate_task "$difficulty" "$task" "$baseline_dir")
        echo "  [BASELINE] Time: ${baseline_time}s, Result: $baseline_result"
        
        # Save to CSV
        echo "$task_id,$difficulty,\"$task\",$kore_time,$kore_result,$baseline_time,$baseline_result" >> "$benchmark_dir/results.csv"
        
        echo ""
    done
    
    # Generate summary
    generate_summary "$benchmark_dir"
}

# Generate summary report
generate_summary() {
    local benchmark_dir="$1"
    
    echo "═══════════════════════════════════════════════════════════════════════"
    echo "  BENCHMARK SUMMARY"
    echo "═══════════════════════════════════════════════════════════════════════"
    echo ""
    
    # Count results
    local kore_pass=$(grep -c ",PASS," "$benchmark_dir/results.csv" 2>/dev/null | head -1 || echo 0)
    local baseline_pass=$(grep ",PASS$" "$benchmark_dir/results.csv" 2>/dev/null | wc -l || echo 0)
    local total=$(tail -n +2 "$benchmark_dir/results.csv" | wc -l)
    
    printf "%-20s %-15s %-15s\n" "" "KORE" "BASELINE"
    printf "%-20s %-15s %-15s\n" "────────────────────" "───────────────" "───────────────"
    printf "%-20s %-15s %-15s\n" "Tasks Passed" "$kore_pass/$total" "$baseline_pass/$total"
    
    # Average times (if bc is available)
    if command -v bc &> /dev/null; then
        local kore_avg=$(awk -F',' 'NR>1 {sum+=$4; count++} END {if(count>0) printf "%.2f", sum/count; else print "N/A"}' "$benchmark_dir/results.csv")
        local baseline_avg=$(awk -F',' 'NR>1 {sum+=$6; count++} END {if(count>0) printf "%.2f", sum/count; else print "N/A"}' "$benchmark_dir/results.csv")
        printf "%-20s %-15s %-15s\n" "Avg Time (s)" "$kore_avg" "$baseline_avg"
    fi
    
    echo ""
    echo "Results saved to: $benchmark_dir"
    echo ""
    
    # Winner determination
    if [ "$kore_pass" -gt "$baseline_pass" ]; then
        echo "🏆 WINNER: KORE (more tasks completed)"
    elif [ "$baseline_pass" -gt "$kore_pass" ]; then
        echo "🏆 WINNER: BASELINE (more tasks completed)"
    else
        echo "🤝 TIE: Both completed same number of tasks"
    fi
}

# Show results
show_results() {
    echo "BENCHMARK RESULTS"
    echo "═══════════════════════════════════════════════════════════════════════"
    echo ""
    
    for result_dir in "$RESULTS_DIR"/*; do
        if [ -f "$result_dir/results.csv" ]; then
            local id=$(basename "$result_dir")
            echo "Benchmark: $id"
            echo "───────────────────────────────────────────────────────────────────────"
            column -t -s',' "$result_dir/results.csv" 2>/dev/null || cat "$result_dir/results.csv"
            echo ""
        fi
    done
}

# List tasks
list_tasks() {
    echo "BENCHMARK TASKS"
    echo "═══════════════════════════════════════════════════════════════════════"
    echo ""
    
    local num=0
    for task_entry in "${TASKS[@]}"; do
        num=$((num + 1))
        local difficulty="${task_entry%%:*}"
        local task="${task_entry#*:}"
        printf "%2d. [%-6s] %s\n" "$num" "$difficulty" "$task"
    done
}

# Main
case "${1:-}" in
    run)
        run_benchmarks "$2"
        ;;
    results)
        show_results
        ;;
    compare)
        show_results  # TODO: detailed comparison
        ;;
    tasks)
        list_tasks
        ;;
    *)
        usage
        ;;
esac
