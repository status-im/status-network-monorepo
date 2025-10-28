# Deployment Guide

## Table Of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Deploying the Full Protocol](#deploying-the-full-protocol)
  - [Using DeployProtocol Script (Recommended)](#using-deployprotocol-script-recommended)
  - [Network Configuration](#network-configuration)
- [Individual Deployment Scripts](#individual-deployment-scripts)

## Overview

This guide covers the deployment of the Status Network protocol contracts. The recommended approach is to use the
`DeployProtocol.s.sol` script which deploys all contracts in the correct order and handles initialization automatically.

## Prerequisites

Before deploying, ensure you have:

- A funded account with the deployment private key or mnemonic
- RPC URL for the target network
- (Optional) Block explorer API details for contract verification

## Deploying the Full Protocol

### Using DeployProtocol Script (Recommended)

The `DeployProtocol.s.sol` script deploys the entire protocol stack including:

- **Karma** (upgradeable proxy + implementation)
- **NFTMetadataGeneratorSVG**
- **KarmaNFT**
- **StakeManager** (upgradeable proxy + implementation)
- **VaultFactory** with **StakeVault** implementation

The script automatically handles:

- Deploying all contracts in the correct dependency order
- Initializing contracts with proper configurations
- Setting up reward distributors and suppliers
- Whitelisting StakeVault implementation codehash

#### Deployment Command

```sh
MNEMONIC=$YOUR_MNEMONIC forge script script/DeployProtocol.s.sol --rpc-url $RPC_URL --broadcast
```

The script will output all deployed contract addresses at the end:

```
Contract addresses:
0x... : Karma (proxy)
0x... : Karma (implementation)
0x... : NFTMetadataGeneratorSVG
0x... : KarmaNFT
0x... : StakeManager (proxy)
0x... : StakeManager (implementation)
0x... : VaultFactory
0x... : StakeVault (implementation)
0x... : StakeVault (proxy clone)
```

### Network Configuration

The deployment script uses `DeploymentConfig.s.sol` to automatically select the correct staking token based on the chain
ID:

- **Anvil/Local** (chain ID 31337, 1337): Deploys a mock token
- **Sepolia** (chain ID 11155111): Uses SNT at `0xE452027cdEF746c7Cd3DB31CB700428b16cD8E51`
- **Status Network Sepolia** (chain ID 1660990954): Uses SNT at `0x1C3Ac2a186c6149Ae7Cb4D716eBbD0766E4f898a`

## Individual Deployment Scripts

If the full protocol deployment fails or you need to deploy/redeploy individual components, use the individual
deployment scripts below.

### Deploy Karma

```sh
MNEMONIC=$YOUR_MNEMONIC forge script script/DeployKarma.s.sol --rpc-url $RPC_URL --broadcast
```

### Deploy MetadataGenerator

```sh
MNEMONIC=$YOUR_MNEMONIC forge script script/DeployMetadataGenerator.s.sol --rpc-url $RPC_URL --broadcast
```

### Deploy KarmaNFT

Requires the Karma contract and MetadataGenerator to be deployed first.

```sh
MNEMONIC=$YOUR_MNEMONIC \
KARMA_ADDRESS=$KARMA_CONTRACT_ADDRESS \
METADATA_GENERATOR_ADDRESS=$METADATA_GENERATOR_ADDRESS \
forge script script/DeployKarmaNFT.s.sol --rpc-url $RPC_URL --broadcast
```

### Deploy StakeManager

Requires the Karma contract to be deployed first.

```sh
MNEMONIC=$YOUR_MNEMONIC \
KARMA_ADDRESS=$KARMA_CONTRACT_ADDRESS \
forge script script/DeployStakeManager.s.sol --rpc-url $RPC_URL --broadcast
```

### Deploy VaultFactory

Requires the StakeManager to be deployed first.

```sh
MNEMONIC=$YOUR_MNEMONIC \
STAKE_MANAGER_ADDRESS=$STAKE_MANAGER_CONTRACT_ADDRESS \
forge script script/DeployVaultFactory.s.sol --rpc-url $RPC_URL --broadcast
```

### Manual Initialization (if needed)

If you deploy contracts individually, you'll need to manually initialize them:

1. Add StakeManager as reward distributor in Karma:

```sh
cast send $KARMA_PROXY_ADDRESS "addRewardDistributor(address)" $STAKE_MANAGER_PROXY_ADDRESS \
  --rpc-url $RPC_URL --mnemonic $YOUR_MNEMONIC
```

2. Whitelist StakeManager for transfers in Karma:

```sh
cast send $KARMA_PROXY_ADDRESS "setAllowedToTransfer(address,bool)" $STAKE_MANAGER_PROXY_ADDRESS true \
  --rpc-url $RPC_URL --mnemonic $YOUR_MNEMONIC
```

3. Set Karma as rewards supplier in StakeManager:

```sh
cast send $STAKE_MANAGER_PROXY_ADDRESS "setRewardsSupplier(address)" $KARMA_PROXY_ADDRESS \
  --rpc-url $RPC_URL --mnemonic $YOUR_MNEMONIC
```

4. Whitelist StakeVault proxy clone codehash in StakeManager:

```sh
# Get the vault proxy clone codehash
VAULT_PROXY_CLONE_CODEHASH=$(cast keccak $(cast code $VAULT_PROXY_CLONE_ADDRESS))

# Whitelist it
cast send $STAKE_MANAGER_PROXY_ADDRESS "setTrustedCodehash(bytes32,bool)" $VAULT_PROXY_CLONE_CODEHASH true \
  --rpc-url $RPC_URL --mnemonic $YOUR_MNEMONIC
```

## Contract Verification

After deployment, verify your contracts on the block explorer.

### Environment Variables Setup

Set up commonly used variables:

```sh
export RPC_URL="your_rpc_url"
export VERIFIER_URL="your_block_explorer_api_url"  # e.g., for Blockscout
export COMPILER_VERSION="v0.8.26+commit.8a97fa7a"  # Check your project's Solidity version
export CHAIN_ID="your_chain_id"  # e.g., 1660990954 for Status Network Sepolia
```

### Verify Karma Implementation

```sh
forge verify-contract \
  --chain-id $CHAIN_ID \
  --num-of-optimizations 10000 \
  --watch \
  --compiler-version $COMPILER_VERSION \
  $KARMA_IMPL_ADDRESS \
  src/Karma.sol:Karma \
  --rpc-url $RPC_URL \
  --verifier-url $VERIFIER_URL \
  --verifier blockscout
```

### Verify Karma Proxy

First, get the implementation address from the proxy:

```sh
KARMA_IMPL_ADDRESS=$(cast storage $KARMA_PROXY_ADDRESS 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc --rpc-url $RPC_URL)
```

Then encode the initialization data:

```sh
INIT_DATA=$(cast calldata "initialize(address)" $DEPLOYER_ADDRESS)
```

Finally, verify the proxy:

```sh
forge verify-contract \
  --chain-id $CHAIN_ID \
  --num-of-optimizations 10000 \
  --watch \
  --compiler-version $COMPILER_VERSION \
  $KARMA_PROXY_ADDRESS \
  lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol:ERC1967Proxy \
  --rpc-url $RPC_URL \
  --verifier-url $VERIFIER_URL \
  --verifier blockscout \
  --constructor-args $(cast abi-encode "constructor(address,bytes)" $KARMA_IMPL_ADDRESS $INIT_DATA)
```

### Verify NFTMetadataGeneratorSVG

```sh
forge verify-contract \
  --chain-id $CHAIN_ID \
  --num-of-optimizations 10000 \
  --watch \
  --compiler-version $COMPILER_VERSION \
  $METADATA_GENERATOR_ADDRESS \
  src/NFTMetadataGeneratorSVG.sol:NFTMetadataGeneratorSVG \
  --rpc-url $RPC_URL \
  --verifier-url $VERIFIER_URL \
  --verifier blockscout
```

### Verify KarmaNFT

```sh
forge verify-contract \
  --chain-id $CHAIN_ID \
  --num-of-optimizations 10000 \
  --watch \
  --compiler-version $COMPILER_VERSION \
  $KARMA_NFT_ADDRESS \
  src/KarmaNFT.sol:KarmaNFT \
  --rpc-url $RPC_URL \
  --verifier-url $VERIFIER_URL \
  --verifier blockscout \
  --constructor-args $(cast abi-encode "constructor(address,address)" $METADATA_GENERATOR_ADDRESS $KARMA_PROXY_ADDRESS)
```

### Verify StakeManager Implementation

```sh
forge verify-contract \
  --chain-id $CHAIN_ID \
  --num-of-optimizations 10000 \
  --watch \
  --compiler-version $COMPILER_VERSION \
  $STAKE_MANAGER_IMPL_ADDRESS \
  src/StakeManager.sol:StakeManager \
  --rpc-url $RPC_URL \
  --verifier-url $VERIFIER_URL \
  --verifier blockscout
```

### Verify StakeManager Proxy

First, get the implementation address:

```sh
STAKE_MANAGER_IMPL_ADDRESS=$(cast storage $STAKE_MANAGER_PROXY_ADDRESS 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc --rpc-url $RPC_URL)
```

Then encode the initialization data:

```sh
INIT_DATA=$(cast calldata "initialize(address,address,address)" $DEPLOYER_ADDRESS $STAKING_TOKEN_ADDRESS $KARMA_PROXY_ADDRESS)
```

Finally, verify the proxy:

```sh
forge verify-contract \
  --chain-id $CHAIN_ID \
  --num-of-optimizations 10000 \
  --watch \
  --compiler-version $COMPILER_VERSION \
  $STAKE_MANAGER_PROXY_ADDRESS \
  lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol:ERC1967Proxy \
  --rpc-url $RPC_URL \
  --verifier-url $VERIFIER_URL \
  --verifier blockscout \
  --constructor-args $(cast abi-encode "constructor(address,bytes)" $STAKE_MANAGER_IMPL_ADDRESS $INIT_DATA)
```

### Verify VaultFactory

First, get the StakeVault implementation address:

```sh
VAULT_IMPL_ADDRESS=$(cast call $VAULT_FACTORY_ADDRESS "vaultImplementation()(address)" --rpc-url $RPC_URL)
```

Then verify:

```sh
forge verify-contract \
  --chain-id $CHAIN_ID \
  --num-of-optimizations 10000 \
  --watch \
  --compiler-version $COMPILER_VERSION \
  $VAULT_FACTORY_ADDRESS \
  src/VaultFactory.sol:VaultFactory \
  --rpc-url $RPC_URL \
  --verifier-url $VERIFIER_URL \
  --verifier blockscout \
  --constructor-args $(cast abi-encode "constructor(address,address,address)" $DEPLOYER_ADDRESS $STAKE_MANAGER_PROXY_ADDRESS $VAULT_IMPL_ADDRESS)
```

### Verify StakeVault Implementation

```sh
forge verify-contract \
  --chain-id $CHAIN_ID \
  --num-of-optimizations 10000 \
  --watch \
  --compiler-version $COMPILER_VERSION \
  $VAULT_IMPL_ADDRESS \
  src/StakeVault.sol:StakeVault \
  --rpc-url $RPC_URL \
  --verifier-url $VERIFIER_URL \
  --verifier blockscout
```

### Verify StakeVault Proxy Clone

```sh
forge verify-contract \
  --chain-id $CHAIN_ID \
  --num-of-optimizations 10000 \
  --watch \
  --compiler-version $COMPILER_VERSION \
  $VAULT_PROXY_CLONE_ADDRESS \
  lib/openzeppelin-contracts/contracts/proxy/Clones.sol:Clones \
  --rpc-url $RPC_URL \
  --verifier-url $VERIFIER_URL \
  --verifier blockscout
```
