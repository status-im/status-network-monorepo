#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory and repo root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Default values
IMAGE_NAME="status-network-postgres"
IMAGE_TAG="v1.0.2"
MULTI_ARCH=false
PUSH_IMAGES=false
NAMESPACE=""
REGISTRY="docker.io"
NO_CACHE_FLAG=""

print_usage() {
    cat << USAGE
Usage: $(basename "$0") [options]

Build the custom PostgreSQL Docker image with the pg_merkle_tree extension.

Options:
  --tag <tag>             Image tag (default: ${IMAGE_TAG})
  --name <name>           Image name (default: ${IMAGE_NAME})
  --multi-arch            Build multi-arch images (linux/amd64 + linux/arm64)
  --push                  Push images to a registry after build
  --namespace <ns>        Namespace/org (e.g. 0xnadeem, statusnetwork)
  --registry <host>       Registry host (default: docker.io for Docker Hub)
  --no-cache              Force clean build
  -h, --help              Show this help

Examples:
  # Local build only
  $(basename "$0")

  # Multi-arch build and push to Docker Hub
  $(basename "$0") --multi-arch --push --namespace 0xnadeem --tag v1.0.2

USAGE
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --tag)
            IMAGE_TAG="$2"; shift 2 ;;
        --name)
            IMAGE_NAME="$2"; shift 2 ;;
        --multi-arch)
            MULTI_ARCH=true; shift ;;
        --push)
            PUSH_IMAGES=true; shift ;;
        --namespace)
            NAMESPACE="$2"; shift 2 ;;
        --registry)
            REGISTRY="$2"; shift 2 ;;
        --no-cache)
            NO_CACHE_FLAG="--no-cache"; shift ;;
        -h|--help)
            print_usage; exit 0 ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"; print_usage; exit 1 ;;
    esac
done

if [[ "$PUSH_IMAGES" == "true" && -z "$NAMESPACE" ]]; then
    echo -e "${RED}Error: --namespace is required when using --push${NC}"
    exit 1
fi

# Check for buildx if multi-arch is enabled
if [[ "$MULTI_ARCH" == "true" ]]; then
    if ! docker buildx version &>/dev/null; then
        echo -e "${RED}Error: Docker buildx is required for multi-arch builds.${NC}"
        exit 1
    fi
fi

# Validate Dockerfile exists
DOCKERFILE="${REPO_ROOT}/pgrx_merkle_tree/docker/Dockerfile"
if [[ ! -f "$DOCKERFILE" ]]; then
    echo -e "${RED}Error: Dockerfile not found: ${DOCKERFILE}${NC}"
    exit 1
fi

# Construct image references
LOCAL_IMAGE="${IMAGE_NAME}:${IMAGE_TAG}"
if [[ -n "$NAMESPACE" ]]; then
    REMOTE_IMAGE="${REGISTRY}/${NAMESPACE}/${IMAGE_NAME}:${IMAGE_TAG}"
else
    REMOTE_IMAGE="${IMAGE_NAME}:${IMAGE_TAG}"
fi

echo -e "${BLUE}Building Custom PostgreSQL Image (pg_merkle_tree)${NC}"
echo -e "  Dockerfile: ${DOCKERFILE}"
echo -e "  Context:    ${REPO_ROOT}"
echo -e "  Image:      ${LOCAL_IMAGE}"
if [[ "$PUSH_IMAGES" == "true" ]]; then
    echo -e "  Remote:     ${REMOTE_IMAGE}"
fi
if [[ "$MULTI_ARCH" == "true" ]]; then
    echo -e "  Platforms:  linux/amd64, linux/arm64"
fi
echo ""

export DOCKER_BUILDKIT=1

if [[ "$MULTI_ARCH" == "true" ]]; then
    # Create/use a buildx builder with multi-platform support
    BUILDER_NAME="multi-arch-builder"
    if ! docker buildx inspect "$BUILDER_NAME" &>/dev/null; then
        echo -e "${YELLOW}Creating buildx builder for multi-arch...${NC}"
        docker buildx create --name "$BUILDER_NAME" --driver docker-container --use
    else
        docker buildx use "$BUILDER_NAME"
    fi

    if [[ "$PUSH_IMAGES" == "true" ]]; then
        echo -e "${YELLOW}Building and pushing multi-arch image to ${REMOTE_IMAGE}...${NC}"
        docker buildx build \
            --platform linux/amd64,linux/arm64 \
            $NO_CACHE_FLAG \
            --push \
            -t "$REMOTE_IMAGE" \
            -f "$DOCKERFILE" \
            "$REPO_ROOT"
        echo -e "${GREEN}Multi-arch image built and pushed: ${REMOTE_IMAGE}${NC}"
    else
        echo -e "${YELLOW}Building multi-arch images locally...${NC}"
        docker buildx build \
            --platform linux/amd64 \
            $NO_CACHE_FLAG \
            --load \
            -t "${LOCAL_IMAGE}-amd64" \
            -f "$DOCKERFILE" \
            "$REPO_ROOT"
        docker buildx build \
            --platform linux/arm64 \
            $NO_CACHE_FLAG \
            --load \
            -t "${LOCAL_IMAGE}-arm64" \
            -f "$DOCKERFILE" \
            "$REPO_ROOT"
        echo -e "${GREEN}Multi-arch images built: ${LOCAL_IMAGE}-amd64, ${LOCAL_IMAGE}-arm64${NC}"
    fi
else
    echo -e "${YELLOW}Building image (this may take several minutes for Rust compilation)...${NC}"
    docker build $NO_CACHE_FLAG -t "$LOCAL_IMAGE" -f "$DOCKERFILE" "$REPO_ROOT"
    echo -e "${GREEN}Image built: ${LOCAL_IMAGE}${NC}"

    if [[ "$PUSH_IMAGES" == "true" ]]; then
        echo -e "${BLUE}Pushing image to registry...${NC}"
        docker tag "$LOCAL_IMAGE" "$REMOTE_IMAGE"
        docker push "$REMOTE_IMAGE"
        echo -e "${GREEN}Image pushed: ${REMOTE_IMAGE}${NC}"
    fi
fi

echo ""
echo -e "${GREEN}Build Complete!${NC}"
echo ""
echo -e "${BLUE}Summary:${NC}"
if [[ "$PUSH_IMAGES" == "true" ]]; then
    echo -e "  Image: ${REMOTE_IMAGE}"
else
    echo -e "  Image: ${LOCAL_IMAGE}"
fi
echo ""
