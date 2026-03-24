# Standalone image for the spam-limit migration tool.
# Produces a minimal image with just the binary — no prover needed.
#
# Build:  docker build -t migrate-spam-limit -f migrate_spam_limit/Dockerfile .
# Run:    docker run --rm migrate-spam-limit --db postgres://... --new-limit 1000000 --dry-run

FROM rust:1.91-slim-bookworm AS builder

RUN apt update && apt install -y \
    pkg-config libssl-dev protobuf-compiler libclang-dev clang build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY proto ./proto
COPY prover ./prover
COPY prover_cli ./prover_cli
COPY prover_client ./prover_client
COPY prover_db_migration_sqlx ./prover_db_migration_sqlx
COPY rln_proof ./rln_proof
COPY smart_contract ./smart_contract
COPY migrate_spam_limit ./migrate_spam_limit

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release -p migrate_spam_limit && \
    cp /app/target/release/migrate_spam_limit /tmp/migrate_spam_limit

FROM debian:bookworm-slim
RUN apt update && apt install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /tmp/migrate_spam_limit /usr/local/bin/migrate_spam_limit
ENTRYPOINT ["migrate_spam_limit"]
