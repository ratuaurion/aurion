# ==============================================================================
# AURION SOVEREIGN BLOCKCHAIN — PRODUCTION & DEVNET DOCKERFILE
# ==============================================================================
# Multi-stage deterministic containerization conforming to AUR-ARCH-001.
# - Stage 1: Reproducible compilation with pinned toolchain (Rust 1.98.1)
# - Stage 2: Minimal runtime image (Debian Bookworm Slim) with non-root security
# ==============================================================================

# ------------------------------------------------------------------------------
# Stage 1: Build Environment
# ------------------------------------------------------------------------------
FROM rust:1.98.1-bookworm AS builder

WORKDIR /usr/src/aurion

# Pre-cache dependencies
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo check --release || true && \
    rm -rf src

# Copy full source tree
COPY . .

# Compile deterministic optimized release binary
ENV RUSTFLAGS="-C target-cpu=generic -C relocation-model=pic"
RUN cargo build --release --bin aurion && \
    strip target/release/aurion

# ------------------------------------------------------------------------------
# Stage 2: Minimal Sovereign Runtime
# ------------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# System security updates & minimal runtime libraries
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

# Create unprivileged system user & group
RUN groupadd -g 1000 aurion && \
    useradd -u 1000 -g aurion -m -s /bin/bash aurion

# Set up data directories with proper permissions
RUN mkdir -p /data /etc/aurion && \
    chown -R aurion:aurion /data /etc/aurion

# Copy sovereign executable from builder
COPY --from=builder --chown=aurion:aurion /usr/src/aurion/target/release/aurion /bin/aurion

# Set working directory & user
USER aurion
WORKDIR /home/aurion

# Expose standard ports:
# 8545: JSON-RPC 2.0 & WebSocket Gateway
# 9000: P2P Wire Protocol (AUR0)
# 9100: Prometheus Metrics
EXPOSE 8545 9000 9100

# Persistent volume for redb ACID database
VOLUME ["/data"]

# Default entrypoint
ENTRYPOINT ["/bin/aurion"]
CMD ["node", "--data-dir", "/data/aurion.redb", "--rpc-bind", "0.0.0.0:8545"]
