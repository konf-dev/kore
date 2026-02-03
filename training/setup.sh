#!/bin/bash
# Kore RL Training Setup Script
# One-command setup for running experiments on a new server

set -e

echo "=================================="
echo "Kore RL Training Setup"
echo "=================================="

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check prerequisites
echo -e "\n${YELLOW}Checking prerequisites...${NC}"

check_command() {
    if command -v $1 &> /dev/null; then
        echo -e "  ${GREEN}✓${NC} $1 found"
        return 0
    else
        echo -e "  ${RED}✗${NC} $1 not found"
        return 1
    fi
}

MISSING=0
check_command cargo || MISSING=1
check_command python3 || MISSING=1
check_command pip || MISSING=1

if [ $MISSING -eq 1 ]; then
    echo -e "\n${RED}Missing prerequisites. Please install:${NC}"
    echo "  - Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo "  - Python 3.10+: apt install python3 python3-pip"
    exit 1
fi

# Check Python version
PYTHON_VERSION=$(python3 -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')
echo -e "  Python version: $PYTHON_VERSION"

# Check for CUDA
if command -v nvidia-smi &> /dev/null; then
    echo -e "  ${GREEN}✓${NC} NVIDIA GPU detected"
    nvidia-smi --query-gpu=name,memory.total --format=csv,noheader 2>/dev/null || true
else
    echo -e "  ${YELLOW}⚠${NC} No NVIDIA GPU detected - will use CPU (slow)"
fi

# Build Kore
echo -e "\n${YELLOW}Building Kore runtime...${NC}"
cd "$(dirname "$0")/.."

if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}Error: Not in kore directory. Run from kore/training/${NC}"
    exit 1
fi

cargo build --release --bin kore-train 2>&1 | tail -5

if [ -f "target/release/kore-train" ]; then
    echo -e "${GREEN}✓${NC} Kore binary built successfully"
else
    echo -e "${RED}✗${NC} Kore build failed"
    exit 1
fi

# Test Kore
echo -e "\n${YELLOW}Testing Kore runtime...${NC}"
RESULT=$(./target/release/kore-train eval "1 2 add" 2>/dev/null || echo "error")
if [[ "$RESULT" == *"3"* ]]; then
    echo -e "${GREEN}✓${NC} Kore runtime working: 1 2 add = 3"
else
    echo -e "${RED}✗${NC} Kore runtime test failed"
    echo "  Got: $RESULT"
    exit 1
fi

# Setup Python environment
echo -e "\n${YELLOW}Setting up Python environment...${NC}"
cd training

# Create venv if it doesn't exist
if [ ! -d "venv" ]; then
    python3 -m venv venv
    echo -e "${GREEN}✓${NC} Created virtual environment"
fi

# Activate venv
source venv/bin/activate

# Install requirements
echo "Installing Python packages..."
pip install --upgrade pip -q
pip install -r requirements.txt -q 2>&1 | tail -3

echo -e "${GREEN}✓${NC} Python packages installed"

# Verify installations
echo -e "\n${YELLOW}Verifying installations...${NC}"
python3 -c "import torch; print(f'  PyTorch: {torch.__version__}')"
python3 -c "import transformers; print(f'  Transformers: {transformers.__version__}')"
python3 -c "import torch; print(f'  CUDA available: {torch.cuda.is_available()}')"

if python3 -c "import torch; exit(0 if torch.cuda.is_available() else 1)" 2>/dev/null; then
    python3 -c "import torch; print(f'  GPU: {torch.cuda.get_device_name(0)}')"
    python3 -c "import torch; print(f'  VRAM: {torch.cuda.get_device_properties(0).total_memory / 1e9:.1f} GB')"
fi

# Test modules
echo -e "\n${YELLOW}Testing training modules...${NC}"

python3 -c "
from kore_runtime import KoreRuntime
runtime = KoreRuntime()
result = runtime.execute('dup mul', [5])
print(f'  kore_runtime: 5 dup mul = {result.stack}')
"

python3 -c "
from task_generators import generate_task
task = generate_task(1)
print(f'  task_generators: Generated task for phase 1')
"

echo -e "${GREEN}✓${NC} All modules working"

# Print next steps
echo -e "\n=================================="
echo -e "${GREEN}Setup Complete!${NC}"
echo "=================================="
echo ""
echo "To run experiments:"
echo ""
echo "  1. Activate the environment:"
echo "     source training/venv/bin/activate"
echo ""
echo "  2. Run supervised training:"
echo "     python run_supervised.py --dry-run  # Test first"
echo "     python run_supervised.py --phases 1,2 --quantization 4bit"
echo ""
echo "  3. Run unsupervised exploration:"
echo "     python run_unsupervised.py --dry-run  # Test first"
echo "     python run_unsupervised.py --episodes 1000 --quantization 4bit"
echo ""
echo "Configuration options:"
echo "  --model \"Qwen/Qwen2.5-Coder-3B-Instruct\"  # Smaller model"
echo "  --quantization 4bit                         # Reduce VRAM usage"
echo "  --config config.yaml                        # Custom config"
echo ""
echo "Happy training! 🚀"
