#!/bin/bash
# Monitor continuous discovery

cd "$(dirname "$0")"

if [ ! -f mnt/discovery.pid ]; then
    echo "No discovery process running (mnt/discovery.pid not found)"
    exit 1
fi

PID=$(cat mnt/discovery.pid)

if ! kill -0 $PID 2>/dev/null; then
    echo "Process $PID is not running"
    exit 1
fi

echo "Discovery process running (PID: $PID)"
echo ""
echo "=== Recent discoveries ==="
tail -20 mnt/trace.log 2>/dev/null || echo "(no traces yet)"
echo ""
echo "=== Knowledge base ==="
wc -l mnt/knowledge.kore 2>/dev/null || echo "(not saved yet)"
echo ""
echo "Following trace log (Ctrl+C to stop)..."
tail -f mnt/trace.log
