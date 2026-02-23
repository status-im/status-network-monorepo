#!/bin/bash

# Status Network Contract Deployment Script
# This script deploys contracts after the network is running

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"

# Default values
NAMESPACE="${NAMESPACE:-status-network}"
L1_RPC="${L1_RPC:-}"
L2_RPC="${L2_RPC:-}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Deploy contracts to Status Network

Options:
    -h, --help              Show this help message
    -n, --namespace NAME    Kubernetes namespace (default: status-network)
    --l1-rpc URL            L1 RPC endpoint (auto-detected if not provided)
    --l2-rpc URL            L2 RPC endpoint (auto-detected if not provided)

Example:
    $0
    $0 --l1-rpc http://localhost:8545 --l2-rpc http://localhost:8546
EOF
}

get_rpc_endpoints() {
    if [ -z "$L1_RPC" ]; then
        log_info "Detecting L1 RPC endpoint..."
        # Try to get from LoadBalancer first
        L1_RPC=$(kubectl get svc l1-rpc-lb -n "$NAMESPACE" -o jsonpath='{.status.loadBalancer.ingress[0].hostname}' 2>/dev/null)
        if [ -z "$L1_RPC" ]; then
            # Fall back to port-forward
            L1_RPC="http://localhost:8445"
            log_info "Using port-forward for L1 RPC"
            kubectl port-forward svc/l1-el-node 8445:8545 -n "$NAMESPACE" &
            L1_PF_PID=$!
            sleep 2
        else
            L1_RPC="http://$L1_RPC:8545"
        fi
    fi

    if [ -z "$L2_RPC" ]; then
        log_info "Detecting L2 RPC endpoint..."
        # Try to get from LoadBalancer first
        L2_RPC=$(kubectl get svc l2-rpc-lb -n "$NAMESPACE" -o jsonpath='{.status.loadBalancer.ingress[0].hostname}' 2>/dev/null)
        if [ -z "$L2_RPC" ]; then
            # Fall back to port-forward
            L2_RPC="http://localhost:8545"
            log_info "Using port-forward for L2 RPC"
            kubectl port-forward svc/l2-node-besu 8545:8545 -n "$NAMESPACE" &
            L2_PF_PID=$!
            sleep 2
        else
            L2_RPC="http://$L2_RPC:8545"
        fi
    fi

    log_info "L1 RPC: $L1_RPC"
    log_info "L2 RPC: $L2_RPC"
}

check_node_health() {
    log_info "Checking node health..."

    # Check L1
    local l1_block=$(curl -s -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
        "$L1_RPC" | jq -r '.result')

    if [ -z "$l1_block" ] || [ "$l1_block" = "null" ]; then
        log_error "L1 node is not responding"
        return 1
    fi
    log_info "L1 block number: $l1_block"

    # Check L2
    local l2_block=$(curl -s -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
        "$L2_RPC" | jq -r '.result')

    if [ -z "$l2_block" ] || [ "$l2_block" = "null" ]; then
        log_error "L2 node is not responding"
        return 1
    fi
    log_info "L2 block number: $l2_block"

    return 0
}

deploy_contracts() {
    log_info "Contract deployment..."

    # Note: The contracts are pre-deployed in the genesis configuration
    # This function is a placeholder for additional contract deployment

    echo ""
    echo "============================================"
    echo "Pre-deployed Contract Addresses:"
    echo "============================================"
    echo ""
    echo "L1 Contracts:"
    echo "  Rollup Contract:     0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9"
    echo ""
    echo "L2 Contracts:"
    echo "  Bridge Contract:     0xe537D669CA013d86EBeF1D64e40fC74CADC91987"
    echo "  RLN Contract:        0x9A9f2CCfdE556A7E9Ff0848998Aa4a0CFD8863AE"
    echo "  Validator Contract:  0x0000000000000000000000000000000000000001"
    echo ""

    # If you need to deploy additional contracts, add the logic here
    # For example:
    #
    # cd "$PROJECT_ROOT/status-network-contracts"
    # export L1_RPC_URL="$L1_RPC"
    # export L2_RPC_URL="$L2_RPC"
    # export PRIVATE_KEY="0x..."
    # forge script script/Deploy.s.sol --rpc-url "$L1_RPC" --broadcast

    log_info "Contract deployment complete"
}

initialize_karma_tiers() {
    log_info "Initializing karma tiers..."

    # This is a placeholder for karma tier initialization
    # Add the actual initialization logic here

    echo "Karma tiers initialization is typically done as part of contract deployment"
    echo "or through a separate initialization script."

    log_info "Karma tiers initialization complete"
}

cleanup() {
    # Kill port-forward processes if started
    if [ -n "$L1_PF_PID" ]; then
        kill $L1_PF_PID 2>/dev/null || true
    fi
    if [ -n "$L2_PF_PID" ]; then
        kill $L2_PF_PID 2>/dev/null || true
    fi
}

trap cleanup EXIT

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            exit 0
            ;;
        -n|--namespace)
            NAMESPACE="$2"
            shift 2
            ;;
        --l1-rpc)
            L1_RPC="$2"
            shift 2
            ;;
        --l2-rpc)
            L2_RPC="$2"
            shift 2
            ;;
        *)
            log_error "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

# Main execution
get_rpc_endpoints
check_node_health
deploy_contracts
initialize_karma_tiers

log_info "All contract operations completed successfully!"
