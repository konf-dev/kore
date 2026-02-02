# Build stage
FROM rust:1.75-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

RUN cargo build --release -p kore-agent

# Runtime stage
FROM debian:bookworm-slim

# Install CA certificates for HTTPS
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /build/target/release/kore-agent /usr/local/bin/kore-agent

# Create non-root user
RUN useradd -m -s /bin/bash kore
USER kore

# Working directory is the mounted workspace
WORKDIR /mnt/workspace

# All config via environment variables
# Required: KORE_PROMPT, KORE_GOAL, OPENAI_API_KEY
# Optional: KORE_WORKSPACE=/mnt/workspace, KORE_LOGS=/mnt/logs

ENTRYPOINT ["kore-agent"]
