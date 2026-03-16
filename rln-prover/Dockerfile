# syntax=docker/dockerfile:1.4
# Stage 1: Build Prover
FROM rust:1.91-slim-bookworm AS builder

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
COPY prover_db_migration_sqlx ./prover_db_migration_sqlx
COPY rln_proof ./rln_proof
COPY smart_contract ./smart_contract
COPY migrate_spam_limit ./migrate_spam_limit

# Build prover_cli and migration tool with full release optimizations
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release -p prover_cli -p migrate_spam_limit && \
    # Copy binaries outside cache mount since cache mounts don't persist in image layers
    cp /app/target/release/prover_cli /tmp/prover_cli && \
    cp /app/target/release/migrate_spam_limit /tmp/migrate_spam_limit

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
COPY --from=builder /tmp/migrate_spam_limit ./migrate_spam_limit
COPY mock ./mock

RUN chown -R user:user /app
RUN chown user:user /usr/local/bin/docker-entrypoint.sh

USER user

# Expose default port
EXPOSE 50051

ENV RUST_LOG=${RUST_LOG_LEVEL}

# Run the prover - shell script will build arguments with parsed env var
ENTRYPOINT ["docker-entrypoint.sh"]
