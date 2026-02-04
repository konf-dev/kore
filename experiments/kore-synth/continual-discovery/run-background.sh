#!/bin/bash
# Run continuous discovery in background

cd "$(dirname "$0")"
mkdir -p mnt

KORE_BIN="../../../target/release/kore"

echo "Starting continuous discovery in background..."
nohup $KORE_BIN continuous.kore > mnt/output.log 2>&1 &
PID=$!

echo "Process ID: $PID"
echo $PID > mnt/discovery.pid

echo "Monitoring:"
echo "  Output: mnt/output.log"
echo "  Traces: mnt/trace.log"
echo "  Knowledge: mnt/knowledge.kore"
echo ""
echo "To stop: kill \$(cat mnt/discovery.pid)"
echo "To monitor: tail -f mnt/trace.log"
