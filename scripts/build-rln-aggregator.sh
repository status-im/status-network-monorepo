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
RLN_AGGREGATOR_DIR="${REPO_ROOT}/rln-aggregator"
RLN_PROVER_PROTO_DIR="${REPO_ROOT}/rln-prover/proto"

# Default values
IMAGE_NAME="${RLN_AGGREGATOR_IMAGE_NAME:-status-network-rln-aggregator}"
IMAGE_TAG="${RLN_AGGREGATOR_TAG:-$(date +%Y%m%d%H%M%S)}"
FULL_IMAGE="${IMAGE_NAME}:${IMAGE_TAG}"
UPDATE_COMPOSE="${UPDATE_COMPOSE:-true}"
RESTART_SERVICES="${RESTART_SERVICES:-false}"
NO_CACHE_FLAG=""

print_usage() {
    cat << USAGE
Usage: $(basename "$0") [options]

Build the RLN Aggregator Docker image. The aggregator subscribes to the
rln-prover's GetProofs gRPC stream and rebroadcasts proofs to slashers via
its own RlnAggregator.GetProofs endpoint. Both rln-aggregator-1 and
rln-aggregator-2 in compose use this same image.

Options:
  -n, --name <name>       Image name (default: ${IMAGE_NAME})
  -t, --tag <tag>         Image tag (default: timestamp)
  --no-compose            Don't update docker-compose file
  --restart               Restart rln-aggregator-{1,2} and rln-aggregator-lb after build
  --no-cache              Force clean build (slow, use only when needed)
  -h, --help              Show this help

Examples:
  $(basename "$0")                    # Build with caching (fast)
  $(basename "$0") -t local           # Build with stable 'local' tag
  $(basename "$0") --restart          # Build and restart aggregator services
  $(basename "$0") --no-cache         # Force clean rebuild (slow)

USAGE
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        -n|--name)
            IMAGE_NAME="$2"; shift 2 ;;
        -t|--tag)
            IMAGE_TAG="$2"; shift 2 ;;
        --no-compose)
            UPDATE_COMPOSE=false; shift ;;
        --restart)
            RESTART_SERVICES=true; shift ;;
        --no-cache)
            NO_CACHE_FLAG="--no-cache"; shift ;;
        -h|--help)
            print_usage; exit 0 ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"; print_usage; exit 1 ;;
    esac
done

FULL_IMAGE="${IMAGE_NAME}:${IMAGE_TAG}"

echo -e "${BLUE}🐳 Building RLN Aggregator Docker Image${NC}"
echo -e "  Directory: ${RLN_AGGREGATOR_DIR}"
echo -e "  Image: ${FULL_IMAGE}"
echo ""

# Check if rln-aggregator directory exists
if [[ ! -d "$RLN_AGGREGATOR_DIR" ]]; then
    echo -e "${RED}❌ Error: RLN aggregator directory not found: ${RLN_AGGREGATOR_DIR}${NC}"
    exit 1
fi

# Check if Dockerfile exists
if [[ ! -f "${RLN_AGGREGATOR_DIR}/Dockerfile" ]]; then
    echo -e "${RED}❌ Error: Dockerfile not found in ${RLN_AGGREGATOR_DIR}${NC}"
    exit 1
fi

# Check if prover proto context exists
if [[ ! -d "$RLN_PROVER_PROTO_DIR" ]]; then
    echo -e "${RED}❌ Error: rln-prover proto directory not found: ${RLN_PROVER_PROTO_DIR}${NC}"
    exit 1
fi

# Build the image
echo -e "${YELLOW}🔨 Building image (this may take several minutes for Rust compilation)...${NC}"
cd "$RLN_AGGREGATOR_DIR"

export DOCKER_BUILDKIT=1

if docker build $NO_CACHE_FLAG \
    --build-context prover_proto="${RLN_PROVER_PROTO_DIR}" \
    -f Dockerfile \
    -t "$FULL_IMAGE" .; then
    echo -e "${GREEN}✅ Successfully built: ${FULL_IMAGE}${NC}"
else
    echo -e "${RED}❌ Docker build failed${NC}"
    exit 1
fi

# Update docker-compose file (both rln-aggregator-1 and rln-aggregator-2)
if [[ "$UPDATE_COMPOSE" == "true" ]]; then
    COMPOSE_FILE="${REPO_ROOT}/docker/compose-spec-l2-services-rln.yml"

    if [[ -f "$COMPOSE_FILE" ]]; then
        echo -e "${BLUE}📝 Updating Docker Compose...${NC}"

        # Create backup
        BACKUP_FILE="${COMPOSE_FILE}.backup.$(date +%Y%m%d%H%M%S)"
        cp "$COMPOSE_FILE" "$BACKUP_FILE"

        # Update image lines for both rln-aggregator-1 and rln-aggregator-2
        awk -v agg_img="$FULL_IMAGE" '
            /^[[:space:]]*container_name:[[:space:]]*rln-aggregator-1$/ { tgt = "agg" }
            /^[[:space:]]*container_name:[[:space:]]*rln-aggregator-2$/ { tgt = "agg" }
            {
              if (tgt != "" && $0 ~ /^[[:space:]]*image:[[:space:]]*/) {
                match($0, /^[[:space:]]*/); lead = substr($0, 1, RLENGTH);
                print lead "image: " agg_img;
                tgt = "";
                next;
              }
              print $0;
            }
        ' "$COMPOSE_FILE" > "${COMPOSE_FILE}.tmp" && mv "${COMPOSE_FILE}.tmp" "$COMPOSE_FILE"

        echo -e "${GREEN}✅ Updated compose file with new image (both aggregator replicas)${NC}"
        echo -e "  Backup: ${BACKUP_FILE}"
    else
        echo -e "${YELLOW}⚠️  Compose file not found: ${COMPOSE_FILE}${NC}"
    fi
fi

# Restart services if requested
if [[ "$RESTART_SERVICES" == "true" ]]; then
    echo -e "${BLUE}🔄 Restarting RLN aggregator services...${NC}"

    # Stop and remove old containers (LB depends on the aggregators so restart it too)
    docker rm -f rln-aggregator-1 rln-aggregator-2 rln-aggregator-lb 2>/dev/null || true

    # Start with the new image
    cd "$REPO_ROOT"
    docker compose -f docker/compose-tracing-v2-rln.yml up -d \
        rln-aggregator-1 rln-aggregator-2 rln-aggregator-lb

    echo -e "${GREEN}✅ Services restarted${NC}"

    # Wait for health check
    echo -e "${YELLOW}⏳ Waiting for services to be healthy...${NC}"
    sleep 10

    # Check status
    docker ps --filter name=rln-aggregator --format "table {{.Names}}\t{{.Status}}"
fi

echo ""
echo -e "${GREEN}🎉 Build Complete!${NC}"
echo ""
echo -e "${BLUE}📋 Summary:${NC}"
echo -e "  Image: ${FULL_IMAGE}"
echo ""
echo -e "${YELLOW}🚀 Next Steps:${NC}"
if [[ "$RESTART_SERVICES" != "true" ]]; then
    echo -e "  To restart aggregator services with new image:"
    echo -e "    ${GREEN}docker rm -f rln-aggregator-1 rln-aggregator-2 rln-aggregator-lb${NC}"
    echo -e "    ${GREEN}docker compose -f docker/compose-tracing-v2-rln.yml up -d rln-aggregator-1 rln-aggregator-2 rln-aggregator-lb${NC}"
fi
echo ""
echo -e "  To view logs:"
echo -e "    ${GREEN}docker logs rln-aggregator-1 -f${NC}"
echo -e "    ${GREEN}docker logs rln-aggregator-2 -f${NC}"
echo ""
echo -e "  To verify Envoy LB sees both backends as healthy:"
echo -e "    ${GREEN}curl -s http://localhost:9901/clusters | grep rln_aggregators${NC}"
