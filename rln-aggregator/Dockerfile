# syntax=docker/dockerfile:1.4
# Stage 1: Build Prover
FROM rust:1.93-slim-bookworm AS builder

RUN apt update && apt install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    libclang-dev \
    clang \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Working directory
WORKDIR /app/rln-prover

COPY --from=prover_proto . ./proto

WORKDIR /app/rln-aggregator

# Copy all source files
COPY Cargo.toml Cargo.lock ./
COPY proto ./proto
COPY aggregator ./aggregator
COPY slasher ./slasher

# Build only aggregator (and its dependencies) with full release optimizations
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release -p aggregator && \
    # Copy binary outside cache mount since cache mounts don't persist in image layers
    cp /app/rln-aggregator/target/release/aggregator /tmp/aggregator

# Stage 2: Run Prover
FROM ubuntu:25.10

ARG RUST_LOG_LEVEL=info

RUN groupadd -r user && useradd -r -g user user

WORKDIR /app

# Copy the entrypoint script and make it executable
# COPY docker-entrypoint.sh /usr/local/bin/
# RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Copy from the builder stage
COPY --from=builder /tmp/aggregator ./aggregator
# COPY mock ./mock

RUN chown -R user:user /app
RUN chmod +x ./aggregator
# RUN chown user:user /usr/local/bin/docker-entrypoint.sh

USER user

# Expose default port
EXPOSE 50052

ENV RUST_LOG=${RUST_LOG_LEVEL}

# Run the prover - shell script will build arguments with parsed env var
# ENTRYPOINT ["docker-entrypoint.sh"]
ENTRYPOINT ["./aggregator"]

