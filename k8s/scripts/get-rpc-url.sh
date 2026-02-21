#!/bin/bash

# Get Status Network RPC URL from Kubernetes

set -e

NAMESPACE="${NAMESPACE:-status-network}"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Get L2 RPC URL
L2_RPC_HOST=$(kubectl get svc l2-rpc-lb -n "$NAMESPACE" -o jsonpath='{.status.loadBalancer.ingress[0].hostname}' 2>/dev/null)
if [ -z "$L2_RPC_HOST" ]; then
    L2_RPC_HOST=$(kubectl get svc l2-rpc-lb -n "$NAMESPACE" -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null)
fi

# Get L1 RPC URL (if enabled)
L1_RPC_HOST=$(kubectl get svc l1-rpc-lb -n "$NAMESPACE" -o jsonpath='{.status.loadBalancer.ingress[0].hostname}' 2>/dev/null)
if [ -z "$L1_RPC_HOST" ]; then
    L1_RPC_HOST=$(kubectl get svc l1-rpc-lb -n "$NAMESPACE" -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null)
fi

echo ""
echo "============================================"
echo "Status Network RPC Endpoints"
echo "============================================"
echo ""

if [ -n "$L2_RPC_HOST" ]; then
    L2_RPC_URL="http://$L2_RPC_HOST:8545"
    echo -e "${GREEN}L2 RPC:${NC} $L2_RPC_URL"
    echo ""
    echo "Test L2 connection:"
    echo "  curl $L2_RPC_URL -X POST -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"eth_blockNumber\",\"params\":[],\"id\":1}'"
else
    echo -e "${YELLOW}L2 RPC LoadBalancer not ready${NC}"
    echo "You can use port-forward instead:"
    echo "  kubectl port-forward svc/l2-node-besu 8545:8545 -n $NAMESPACE"
fi

echo ""

if [ -n "$L1_RPC_HOST" ]; then
    L1_RPC_URL="http://$L1_RPC_HOST:8545"
    echo -e "${GREEN}L1 RPC:${NC} $L1_RPC_URL"
    echo ""
    echo "Test L1 connection:"
    echo "  curl $L1_RPC_URL -X POST -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"eth_blockNumber\",\"params\":[],\"id\":1}'"
else
    echo "L1 RPC LoadBalancer not enabled or not ready"
    echo "You can use port-forward instead:"
    echo "  kubectl port-forward svc/l1-el-node 8445:8545 -n $NAMESPACE"
fi

echo ""
echo "============================================"
echo ""

# Show pod status
echo "Pod Status:"
kubectl get pods -n "$NAMESPACE" --no-headers | while read line; do
    name=$(echo "$line" | awk '{print $1}')
    status=$(echo "$line" | awk '{print $3}')
    if [ "$status" = "Running" ]; then
        echo -e "  ${GREEN}✓${NC} $name"
    else
        echo -e "  ${YELLOW}○${NC} $name ($status)"
    fi
done

echo ""
