#!/bin/bash
# Start RLN environment in production mode
# This script:
# 1. Starts the network with mock RLN (to allow L2 node to start)
# 2. Deploys contracts
# 3. Initializes karma tiers
# 4. Restarts RLN services in production mode

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEPLOYMENTS_DIR="${REPO_ROOT}/status-network-contracts/deployments"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}🚀 Starting RLN Environment in PRODUCTION Mode${NC}"
echo "=================================================="

# Step 1: Clean and start environment
echo -e "\n${YELLOW}Step 1: Starting network with mock RLN...${NC}"
cd "$REPO_ROOT"
make clean-environment 2>/dev/null || true
make start-env COMPOSE_FILE=docker/compose-tracing-v2-rln.yml LINEA_PROTOCOL_CONTRACTS_ONLY=true STATUS_NETWORK_CONTRACTS_ENABLED=true

# Step 2: Wait for deployment files
echo -e "\n${YELLOW}Step 2: Waiting for contract deployment...${NC}"
MAX_WAIT=120
WAITED=0
while [ ! -f "${DEPLOYMENTS_DIR}/karma_address.txt" ] && [ $WAITED -lt $MAX_WAIT ]; do
    sleep 5
    WAITED=$((WAITED + 5))
    echo "  Waiting for contracts... (${WAITED}s)"
done

if [ ! -f "${DEPLOYMENTS_DIR}/karma_address.txt" ]; then
    echo -e "${RED}❌ Contract deployment files not found after ${MAX_WAIT}s${NC}"
    exit 1
fi

# Step 3: Read deployed addresses
echo -e "\n${YELLOW}Step 3: Reading deployed contract addresses...${NC}"
KARMA_ADDR=$(cat "${DEPLOYMENTS_DIR}/karma_address.txt" 2>/dev/null)
RLN_ADDR=$(cat "${DEPLOYMENTS_DIR}/rln_address.txt" 2>/dev/null)
TIERS_ADDR=$(cat "${DEPLOYMENTS_DIR}/karma_tiers_address.txt" 2>/dev/null)

echo -e "  Karma:      ${GREEN}${KARMA_ADDR}${NC}"
echo -e "  RLN:        ${GREEN}${RLN_ADDR}${NC}"
echo -e "  KarmaTiers: ${GREEN}${TIERS_ADDR}${NC}"

# Validate addresses
if [ -z "$KARMA_ADDR" ] || [ -z "$RLN_ADDR" ] || [ -z "$TIERS_ADDR" ]; then
    echo -e "${RED}❌ One or more contract addresses are empty${NC}"
    exit 1
fi

# Step 4: Initialize karma tiers
echo -e "\n${YELLOW}Step 4: Initializing karma tiers...${NC}"
cd "${REPO_ROOT}/e2e"
if command -v npx &> /dev/null; then
    KARMA_TIERS_ADDRESS="$TIERS_ADDR" npx ts-node ../scripts/initialize-karma-tiers.ts || {
        echo -e "${YELLOW}⚠️  Karma tiers initialization failed (may already be initialized)${NC}"
    }
else
    echo -e "${YELLOW}⚠️  npx not found, skipping tier initialization${NC}"
fi

# Step 5: Restart RLN services in production mode
echo -e "\n${YELLOW}Step 5: Restarting RLN services in production mode...${NC}"
cd "$REPO_ROOT"

# Stop current RLN services
docker compose -f docker/compose-tracing-v2-rln.yml stop rln-prover karma-service

# Start with production configuration
export RLN_MOCK_MODE=false
export KARMA_SC_ADDRESS="$KARMA_ADDR"
export RLN_SC_ADDRESS="$RLN_ADDR"
export KARMA_TIERS_SC_ADDRESS="$TIERS_ADDR"
export WS_RPC_URL="ws://l2-node-besu:8546"

docker compose -f docker/compose-tracing-v2-rln.yml up -d rln-prover karma-service

# Step 6: Wait for services to be healthy
echo -e "\n${YELLOW}Step 6: Waiting for RLN services to be healthy...${NC}"
sleep 10

# Check health
for container in rln-prover karma-service; do
    STATUS=$(docker inspect --format='{{.State.Health.Status}}' $container 2>/dev/null || echo "unknown")
    if [ "$STATUS" = "healthy" ]; then
        echo -e "  ${container}: ${GREEN}healthy${NC}"
    else
        echo -e "  ${container}: ${YELLOW}${STATUS}${NC}"
    fi
done

echo -e "\n${GREEN}✅ RLN Environment running in PRODUCTION mode!${NC}"
echo ""
echo "Contract Addresses:"
echo "  KARMA_SC_ADDRESS=${KARMA_ADDR}"
echo "  RLN_SC_ADDRESS=${RLN_ADDR}"
echo "  KARMA_TIERS_SC_ADDRESS=${TIERS_ADDR}"
echo ""
echo "To test gasless transactions, users need Karma tokens."
echo "Use the karma-manager utility to mint Karma for test users."
echo ""
echo "Run tests with: cd e2e && pnpm run test:rln:local"

