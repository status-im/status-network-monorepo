#!/bin/bash
# Build librln_bridge.so for ARM64 Linux

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== Building RLN bridge for ARM64 Linux ==="

# Create a Dockerfile for building
cat > Dockerfile.arm64 << 'EOF'
FROM rust:1.82-bookworm

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    clang \
    llvm \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy source
COPY Cargo.toml Cargo.lock* ./
COPY src ./src

# Build
RUN cargo build --release

# Output location
RUN ls -la target/release/librln_bridge.so
EOF

# Build using Docker
echo "Building Docker image for ARM64..."
docker build --platform linux/arm64 -t rln-bridge-arm64-builder -f Dockerfile.arm64 .

# Extract the library
echo "Extracting librln_bridge.so..."
mkdir -p target/aarch64-unknown-linux-gnu/release
docker create --name rln-extract rln-bridge-arm64-builder
docker cp rln-extract:/build/target/release/librln_bridge.so target/aarch64-unknown-linux-gnu/release/
docker rm rln-extract

echo "=== Build complete ==="
file target/aarch64-unknown-linux-gnu/release/librln_bridge.so
ls -la target/aarch64-unknown-linux-gnu/release/librln_bridge.so

