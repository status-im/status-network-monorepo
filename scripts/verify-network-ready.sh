#!/bin/bash

# Script to verify network readiness before contract deployment
# Usage: ./verify-network-ready.sh

set -e

echo "🔍 Verifying network readiness..."

# Function to check RPC endpoint
check_rpc() {
    local rpc_url=$1
    local network_name=$2
    local max_attempts=60  # ~5 min wait (Besu + JVM + plugin init takes 3-4 min)
    local attempt=1
    
    echo "📡 Checking $network_name at $rpc_url..."
    
    while [ $attempt -le $max_attempts ]; do
        # Check for valid JSON-RPC response (not timeout/gateway errors)
        local response
        response=$(curl -s -m 5 -X POST -H "Content-Type: application/json" \
           --data '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}' \
           "$rpc_url" 2>&1) || true
        
        # Use [[ ]] which doesn't trigger set -e on pattern mismatch
        if [[ "$response" == *'"jsonrpc"'* ]]; then
            echo "✅ $network_name is responsive"
            return 0
        fi
        
        echo "⏳ $network_name not ready (attempt $attempt/$max_attempts)..."
        sleep 5
        attempt=$((attempt + 1))
    done
    
    echo "❌ $network_name failed to respond after $max_attempts attempts"
    return 1
}

# Function to get chain ID
get_chain_id() {
    local rpc_url=$1
    curl -s -X POST -H "Content-Type: application/json" \
         --data '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}' \
         "$rpc_url" | jq -r '.result // "unknown"' 2>/dev/null
}

# Function to get block number
get_block_number() {
    local rpc_url=$1
    curl -s -X POST -H "Content-Type: application/json" \
         --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
         "$rpc_url" | jq -r '.result // "unknown"' 2>/dev/null
}

# Check L1 node (port 8445)
check_rpc "http://localhost:8445" "L1 Node"
L1_CHAIN_ID=$(get_chain_id "http://localhost:8445")
L1_BLOCK=$(get_block_number "http://localhost:8445")

# Check L2 sequencer (port 8545)  
check_rpc "http://localhost:8545" "L2 Sequencer"
L2_CHAIN_ID=$(get_chain_id "http://localhost:8545")
L2_BLOCK=$(get_block_number "http://localhost:8545")

# Check L2 RPC node (port 9045) - used for contract deployment
# This node may take longer to start as it needs to sync with sequencer
check_rpc "http://localhost:9045" "L2 RPC Node"

L2_RPC_CHAIN_ID=$(get_chain_id "http://localhost:9045")
L2_RPC_BLOCK=$(get_block_number "http://localhost:9045")

echo ""
echo "📊 Network Status Summary:"
echo "   🔗 L1 Node: Chain ID $L1_CHAIN_ID, Block $(printf "%d" $L1_BLOCK 2>/dev/null || echo "unknown")"
echo "   🔗 L2 Sequencer: Chain ID $L2_CHAIN_ID, Block $(printf "%d" $L2_BLOCK 2>/dev/null || echo "unknown")"
echo "   🔗 L2 RPC Node: Chain ID $L2_RPC_CHAIN_ID, Block $(printf "%d" $L2_RPC_BLOCK 2>/dev/null || echo "unknown")"

# Verify L2 is producing blocks
echo ""
echo "🔍 Verifying L2 block production..."
INITIAL_BLOCK=$(printf "%d" $L2_BLOCK 2>/dev/null || echo "0")
sleep 3
NEW_L2_BLOCK=$(get_block_number "http://localhost:8545")
NEW_BLOCK_NUM=$(printf "%d" $NEW_L2_BLOCK 2>/dev/null || echo "0")

if [ "$NEW_BLOCK_NUM" -gt "$INITIAL_BLOCK" ]; then
    echo "✅ L2 is actively producing blocks (advanced from $INITIAL_BLOCK to $NEW_BLOCK_NUM)"
else
    echo "⚠️  L2 block production may be stalled (block number unchanged: $INITIAL_BLOCK)"
fi

echo ""
echo "✅ Network readiness verification completed!"
echo "🚀 Ready for contract deployment!"