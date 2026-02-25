# Public Testnet Contract Deployment Guide

Deployment instructions for Status Network contracts on the public testnet (L2 chain ID 374, L1 Hoodi).

- **Deploy on L1 or L2?** All Status Network contracts deploy on **L2** (chain ID 374).
- **Deploy before or after RLN prover?** Deploy contracts **before** starting the RLN prover.
- **Do I need ETH on the deployer?** Yes. Contract deployment needs gas even though the network is gasless for end users
  since RLN prover isn't online yet. Budget ~0.5 ETH.
- **Will the addresses match local dev?** No. Addresses depend on the deployer address and nonce.

## Prerequisites

1. **Foundry** installed (`forge`, `cast`):

   ```sh
   curl -L https://foundry.paradigm.xyz | bash
   foundryup
   ```

2. A **funded deployer account** on L2 (chain ID 374) with ~0.5-1 ETH.

3. The **L2 RPC URL**.

## Step 1: Install Contract Dependencies

```sh
cd status-network-contracts
rm -f foundry.lock

forge install OpenZeppelin/openzeppelin-contracts@v4.8.3 --no-commit
forge install foundry-rs/forge-std@v1.8.2 --no-commit
forge install nomad-xyz/ExcessivelySafeCall --no-commit
```

## Step 2: Deploy All Contracts

Set your environment:

```sh
cd status-network-contracts
export L2_RPC_URL="<your L2 RPC URL>"
export DEPLOYER_KEY="<your private key>"
export ETH_FROM="<your deployer address>"
```

### 2a. Deploy Protocol Contracts

This deploys Karma, MetadataGenerator, KarmaNFT, StakeManager, VaultFactory, SimpleKarmaDistributor, and KarmaTiers in
one batch. It does **not** deploy RLN (that's next).

```sh
MNEMONIC="<your mnemonic>" \
MAX_VAULTS_PER_USER=5 \
  forge script script/DeployProtocol.s.sol \
  --rpc-url $L2_RPC_URL \
  --broadcast
```

Save the Karma proxy address from the output.

### 2b. Deploy RLN

```sh
MNEMONIC="<your mnemonic>" \
KARMA_ADDRESS=<karma_proxy_address_from_step_2a> \
DEPTH=20 \
  forge script script/RLN.s.sol:DeployRLNScript \
  --rpc-url $L2_RPC_URL \
  --broadcast
```

### Extract Deployed Addresses

```sh
echo "KarmaTiers:   $(./scripts/get-deployed-address.sh DeployKarmaTiers.s.sol KarmaTiers)"
echo "Karma:        $(./scripts/get-deployed-address.sh DeployKarma.s.sol Karma)"
echo "StakeManager: $(./scripts/get-deployed-address.sh DeployStakeManager.s.sol StakeManager)"
echo "RLN:          $(./scripts/get-deployed-address.sh RLN.s.sol RLN)"
echo "KarmaNFT:     $(./scripts/get-deployed-address.sh DeployKarmaNFT.s.sol KarmaNFT)"
```

If a step fails and you need to deploy individual contracts, see the individual scripts in `script/` — deploy order is:
KarmaTiers, Karma, StakeManager, RLN, KarmaNFT.

## Step 3: Initialize Karma Tiers

```sh
KARMA_TIERS_ADDRESS=<deployed_karma_tiers_address> \
MNEMONIC="<your mnemonic>" \
  forge script script/InitializeKarmaTiers.s.sol \
  --rpc-url $L2_RPC_URL \
  --broadcast
```

This writes the following tier configuration on-chain (defined in `script/InitializeKarmaTiers.s.sol`):

| Tier            | Min Karma  | Gasless TX / Epoch |
| --------------- | ---------- | ------------------ |
| none            | 0          | 0                  |
| entry           | 1          | 2                  |
| newbie          | >1         | 6                  |
| basic           | 50         | 16                 |
| active          | 500        | 96                 |
| regular         | 5,000      | 480                |
| power           | 20,000     | 960                |
| pro             | 100,000    | 10,080             |
| high-throughput | 500,000    | 108,000            |
| s-tier          | 5,000,000  | 240,000            |
| legendary       | 10,000,000 | 480,000            |

All karma values are in whole tokens (18 decimals, 1 Karma = 1e18 wei).

## Step 4: Grant OPERATOR_ROLE on Karma

The deployer needs `OPERATOR_ROLE` to mint karma to users:

```sh
cast send $KARMA_ADDRESS "grantRole(bytes32,address)" \
  $(cast call $KARMA_ADDRESS "OPERATOR_ROLE()(bytes32)" --rpc-url $L2_RPC_URL) \
  $ETH_FROM \
  --rpc-url $L2_RPC_URL \
  --private-key $DEPLOYER_KEY
```

## Step 5: Setup RLN Prover Account

The RLN prover needs its own account with `REGISTER_ROLE` on the RLN contract. Generate a prover private key, then:

```sh
PROVER_ADDRESS=<address derived from prover private key>

# Grant REGISTER_ROLE
cast send $RLN_ADDRESS "grantRole(bytes32,address)" \
  $(cast call $RLN_ADDRESS "REGISTER_ROLE()(bytes32)" --rpc-url $L2_RPC_URL) \
  $PROVER_ADDRESS \
  --rpc-url $L2_RPC_URL \
  --private-key $DEPLOYER_KEY

# Fund the prover account (needs gas for register() calls)
cast send $PROVER_ADDRESS --value 1ether \
  --rpc-url $L2_RPC_URL \
  --private-key $DEPLOYER_KEY
```

## Step 6: Configure Services with New Addresses

### RLN Prover

- `--ksc` = Karma proxy address (Helm: `l2.rlnProver.karmaContract`)
- `--rlnsc` = RLN proxy address (Helm: `l2.rlnProver.rlnContract`)
- `--tsc` = KarmaTiers address (Helm: `l2.rlnProver.tiersContract`)
- `PRIVATE_KEY` env var = Prover private key with REGISTER_ROLE (Helm: `l2.rlnProver.privateKey`)
- `--ws-rpc-url` = WebSocket endpoint of the sequencer

## Contract Verification

See `status-network-contracts/docs/deployment.md` for full verification instructions using Blockscout.
