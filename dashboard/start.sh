#!/bin/bash
# Start the Kore Benchmark Dashboard

cd "$(dirname "$0")"

# Check for venv
if [ ! -d "venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv venv
    source venv/bin/activate
    pip install -r requirements.txt
else
    source venv/bin/activate
fi

echo "🚀 Starting Kore Benchmark Dashboard..."
echo "   Open http://localhost:8080 in your browser"
echo ""

python app.py
