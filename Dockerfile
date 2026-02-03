# Kore World - Isolated AI Development Environment
# Supports: CPU, NVIDIA GPU, AMD GPU (ROCm)
#
# Build: docker build -t kore-world .
# Run CPU: docker run -it --rm -v $(pwd)/world:/world kore-world
# Run GPU: docker run -it --rm --gpus all -v $(pwd)/world:/world kore-world

# =============================================================================
# Stage 1: Build Kore
# =============================================================================
FROM rust:1.83-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates
COPY tests ./tests
COPY stdlib ./stdlib

# Build release binaries
RUN cargo build --release -p kore-agent

# =============================================================================
# Stage 2: Runtime Environment
# =============================================================================
FROM debian:bookworm-slim

# Core system utilities
RUN apt-get update && apt-get install -y \
    # Essentials
    ca-certificates curl wget git \
    # Build tools (for installing packages)
    build-essential pkg-config \
    # Python 3.11
    python3 python3-pip python3-venv python3-dev \
    # Network tools
    net-tools iputils-ping dnsutils netcat-openbsd \
    # Text processing
    jq vim nano less \
    # Process management
    procps htop \
    # Archive tools
    zip unzip tar \
    # SSL/TLS
    openssl libssl-dev \
    && ln -sf /usr/bin/python3 /usr/bin/python \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js 20 LTS
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Install uv (fast Python package manager)
RUN curl -LsSf https://astral.sh/uv/install.sh | sh \
    && ln -sf /root/.cargo/bin/uv /usr/local/bin/uv

# Copy Kore binaries
COPY --from=builder /build/target/release/kore-agent /usr/local/bin/kore-agent

# Copy Kore documentation and libraries
COPY stdlib /opt/kore/stdlib
COPY lib /opt/kore/lib
COPY docs/REFERENCE.md /opt/kore/docs/REFERENCE.md
COPY PREAMBLE.md /opt/kore/docs/PREAMBLE.md
COPY genesis-prompt.md /opt/kore/genesis-prompt.md
COPY examples /opt/kore/examples

# Environment
ENV KORE_HOME=/opt/kore
ENV KORE_PROMPT=/opt/kore/genesis-prompt.md
ENV KORE_CAPS=all
ENV PATH="/opt/kore/bin:${PATH}"

# Working directory - mount your world here
WORKDIR /world

# Healthcheck
HEALTHCHECK --interval=30s --timeout=3s \
    CMD pgrep kore-agent || exit 1

# Default: interactive shell (override with kore-agent for autonomous mode)
CMD ["/bin/bash"]
