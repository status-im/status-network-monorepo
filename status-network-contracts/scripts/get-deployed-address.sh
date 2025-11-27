#!/bin/bash

# Script to extract deployed contract addresses from forge broadcast files
# Usage: ./get-deployed-address.sh <script-name> <contract-name> [chain-id]
#
# For UUPS proxy contracts (Karma, RLN, StakeManager), this returns the PROXY address
# which is what users interact with, not the implementation address.

SCRIPT_NAME=$1
CONTRACT_NAME=$2
CHAIN_ID=${3:-1337}

if [ -z "$SCRIPT_NAME" ] || [ -z "$CONTRACT_NAME" ]; then
    echo "Usage: $0 <script-name> <contract-name> [chain-id]"
    echo "Example: $0 DeployKarma.s.sol Karma 1337"
    exit 1
fi

BROADCAST_FILE="broadcast/${SCRIPT_NAME}/${CHAIN_ID}/run-latest.json"

if [ ! -f "$BROADCAST_FILE" ]; then
    echo "Error: Broadcast file not found: $BROADCAST_FILE"
    exit 1
fi

# Contracts that use UUPS proxy pattern - we need to return the proxy address
PROXY_CONTRACTS="Karma RLN StakeManager"

# Check if this is a proxy contract
IS_PROXY=false
for pc in $PROXY_CONTRACTS; do
    if [ "$CONTRACT_NAME" = "$pc" ]; then
        IS_PROXY=true
        break
    fi
done

if [ "$IS_PROXY" = true ]; then
    # For proxy contracts, find the ERC1967Proxy address
    # The proxy is deployed after the implementation, so we look for ERC1967Proxy
    ADDRESS=$(cat "$BROADCAST_FILE" | jq -r '.transactions[] | select(.contractName == "ERC1967Proxy") | .contractAddress' 2>/dev/null | head -1)
    
    if [ -z "$ADDRESS" ] || [ "$ADDRESS" = "null" ]; then
        # Fallback: try to find any proxy contract
        ADDRESS=$(cat "$BROADCAST_FILE" | jq -r '.transactions[] | select(.contractName | test("Proxy")) | .contractAddress' 2>/dev/null | head -1)
    fi
else
    # For non-proxy contracts, return the direct contract address
    ADDRESS=$(cat "$BROADCAST_FILE" | jq -r --arg contract "$CONTRACT_NAME" '.transactions[] | select(.contractName == $contract) | .contractAddress' 2>/dev/null | head -1)
fi

if [ -z "$ADDRESS" ] || [ "$ADDRESS" = "null" ]; then
    echo "Error: Contract $CONTRACT_NAME not found in $BROADCAST_FILE"
    exit 1
fi

echo "$ADDRESS"
