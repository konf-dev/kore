#!/bin/bash
# Continual Discovery Runner
# Runs discovery in a loop, each iteration builds on previous knowledge

set -e

cd "$(dirname "$0")"

echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║  KORE CONTINUAL DISCOVERY                                        ║"
echo "║  Each run discovers more math, building on previous knowledge    ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""

# Check if running local or docker
if [ "$1" = "local" ]; then
    echo "Running locally with symlink to mnt..."
    sudo mkdir -p /mnt 2>/dev/null || true
    sudo ln -sf "$(pwd)/mnt/knowledge.kore" /mnt/knowledge.kore 2>/dev/null || true
    sudo ln -sf "$(pwd)/mnt/registry.kore" /mnt/registry.kore 2>/dev/null || true
    
    # Run directly
    cd ../../..
    ./target/release/kore experiments/kore-synth/continual-discovery/discovery.kore
else
    # Build first
    echo "Building Docker image..."
    docker compose build kore-discovery

    # Run discovery
    echo ""
    echo "Running discovery..."
    docker compose run --rm kore-discovery
fi

# Show what was saved
echo ""
echo "═══════════════════════════════════════════════════════════════════"
echo "PERSISTED KNOWLEDGE (mnt/knowledge.kore):"
echo "═══════════════════════════════════════════════════════════════════"
cat mnt/knowledge.kore 2>/dev/null || echo "(no knowledge yet)"

echo ""
echo "Run again to see it load the knowledge and continue building!"
echo "  ./run.sh        # Docker"
echo "  ./run.sh local  # Local (requires sudo for /mnt symlink)"
echo ""
echo "Or start interactive shell:"
echo "  docker compose run --rm kore-shell"
