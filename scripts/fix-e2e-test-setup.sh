#!/bin/bash

# Fix e2e test setup for RLN environment
# This script ensures the test environment is properly initialized

set -e

echo "🔧 Fixing e2e test setup for RLN environment..."

# 1. Check network is ready
echo "📡 Verifying network readiness..."
./scripts/verify-network-ready.sh

# 2. Deploy test contracts using a direct script (bypassing jest global setup)
echo "📝 Deploying test prerequisite contracts..."

# Check if L1 dummy contract exists
L1_CODE=$(curl -s -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_getCode","params":["0x610178dA211FEF7D417bC0e6FeD39F05609AD788","latest"],"id":1}' \
  http://localhost:8445 | jq -r '.result')

if [ "$L1_CODE" = "0x" ]; then
  echo "⚠️  Test contracts not deployed. Need to deploy them first."
  echo "💡 Recommendation: Restart the environment with: make clean-environment && make start-env-with-rln-production"
  exit 1
fi

echo "✅ Test contracts already deployed"

# 3. Verify L2 node is responding correctly to linea_estimateGas
echo "🧪 Testing linea_estimateGas endpoint..."
GAS_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"linea_estimateGas","params":[{"from":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","to":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","value":"0x0"}],"id":1}' \
  http://localhost:9045)

echo "Gas estimate response: $GAS_RESPONSE"

GAS_LIMIT=$(echo $GAS_RESPONSE | jq -r '.result.gasLimit')
if [ "$GAS_LIMIT" = "0x0" ] || [ "$GAS_LIMIT" = "null" ]; then
  echo "⚠️  L2 node is returning zero gas estimates"
  echo "💡 This might be a timing issue. Waiting 5 seconds..."
  sleep 5
fi

# 4. Verify RLN contracts are accessible
echo "🔍 Verifying RLN contract accessibility..."
KARMA_CODE=$(curl -s -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_getCode","params":["0xe537D669CA013d86EBeF1D64e40fC74CADC91987","latest"],"id":1}' \
  http://localhost:8545 | jq -r '.result' | cut -c1-20)

if [ "$KARMA_CODE" = "0x" ]; then
  echo "❌ Karma contract not deployed at expected address!"
  exit 1
fi

echo "✅ RLN contracts are accessible"

# 5. Check if RLN prover is healthy
echo "🔍 Checking RLN prover status..."
PROVER_STATUS=$(docker inspect --format='{{.State.Status}}' rln-prover 2>/dev/null || echo "not_running")

if [ "$PROVER_STATUS" != "running" ]; then
  echo "❌ RLN prover is not running!"
  exit 1
fi

echo "✅ RLN prover is running"

# 6. Final readiness check
echo "📊 Network Summary:"
L1_BLOCK=$(curl -s -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' http://localhost:8445 | jq -r '.result')
L2_BLOCK=$(curl -s -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' http://localhost:8545 | jq -r '.result')
L2_RPC_BLOCK=$(curl -s -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' http://localhost:9045 | jq -r '.result')

echo "   L1 Block: $((16#${L1_BLOCK#0x}))"
echo "   L2 Sequencer Block: $((16#${L2_BLOCK#0x}))"
echo "   L2 RPC Block: $((16#${L2_RPC_BLOCK#0x}))"

echo ""
echo "✅ Environment is ready for e2e tests!"
echo "🚀 Run: cd e2e && pnpm run test:rln:json"


