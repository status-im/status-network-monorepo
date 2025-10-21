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

COPY Cargo.toml Cargo.lock ./
COPY proto ./proto
COPY prover ./prover
COPY prover_cli ./prover_cli
COPY prover_client ./prover_client
COPY rln_proof ./rln_proof
COPY smart_contract ./smart_contract
RUN cargo build --release

# Stage 2: Run Prover
FROM ubuntu:25.10

ARG RUST_LOG_LEVEL=info

RUN groupadd -r user && useradd -r -g user user

WORKDIR /app

# Copy the entrypoint script and make it executable
COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Copy from the builder stage
COPY --from=builder /app/target/release/prover_cli ./prover_cli
COPY mock ./mock

RUN chown -R user:user /app
RUN chown user:user /usr/local/bin/docker-entrypoint.sh

USER user

# Exppose default port
EXPOSE 50051

ENV RUST_LOG=${RUST_LOG_LEVEL}

# Run the prover - shell script will build arguments with parsed env var
ENTRYPOINT ["docker-entrypoint.sh"]
