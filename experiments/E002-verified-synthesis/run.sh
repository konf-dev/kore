#!/bin/bash
# Experiment Runner Template
# Copy this to your experiment directory

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KORE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
EXPERIMENT_ID="E002"
EXPERIMENT_NAME="verified-synthesis"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Timestamp for this run
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_DIR="$SCRIPT_DIR/results/$TIMESTAMP"

main() {
    log_info "Starting experiment: $EXPERIMENT_ID - $EXPERIMENT_NAME"
    log_info "Timestamp: $TIMESTAMP"
    
    # Create results directory
    mkdir -p "$RESULTS_DIR"
    
    # Log system info
    log_info "Recording system info..."
    {
        echo "=== System Info ==="
        echo "Date: $(date)"
        echo "Host: $(hostname)"
        echo "OS: $(uname -a)"
        echo "Rust: $(rustc --version)"
        echo "Kore: $(cd $KORE_ROOT && cargo pkgid 2>/dev/null || echo 'dev')"
        echo ""
    } > "$RESULTS_DIR/system_info.txt"
    
    # Build kore if needed
    log_info "Building Kore..."
    (cd "$KORE_ROOT" && cargo build --release 2>&1) | tee "$RESULTS_DIR/build.log"
    
    # Run the experiment
    log_info "Running experiment..."
    run_experiment 2>&1 | tee "$RESULTS_DIR/output.log"
    
    # Collect results
    log_info "Collecting results..."
    collect_results
    
    log_success "Experiment complete! Results in: $RESULTS_DIR"
}

run_experiment() {
    # Override this function in your experiment
    log_warn "No experiment implementation - override run_experiment()"
}

collect_results() {
    # Override this function to collect specific results
    log_info "No custom result collection defined"
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
