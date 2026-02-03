#!/bin/bash
# Kore World - Control Script
#
# Architecture:
#   HOST (you)         CONTAINER (agent)
#   ./mnt/inbox.txt ──▶ /mnt/inbox.txt    (your commands)
#   ./mnt/trace.jsonl ◀── /mnt/trace.jsonl (agent logs)
#
# Usage:
#   ./kore-world.sh build         # Build the Docker image
#   ./kore-world.sh shell         # Interactive shell (for debugging)
#   ./kore-world.sh agent         # Start autonomous agent
#   ./kore-world.sh watch         # Watch agent execution
#   ./kore-world.sh send "msg"    # Send message to agent
#   ./kore-world.sh stop          # Stop all containers

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Load .env if exists
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
fi

# Ensure directories exist
mkdir -p mnt world

case "${1:-help}" in
    build)
        echo "🔨 Building Kore World image..."
        docker compose build
        echo "✅ Build complete"
        ;;
    
    shell)
        echo "🌍 Starting Kore World shell..."
        docker compose run --rm kore
        ;;
    
    agent)
        echo "🤖 Starting Kore Agent..."
        echo ""
        echo "   Goal:  ${KORE_GOAL:-Explore Kore and build something useful}"
        echo "   Watch: ./kore-world.sh watch"
        echo "   Send:  ./kore-world.sh send \"your message\""
        echo ""
        echo "─────────────────────────────────────────"
        docker compose up kore-agent
        ;;
    
    agent-gpu)
        echo "🤖 Starting Kore Agent (GPU)..."
        echo ""
        echo "   Goal:  ${KORE_GOAL:-Build a Kore world with GPU capabilities}"
        echo "   Watch: ./kore-world.sh watch"
        echo "   Send:  ./kore-world.sh send \"your message\""
        echo ""
        echo "─────────────────────────────────────────"
        docker compose up kore-agent-gpu
        ;;
    
    watch)
        echo "👁️  Watching agent trace..."
        echo "   (Ctrl+C to stop watching)"
        echo "─────────────────────────────────────────"
        if command -v jq &> /dev/null; then
            tail -f mnt/trace.jsonl 2>/dev/null | jq -r '"[\(.kind)] \(.message)"' 2>/dev/null || tail -f mnt/trace.jsonl
        else
            tail -f mnt/trace.jsonl
        fi
        ;;
    
    send)
        if [ -z "$2" ]; then
            echo "Usage: $0 send \"your message\""
            exit 1
        fi
        echo "$2" >> mnt/inbox.txt
        echo "📬 Sent to inbox: $2"
        ;;
    
    inbox)
        echo "📬 Current inbox:"
        cat mnt/inbox.txt 2>/dev/null || echo "(empty)"
        ;;
    
    clear-inbox)
        > mnt/inbox.txt
        echo "📭 Inbox cleared"
        ;;
    
    stop)
        echo "🛑 Stopping all Kore containers..."
        docker compose down
        ;;
    
    clean)
        echo "🧹 Cleaning up..."
        docker compose down -v
        rm -f mnt/trace.jsonl mnt/inbox.txt
        echo "✅ Cleaned"
        ;;
    
    status)
        echo "📊 Kore World Status"
        echo "─────────────────────────────────────────"
        echo "Containers:"
        docker compose ps 2>/dev/null || echo "  (none running)"
        echo ""
        echo "Inbox:"
        if [ -s mnt/inbox.txt ]; then
            cat mnt/inbox.txt | head -5
        else
            echo "  (empty)"
        fi
        echo ""
        echo "Recent trace:"
        if [ -f mnt/trace.jsonl ]; then
            tail -3 mnt/trace.jsonl | jq -r '"[\(.kind)] \(.message)"' 2>/dev/null || tail -3 mnt/trace.jsonl
        else
            echo "  (no trace yet)"
        fi
        ;;
    
    help|*)
        echo "╔════════════════════════════════════════════════════════════╗"
        echo "║              Kore World - AI Development Sandbox           ║"
        echo "╚════════════════════════════════════════════════════════════╝"
        echo ""
        echo "Usage: $0 <command>"
        echo ""
        echo "Commands:"
        echo "  build       Build the Docker image"
        echo "  shell       Start interactive shell (for debugging)"
        echo "  agent       Start autonomous agent (CPU)"
        echo "  agent-gpu   Start autonomous agent (GPU)"
        echo "  watch       Watch agent execution in real-time"
        echo "  send \"msg\"  Send message to agent inbox"
        echo "  inbox       View current inbox contents"
        echo "  clear-inbox Clear the inbox"
        echo "  stop        Stop all containers"
        echo "  clean       Stop and remove all data"
        echo "  status      Show status of containers and recent activity"
        echo ""
        echo "Quick Start:"
        echo "  1. Copy .env.example to .env and add your API key"
        echo "  2. ./kore-world.sh build"
        echo "  3. ./kore-world.sh agent      # start the agent"
        echo "  4. ./kore-world.sh watch      # in another terminal"
        echo "  5. ./kore-world.sh send \"focus on building a calculator\""
        echo ""
        ;;
esac
