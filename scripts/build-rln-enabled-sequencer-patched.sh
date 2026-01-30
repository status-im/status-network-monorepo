#!/bin/bash
set -e

echo "🚀 Building RLN-Enabled Sequencer with Gasless Block Fix (ARM64)"
echo "   Using exact Linea besu source from Consensys/linea-besu"
echo "   Building for ARM64 architecture (Apple Silicon Macs)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Build paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
LINEA_SEQUENCER_DIR="${REPO_ROOT}/besu-plugins/linea-sequencer"
STATUS_RLN_PROVER_DIR="${REPO_ROOT}/rln-prover"
CUSTOM_BESU_DIR="${REPO_ROOT}/custom-besu-patched"

# Linea besu source info - EXACT commit used in the official image
# From: consensys/linea-besu-package:beta-v4.4-rc7-20251215105212-1b78d76
# The JAR Manifest Commit-Hash is the actual source commit
LINEA_BESU_REPO="https://github.com/Consensys/linea-besu.git"
LINEA_BESU_COMMIT="2bccdf9928006e6e639c31391a775c2dd8d20548"
LINEA_BESU_DIR="${REPO_ROOT}/.linea-besu-source"

echo -e "${BLUE}📁 Working directories:${NC}"
echo -e "  Script: ${SCRIPT_DIR}"
echo -e "  Repo Root: ${REPO_ROOT}"
echo -e "  Linea Besu Source: ${LINEA_BESU_DIR}"
echo -e "  Sequencer: ${LINEA_SEQUENCER_DIR}"

# Use the exact same image version as the official Linea setup
BESU_PACKAGE_TAG="beta-v4.4-rc7-20251215105212-1b78d76"
BESU_BASE_IMAGE="consensys/linea-besu-package:${BESU_PACKAGE_TAG}"

# Build options
BUILD_PROVER=${BUILD_PROVER:-false}
RESTART_SERVICES=${RESTART_SERVICES:-false}
SKIP_SOURCE_CLONE=${SKIP_SOURCE_CLONE:-false}

# Publish options
PUSH_IMAGES=${PUSH_IMAGES:-false}
REGISTRY=${REGISTRY:-}
NAMESPACE=${NAMESPACE:-}
BESU_IMAGE_NAME=${BESU_IMAGE_NAME:-linea-besu-minimal-rln}
RLN_PROVER_IMAGE_NAME=${RLN_PROVER_IMAGE_NAME:-status-rln-prover}
IMAGE_TAG_SUFFIX=${IMAGE_TAG_SUFFIX:--gasless-fix}

print_usage() {
    cat << USAGE
Usage: $(basename "$0") [options]

This script builds the RLN-enabled sequencer WITH the gasless block fix.
It clones the EXACT Linea besu source, applies the patch, and builds.

Key info:
  Linea Besu Repo: ${LINEA_BESU_REPO}
  Commit: ${LINEA_BESU_COMMIT}
  Base Image: ${BESU_BASE_IMAGE}

Options:
  --all                        Build everything (Besu + RLN Prover) - default: Besu only
  --with-prover                Same as --all
  --restart                    Restart services after build
  --skip-clone                 Skip cloning if source already exists at correct commit
  --push                       Push images to a registry after build
  --registry <host>            Registry host (e.g. ghcr.io, docker.io)
  --namespace <ns>             Namespace/org (e.g. status-im)
  --besu-name <name>           Besu image repository name (default: ${BESU_IMAGE_NAME})
  --prover-name <name>         RLN prover image repository name (default: ${RLN_PROVER_IMAGE_NAME})
  --tag-suffix <suffix>        Optional tag suffix (default: ${IMAGE_TAG_SUFFIX})
  -h, --help                   Show this help

Environment vars:
  BUILD_PROVER, PUSH_IMAGES, REGISTRY, NAMESPACE, BESU_IMAGE_NAME, RLN_PROVER_IMAGE_NAME, IMAGE_TAG_SUFFIX
USAGE
}

# Simple args parser
while [[ $# -gt 0 ]]; do
    case "$1" in
        --all|--with-prover)
            BUILD_PROVER=true; shift ;;
        --restart)
            RESTART_SERVICES=true; shift ;;
        --skip-clone)
            SKIP_SOURCE_CLONE=true; shift ;;
        --push)
            PUSH_IMAGES=true; shift ;;
        --registry)
            REGISTRY="$2"; shift 2 ;;
        --namespace)
            NAMESPACE="$2"; shift 2 ;;
        --besu-name)
            BESU_IMAGE_NAME="$2"; shift 2 ;;
        --prover-name)
            RLN_PROVER_IMAGE_NAME="$2"; shift 2 ;;
        --tag-suffix)
            IMAGE_TAG_SUFFIX="$2"; shift 2 ;;
        -h|--help)
            print_usage; exit 0 ;;
        *)
            echo -e "${YELLOW}Unknown option: $1${NC}"; print_usage; exit 1 ;;
    esac
done

if [[ "$PUSH_IMAGES" == "true" ]]; then
    if [[ -z "$REGISTRY" || -z "$NAMESPACE" ]]; then
        echo -e "${RED}❌ When using --push, both --registry and --namespace are required.${NC}"
        exit 1
    fi
fi

# Step 1: Clone or verify Linea besu source
echo -e "${BLUE}🔧 Step 1: Getting exact Linea besu source...${NC}"

NEED_CLONE=true
if [[ -d "$LINEA_BESU_DIR/.git" ]]; then
    cd "$LINEA_BESU_DIR"
    CURRENT_COMMIT=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
    if [[ "$CURRENT_COMMIT" == "$LINEA_BESU_COMMIT" ]]; then
        echo -e "${GREEN}✅ Source already at correct commit: ${LINEA_BESU_COMMIT:0:8}${NC}"
        if [[ "$SKIP_SOURCE_CLONE" == "true" ]]; then
            NEED_CLONE=false
        else
            echo -e "${YELLOW}  Re-cloning to ensure clean state (use --skip-clone to skip)${NC}"
        fi
    else
        echo -e "${YELLOW}  Current commit: $CURRENT_COMMIT${NC}"
        echo -e "${YELLOW}  Expected commit: $LINEA_BESU_COMMIT${NC}"
        echo -e "${YELLOW}  Will re-clone...${NC}"
    fi
    cd "$REPO_ROOT"
fi

if [[ "$NEED_CLONE" == "true" ]]; then
    echo -e "${YELLOW}📥 Cloning Consensys/linea-besu at commit ${LINEA_BESU_COMMIT:0:8}...${NC}"
    rm -rf "$LINEA_BESU_DIR"
    
    # Shallow clone to save time/space, then fetch the specific commit
    git clone --depth 1 "$LINEA_BESU_REPO" "$LINEA_BESU_DIR"
    cd "$LINEA_BESU_DIR"
    
    # Fetch the specific commit we need
    git fetch --depth 1 origin "$LINEA_BESU_COMMIT"
    git checkout "$LINEA_BESU_COMMIT"
    
    echo -e "${GREEN}✅ Cloned and checked out commit: $(git rev-parse --short HEAD)${NC}"
    cd "$REPO_ROOT"
fi

# Step 2: Apply the gasless fix patch
echo -e "${BLUE}🩹 Step 2: Applying gasless block fix to PostMergeContext.java...${NC}"

POSTMERGE_FILE="${LINEA_BESU_DIR}/consensus/merge/src/main/java/org/hyperledger/besu/consensus/merge/PostMergeContext.java"

if [[ ! -f "$POSTMERGE_FILE" ]]; then
    echo -e "${RED}❌ Error: PostMergeContext.java not found at:${NC}"
    echo -e "   $POSTMERGE_FILE"
    exit 1
fi

# Check if already patched (look for our new logging signature)
if grep -q "SKIPPING CORRUPTED block" "$POSTMERGE_FILE"; then
    echo -e "${GREEN}✅ Gasless fix with corruption filter already applied${NC}"
else
    echo -e "${YELLOW}  Applying patch using Python (most reliable method)...${NC}"
    
    # Always use Python for reliability - sed is too fragile for complex patches
    # Fetch the original file
    curl -s "https://raw.githubusercontent.com/Consensys/linea-besu/2bccdf9928006e6e639c31391a775c2dd8d20548/consensus/merge/src/main/java/org/hyperledger/besu/consensus/merge/PostMergeContext.java" > "${POSTMERGE_FILE}.orig"
    
    # Apply the fix using Python for reliability
    python3 << PYFIX
import re

with open("${POSTMERGE_FILE}.orig", 'r') as f:
    content = f.read()

# Fix 1: putPayloadById - add transaction count comparison
old_put = '''      maybeCurrBestPayload.ifPresent(
          currBestPayload -> {
            if (newBlockValue.greaterThan(currBestPayload.blockValue())) {
              LOG.atInfo()
                  .setMessage(
                      "New proposal for payloadId {} {} is better than the previous one by {}")
                  .addArgument(newPayload.payloadIdentifier())
                  .addArgument(
                      () -> logBlockProposal(newBlockWithReceipts.getBlock(), newBlockValue))
                  .addArgument(
                      () ->
                          newBlockValue
                              .subtract(currBestPayload.blockValue())
                              .toHumanReadableString())
                  .log();

              blocksInProgress.removeAll(
                  streamPayloadsById(newPayload.payloadIdentifier()).toList());

              logCurrentBestBlock(newPayload);
            }
          });'''

new_put = '''      maybeCurrBestPayload.ifPresent(
          currBestPayload -> {
            final int newTxCount =
                newBlockWithReceipts.getBlock().getBody().getTransactions().size();
            final int currTxCount =
                currBestPayload.blockWithReceipts().getBlock().getBody().getTransactions().size();

            // Fix for gasless transactions: consider a block "better" if it has higher value
            // OR if values are equal but it has more transactions.
            // This ensures gasless blocks (value=0 with txs) replace empty blocks (value=0 no txs).
            final boolean isBetterByValue = newBlockValue.greaterThan(currBestPayload.blockValue());
            final boolean isBetterByTxCount =
                newBlockValue.equals(currBestPayload.blockValue()) && newTxCount > currTxCount;

            if (isBetterByValue || isBetterByTxCount) {
              LOG.atInfo()
                  .setMessage(
                      "New proposal for payloadId {} {} is better than the previous one by {}")
                  .addArgument(newPayload.payloadIdentifier())
                  .addArgument(
                      () -> logBlockProposal(newBlockWithReceipts.getBlock(), newBlockValue))
                  .addArgument(
                      () ->
                          isBetterByValue
                              ? newBlockValue
                                  .subtract(currBestPayload.blockValue())
                                  .toHumanReadableString()
                              : (newTxCount - currTxCount) + " more transactions")
                  .log();

              blocksInProgress.removeAll(
                  streamPayloadsById(newPayload.payloadIdentifier()).toList());

              logCurrentBestBlock(newPayload);
            }
          });'''

content = content.replace(old_put, new_put)

# Fix 2: retrievePayloadById - filter corrupted blocks, prefer by tx count, with comprehensive logging
# NOTE: The original code has @Override before the method, so we must include it in the pattern
old_retrieve = '''  @Override
  public Optional<PayloadWrapper> retrievePayloadById(final PayloadIdentifier payloadId) {
    synchronized (blocksInProgress) {
      return streamPayloadsById(payloadId).max(Comparator.comparing(PayloadWrapper::blockValue));
    }
  }'''

new_retrieve = '''  // Empty transactions root hash (keccak256 of empty RLP list)
  private static final Hash EMPTY_TRIE_ROOT = Hash.fromHexString(
      "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421");

  @Override
  public Optional<PayloadWrapper> retrievePayloadById(final PayloadIdentifier payloadId) {
    synchronized (blocksInProgress) {
      // Get all payloads for this ID
      final var allPayloads = streamPayloadsById(payloadId).toList();
      
      // Log all available payloads for debugging
      LOG.info("retrievePayloadById({}): {} payloads available", payloadId, allPayloads.size());
      for (int i = 0; i < allPayloads.size(); i++) {
        PayloadWrapper pw = allPayloads.get(i);
        Block b = pw.blockWithReceipts().getBlock();
        LOG.info("  [{}] hash={} txCount={} gasUsed={} value={} txRoot={}",
            i,
            b.getHeader().getHash().toHexString().substring(0, 18) + "...",
            b.getBody().getTransactions().size(),
            b.getHeader().getGasUsed(),
            pw.blockValue().toHumanReadableString(),
            b.getHeader().getTransactionsRoot().toHexString().substring(0, 18) + "...");
      }
      
      // Filter out corrupted blocks and log which ones are skipped
      final var validPayloads = allPayloads.stream()
          .filter(pw -> {
            Block b = pw.blockWithReceipts().getBlock();
            int txCount = b.getBody().getTransactions().size();
            boolean hasEmptyRoot = b.getHeader().getTransactionsRoot().equals(EMPTY_TRIE_ROOT);
            
            if (txCount > 0 && hasEmptyRoot) {
              LOG.warn("SKIPPING CORRUPTED block {}: has {} txs but transactionsRoot is EMPTY! hash={}",
                  b.getHeader().getNumber(),
                  txCount,
                  b.getHeader().getHash().toHexString().substring(0, 18) + "...");
              return false;
            }
            return true;
          })
          .toList();
      
      LOG.info("After filtering: {} valid payloads (filtered out {} corrupted)",
          validPayloads.size(), allPayloads.size() - validPayloads.size());
      
      // Select best payload by value, then transaction count
      final Optional<PayloadWrapper> result = validPayloads.stream()
          .max(Comparator.comparing(PayloadWrapper::blockValue)
              .thenComparing(pw -> pw.blockWithReceipts().getBlock().getBody().getTransactions().size()));
      
      // Log the selected payload
      if (result.isPresent()) {
        Block selected = result.get().blockWithReceipts().getBlock();
        LOG.info("SELECTED: hash={} txCount={} gasUsed={} value={}",
            selected.getHeader().getHash().toHexString().substring(0, 18) + "...",
            selected.getBody().getTransactions().size(),
            selected.getHeader().getGasUsed(),
            result.get().blockValue().toHumanReadableString());
      } else {
        LOG.warn("No valid payload found for {}", payloadId);
      }
      
      return result;
    }
  }'''

content = content.replace(old_retrieve, new_retrieve)

with open("${POSTMERGE_FILE}", 'w') as f:
    f.write(content)

print("Patched successfully with corruption filtering and comprehensive logging")
PYFIX
    
    rm -f "${POSTMERGE_FILE}.orig"
    
    # Verify patch was applied - look for our corruption filter signature
    if grep -q "SKIPPING CORRUPTED block" "$POSTMERGE_FILE"; then
        echo -e "${GREEN}✅ Gasless fix with corruption filtering applied successfully${NC}"
    else
        echo -e "${RED}❌ Failed to apply fix. Please apply manually.${NC}"
        exit 1
    fi
fi

# Step 3: Build the patched consensus-merge JAR
echo -e "${BLUE}⚙️ Step 3: Building patched consensus-merge module...${NC}"

cd "$LINEA_BESU_DIR"

# Build only the consensus-merge module
./gradlew :consensus:merge:jar -x test -x spotlessCheck --no-daemon

PATCHED_MERGE_JAR=$(find "${LINEA_BESU_DIR}/consensus/merge/build/libs" -name "besu-consensus-merge-*.jar" -not -name "*-sources*" -not -name "*-javadoc*" 2>/dev/null | head -1)

if [[ ! -f "$PATCHED_MERGE_JAR" ]]; then
    echo -e "${RED}❌ Error: Patched consensus-merge JAR not found!${NC}"
    echo -e "${YELLOW}Try building manually:${NC}"
    echo -e "  cd ${LINEA_BESU_DIR} && ./gradlew :consensus:merge:jar -x test -x spotlessCheck"
    exit 1
fi

echo -e "${GREEN}✅ Patched JAR built: $(basename "$PATCHED_MERGE_JAR")${NC}"
echo -e "   Size: $(ls -lh "$PATCHED_MERGE_JAR" | awk '{print $5}')"

cd "$REPO_ROOT"

# Step 4: Build RLN Bridge library for ARM64 Linux
echo -e "${BLUE}🦀 Step 4: Building RLN Bridge Rust Library for ARM64 Linux...${NC}"
cd "${LINEA_SEQUENCER_DIR}/sequencer/src/main/rust/rln_bridge"

RLN_LIB_FILE="${LINEA_SEQUENCER_DIR}/sequencer/src/main/rust/rln_bridge/target/aarch64-unknown-linux-gnu/release/librln_bridge.so"

if [[ ! -f "$RLN_LIB_FILE" ]]; then
    echo -e "${YELLOW}🐳 Building native ARM64 library using Docker...${NC}"

    # Create temporary Dockerfile for ARM64 build
    cat > Dockerfile.arm64-build << 'DOCKEREOF'
FROM rust:1.85-bookworm

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

# Build for ARM64
RUN cargo build --release

# Verify output
RUN ls -la target/release/librln_bridge.so
DOCKEREOF

    # Build using native ARM64 Docker container (no cross-compilation)
    docker build --platform linux/arm64 -t rln-bridge-arm64-builder -f Dockerfile.arm64-build .

    # Extract the library
    echo -e "${YELLOW}📦 Extracting librln_bridge.so...${NC}"
    mkdir -p target/aarch64-unknown-linux-gnu/release
    docker create --name rln-extract rln-bridge-arm64-builder
    docker cp rln-extract:/build/target/release/librln_bridge.so target/aarch64-unknown-linux-gnu/release/
    docker rm rln-extract

    # Cleanup
    rm -f Dockerfile.arm64-build

    # Verify the library
    echo -e "${YELLOW}🔍 Verifying library architecture...${NC}"
    file "$RLN_LIB_FILE"
fi

if [[ ! -f "$RLN_LIB_FILE" ]]; then
    echo -e "${RED}❌ Error: ARM64 RLN library not found: $RLN_LIB_FILE${NC}"
    exit 1
fi
echo -e "${GREEN}✅ RLN Bridge library ready (ARM64)${NC}"

# Step 5: Build Custom Sequencer JAR
echo -e "${BLUE}☕ Step 5: Building Custom Sequencer JAR...${NC}"
cd "$REPO_ROOT"

./gradlew :besu-plugins:linea-sequencer:sequencer:artifacts -x test -x checkSpdxHeader -x spotlessJavaCheck -x spotlessGroovyGradleCheck --no-daemon

SEQUENCER_JAR=$(ls -t "${LINEA_SEQUENCER_DIR}/sequencer/build/libs"/linea-sequencer-*.jar 2>/dev/null | head -1)
SEQUENCER_DIST=$(ls -t "${LINEA_SEQUENCER_DIR}/sequencer/build/distributions"/linea-sequencer-*.zip 2>/dev/null | head -1)

if [[ ! -f "$SEQUENCER_JAR" ]]; then
    echo -e "${RED}❌ Error: Sequencer JAR not found!${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Custom Sequencer JAR built: $(basename "$SEQUENCER_JAR")${NC}"

# Step 6: Build RLN Prover if requested
if [[ "$BUILD_PROVER" == "true" ]]; then
    echo -e "${BLUE}🦀 Step 6: Building RLN Prover Service...${NC}"
    cd "$STATUS_RLN_PROVER_DIR"
    cargo build --release
    
    PROVER_BINARY="${STATUS_RLN_PROVER_DIR}/target/release/prover_cli"
    if [[ ! -f "$PROVER_BINARY" ]]; then
        echo -e "${RED}❌ Error: RLN Prover binary not found!${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ RLN Prover service built${NC}"
else
    echo -e "${YELLOW}⏭️  Skipping RLN Prover build (use --all to include)${NC}"
fi

# Step 7: Build the patched Docker image
echo -e "${BLUE}🐳 Step 7: Building Patched Besu Docker Image...${NC}"
mkdir -p "$CUSTOM_BESU_DIR"
cd "$CUSTOM_BESU_DIR"

# Extract the entire Besu distribution from official Linea image
echo -e "${YELLOW}📥 Extracting base Besu from official Linea image...${NC}"
docker rm temp-besu-extract 2>/dev/null || true
docker create --name temp-besu-extract "${BESU_BASE_IMAGE}"
docker cp temp-besu-extract:/opt/besu/ ./besu/
docker rm temp-besu-extract

echo -e "${YELLOW}🔍 Original plugins from Linea image:${NC}"
ls -1 ./besu/plugins/ | head -10

echo -e "${YELLOW}🔍 Original lib JARs (merge related):${NC}"
ls -1 ./besu/lib/ | grep -i merge || echo "  (none)"

# Step 8: Apply the gasless fix - replace the consensus-merge JAR
echo -e "${BLUE}🩹 Step 8: Replacing consensus-merge JAR with patched version...${NC}"

# Find and replace the original consensus-merge JAR
ORIGINAL_MERGE_JAR=$(ls ./besu/lib/besu-consensus-merge-*.jar 2>/dev/null | head -1)
if [[ -f "$ORIGINAL_MERGE_JAR" ]]; then
    echo -e "${YELLOW}  Removing original: $(basename "$ORIGINAL_MERGE_JAR")${NC}"
    rm -f "$ORIGINAL_MERGE_JAR"
fi

# Copy our patched JAR with the SAME name as the original (important for classpath)
PATCHED_JAR_NAME=$(basename "$ORIGINAL_MERGE_JAR" 2>/dev/null || echo "besu-consensus-merge-25.11.0-linea4.2.jar")
cp "$PATCHED_MERGE_JAR" "./besu/lib/$PATCHED_JAR_NAME"
echo -e "${GREEN}  ✅ Installed patched: $PATCHED_JAR_NAME${NC}"

# Verify the patched JAR
echo -e "${YELLOW}  Verifying patched JAR...${NC}"
if unzip -l "./besu/lib/$PATCHED_JAR_NAME" | grep -q "PostMergeContext.class"; then
    echo -e "${GREEN}  ✅ Patched JAR contains PostMergeContext.class${NC}"
else
    echo -e "${RED}  ❌ Warning: PostMergeContext.class not found in patched JAR${NC}"
fi

# Step 9: Replace sequencer plugin (keeping all other Linea plugins)
echo -e "${YELLOW}🔄 Replacing sequencer plugin (keeping other Linea plugins)...${NC}"
rm -f ./besu/plugins/linea-sequencer*.jar
cp "$SEQUENCER_JAR" ./besu/plugins/
echo -e "${GREEN}  ✅ Installed: $(basename "$SEQUENCER_JAR")${NC}"

# Install plugin dependencies
if [[ -f "$SEQUENCER_DIST" ]]; then
    echo -e "${YELLOW}📦 Installing plugin dependencies...${NC}"
    mkdir -p ./besu/plugins/lib
    unzip -q "$SEQUENCER_DIST" -d extracted-deps/
    
    EXCLUDE_PATTERN="(tuweni|besu-|linea-sequencer-)"
    for jar in extracted-deps/*/*.jar; do
        jarname=$(basename "$jar")
        if [[ ! "$jarname" =~ $EXCLUDE_PATTERN ]]; then
            cp "$jar" ./besu/plugins/lib/
        fi
    done
    rm -rf extracted-deps/
    echo -e "${GREEN}  ✅ Plugin dependencies installed${NC}"
fi

# Install RLN native library
echo -e "${YELLOW}📚 Installing RLN native library...${NC}"
mkdir -p ./besu/lib/native
cp "$RLN_LIB_FILE" ./besu/lib/native/
echo -e "${GREEN}  ✅ Installed: librln_bridge.so${NC}"

# Update classpath in startup scripts
echo -e "${YELLOW}⚙️ Updating Besu startup scripts...${NC}"
for script in besu besu.bat besu-untuned besu-untuned.bat; do
    if [[ -f "./besu/bin/$script" ]]; then
        cp "./besu/bin/$script" "./besu/bin/$script.backup"
        if [[ "$script" == *.bat ]]; then
            sed -i.tmp 's|CLASSPATH=%APP_HOME%\\lib\\*|CLASSPATH=%APP_HOME%\\plugins\\lib\\*;%APP_HOME%\\plugins\\*;%APP_HOME%\\lib\\*|g' "./besu/bin/$script"
        else
            sed -i.tmp 's|CLASSPATH=/opt/besu/lib/\*:/opt/besu/plugins/\*|CLASSPATH=/opt/besu/plugins/lib/*:/opt/besu/plugins/*:/opt/besu/lib/*|g' "./besu/bin/$script"
        fi
        rm -f "./besu/bin/$script.tmp"
    fi
done
echo -e "${GREEN}  ✅ Startup scripts updated${NC}"

# Final verification
echo -e "${BLUE}📋 Final inventory:${NC}"
echo -e "  ${YELLOW}Plugins:${NC}"
ls -1 ./besu/plugins/ | grep -E "\.(jar|JAR)$" | while read -r f; do echo "    📦 $f"; done
echo -e "  ${YELLOW}Merge JAR (patched):${NC}"
ls -1 ./besu/lib/ | grep -i merge | while read -r f; do echo "    🩹 $f"; done

# Create Dockerfile
cat > Dockerfile << 'EOF'
FROM ubuntu:24.04

RUN apt-get update && \
    apt-get install -y openjdk-21-jre-headless libjemalloc-dev && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* && \
    (groupadd -g 1000 besu || true) && \
    useradd -u 1000 -g 1000 -m -s /bin/bash besu || \
    (userdel -r besu 2>/dev/null || true && groupdel besu 2>/dev/null || true && \
     groupadd -g 1001 besu && useradd -u 1001 -g besu -m -s /bin/bash besu)

USER besu
WORKDIR /opt/besu

# Copy entire Besu distribution with patched consensus-merge and custom sequencer
COPY --chown=besu:besu besu/ /opt/besu/

# Set library paths for RLN
ENV LD_LIBRARY_PATH="/opt/besu/lib/native:/usr/local/lib:/usr/lib"
ENV JAVA_LIBRARY_PATH="/opt/besu/lib/native"
ENV PATH="/opt/besu/bin:${PATH}"

EXPOSE 8545 8546 8547 8550 8551 30303

ENTRYPOINT ["besu"]
HEALTHCHECK --start-period=5s --interval=5s --timeout=1s --retries=10 CMD bash -c "[ -f /tmp/pid ]"
EOF

# Build Docker image
TIMESTAMP=$(date +%Y%m%d%H%M%S)
TAG_WITH_TIME="${TIMESTAMP}${IMAGE_TAG_SUFFIX}"
BESU_IMAGE_TAG="${BESU_IMAGE_NAME}:${TAG_WITH_TIME}"

echo -e "${YELLOW}🔨 Building Docker image...${NC}"
docker build --platform linux/arm64 -t "$BESU_IMAGE_TAG" .
echo -e "${GREEN}✅ Patched Besu image built: $BESU_IMAGE_TAG${NC}"

# Build RLN Prover image if requested
RLN_PROVER_TAG=""
if [[ "$BUILD_PROVER" == "true" ]]; then
    echo -e "${BLUE}🐳 Building RLN Prover Docker image...${NC}"
    cd "$STATUS_RLN_PROVER_DIR"
    RLN_PROVER_TAG="${RLN_PROVER_IMAGE_NAME}:${TAG_WITH_TIME}"
    docker build --platform linux/arm64 -t "$RLN_PROVER_TAG" .
    echo -e "${GREEN}✅ RLN Prover image built: $RLN_PROVER_TAG${NC}"
fi

# Handle image naming for push
BESU_IMAGE_REMOTE="$BESU_IMAGE_TAG"
RLN_PROVER_IMAGE_REMOTE="$RLN_PROVER_TAG"

if [[ -n "$REGISTRY" || -n "$NAMESPACE" ]]; then
    BESU_IMAGE_REMOTE="${REGISTRY:+${REGISTRY}/}${NAMESPACE:+${NAMESPACE}/}${BESU_IMAGE_NAME}:${TAG_WITH_TIME}"
    RLN_PROVER_IMAGE_REMOTE="${REGISTRY:+${REGISTRY}/}${NAMESPACE:+${NAMESPACE}/}${RLN_PROVER_IMAGE_NAME}:${TAG_WITH_TIME}"
fi

# Push images if requested
if [[ "$PUSH_IMAGES" == "true" ]]; then
    echo -e "${BLUE}📤 Pushing images to registry...${NC}"
    docker tag "$BESU_IMAGE_TAG" "$BESU_IMAGE_REMOTE"
    docker push "$BESU_IMAGE_REMOTE"
    
    if [[ -n "$RLN_PROVER_TAG" ]]; then
        docker tag "$RLN_PROVER_TAG" "$RLN_PROVER_IMAGE_REMOTE"
        docker push "$RLN_PROVER_IMAGE_REMOTE"
    fi
    echo -e "${GREEN}✅ Images pushed successfully${NC}"
fi

# Update Docker Compose
echo -e "${BLUE}📝 Updating Docker Compose...${NC}"
COMPOSE_FILE="${REPO_ROOT}/docker/compose-spec-l2-services-rln.yml"
if [[ -f "$COMPOSE_FILE" ]]; then
    cp "$COMPOSE_FILE" "${COMPOSE_FILE}.backup.$(date +%Y%m%d%H%M%S)"
    
    if [[ -n "$RLN_PROVER_TAG" ]]; then
        awk -v besu_img="$BESU_IMAGE_REMOTE" -v rln_img="$RLN_PROVER_IMAGE_REMOTE" '
            /^[[:space:]]*container_name:[[:space:]]*sequencer$/ { tgt = "besu" }
            /^[[:space:]]*container_name:[[:space:]]*l2-node-besu$/ { tgt = "besu" }
            /^[[:space:]]*container_name:[[:space:]]*rln-prover$/ { tgt = "rln" }
            /^[[:space:]]*container_name:[[:space:]]*karma-service$/ { tgt = "rln" }
            {
              if (tgt != "" && $0 ~ /^[[:space:]]*image:[[:space:]]*/) {
                match($0, /^[[:space:]]*/); lead = substr($0, 1, RLENGTH);
                if (tgt == "besu") {
                  print lead "image: " besu_img;
                } else {
                  print lead "image: " rln_img;
                }
                tgt = "";
                next;
              }
              print $0;
            }
        ' "$COMPOSE_FILE" > "${COMPOSE_FILE}.tmp" && mv "${COMPOSE_FILE}.tmp" "$COMPOSE_FILE"
    else
        awk -v besu_img="$BESU_IMAGE_REMOTE" '
            /^[[:space:]]*container_name:[[:space:]]*sequencer$/ { tgt = "besu" }
            /^[[:space:]]*container_name:[[:space:]]*l2-node-besu$/ { tgt = "besu" }
            {
              if (tgt == "besu" && $0 ~ /^[[:space:]]*image:[[:space:]]*/) {
                match($0, /^[[:space:]]*/); lead = substr($0, 1, RLENGTH);
                print lead "image: " besu_img;
                tgt = "";
                next;
              }
              print $0;
            }
        ' "$COMPOSE_FILE" > "${COMPOSE_FILE}.tmp" && mv "${COMPOSE_FILE}.tmp" "$COMPOSE_FILE"
    fi
    echo -e "${GREEN}✅ Updated Docker Compose with patched images${NC}"
fi

# Cleanup
cd "$REPO_ROOT"
rm -rf "$CUSTOM_BESU_DIR"

echo ""
echo -e "${GREEN}🎉 Build Complete with Gasless Block Fix!${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}📋 What was done:${NC}"
echo -e "  1. Cloned exact Linea besu source: ${LINEA_BESU_COMMIT:0:8}"
echo -e "  2. Applied gasless fix to PostMergeContext.java"
echo -e "  3. Built patched consensus-merge JAR"
echo -e "  4. Replaced JAR in official Linea image"
echo ""
echo -e "${BLUE}📋 The Fix:${NC}"
echo -e "  • putPayloadById: Now prefers blocks with more txs when values equal"
echo -e "  • retrievePayloadById: Filters corrupted blocks, prefers blocks with more txs"
echo -e "  • Gasless blocks (value=0 with txs) now beat empty blocks (value=0)"
echo ""
echo -e "${BLUE}📋 Built Components (ARM64):${NC}"
echo -e "  Patched Merge JAR: $(basename "$PATCHED_MERGE_JAR")"
echo -e "  Custom Sequencer JAR: $(basename "$SEQUENCER_JAR")"
echo -e "  RLN Library: librln_bridge.so (aarch64)"
echo -e "  Besu Image: $BESU_IMAGE_REMOTE (arm64)"
if [[ -n "$RLN_PROVER_TAG" ]]; then
    echo -e "  RLN Prover Image: $RLN_PROVER_IMAGE_REMOTE"
fi
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"

# Restart services if requested
if [[ "$RESTART_SERVICES" == "true" ]]; then
    echo -e "${BLUE}🔄 Restarting services...${NC}"
    cd "$REPO_ROOT"
    docker compose -f "$COMPOSE_FILE" up -d --force-recreate sequencer l2-node-besu
    if [[ -n "$RLN_PROVER_TAG" ]]; then
        docker compose -f "$COMPOSE_FILE" up -d --force-recreate rln-prover karma-service
    fi
    echo -e "${GREEN}✅ Services restarted${NC}"
fi

echo ""
echo -e "${YELLOW}🚀 Next Steps:${NC}"
if [[ "$RESTART_SERVICES" != "true" ]]; then
    echo -e "  1. Restart services:"
    echo -e "     ${GREEN}cd $REPO_ROOT && docker compose -f docker/compose-spec-l2-services-rln.yml up -d --force-recreate${NC}"
fi
echo -e "  2. Test gasless transactions - they should now be mined standalone!"
echo -e "  3. Check logs: ${GREEN}docker logs sequencer${NC}"
echo ""
