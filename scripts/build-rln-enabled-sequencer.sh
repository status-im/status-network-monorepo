#!/bin/bash
set -e

echo "Building RLN-Enabled Status Network Besu"

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
CUSTOM_BESU_DIR="${REPO_ROOT}/custom-besu-build"

echo -e "${BLUE}Working directories:${NC}"
echo -e "  Repo Root: ${REPO_ROOT}"
echo -e "  Sequencer: ${LINEA_SEQUENCER_DIR}"
echo -e "  RLN Prover: ${STATUS_RLN_PROVER_DIR}"

# Use the exact same image version as the official Linea setup
BESU_PACKAGE_TAG="beta-v6.2-20260413130658-9cb6f11"
BESU_BASE_IMAGE="consensys/linea-besu-package:${BESU_PACKAGE_TAG}"

# Shomei plugin version to upgrade to (base image ships an older version)
SHOMEI_PLUGIN_VERSION="1.0.3"

# Build options
BUILD_PROVER=${BUILD_PROVER:-false}
BUILD_POSTGRES=${BUILD_POSTGRES:-false}
RESTART_SERVICES=${RESTART_SERVICES:-false}
MULTI_ARCH=${MULTI_ARCH:-false}

# Publish options
PUSH_IMAGES=${PUSH_IMAGES:-false}
REGISTRY=${REGISTRY:-docker.io}
NAMESPACE=${NAMESPACE:-}
BESU_IMAGE_NAME=${BESU_IMAGE_NAME:-status-network-besu}
RLN_PROVER_IMAGE_NAME=${RLN_PROVER_IMAGE_NAME:-status-network-rln-prover}
POSTGRES_IMAGE_NAME=${POSTGRES_IMAGE_NAME:-status-network-postgres}
IMAGE_TAG=${IMAGE_TAG:-}
IMAGE_TAG_SUFFIX=${IMAGE_TAG_SUFFIX:-}

print_usage() {
    cat << USAGE
Usage: $(basename "$0") [options]

Builds the RLN-enabled Status Network Besu image.
Extracts the official Linea Besu package, replaces the sequencer plugin with
our custom RLN-enabled version, upgrades besu-shomei-plugin, and adds the RLN
native bridge library.

Base Image:    ${BESU_BASE_IMAGE}
Shomei Plugin: v${SHOMEI_PLUGIN_VERSION}

Options:
  --all                        Build everything (Besu + RLN Prover + Postgres)
  --with-prover                Also build the RLN Prover image
  --with-postgres              Also build the custom PostgreSQL image (pg_merkle_tree)
  --restart                    Restart services after build
  --multi-arch                 Build multi-arch images (linux/amd64 + linux/arm64)
  --push                       Push images to a registry after build
  --registry <host>            Registry host (default: docker.io)
  --namespace <ns>             Namespace/org (e.g. 0xnadeem, statusnetwork)
  --besu-name <name>           Besu image name (default: ${BESU_IMAGE_NAME})
  --prover-name <name>         RLN prover image name (default: ${RLN_PROVER_IMAGE_NAME})
  --postgres-name <name>       Postgres image name (default: ${POSTGRES_IMAGE_NAME})
  --tag <tag>                  Image tag (e.g. v2.0.0) - overrides auto-generated tag
  --tag-suffix <suffix>        Optional suffix appended to auto-generated tag
  -h, --help                   Show this help

Examples:
  # Build Besu only (local ARM64)
  $(basename "$0")

  # Build and push all images to Docker Hub
  $(basename "$0") --all --multi-arch --push --namespace 0xnadeem --tag v2.0.0

Environment vars:
  BUILD_PROVER, BUILD_POSTGRES, PUSH_IMAGES, MULTI_ARCH, REGISTRY, NAMESPACE,
  BESU_IMAGE_NAME, RLN_PROVER_IMAGE_NAME, POSTGRES_IMAGE_NAME, IMAGE_TAG, IMAGE_TAG_SUFFIX
USAGE
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --all)
            BUILD_PROVER=true; BUILD_POSTGRES=true; shift ;;
        --with-prover)
            BUILD_PROVER=true; shift ;;
        --with-postgres)
            BUILD_POSTGRES=true; shift ;;
        --restart)
            RESTART_SERVICES=true; shift ;;
        --multi-arch)
            MULTI_ARCH=true; shift ;;
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
        --postgres-name)
            POSTGRES_IMAGE_NAME="$2"; shift 2 ;;
        --tag)
            IMAGE_TAG="$2"; shift 2 ;;
        --tag-suffix)
            IMAGE_TAG_SUFFIX="$2"; shift 2 ;;
        -h|--help)
            print_usage; exit 0 ;;
        *)
            echo -e "${YELLOW}Unknown option: $1${NC}"; print_usage; exit 1 ;;
    esac
done

if [[ "$PUSH_IMAGES" == "true" && -z "$NAMESPACE" ]]; then
    echo -e "${RED}When using --push, --namespace is required.${NC}"
    exit 1
fi

if [[ "$MULTI_ARCH" == "true" ]]; then
    if ! docker buildx version &>/dev/null; then
        echo -e "${RED}Docker buildx is required for multi-arch builds.${NC}"
        exit 1
    fi
    echo -e "${BLUE}Multi-arch mode: building for linux/amd64 + linux/arm64${NC}"
fi

###############################################################################
# Step 1: Build RLN Bridge native library
###############################################################################
cd "${LINEA_SEQUENCER_DIR}/sequencer/src/main/rust/rln_bridge"

if [[ "$MULTI_ARCH" == "true" ]]; then
    echo -e "${BLUE}Step 1: Building RLN Bridge for ARM64 + AMD64...${NC}"
    RLN_ARCHS=("arm64" "amd64")
else
    echo -e "${BLUE}Step 1: Building RLN Bridge for ARM64...${NC}"
    RLN_ARCHS=("arm64")
fi

RLN_LIB_ARM64="${LINEA_SEQUENCER_DIR}/sequencer/src/main/rust/rln_bridge/target/aarch64-unknown-linux-gnu/release/librln_bridge.so"
RLN_LIB_AMD64="${LINEA_SEQUENCER_DIR}/sequencer/src/main/rust/rln_bridge/target/x86_64-unknown-linux-gnu/release/librln_bridge.so"

# Temporary Dockerfile for native Rust builds
cat > Dockerfile.rln-build << 'DOCKEREOF'
FROM rust:1.85-bookworm
RUN apt-get update && apt-get install -y build-essential pkg-config libssl-dev clang llvm && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY resources ./resources
RUN cargo build --release
RUN ls -la target/release/librln_bridge.so
DOCKEREOF

for arch in "${RLN_ARCHS[@]}"; do
    if [[ "$arch" == "arm64" ]]; then
        RLN_LIB_FILE="$RLN_LIB_ARM64"
        TARGET_DIR="target/aarch64-unknown-linux-gnu/release"
        PLATFORM="linux/arm64"
    else
        RLN_LIB_FILE="$RLN_LIB_AMD64"
        TARGET_DIR="target/x86_64-unknown-linux-gnu/release"
        PLATFORM="linux/amd64"
    fi

    if [[ ! -f "$RLN_LIB_FILE" ]]; then
        echo -e "${YELLOW}Building ${arch} library via Docker...${NC}"
        docker build --platform "$PLATFORM" -t "rln-bridge-${arch}-builder" -f Dockerfile.rln-build .
        mkdir -p "$TARGET_DIR"
        docker rm -f "rln-extract-${arch}" 2>/dev/null || true
        docker create --name "rln-extract-${arch}" "rln-bridge-${arch}-builder"
        docker cp "rln-extract-${arch}:/build/target/release/librln_bridge.so" "$TARGET_DIR/"
        docker rm "rln-extract-${arch}"
        file "$RLN_LIB_FILE"
    else
        echo -e "${GREEN}RLN library already exists for ${arch}${NC}"
    fi

    if [[ ! -f "$RLN_LIB_FILE" ]]; then
        echo -e "${RED}Error: ${arch} RLN library not found: $RLN_LIB_FILE${NC}"
        exit 1
    fi
done

rm -f Dockerfile.rln-build
echo -e "${GREEN}RLN Bridge ready for: ${RLN_ARCHS[*]}${NC}"

###############################################################################
# Step 2: Build custom sequencer JAR
###############################################################################
echo -e "${BLUE}Step 2: Building custom sequencer JAR...${NC}"
cd "$REPO_ROOT"

# Wipe build/libs and build/distributions so stale artifacts from older
# branches (e.g. linea-sequencer-beta-v4.4-rc7-dev-*.jar) cannot be picked up
# by the globs below. Those shadow ours at runtime and cause NoSuchMethodError.
rm -rf "${LINEA_SEQUENCER_DIR}/sequencer/build/libs" \
       "${LINEA_SEQUENCER_DIR}/sequencer/build/distributions" \
       "${REPO_ROOT}/tracer/plugins/build/libs" \
       "${REPO_ROOT}/tracer/plugins/build/distributions"

LINEA_DEV_ALLOW_WARNINGS=true ./gradlew \
    :besu-plugins:linea-sequencer:sequencer:jar \
    :besu-plugins:linea-sequencer:sequencer:assembleDist \
    :tracer:plugins:jar \
    :tracer:plugins:assembleDist \
    -x test -x checkSpdxHeader -x spotlessJavaCheck -x spotlessGroovyGradleCheck \
    --no-daemon --configure-on-demand

SEQUENCER_JAR=$(ls -t "${LINEA_SEQUENCER_DIR}/sequencer/build/libs"/linea-sequencer-*.jar 2>/dev/null | head -1)
# Dist zip is named after the project (sequencer-*), not the jar archiveBaseName.
SEQUENCER_DIST=$(ls -t "${LINEA_SEQUENCER_DIR}/sequencer/build/distributions"/sequencer-*.zip 2>/dev/null | head -1)
TRACER_PLUGIN_JAR=$(ls -t "${REPO_ROOT}/tracer/plugins/build/libs"/linea-tracer-*.jar 2>/dev/null | head -1)
TRACER_PLUGIN_DIST=$(ls -t "${REPO_ROOT}/tracer/plugins/build/distributions"/linea-tracer-*.zip 2>/dev/null | head -1)

if [[ ! -f "$SEQUENCER_JAR" ]]; then
    echo -e "${RED}Error: Sequencer JAR not found!${NC}"
    exit 1
fi
if [[ ! -f "$TRACER_PLUGIN_JAR" ]]; then
    echo -e "${RED}Error: Tracer plugin JAR not found!${NC}"
    exit 1
fi
echo -e "${GREEN}Sequencer JAR: $(basename "$SEQUENCER_JAR")${NC}"
echo -e "${GREEN}Tracer JAR: $(basename "$TRACER_PLUGIN_JAR")${NC}"

###############################################################################
# Step 3: Build RLN Prover (optional)
###############################################################################
if [[ "$BUILD_PROVER" == "true" ]]; then
    echo -e "${BLUE}Step 3: Building RLN Prover...${NC}"
    cd "$STATUS_RLN_PROVER_DIR"
    cargo build --release
    if [[ ! -f "${STATUS_RLN_PROVER_DIR}/target/release/prover_cli" ]]; then
        echo -e "${RED}Error: RLN Prover binary not found!${NC}"
        exit 1
    fi
    echo -e "${GREEN}RLN Prover built${NC}"
else
    echo -e "${YELLOW}Skipping RLN Prover (use --all or --with-prover)${NC}"
fi

###############################################################################
# Step 4: Assemble Docker image
###############################################################################
echo -e "${BLUE}Step 4: Assembling Docker image...${NC}"
mkdir -p "$CUSTOM_BESU_DIR"
cd "$CUSTOM_BESU_DIR"

# Extract base Besu from official Linea image
echo -e "${YELLOW}Extracting base Besu from ${BESU_BASE_IMAGE}...${NC}"
docker rm temp-besu-extract 2>/dev/null || true
docker create --name temp-besu-extract "${BESU_BASE_IMAGE}"
docker cp temp-besu-extract:/opt/besu/ ./besu/
docker rm temp-besu-extract

# Upgrade shomei plugin to the pinned version (base image may ship an older build)
echo -e "${YELLOW}Upgrading besu-shomei-plugin to v${SHOMEI_PLUGIN_VERSION}...${NC}"
SHOMEI_ZIP="besu-shomei-plugin-v${SHOMEI_PLUGIN_VERSION}.zip"
SHOMEI_URL="https://github.com/Consensys/besu-shomei-plugin/releases/download/v${SHOMEI_PLUGIN_VERSION}/${SHOMEI_ZIP}"
rm -f ./besu/plugins/besu-shomei-plugin-*.jar
curl -fsSL -o "./${SHOMEI_ZIP}" "$SHOMEI_URL"
unzip -j -o "./${SHOMEI_ZIP}" -d ./besu/plugins/
rm -f "./${SHOMEI_ZIP}"
echo -e "${GREEN}  Installed: besu-shomei-plugin-v${SHOMEI_PLUGIN_VERSION}${NC}"

# Replace sequencer AND tracer plugins — both must come from the same source
# tree so the bundled linea-plugins-common / arithmetization versions match
# (mixing stock linea-tracer-*.jar with our sequencer causes NoSuchMethodError
#  at startup due to shadowed AbstractLineaOptionsPlugin classes).
echo -e "${YELLOW}Installing custom sequencer + tracer plugins...${NC}"
rm -f ./besu/plugins/linea-sequencer*.jar \
      ./besu/plugins/linea-tracer*.jar \
      ./besu/plugins/arithmetization-*.jar
cp "$SEQUENCER_JAR" ./besu/plugins/
cp "$TRACER_PLUGIN_JAR" ./besu/plugins/
echo -e "  Installed: $(basename "$SEQUENCER_JAR")"
echo -e "  Installed: $(basename "$TRACER_PLUGIN_JAR")"

# Install plugin dependencies in separate directory to avoid version conflicts
if [[ -f "$SEQUENCER_DIST" || -f "$TRACER_PLUGIN_DIST" ]]; then
    echo -e "${YELLOW}Installing plugin dependencies...${NC}"
    mkdir -p ./besu/plugins/lib
    [[ -f "$SEQUENCER_DIST" ]] && unzip -q "$SEQUENCER_DIST" -d extracted-deps/
    [[ -f "$TRACER_PLUGIN_DIST" ]] && unzip -q -o "$TRACER_PLUGIN_DIST" -d extracted-deps/
    # Exclude JARs already provided by the base Besu image to avoid version conflicts.
    # NOTE: grpc-stub and grpc-protobuf are NOT in the base image — they must be included.
    # We exclude grpc-netty, grpc-core, grpc-api, grpc-context, grpc-util (all in base).
    # Only exclude JARs that are KNOWN to exist in the base Besu lib/ directory.
    # Check the base image lib/ to see what's already provided.
    EXCLUDE_PATTERN="(tuweni|besu-|linea-sequencer-|grpc-netty|grpc-core|grpc-util|grpc-context|grpc-api|netty-|guava-|gson-|error_prone|failureaccess|perfmark|animal-sniffer|jspecify)"
    for jar in extracted-deps/*/*.jar; do
        jarname=$(basename "$jar")
        if [[ ! "$jarname" =~ $EXCLUDE_PATTERN ]]; then
            cp "$jar" ./besu/plugins/lib/
        fi
    done
    rm -rf extracted-deps/
    echo -e "${GREEN}  Plugin dependencies installed${NC}"
fi

# Install RLN native libraries
echo -e "${YELLOW}Installing RLN native libraries...${NC}"
mkdir -p ./besu/lib/native-arm64 ./besu/lib/native-amd64

if [[ -f "$RLN_LIB_ARM64" ]]; then
    cp "$RLN_LIB_ARM64" ./besu/lib/native-arm64/librln_bridge.so
    echo -e "${GREEN}  Installed: librln_bridge.so (arm64)${NC}"
fi
if [[ -f "$RLN_LIB_AMD64" ]]; then
    cp "$RLN_LIB_AMD64" ./besu/lib/native-amd64/librln_bridge.so
    echo -e "${GREEN}  Installed: librln_bridge.so (amd64)${NC}"
fi

# For single-arch builds, also create the standard path
if [[ "$MULTI_ARCH" != "true" ]] && [[ -f "$RLN_LIB_ARM64" ]]; then
    mkdir -p ./besu/lib/native
    cp "$RLN_LIB_ARM64" ./besu/lib/native/librln_bridge.so
fi

# Copy plugin dependency JARs into the main plugins dir so they're on the classpath
# (beta-v6 uses explicit classpath, plugins/* is included but not plugins/lib/*)
if [[ -d "./besu/plugins/lib" ]]; then
    echo -e "${YELLOW}Moving plugin deps into plugins dir...${NC}"
    mv ./besu/plugins/lib/*.jar ./besu/plugins/ 2>/dev/null || true
    rmdir ./besu/plugins/lib 2>/dev/null || true
fi

# Create Dockerfile
if [[ "$MULTI_ARCH" == "true" ]]; then
    cat > Dockerfile << 'EOF'
FROM ubuntu:24.04
ARG TARGETARCH
RUN apt-get update && \
    apt-get install -y openjdk-25-jre-headless libjemalloc-dev && \
    apt-get clean && rm -rf /var/lib/apt/lists/* && \
    (groupadd -g 1000 besu || true) && \
    useradd -u 1000 -g 1000 -m -s /bin/bash besu || \
    (userdel -r besu 2>/dev/null || true && groupdel besu 2>/dev/null || true && \
     groupadd -g 1001 besu && useradd -u 1001 -g besu -m -s /bin/bash besu)
USER besu
WORKDIR /opt/besu
COPY --chown=besu:besu besu/ /opt/besu/
COPY --chown=besu:besu besu/lib/native-${TARGETARCH}/librln_bridge.so /opt/besu/lib/native/librln_bridge.so
ENV LD_LIBRARY_PATH="/opt/besu/lib/native:/usr/local/lib:/usr/lib"
ENV JAVA_LIBRARY_PATH="/opt/besu/lib/native"
ENV PATH="/opt/besu/bin:${PATH}"
EXPOSE 8545 8546 8547 8550 8551 30303
ENTRYPOINT ["besu"]
HEALTHCHECK --start-period=5s --interval=5s --timeout=1s --retries=10 CMD bash -c "[ -f /tmp/pid ]"
EOF
else
    cat > Dockerfile << 'EOF'
FROM ubuntu:24.04
RUN apt-get update && \
    apt-get install -y openjdk-25-jre-headless libjemalloc-dev && \
    apt-get clean && rm -rf /var/lib/apt/lists/* && \
    (groupadd -g 1000 besu || true) && \
    useradd -u 1000 -g 1000 -m -s /bin/bash besu || \
    (userdel -r besu 2>/dev/null || true && groupdel besu 2>/dev/null || true && \
     groupadd -g 1001 besu && useradd -u 1001 -g besu -m -s /bin/bash besu)
USER besu
WORKDIR /opt/besu
COPY --chown=besu:besu besu/ /opt/besu/
ENV LD_LIBRARY_PATH="/opt/besu/lib/native:/usr/local/lib:/usr/lib"
ENV JAVA_LIBRARY_PATH="/opt/besu/lib/native"
ENV PATH="/opt/besu/bin:${PATH}"
EXPOSE 8545 8546 8547 8550 8551 30303
ENTRYPOINT ["besu"]
HEALTHCHECK --start-period=5s --interval=5s --timeout=1s --retries=10 CMD bash -c "[ -f /tmp/pid ]"
EOF
fi

###############################################################################
# Step 5: Build and push Docker images
###############################################################################

# Determine image tag
if [[ -n "$IMAGE_TAG" ]]; then
    FINAL_TAG="$IMAGE_TAG"
else
    TIMESTAMP=$(date +%Y%m%d%H%M%S)
    FINAL_TAG="${TIMESTAMP}${IMAGE_TAG_SUFFIX}"
fi

# Image names
if [[ -n "$NAMESPACE" ]]; then
    BESU_IMAGE_REMOTE="${REGISTRY}/${NAMESPACE}/${BESU_IMAGE_NAME}:${FINAL_TAG}"
    RLN_PROVER_IMAGE_REMOTE="${REGISTRY}/${NAMESPACE}/${RLN_PROVER_IMAGE_NAME}:${FINAL_TAG}"
    POSTGRES_IMAGE_REMOTE="${REGISTRY}/${NAMESPACE}/${POSTGRES_IMAGE_NAME}:${FINAL_TAG}"
else
    BESU_IMAGE_REMOTE="${BESU_IMAGE_NAME}:${FINAL_TAG}"
    RLN_PROVER_IMAGE_REMOTE="${RLN_PROVER_IMAGE_NAME}:${FINAL_TAG}"
    POSTGRES_IMAGE_REMOTE="${POSTGRES_IMAGE_NAME}:${FINAL_TAG}"
fi

BESU_IMAGE_LOCAL="${BESU_IMAGE_NAME}:${FINAL_TAG}"

echo -e "${BLUE}Step 5: Building Docker image...${NC}"

if [[ "$MULTI_ARCH" == "true" ]]; then
    BUILDER_NAME="sn-multi-arch-builder"
    if ! docker buildx inspect "$BUILDER_NAME" &>/dev/null; then
        docker buildx create --name "$BUILDER_NAME" --driver docker-container --use
    else
        docker buildx use "$BUILDER_NAME"
    fi

    if [[ "$PUSH_IMAGES" == "true" ]]; then
        echo -e "${YELLOW}Building and pushing multi-arch: ${BESU_IMAGE_REMOTE}${NC}"
        docker buildx build --platform linux/amd64,linux/arm64 --push -t "$BESU_IMAGE_REMOTE" .
    else
        docker buildx build --platform linux/amd64 --load -t "${BESU_IMAGE_LOCAL}-amd64" .
        docker buildx build --platform linux/arm64 --load -t "${BESU_IMAGE_LOCAL}-arm64" .
    fi
else
    echo -e "${YELLOW}Building: ${BESU_IMAGE_LOCAL}${NC}"
    docker build --platform linux/arm64 -t "$BESU_IMAGE_LOCAL" .

    if [[ "$PUSH_IMAGES" == "true" ]]; then
        docker tag "$BESU_IMAGE_LOCAL" "$BESU_IMAGE_REMOTE"
        docker push "$BESU_IMAGE_REMOTE"
    fi
fi

echo -e "${GREEN}Besu image ready: ${BESU_IMAGE_REMOTE:-$BESU_IMAGE_LOCAL}${NC}"

# Build RLN Prover image
if [[ "$BUILD_PROVER" == "true" ]]; then
    echo -e "${BLUE}Building RLN Prover Docker image...${NC}"
    cd "$STATUS_RLN_PROVER_DIR"
    RLN_PROVER_LOCAL="${RLN_PROVER_IMAGE_NAME}:${FINAL_TAG}"

    if [[ "$MULTI_ARCH" == "true" ]]; then
        if [[ "$PUSH_IMAGES" == "true" ]]; then
            docker buildx build --platform linux/amd64,linux/arm64 --push -t "$RLN_PROVER_IMAGE_REMOTE" .
        else
            docker buildx build --platform linux/amd64 --load -t "${RLN_PROVER_LOCAL}-amd64" .
            docker buildx build --platform linux/arm64 --load -t "${RLN_PROVER_LOCAL}-arm64" .
        fi
    else
        docker build --platform linux/arm64 -t "$RLN_PROVER_LOCAL" .
        if [[ "$PUSH_IMAGES" == "true" ]]; then
            docker tag "$RLN_PROVER_LOCAL" "$RLN_PROVER_IMAGE_REMOTE"
            docker push "$RLN_PROVER_IMAGE_REMOTE"
        fi
    fi
    echo -e "${GREEN}RLN Prover ready: ${RLN_PROVER_IMAGE_REMOTE:-$RLN_PROVER_LOCAL}${NC}"
fi

# Build PostgreSQL image
if [[ "$BUILD_POSTGRES" == "true" ]]; then
    echo -e "${BLUE}Building custom PostgreSQL image...${NC}"
    POSTGRES_DOCKERFILE="${REPO_ROOT}/pgrx_merkle_tree/docker/Dockerfile"
    POSTGRES_LOCAL="${POSTGRES_IMAGE_NAME}:${FINAL_TAG}"

    if [[ "$MULTI_ARCH" == "true" ]]; then
        if [[ "$PUSH_IMAGES" == "true" ]]; then
            docker buildx build --platform linux/amd64,linux/arm64 --push -t "$POSTGRES_IMAGE_REMOTE" -f "$POSTGRES_DOCKERFILE" "$REPO_ROOT"
        else
            docker buildx build --platform linux/amd64 --load -t "${POSTGRES_LOCAL}-amd64" -f "$POSTGRES_DOCKERFILE" "$REPO_ROOT"
            docker buildx build --platform linux/arm64 --load -t "${POSTGRES_LOCAL}-arm64" -f "$POSTGRES_DOCKERFILE" "$REPO_ROOT"
        fi
    else
        docker build -t "$POSTGRES_LOCAL" -f "$POSTGRES_DOCKERFILE" "$REPO_ROOT"
        if [[ "$PUSH_IMAGES" == "true" ]]; then
            docker tag "$POSTGRES_LOCAL" "$POSTGRES_IMAGE_REMOTE"
            docker push "$POSTGRES_IMAGE_REMOTE"
        fi
    fi
    echo -e "${GREEN}PostgreSQL ready: ${POSTGRES_IMAGE_REMOTE:-$POSTGRES_LOCAL}${NC}"
fi

###############################################################################
# Step 6: Update Docker Compose
###############################################################################
echo -e "${BLUE}Step 6: Updating Docker Compose...${NC}"
COMPOSE_FILE="${REPO_ROOT}/docker/compose-spec-l2-services-rln.yml"
if [[ -f "$COMPOSE_FILE" ]]; then
    cp "$COMPOSE_FILE" "${COMPOSE_FILE}.backup.$(date +%Y%m%d%H%M%S)"

    # Update Besu image for sequencer, l2-node-besu, l2-node-besu-follower
    BESU_IMG="${BESU_IMAGE_REMOTE:-$BESU_IMAGE_LOCAL}"
    awk -v besu_img="$BESU_IMG" '
        /^[[:space:]]*container_name:[[:space:]]*(sequencer|l2-node-besu|l2-node-besu-follower)$/ { tgt = "besu" }
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
    echo -e "${GREEN}Updated compose with: $BESU_IMG${NC}"
fi

# Cleanup
cd "$REPO_ROOT"
rm -rf "$CUSTOM_BESU_DIR"

echo -e "${GREEN}Build complete!${NC}"
echo -e "  Besu: ${BESU_IMAGE_REMOTE:-$BESU_IMAGE_LOCAL}"
[[ "$BUILD_PROVER" == "true" ]] && echo -e "  RLN Prover: ${RLN_PROVER_IMAGE_REMOTE:-$RLN_PROVER_LOCAL}"
[[ "$BUILD_POSTGRES" == "true" ]] && echo -e "  PostgreSQL: ${POSTGRES_IMAGE_REMOTE:-$POSTGRES_LOCAL}"

# Restart services if requested
if [[ "$RESTART_SERVICES" == "true" ]]; then
    echo -e "${BLUE}Restarting services...${NC}"
    cd "$REPO_ROOT"
    docker compose -f "$COMPOSE_FILE" up -d --force-recreate sequencer l2-node-besu l2-node-besu-follower
    echo -e "${GREEN}Services restarted${NC}"
fi
