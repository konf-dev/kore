FROM rust:1.83-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/kore /usr/local/bin/kore

# Create mount point for persistent knowledge
RUN mkdir -p /mnt /app/mnt

# Copy experiment files
COPY experiments/kore-synth/continual-discovery/discovery.kore /app/discovery.kore
COPY experiments/kore-synth/continual-discovery/continuous.kore /app/continuous.kore

WORKDIR /app

# Default: run discovery (override in docker-compose for continuous mode)
CMD ["kore", "discovery.kore"]
