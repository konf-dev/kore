#!/bin/bash
# Stop continuous discovery

cd "$(dirname "$0")"

if [ ! -f mnt/discovery.pid ]; then
    echo "No discovery process running (mnt/discovery.pid not found)"
    exit 1
fi

PID=$(cat mnt/discovery.pid)

echo "Stopping discovery process (PID: $PID)..."
kill $PID 2>/dev/null

if [ $? -eq 0 ]; then
    echo "Process stopped"
    rm mnt/discovery.pid
    echo ""
    echo "Final stats:"
    echo "  Total discoveries: $(wc -l < mnt/trace.log 2>/dev/null || echo 0)"
    echo "  Knowledge file: $(wc -l < mnt/knowledge.kore 2>/dev/null || echo 0) lines"
else
    echo "Process not running or already stopped"
fi
