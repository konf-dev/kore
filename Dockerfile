# Build stage
FROM rust:1.83-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

RUN cargo build --release -p kore-agent

# Runtime stage - Full Linux sandbox
FROM debian:bookworm

# Install comprehensive toolset - agent can install more with apt
RUN apt-get update && apt-get install -y \
    # Core utilities
    ca-certificates curl wget git \
    # Build essentials
    build-essential pkg-config \
    # Python
    python3 python3-pip python3-venv \
    # Networking tools
    net-tools iputils-ping dnsutils \
    # Text processing
    jq vim nano \
    # Process management
    procps htop \
    # Archive tools
    zip unzip tar \
    && ln -s /usr/bin/python3 /usr/bin/python \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js 20 LTS (via NodeSource for modern version)
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Copy the kore-agent binary
COPY --from=builder /build/target/release/kore-agent /usr/local/bin/kore-agent

# Agent runs as ROOT inside sandbox - can install anything
# The sandbox itself is isolated from host

WORKDIR /workspace

# All config via environment variables
# Required: KORE_PROMPT, KORE_GOAL, OPENAI_API_KEY
# Optional: KORE_WORKSPACE=/workspace, KORE_LOGS=/logs

ENTRYPOINT ["kore-agent"]
