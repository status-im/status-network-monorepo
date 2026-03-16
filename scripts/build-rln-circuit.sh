#!/bin/bash
# Build RLN circuit with custom LIMIT_BIT_SIZE for ~1M transaction limit.
#
# This generates the circuit artifacts needed by the rln-prover (proof generation)
# and rln_bridge (proof verification):
#   - rln_final.arkzkey  (proving key + constraint matrices in arkzkey format)
#   - graph.bin           (witness calculation graph)
#   - verification_key.json (verification key for reference)
#
# Prerequisites:
#   - Rust toolchain (cargo)
#   - Node.js + npm
#   - ~5GB free disk space for intermediate artifacts
#
# Usage:
#   ./scripts/build-rln-circuit.sh [TREE_DEPTH] [LIMIT_BIT_SIZE]
#   ./scripts/build-rln-circuit.sh          # defaults: DEPTH=20, LIMIT_BIT_SIZE=20
#   ./scripts/build-rln-circuit.sh 20 20    # explicit: ~1M limit (2^20 = 1,048,576)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

TREE_DEPTH="${1:-20}"
LIMIT_BIT_SIZE="${2:-20}"
WORK_DIR=$(mktemp -d)
OUTPUT_DIR="$REPO_ROOT/rln-circuit-artifacts"

echo "============================================="
echo "  RLN Circuit Builder"
echo "============================================="
echo "  Tree depth (N):      $TREE_DEPTH  (max members: 2^$TREE_DEPTH = $((2**TREE_DEPTH)))"
echo "  Limit bit size (M):  $LIMIT_BIT_SIZE  (max message limit: 2^$LIMIT_BIT_SIZE = $((2**LIMIT_BIT_SIZE)))"
echo "  Work directory:      $WORK_DIR"
echo "  Output directory:    $OUTPUT_DIR"
echo "============================================="
echo ""

cleanup() {
    echo ""
    echo "Temporary files preserved at: $WORK_DIR"
    echo "You can delete them with: rm -rf $WORK_DIR"
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# Step 1: Install circom v2.1.0
# ---------------------------------------------------------------------------
echo "=== Step 1/5: Installing circom v2.1.0 ==="

if command -v circom &>/dev/null && [[ "$(circom --version 2>&1)" == *"2.1.0"* ]]; then
    echo "circom v2.1.0 already installed, skipping."
else
    cd "$WORK_DIR"
    git clone https://github.com/iden3/circom.git
    cd circom && git checkout v2.1.0
    cargo build --release
    export PATH="$WORK_DIR/circom/target/release:$PATH"
    echo "circom installed: $(circom --version)"
fi
echo ""

# ---------------------------------------------------------------------------
# Step 2: Clone circom-rln and set parameters
# ---------------------------------------------------------------------------
echo "=== Step 2/5: Setting up circom-rln with RLN($TREE_DEPTH, $LIMIT_BIT_SIZE) ==="

cd "$WORK_DIR"
git clone https://github.com/rate-limiting-nullifier/circom-rln
cd circom-rln

# Replace the last line that instantiates the circuit component
# Original: component main { public [x, externalNullifier] } = RLN(20, 16);
# New:      component main { public [x, externalNullifier] } = RLN(TREE_DEPTH, LIMIT_BIT_SIZE);
sed -i.bak "s/RLN(20, 16)/RLN($TREE_DEPTH, $LIMIT_BIT_SIZE)/" circuits/rln.circom

echo "Modified circuit (last line):"
tail -1 circuits/rln.circom

npm install
echo ""

# ---------------------------------------------------------------------------
# Step 3: Build circuits (zkey, verification key, wasm)
# ---------------------------------------------------------------------------
echo "=== Step 3/5: Compiling circuit and running Groth16 trusted setup ==="
echo "  (This downloads a ~500MB Powers of Tau file on first run)"

./scripts/build-circuits.sh rln

echo ""
echo "Circuit compiled. Artifacts in zkeyFiles/rln/:"
ls -la zkeyFiles/rln/
echo ""

# ---------------------------------------------------------------------------
# Step 4: Generate witness calculation graph
# ---------------------------------------------------------------------------
echo "=== Step 4/5: Generating witness calculation graph (graph.bin) ==="

cd "$WORK_DIR"
# Use the exact commit that rln-0.9.0's iden3calc is based on (produces .001 format)
# Newer versions produce .002 format which is incompatible with rln-0.9.0
git clone https://github.com/iden3/circom-witnesscalc
cd circom-witnesscalc
git checkout 5cb365b6e4d9052ecc69d4567fcf5bc061c20e94
cargo build --release

cargo run --release --bin build-circuit \
    "$WORK_DIR/circom-rln/circuits/rln.circom" \
    "$WORK_DIR/graph.bin"

echo "graph.bin generated: $(ls -la "$WORK_DIR/graph.bin")"
echo ""

# ---------------------------------------------------------------------------
# Step 5: Convert zkey to arkzkey format
# ---------------------------------------------------------------------------
echo "=== Step 5/5: Converting zkey to arkzkey format ==="

cd "$WORK_DIR"
git clone https://github.com/seemenkina/ark-zkey.git
cd ark-zkey
cargo build --release

# arkzkey-util creates .arkzkey in the same directory as the input file
cargo run --release --bin arkzkey-util "$WORK_DIR/circom-rln/zkeyFiles/rln/final.zkey"

ARKZKEY_FILE="$WORK_DIR/circom-rln/zkeyFiles/rln/final.arkzkey"
if [ ! -f "$ARKZKEY_FILE" ]; then
    echo "ERROR: arkzkey file not generated at expected path: $ARKZKEY_FILE"
    echo "Searching for .arkzkey files..."
    find "$WORK_DIR" -name "*.arkzkey" -ls
    exit 1
fi
echo "arkzkey generated: $(ls -la "$ARKZKEY_FILE")"
echo ""

# ---------------------------------------------------------------------------
# Collect artifacts
# ---------------------------------------------------------------------------
echo "=== Collecting artifacts ==="
mkdir -p "$OUTPUT_DIR"

cp "$ARKZKEY_FILE" "$OUTPUT_DIR/rln_final.arkzkey"
cp "$WORK_DIR/graph.bin" "$OUTPUT_DIR/graph.bin"
cp "$WORK_DIR/circom-rln/zkeyFiles/rln/verification_key.json" "$OUTPUT_DIR/verification_key.json"
cp "$WORK_DIR/circom-rln/zkeyFiles/rln/final.zkey" "$OUTPUT_DIR/rln_final.zkey"

echo ""
echo "============================================="
echo "  Circuit artifacts generated successfully!"
echo "============================================="
echo ""
ls -lh "$OUTPUT_DIR/"
echo ""
echo "Next steps — install the artifacts:"
echo ""
echo "  # 1. Copy circuit resources for rln-prover (proof generation)"
echo "  cp $OUTPUT_DIR/rln_final.arkzkey rln-prover/rln_proof/resources/rln_final.arkzkey"
echo "  cp $OUTPUT_DIR/graph.bin         rln-prover/rln_proof/resources/graph.bin"
echo ""
echo "  # 2. Copy circuit resources for rln_bridge (proof verification at sequencer)"
echo "  mkdir -p besu-plugins/linea-sequencer/sequencer/src/main/rust/rln_bridge/resources"
echo "  cp $OUTPUT_DIR/rln_final.arkzkey besu-plugins/linea-sequencer/sequencer/src/main/rust/rln_bridge/resources/rln_final.arkzkey"
echo ""
echo "  # 3. Copy circuit resources for slasher (proof generation in tests)"
echo "  cp $OUTPUT_DIR/rln_final.arkzkey rln-aggregator/slasher/resources/rln_final.arkzkey"
echo "  cp $OUTPUT_DIR/graph.bin         rln-aggregator/slasher/resources/graph.bin"
echo ""
echo "  # 4. Keep verification_key.json for reference / external verifiers"
echo "  cp $OUTPUT_DIR/verification_key.json docker/rln_verifying_key.json"
echo ""
echo "Parameters: RLN($TREE_DEPTH, $LIMIT_BIT_SIZE)"
echo "Max user message limit: $((2**LIMIT_BIT_SIZE)) (was 65536 with LIMIT_BIT_SIZE=16)"
