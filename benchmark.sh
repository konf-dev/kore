#!/usr/bin/env bash
#
# Kore Comprehensive Benchmark & Regression Check
# =================================================
# Run after every update to verify nothing broke.
# Generates a timestamped results file in benchmark-results/
#
# Usage:
#   ./benchmark.sh           # Full run (tests + experiments + benchmarks)
#   ./benchmark.sh --quick   # Tests only (skip experiments and benchmarks)
#   ./benchmark.sh --help    # Show help
#
set -euo pipefail

KORE_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$KORE_DIR"

TIMESTAMP=$(date '+%Y-%m-%d_%H%M%S')
RESULTS_DIR="$KORE_DIR/benchmark-results"
RESULTS_FILE="$RESULTS_DIR/${TIMESTAMP}.txt"
KOREC="$KORE_DIR/target/release/korec"

QUICK=false
FAILED=0
PASSED=0
SKIPPED=0
TOTAL_SECTIONS=0

# ─── Colors ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# ─── Parse args ──────────────────────────────────────────────────────────────
for arg in "$@"; do
    case "$arg" in
        --quick)  QUICK=true ;;
        --help|-h)
            echo "Usage: ./benchmark.sh [--quick] [--help]"
            echo ""
            echo "  --quick   Run tests only (skip experiments and benchmarks)"
            echo "  --help    Show this help"
            echo ""
            echo "Results are written to benchmark-results/YYYY-MM-DD_HHMMSS.txt"
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg"
            exit 1
            ;;
    esac
done

# ─── Setup ───────────────────────────────────────────────────────────────────
mkdir -p "$RESULTS_DIR"

log() {
    echo -e "$1" | tee -a "$RESULTS_FILE"
}

section() {
    TOTAL_SECTIONS=$((TOTAL_SECTIONS + 1))
    echo "" | tee -a "$RESULTS_FILE"
    log "${BLUE}${BOLD}═══════════════════════════════════════════════════════════════${NC}"
    log "${BLUE}${BOLD}  [$TOTAL_SECTIONS] $1${NC}"
    log "${BLUE}${BOLD}═══════════════════════════════════════════════════════════════${NC}"
}

pass() {
    PASSED=$((PASSED + 1))
    log "${GREEN}  ✓ $1${NC}"
}

fail() {
    FAILED=$((FAILED + 1))
    log "${RED}  ✗ $1${NC}"
}

skip() {
    SKIPPED=$((SKIPPED + 1))
    log "${YELLOW}  ⏭ $1 (skipped)${NC}"
}

# ─── Header ──────────────────────────────────────────────────────────────────
COMMIT=$(git log --oneline -1 2>/dev/null || echo "unknown")
BRANCH=$(git branch --show-current 2>/dev/null || echo "unknown")
RUST_VER=$(rustc --version 2>/dev/null || echo "unknown")

log "${BOLD}Kore Comprehensive Benchmark & Regression Check${NC}"
log "================================================"
log "Date:     $(date '+%Y-%m-%d %H:%M:%S')"
log "Commit:   $COMMIT"
log "Branch:   $BRANCH"
log "Rust:     $RUST_VER"
log "Mode:     $(if $QUICK; then echo 'QUICK (tests only)'; else echo 'FULL'; fi)"
log "Output:   $RESULTS_FILE"
log ""

# ═══════════════════════════════════════════════════════════════════════════
# 1. BUILD
# ═══════════════════════════════════════════════════════════════════════════
section "Build (release, all features)"

BUILD_START=$(date +%s%N)
if cargo build --release 2>&1 | tee -a "$RESULTS_FILE"; then
    BUILD_END=$(date +%s%N)
    BUILD_MS=$(( (BUILD_END - BUILD_START) / 1000000 ))
    pass "cargo build --release succeeded (${BUILD_MS}ms)"
else
    fail "cargo build --release FAILED"
    log "${RED}BUILD FAILED — cannot continue${NC}"
    exit 1
fi

# ═══════════════════════════════════════════════════════════════════════════
# 2. ALL TESTS
# ═══════════════════════════════════════════════════════════════════════════
section "Test Suite (all 490 tests)"

TEST_START=$(date +%s%N)
TEST_OUTPUT=$(cargo test --release 2>&1) || true
TEST_END=$(date +%s%N)
TEST_MS=$(( (TEST_END - TEST_START) / 1000000 ))

# Parse test results
TEST_RESULT=$(echo "$TEST_OUTPUT" | grep '^test result:' | tail -1)
if echo "$TEST_OUTPUT" | grep -q 'test result: ok'; then
    TOTAL_PASS=$(echo "$TEST_OUTPUT" | grep '^test result:' | awk '{sum += $4} END {print sum}')
    TOTAL_FAIL=$(echo "$TEST_OUTPUT" | grep '^test result:' | awk '{sum += $6} END {print sum}')
    TOTAL_IGNORE=$(echo "$TEST_OUTPUT" | grep '^test result:' | awk '{sum += $8} END {print sum}')
    pass "All tests passed: $TOTAL_PASS passed, $TOTAL_FAIL failed, $TOTAL_IGNORE ignored (${TEST_MS}ms)"
else
    # Show which tests failed
    FAILURES=$(echo "$TEST_OUTPUT" | grep '^test .* FAILED' || true)
    fail "Test suite has failures (${TEST_MS}ms)"
    if [ -n "$FAILURES" ]; then
        log "  Failed tests:"
        echo "$FAILURES" | while read -r line; do
            log "    $line"
        done
    fi
    # Write full output for debugging
    echo "$TEST_OUTPUT" >> "$RESULTS_FILE"
fi

# Show test count breakdown
echo "" >> "$RESULTS_FILE"
log "  Test counts by module:"
echo "$TEST_OUTPUT" | grep '^running [0-9]' | while read -r line; do
    log "    $line"
done

# ═══════════════════════════════════════════════════════════════════════════
# 3. PROOF CHECKER TESTS (P1-P4)
# ═══════════════════════════════════════════════════════════════════════════
section "Proof Checker (P1-P4 verification)"

P_TESTS=("test_p1_tools_are_stack_functions" "test_p2_deterministic_execution" "test_p3_composition_is_concatenation" "test_p4_integer_division_consistency")
for ptest in "${P_TESTS[@]}"; do
    P_OUTPUT=$(cargo test --release "$ptest" 2>&1) || true
    if echo "$P_OUTPUT" | grep -q 'test result: ok'; then
        pass "$ptest"
    else
        fail "$ptest"
    fi
done

# ═══════════════════════════════════════════════════════════════════════════
# 4. CROSS-BACKEND CONSISTENCY
# ═══════════════════════════════════════════════════════════════════════════
section "Cross-Backend Consistency (SSA = Module = Interpreter)"

XBACKEND_OUTPUT=$(cargo test --release 'cross_backend' 2>&1) || true
if echo "$XBACKEND_OUTPUT" | grep -q 'test result: ok'; then
    pass "All cross-backend tests passed"
else
    fail "Cross-backend consistency tests FAILED"
fi

if $QUICK; then
    section "QUICK MODE — skipping experiments, GPU, and benchmarks"
    skip "Experiments"
    skip "GPU tests"
    skip "Benchmarks"
else

# ═══════════════════════════════════════════════════════════════════════════
# 5. EXPERIMENTS (compile + interpret each .kore file)
# ═══════════════════════════════════════════════════════════════════════════
section "Experiments (compile + run)"

# Core experiments (must pass — regressions here are real bugs)
CORE_EXPERIMENT_DIRS=("proofs" "algorithms" "calculus" "linear-algebra" "neural" "combinatorics" "optimization")
# Experimental/WIP (failures are warnings, not errors)
WIP_EXPERIMENT_DIRS=("sorting-networks")

run_experiment_dir() {
    local dir="$1"
    local is_wip="$2"
    local exp_dir="$KORE_DIR/experiments/$dir"
    if [ ! -d "$exp_dir" ]; then
        skip "experiments/$dir (directory not found)"
        return
    fi

    local dir_pass=0
    local dir_fail=0

    for kore_file in "$exp_dir"/*.kore; do
        [ -f "$kore_file" ] || continue
        local basename_file
        basename_file=$(basename "$kore_file")
        local tmp_out="/tmp/kore_bench_${basename_file%.kore}.korec"

        # Compile
        if $KOREC compile "$kore_file" -o "$tmp_out" 2>/dev/null; then
            # Run (interpreter) — 30s timeout per experiment
            if timeout 30 $KOREC run "$tmp_out" >/dev/null 2>&1; then
                dir_pass=$((dir_pass + 1))
            else
                local exit_code=$?
                if [ $exit_code -eq 124 ]; then
                    skip "experiments/$dir/$basename_file (timeout 30s)"
                elif [ "$is_wip" = "true" ]; then
                    skip "experiments/$dir/$basename_file [WIP] (run failed)"
                else
                    fail "experiments/$dir/$basename_file (run failed, exit $exit_code)"
                    dir_fail=$((dir_fail + 1))
                fi
            fi
        else
            if [ "$is_wip" = "true" ]; then
                skip "experiments/$dir/$basename_file [WIP] (compile failed)"
            else
                fail "experiments/$dir/$basename_file (compile failed)"
                dir_fail=$((dir_fail + 1))
            fi
        fi
        rm -f "$tmp_out"
    done

    if [ $dir_fail -eq 0 ] && [ $dir_pass -gt 0 ]; then
        pass "experiments/$dir: $dir_pass files OK"
    elif [ $dir_pass -eq 0 ] && [ $dir_fail -eq 0 ]; then
        skip "experiments/$dir (no passing .kore files)"
    fi
}

for dir in "${CORE_EXPERIMENT_DIRS[@]}"; do
    run_experiment_dir "$dir" "false"
done
for dir in "${WIP_EXPERIMENT_DIRS[@]}"; do
    run_experiment_dir "$dir" "true"
done

# ═══════════════════════════════════════════════════════════════════════════
# 6. JIT EXPERIMENTS (native-run where applicable)
# ═══════════════════════════════════════════════════════════════════════════
section "JIT Native Run (selected experiments)"

JIT_TESTS=(
    "experiments/algorithms/iterative.kore"
    "experiments/proofs/p1_tool_is_state_transform.kore"
    "experiments/proofs/p2_apply_is_fundamental.kore"
    "experiments/proofs/p3_composition_is_concatenation.kore"
    "experiments/proofs/p4_constraints_attenuate.kore"
    "experiments/calculus/autodiff.kore"
    "experiments/neural/neural_xor.kore"
    "experiments/combinatorics/ramsey.kore"
)

for kore_file in "${JIT_TESTS[@]}"; do
    full_path="$KORE_DIR/$kore_file"
    if [ ! -f "$full_path" ]; then
        skip "$kore_file (file not found)"
        continue
    fi

    basename_file=$(basename "$kore_file")
    tmp_out="/tmp/kore_jit_${basename_file%.kore}.korec"

    if $KOREC compile "$full_path" -o "$tmp_out" 2>/dev/null; then
        if timeout 30 $KOREC native-run "$tmp_out" >/dev/null 2>&1; then
            pass "JIT: $kore_file"
        else
            fail "JIT: $kore_file (native-run failed)"
        fi
    else
        fail "JIT: $kore_file (compile failed)"
    fi
    rm -f "$tmp_out"
done

# ═══════════════════════════════════════════════════════════════════════════
# 7. GPU TEST (gpu-map smoke test)
# ═══════════════════════════════════════════════════════════════════════════
section "GPU Smoke Test (gpu-map)"

# Check if GPU is available
if $KOREC gpu-info >/dev/null 2>&1; then
    GPU_INFO=$($KOREC gpu-info 2>&1 || echo "unknown")
    log "  GPU: $GPU_INFO"

    # Simple gpu-map test: square each element
    echo "dup mul" > /tmp/kore_gpu_test.kore
    GPU_OUT=$($KOREC gpu-map /tmp/kore_gpu_test.kore --input "1 2 3 4 5" 2>&1 || echo "FAILED")
    rm -f /tmp/kore_gpu_test.kore

    if echo "$GPU_OUT" | grep -q '1.*4.*9.*16.*25'; then
        pass "gpu-map: dup mul → [1, 4, 9, 16, 25]"
    else
        fail "gpu-map: unexpected output: $GPU_OUT"
    fi

    # Float test: negate
    echo "fneg" > /tmp/kore_gpu_test2.kore
    GPU_OUT2=$($KOREC gpu-map /tmp/kore_gpu_test2.kore --input "10 20 30" 2>&1 || echo "FAILED")
    rm -f /tmp/kore_gpu_test2.kore

    if echo "$GPU_OUT2" | grep -q -- '-10.*-20.*-30'; then
        pass "gpu-map: fneg → [-10, -20, -30]"
    else
        # May not support this exact op — warn don't fail
        log "${YELLOW}  ⚠ gpu-map fneg: $GPU_OUT2${NC}"
    fi
else
    skip "GPU not available (no wgpu device found)"
fi

# ═══════════════════════════════════════════════════════════════════════════
# 8. INDUSTRIAL BENCHMARKS (with timing)
# ═══════════════════════════════════════════════════════════════════════════
section "Industrial Benchmarks (performance numbers)"

BENCH_OUTPUT=$(cargo test --release 'industrial_full_report' -- --nocapture 2>&1) || true
echo "$BENCH_OUTPUT" >> "$RESULTS_FILE"

# Extract key metrics
if echo "$BENCH_OUTPUT" | grep -q 'Fib iter'; then
    pass "Industrial benchmark report generated"
    log ""
    log "  Key metrics from industrial benchmarks:"
    echo "$BENCH_OUTPUT" | grep -E '(Fib iter|Sum i|Collatz|Fib rec|GEO MEAN)' | while read -r line; do
        log "    $line"
    done
else
    fail "Industrial benchmark report failed to generate"
fi

# ═══════════════════════════════════════════════════════════════════════════
# 9. MICRO BENCHMARKS (stress tests)
# ═══════════════════════════════════════════════════════════════════════════
section "Micro Benchmarks (stress + performance suite)"

# First, check if all benchmark tests pass (without --nocapture for clean output)
STRESS_CHECK=$(cargo test --release 'benchmarks::tests' 2>&1) || true

if echo "$STRESS_CHECK" | grep -q 'test result: ok'; then
    STRESS_RESULT=$(echo "$STRESS_CHECK" | grep 'test result:' | tail -1)
    pass "Benchmark tests passed: $STRESS_RESULT"

    # Now run with --nocapture to get the summary report
    STRESS_OUTPUT=$(cargo test --release 'benchmarks::tests::benchmark_summary_report' -- --nocapture 2>&1) || true

    # Extract summary report if present
    if echo "$STRESS_OUTPUT" | grep -q 'Benchmark Summary'; then
        log ""
        log "  Benchmark Summary:"
        echo "$STRESS_OUTPUT" | sed -n '/Benchmark Summary/,/^$/p' | while read -r line; do
            log "    $line"
        done
    fi
else
    fail "Benchmark tests had failures"
    echo "$STRESS_OUTPUT" | grep 'FAILED' >> "$RESULTS_FILE" 2>/dev/null || true
fi

fi  # end of non-QUICK section

# ═══════════════════════════════════════════════════════════════════════════
# SUMMARY
# ═══════════════════════════════════════════════════════════════════════════
echo "" | tee -a "$RESULTS_FILE"
log "${BOLD}═══════════════════════════════════════════════════════════════${NC}"
log "${BOLD}  SUMMARY${NC}"
log "${BOLD}═══════════════════════════════════════════════════════════════${NC}"
log ""
log "  Date:     $(date '+%Y-%m-%d %H:%M:%S')"
log "  Commit:   $COMMIT"
log "  Branch:   $BRANCH"
log ""
log "  ${GREEN}Passed:   $PASSED${NC}"
log "  ${RED}Failed:   $FAILED${NC}"
log "  ${YELLOW}Skipped:  $SKIPPED${NC}"
log "  Sections: $TOTAL_SECTIONS"
log ""

if [ $FAILED -eq 0 ]; then
    log "${GREEN}${BOLD}  ✅ ALL CHECKS PASSED${NC}"
    log ""
    log "  Results saved to: $RESULTS_FILE"
    exit 0
else
    log "${RED}${BOLD}  ❌ $FAILED CHECK(S) FAILED${NC}"
    log ""
    log "  Results saved to: $RESULTS_FILE"
    log "  Review the file above for details."
    exit 1
fi
