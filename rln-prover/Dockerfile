# syntax=docker/dockerfile:1.4
# Stage 1: Build Prover
FROM rust:1.90-slim-bookworm AS builder

RUN apt update && apt install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    libclang-dev \
    clang \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Working directory
WORKDIR /app

# Copy all source files
COPY Cargo.toml Cargo.lock ./
COPY proto ./proto
COPY prover ./prover
COPY prover_cli ./prover_cli
COPY prover_client ./prover_client
COPY prover_db_migration ./prover_db_migration
COPY prover_db_entity ./prover_db_entity
COPY prover_pmtree ./prover_pmtree
COPY prover_pmtree_db_impl ./prover_pmtree_db_impl
COPY rln_proof ./rln_proof
COPY smart_contract ./smart_contract

# Build with cargo cache mount for faster rebuilds
# Uses release-dev profile (thin LTO) - much faster than full release
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --profile release-dev && \
    cp /app/target/release-dev/prover_cli /tmp/prover_cli

# Stage 2: Run Prover
FROM ubuntu:25.10

ARG RUST_LOG_LEVEL=info

RUN groupadd -r user && useradd -r -g user user

WORKDIR /app

# Copy the entrypoint script and make it executable
COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Copy from the builder stage
COPY --from=builder /tmp/prover_cli ./prover_cli
COPY mock ./mock

RUN chown -R user:user /app
RUN chown user:user /usr/local/bin/docker-entrypoint.sh

USER user

# Expose default port
EXPOSE 50051

ENV RUST_LOG=${RUST_LOG_LEVEL}

# Run the prover - shell script will build arguments with parsed env var
ENTRYPOINT ["docker-entrypoint.sh"]
