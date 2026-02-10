#!/bin/bash
# Kore End-to-End Verification
# Ensures all backends produce identical results

echo "=== KORE VERIFICATION SUITE ==="
echo ""

cd "$(dirname "$0")/.."

# Build
cargo build --release 2>/dev/null
KOREC="./target/release/korec"

PASSED=0
FAILED=0

run_test() {
    local prog="$1"
    local expected="$2"
    
    echo "Test: $prog = $expected"
    
    # Create temp file
    echo "$prog" > /tmp/test.kore
    
    # Compile
    $KOREC compile /tmp/test.kore -o /tmp/test.korec >/dev/null 2>&1
    
    # Interpreter
    interp_result=$($KOREC run /tmp/test.korec 2>/dev/null | grep -oP 'Int\(\K[0-9-]+' || echo "ERROR")
    
    # WASM
    $KOREC wasm /tmp/test.korec -o /tmp/test.wasm >/dev/null 2>&1
    wasm_result=$(node -e "
        const fs = require('fs');
        WebAssembly.instantiate(fs.readFileSync('/tmp/test.wasm'))
            .then(({instance}) => console.log(Number(instance.exports.main())));
    " 2>/dev/null || echo "ERROR")
    
    # GPU (if available)
    if $KOREC gpu-info 2>/dev/null | grep -q "Available"; then
        $KOREC spirv /tmp/test.korec -o /tmp/test.spv >/dev/null 2>&1
        gpu_result=$($KOREC gpu-run /tmp/test.spv 2>/dev/null | grep -oP 'Result\[0\] = \K[0-9-]+' || echo "ERROR")
    else
        gpu_result="SKIP"
    fi
    
    # Verify
    if [ "$interp_result" = "$expected" ] && [ "$wasm_result" = "$expected" ] && { [ "$gpu_result" = "$expected" ] || [ "$gpu_result" = "SKIP" ]; }; then
        echo "  ✓ Interpreter: $interp_result, WASM: $wasm_result, GPU: $gpu_result"
        PASSED=$((PASSED + 1))
    else
        echo "  ✗ FAILED! Interpreter: $interp_result, WASM: $wasm_result, GPU: $gpu_result (expected $expected)"
        FAILED=$((FAILED + 1))
    fi
}

# Run tests
run_test "5 3 + 2 - 4 *" "24"     # ((5+3)-2)*4
run_test "2 3 *" "6"               # 2*3
run_test "10 3 /" "3"              # 10/3 (integer division)
run_test "5 dup *" "25"            # 5*5
run_test "1 2 swap -" "1"          # 2-1
run_test "3 4 + 2 *" "14"          # (3+4)*2

echo ""
echo "=== RESULTS ==="
echo "Passed: $PASSED"
echo "Failed: $FAILED"

# Cleanup
rm -f /tmp/test.kore /tmp/test.korec /tmp/test.wasm /tmp/test.spv

exit $FAILED
