#!/bin/bash
# Progressive Benchmark - Persistent agents with escalating tasks
# Both Kore and Baseline stay running and receive tasks via inbox

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/benchmarks/progressive"

# Load .env
[ -f "$SCRIPT_DIR/.env" ] && source "$SCRIPT_DIR/.env"

MODEL="${KORE_MODEL:-gpt-4o}"
TASK_TIMEOUT=120  # seconds per task

# Exponentially harder tasks (fair for both agents)
TASKS=(
    "Create a file called hello.txt containing 'Hello World'"
    "Create two files: greet.txt with 'Hello' and farewell.txt with 'Goodbye'"
    "Read hello.txt and create hello_upper.txt with the content in UPPERCASE"
    "Create a manifest.txt listing all .txt files in /world (one per line)"
    "Fetch https://httpbin.org/json and save the raw response to api_response.json"
    "Create a config.json file with: {\"name\": \"Agent\", \"version\": 1, \"tasks_completed\": 5}"
)

# Container names
KORE_CONTAINER="kore-persistent"
BASELINE_CONTAINER="baseline-persistent"

cleanup() {
    echo "Cleaning up..."
    docker rm -f $KORE_CONTAINER $BASELINE_CONTAINER 2>/dev/null || true
}

trap cleanup EXIT

start_containers() {
    local run_id="$1"
    local kore_dir="$RESULTS_DIR/$run_id/kore"
    local baseline_dir="$RESULTS_DIR/$run_id/baseline"
    
    mkdir -p "$kore_dir" "$baseline_dir" "$kore_dir/mnt" "$baseline_dir/mnt"
    chmod -R 777 "$kore_dir" "$baseline_dir"
    
    echo "Starting persistent containers..."
    
    # Start Kore (loops forever, checks inbox)
    docker run -d --name $KORE_CONTAINER \
        -e "KORE_GOAL=You are a persistent agent. Check /mnt/inbox.txt for new tasks. Complete each task, call done, then wait for next task." \
        -e "KORE_CAPS=all" \
        -e "KORE_MODEL=$MODEL" \
        -e "KORE_MAX_ITERATIONS=0" \
        -e "OPENAI_API_KEY=$OPENAI_API_KEY" \
        -e "OPENAI_BASE_URL=$OPENAI_BASE_URL" \
        -v "$kore_dir:/world" \
        -v "$kore_dir/mnt:/mnt" \
        kore-world:latest kore-agent > /dev/null
    
    # Start Baseline (need to modify baseline agent to be persistent)
    docker run -d --name $BASELINE_CONTAINER \
        -e "AGENT_GOAL=You are a persistent agent. Check /workspace/inbox.txt for new tasks." \
        -e "AGENT_MODEL=$MODEL" \
        -e "AGENT_MAX_ITERATIONS=0" \
        -e "AGENT_PERSISTENT=true" \
        -e "OPENAI_API_KEY=$OPENAI_API_KEY" \
        -e "OPENAI_BASE_URL=$OPENAI_BASE_URL" \
        -v "$baseline_dir:/workspace" \
        baseline-agent:latest > /dev/null
    
    echo "  Kore: $KORE_CONTAINER"
    echo "  Baseline: $BASELINE_CONTAINER"
    
    # Return paths for later use
    echo "$kore_dir" > /tmp/kore_dir
    echo "$baseline_dir" > /tmp/baseline_dir
}

send_task() {
    local agent="$1"
    local task="$2"
    local task_num="$3"
    
    if [ "$agent" = "kore" ]; then
        local dir=$(cat /tmp/kore_dir)
        echo "TASK $task_num: $task" > "$dir/mnt/inbox.txt"
    else
        local dir=$(cat /tmp/baseline_dir)
        echo "TASK $task_num: $task" > "$dir/inbox.txt"
    fi
}

wait_for_completion() {
    local agent="$1"
    local timeout="$2"
    local start=$(date +%s)
    
    local done_file
    if [ "$agent" = "kore" ]; then
        local dir=$(cat /tmp/kore_dir)
        done_file="$dir/mnt/done.txt"
    else
        local dir=$(cat /tmp/baseline_dir)
        done_file="$dir/done.txt"
    fi
    
    while true; do
        local now=$(date +%s)
        local elapsed=$((now - start))
        
        if [ $elapsed -ge $timeout ]; then
            echo "TIMEOUT"
            return 1
        fi
        
        # Check for done file
        if [ -f "$done_file" ]; then
            echo "$elapsed"
            # Clear done file for next task
            rm -f "$done_file"
            return 0
        fi
        
        sleep 2
    done
}

run_progressive() {
    local run_id=$(date +%Y-%m-%d_%H-%M-%S)
    
    echo "════════════════════════════════════════════════════════════════"
    echo "  PROGRESSIVE BENCHMARK"
    echo "  Run ID: $run_id"
    echo "  Model: $MODEL"
    echo "  Tasks: ${#TASKS[@]}"
    echo "════════════════════════════════════════════════════════════════"
    echo ""
    
    # Cleanup any existing containers
    docker rm -f $KORE_CONTAINER $BASELINE_CONTAINER 2>/dev/null || true
    
    start_containers "$run_id"
    
    # Give containers time to start
    sleep 3
    
    local kore_dir=$(cat /tmp/kore_dir)
    local baseline_dir=$(cat /tmp/baseline_dir)
    
    # Results
    echo "task_num,task,kore_time,kore_result,baseline_time,baseline_result" > "$RESULTS_DIR/$run_id/results.csv"
    
    local task_num=0
    for task in "${TASKS[@]}"; do
        task_num=$((task_num + 1))
        
        echo ""
        echo "────────────────────────────────────────────────────────────────"
        echo "TASK $task_num: $task"
        echo "────────────────────────────────────────────────────────────────"
        
        # Clear previous completion markers
        docker logs $KORE_CONTAINER > "$kore_dir/pre_task_${task_num}.log" 2>&1
        docker logs $BASELINE_CONTAINER > "$baseline_dir/pre_task_${task_num}.log" 2>&1
        
        # Send task to both agents
        echo "  Sending to Kore..."
        send_task "kore" "$task" "$task_num"
        
        echo "  Sending to Baseline..."
        send_task "baseline" "$task" "$task_num"
        
        # Wait for both to complete (in parallel would be better, but sequential for now)
        echo "  Waiting for Kore..."
        local kore_time=$(wait_for_completion "kore" $TASK_TIMEOUT)
        local kore_status=$?
        
        echo "  Waiting for Baseline..."
        local baseline_time=$(wait_for_completion "baseline" $TASK_TIMEOUT)
        local baseline_status=$?
        
        # Determine results
        local kore_result="FAIL"
        local baseline_result="FAIL"
        
        [ $kore_status -eq 0 ] && kore_result="PASS"
        [ $baseline_status -eq 0 ] && baseline_result="PASS"
        
        # Save logs
        docker logs $KORE_CONTAINER > "$kore_dir/task_${task_num}.log" 2>&1
        docker logs $BASELINE_CONTAINER > "$baseline_dir/task_${task_num}.log" 2>&1
        
        echo ""
        echo "  ⏱ Kore: ${kore_time}s - $kore_result"
        echo "  ⏱ Baseline: ${baseline_time}s - $baseline_result"
        
        # Save to CSV
        echo "$task_num,\"$task\",$kore_time,$kore_result,$baseline_time,$baseline_result" >> "$RESULTS_DIR/$run_id/results.csv"
        
        # If both failed, maybe stop early
        if [ "$kore_result" = "FAIL" ] && [ "$baseline_result" = "FAIL" ]; then
            echo ""
            echo "⚠️  Both agents failed. Continuing to next task..."
        fi
        
        # Small pause between tasks
        sleep 2
    done
    
    # Final summary
    echo ""
    echo "════════════════════════════════════════════════════════════════"
    echo "  FINAL RESULTS"
    echo "════════════════════════════════════════════════════════════════"
    echo ""
    cat "$RESULTS_DIR/$run_id/results.csv" | column -t -s,
    echo ""
    echo "Results saved to: $RESULTS_DIR/$run_id/"
    echo ""
    
    # Show workspace contents
    echo "Kore workspace:"
    ls -la "$kore_dir/" 2>/dev/null | head -20
    echo ""
    echo "Baseline workspace:"
    ls -la "$baseline_dir/" 2>/dev/null | head -20
}

# Main
run_progressive
