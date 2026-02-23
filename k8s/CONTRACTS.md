# Status Network - Contract Addresses & Deployment Reference

This document catalogs every smart contract deployed across the L1 and L2 layers of the Status Network.

## Table of Contents

- [Deployment Modes](#deployment-modes)
- [L1 Contracts (Linea Protocol)](#l1-contracts-linea-protocol)
- [L2 System Contracts (Genesis Pre-deploys)](#l2-system-contracts-genesis-pre-deploys)
- [L2 Linea Protocol Contracts](#l2-linea-protocol-contracts)
- [L2 Status Network Contracts](#l2-status-network-contracts)
- [Helm Values Reference](#helm-values-reference)
- [Deployer Accounts](#deployer-accounts)
- [Operator & Signer Accounts](#operator--signer-accounts)
- [Deployment Flow](#deployment-flow)

---

## Deployment Modes

The network supports two L1 modes configured via `network.l1Network`:

| Mode | L1 Chain ID | L1 Nodes | Description |
|------|-------------|----------|-------------|
| `local` | 1337 | Besu + Teku (in-cluster) | Private L1 chain with custom genesis |
| `hoodi` | 560048 | **None** (external RPC) | Hoodi Ethereum testnet via `network.l1RpcEndpoint` |

L2 Chain ID: **59141** (both modes)

In Hoodi mode, `l1.enabled` is set to `false` - no L1 nodes, PVCs, services, or secrets are created. All L2 services connect to L1 via the external RPC endpoint. The `wait-for-l1` init containers are skipped when `network.l1RpcEndpoint` is set.

---

## L1 Contracts (Linea Protocol)

These contracts are deployed on L1 and manage the rollup protocol (blob submission, proof verification, message anchoring).

### Local Mode (Default Hardhat Addresses)

| Contract | Address | Deploy Target | Description |
|----------|---------|---------------|-------------|
| **LineaRollup (v6)** | `0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9` | `deploy-linea-rollup` | Main rollup contract: blob submission, proof finalization, L1->L2 messaging |
| **PlonkVerifier** | *(deployed with LineaRollup)* | `deploy-linea-rollup` | ZK proof verification (IntegrationTestTrueVerifier for testnet) |
| **TokenBridge (L1)** | `0x959922bE3CAee4b8Cd9a407cc3ac1C251C2007B1` | `deploy-token-bridge-l1` | L1 side of the canonical token bridge |
| **TestERC20 (L1)** | *(varies per deployment)* | `deploy-l1-test-erc20` | Test token for bridge testing |

Address files: `contracts/local-deployments-artifacts/L1RollupAddress.txt`, `TokenBridgeL1Address.txt`

### Hoodi Mode (Deployed 2026-02-21)

| Contract | Address | Block | Description |
|----------|---------|-------|-------------|
| **IntegrationTestTrueVerifier** | `0x0213ad0b1E5b942c684035d2Ec3979ea4e4f61D4` | 2275678 | ZK proof verification (test verifier) |
| **LineaRollupV6 Implementation** | `0x83A709BbE73C2a26fd38e3a1daE3Bb8a3AF1329d` | 2275680 | Implementation contract |
| **ProxyAdmin** | `0x1A89cB95F3aD4AE20abEDB87800436fddaEF4d52` | 2275683 | Proxy admin |
| **LineaRollupV6 (Proxy)** | `0x64269d08c795d66A542903AA35B6e775B031872A` | 2275685 | Main rollup contract |
| **TokenBridge (L1)** | *Not deployed* | - | Not needed for initial testnet |

Deployer: `0x8E5bA9C1DF138754076FAfaC0DeeDAf3d598ed35` (Hoodi chain ID 560048)

---

## L2 System Contracts (Genesis Pre-deploys)

These contracts are baked into the L2 genesis block (`docker/config/l2-genesis-initialization/genesis-besu.json`). They exist at block 0 and do not require deployment.

| Address | Contract | Purpose |
|---------|----------|---------|
| `0x3a69eD7FA6f6CA1dB2649327c4A2E666130823bE` | Withdrawal Request Queue (ERC1967Proxy) | EIP-6110 withdrawal request processing |
| `0x9AFC00F4CEaadb97A3822FCA2225FaD78839FBde` | Consolidation Request Queue (ERC1967Proxy) | EIP-6110 consolidation request processing |
| `0x0000F90827F1C53a10cb7A02335B175320002935` | MCOPY Precompile | EIP-5656 memory copy opcode support |
| `0x000F3df6D732807Ef1319fB7B8bB8522d0Beac02` | BLOCKHASH Precompile | EIP-2935 historical block hash access |

Additionally, 22 test EOAs are pre-funded with ETH in genesis for development purposes.

---

## L2 Linea Protocol Contracts

These contracts are deployed on L2 after genesis via Hardhat scripts. They handle L2->L1 messaging and the L2 side of the token bridge.

### Local Mode

| Contract | Address | Deploy Target | Description |
|----------|---------|---------------|-------------|
| **L2MessageService** | `0x5767aB2Ed64666bFE27e52D4675EDd60Ec7D6EDF` | `deploy-l2messageservice` | L2->L1 message dispatching, L1->L2 message claiming |
| **TokenBridge (L2)** | `0x438d5c7da79D918a26aD012c617066293f949D27` | `deploy-token-bridge-l2` | L2 side of the canonical token bridge |
| **Upgradeable Predeploys** | *(varies)* | `deploy-upgradeable-predeploys` | Upgradeable wrappers for EIP-6110 system contracts |
| **EIP System Contracts** | *(varies)* | `deploy-eip-system-contracts` | EIP-required system contracts |
| **TestERC20 (L2)** | *(varies per deployment)* | `deploy-l2-test-erc20` | Test token for bridge testing |

Address files: `contracts/local-deployments-artifacts/L2MessageServiceAddress.txt`, `TokenBridgeL2Address.txt`

> **Note:** The L2MessageService address used in Helm configs (`0xe537D669CA013d86EBeF1D64e40fC74CADC91987`) is the *default values.yaml* value. The actual deployed address from the last local deployment is `0x5767aB2Ed64666bFE27e52D4675EDd60Ec7D6EDF`. These diverge because the Helm defaults use deterministic Hardhat addresses while actual deployments depend on deployer nonce.

### Internal Testnet (Deployed 2026-02-21)

| Contract | Address | Block | Description |
|----------|---------|-------|-------------|
| **L2MessageService** | `0x2f6dAaF8A81AB675fbD37Ca6Ed5b72cf86237453` | 27745 | L2->L1 message dispatching, L1->L2 message claiming |

---

## L2 Status Network Contracts

These contracts are deployed on L2 via Foundry (forge scripts). They implement the Status Network protocol: reputation (Karma), staking, rate limiting (RLN), and NFTs.

### Local Deployment Addresses

| Contract | Address | Deploy Script | Description |
|----------|---------|---------------|-------------|
| **Karma** | `0xe537d669ca013d86ebef1d64e40fc74cadc91987` | `DeployKarma.s.sol` | ERC20 reputation token (upgradeable proxy) |
| **KarmaTiers** | `0x729409fad88cafda895e41f9ed00ef4094f8d130` | `DeployKarmaTiers.s.sol` | Tier management for Karma holders |
| **StakeManager** | `0xeb0b0a14f92e3ba35aef3a2b6a24d7ed1d11631b` | `DeployStakeManager.s.sol` | Manages user staking and creates StakeVault clones |
| **RLN** | `0x5c95bcd50e6d1b4e3cdc478484c9030ff0a7d493` | `RLN.s.sol` | Rate Limiting Nullifier for gasless transactions |
| **KarmaNFT** | `0xcc1b08b17301e090cbb4c1f5598cbaa096d591fb` | `DeployKarmaNFT.s.sol` | NFT representation of Karma holdings |
| **VaultFactory** | *(deployed with StakeManager)* | `DeployVaultFactory.s.sol` | Creates StakeVault proxy clones per user |
| **PoseidonHasher** | *(deployed with RLN)* | `RLN.s.sol` | Cryptographic hasher library for RLN proofs |

### Internal Testnet Addresses (Deployed 2026-02-21)

| Contract | Address | Deploy Script | Description |
|----------|---------|---------------|-------------|
| **KarmaTiers** | `0xeB0b0a14F92e3BA35aEF3a2B6A24D7ED1D11631B` | `DeployKarmaTiers.s.sol` | Tier management (11 tiers initialized) |
| **Karma (proxy)** | `0x9145615d34Afba9F8ECB4e2384325646f2393dde` | `DeployKarma.s.sol` | ERC20 reputation token |
| **StakeManager (proxy)** | `0xCC1B08B17301e090cbb4c1F5598Cbaa096d591FB` | `DeployStakeManager.s.sol` | Staking management |
| **RLN (proxy)** | `0xFCc2155b495B6Bf6701eb322D3a97b7817898306` | `RLN.s.sol` | Rate Limiting Nullifier |
| **MetadataGenerator** | `0x7917AbB0cDbf3D3C4057d6a2808eE85ec16260C1` | `DeployMetadataGenerator.s.sol` | NFT SVG metadata |
| **KarmaNFT** | `0x670365526A9971E4A225c38538c5D7Ac248e4087` | `DeployKarmaNFT.s.sol` | NFT representation |
| **SimpleKarmaDistributor (proxy)** | `0xc407C7Bc2b3C109b8bCDE7C681d84a6a4B600eA5` | `DeploySimpleKarmaDistributor.s.sol` | Karma distribution |
| **VaultFactory** | `0x94816619EA798768f227DdA95aB2f95d8de93389` | `DeployVaultFactory.s.sol` | Vault proxy clones |

Deployer: `0x1B9AbEeC3215D8AdE8A33607f2cF0f4F60e5F0D0` (L2 chain ID 59141)

Address files: `status-network-contracts/deployments/` (`karma_address.txt`, `karma_tiers_address.txt`, `rln_address.txt`, `stake_manager_address.txt`, `karma_nft_address.txt`)

### Deployment Order

The contracts must be deployed in this order due to dependencies:

1. **KarmaTiers** (no dependencies)
2. **Karma** (no dependencies)
3. **StakeManager** (depends on Karma address)
4. **RLN** (depends on Karma address)
5. **KarmaNFT** (depends on Karma address)

---

## Helm Values Reference

These Helm values control which contract addresses are injected into service configurations.

### `values.yaml` - Network Contract Addresses

```yaml
network:
  contracts:
    l1Rollup: "0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9"      # LineaRollup on L1
    l2MessageService: "0xe537D669CA013d86EBeF1D64e40fC74CADC91987"  # L2MessageService on L2
```

### Where These Addresses Are Used

| Service | Config Key / Env Var | Which Address |
|---------|---------------------|---------------|
| **Coordinator** | `[protocol.l1] contract-address` | `network.contracts.l1Rollup` |
| **Coordinator** | `[protocol.l2] contract-address` | `network.contracts.l2MessageService` |
| **Coordinator** | `[defaults] l1-endpoint` | `network.l1RpcEndpoint` or internal L1 |
| **Postman** | `L1_CONTRACT_ADDRESS` | `network.contracts.l1Rollup` |
| **Postman** | `L2_CONTRACT_ADDRESS` | `network.contracts.l2MessageService` |
| **Postman** | `L1_RPC_URL` | `network.l1RpcEndpoint` or internal L1 |
| **Maru** | `[linea] contract-address` | `network.contracts.l1Rollup` |
| **Maru** | `[linea] l1-eth-api` | `network.l1RpcEndpoint` or internal L1 |
| **Sequencer** | `plugin-linea-l1l2-bridge-contract` | `network.contracts.l2MessageService` |
| **L2 Node Besu** | `plugin-linea-l1l2-bridge-contract` | `network.contracts.l2MessageService` |
| **Traces Node** | `plugin-linea-l1l2-bridge-contract` | `network.contracts.l2MessageService` |

### RLN-Specific Addresses (values.yaml)

```yaml
rln:
  contractAddress: "0x9A9f2CCfdE556A7E9Ff0848998Aa4a0CFD8863AE"
```

### RLN Prover Addresses (values-internal-testnet.yaml)

```yaml
l2:
  rlnProver:
    karmaContract: "0x9145615d34Afba9F8ECB4e2384325646f2393dde"
    rlnContract: "0xFCc2155b495B6Bf6701eb322D3a97b7817898306"
    tiersContract: "0xeB0b0a14F92e3BA35aEF3a2B6A24D7ED1D11631B"
```

---

## Deployer Accounts

### L1 Deployers

| Account | Address | Source | Used For |
|---------|---------|--------|----------|
| L1 Deployer (Hardhat #0) | `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` | Well-known Hardhat test key | LineaRollup, TokenBridge L1, TestERC20 L1 |

### L2 Deployers

| Account | Address | Source | Used For |
|---------|---------|--------|----------|
| L2 Deployer | `0x1B9AbEeC3215D8AdE8A33607f2cF0f4F60e5F0D0` | `values-secrets.yaml` | L2MessageService, Status Network contracts |
| L2 Alt Deployer | `0xb17202c37cce9498e6f7dcdc1abd207802d09b5eee96677ea219ac867a198b91` | Docker Compose config | EIP system contracts, predeploys |

### Hoodi Deployer

| Account | Address | Source | Used For |
|---------|---------|--------|----------|
| Hoodi Account | `0x8E5bA9C1DF138754076FAfaC0DeeDAf3d598ed35` | `values-secrets.yaml` | Linea contract deployment on Hoodi |

> **Note:** Private keys for deployers and signers are stored in `values-secrets.yaml` (gitignored). The local dev defaults in `values.yaml` use well-known Hardhat test accounts which are publicly documented.

---

## Operator & Signer Accounts

These accounts are used by services at runtime to submit transactions. In local dev mode, they use well-known Hardhat test account keys configured in `values.yaml`. For testnet/production, real keys are provided via `values-secrets.yaml`.

### Coordinator Signers

| Role | Source | Used For |
|------|--------|----------|
| Blob Submitter | `values.yaml` (Hardhat #2) / `values-secrets.yaml` | L1 blob submission |
| Aggregation Submitter | `values.yaml` (Hardhat #1) / `values-secrets.yaml` | L1 proof aggregation submission |
| Message Anchorer | `values.yaml` (Hardhat #4) / `values-secrets.yaml` | L1->L2 message anchoring |

### Postman Signers

| Role | Source | Used For |
|------|--------|----------|
| L1 Signer | `values.yaml` (Hardhat #5) / `values-secrets.yaml` | L1 message claiming |
| L2 Signer | `values.yaml` / `values-secrets.yaml` | L2 message claiming |

### Rollup Operators

| Address | Role |
|---------|------|
| `0x70997970C51812dc3A010C7d01b50e0d17dc79C8` | LineaRollup operator (Hardhat #1) |
| `0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC` | LineaRollup operator (Hardhat #2) |

### Security Councils

| Address | Role |
|---------|------|
| `0x90F79bf6EB2c4f870365E785982E1f101E93b906` | L1 Rollup + L1 TokenBridge security council (Hardhat #3) |
| `0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266` | L2MessageService security council (Hardhat #0) |
| `0xf17f52151EbEF6C7334FAD080c5704D77216b732` | L2 TokenBridge security council |

---

## Deployment Flow

### Local Mode (`make deploy-contracts`)

```
1. deploy-linea-rollup-v6     → PlonkVerifier + LineaRollup on L1
2. deploy-token-bridge-l1     → TokenBridge on L1
3. deploy-l1-test-erc20       → TestERC20 on L1
4. deploy-l2messageservice    → L2MessageService on L2
5. deploy-token-bridge-l2     → TokenBridge on L2
6. deploy-l2-test-erc20       → TestERC20 on L2
7. deploy-l2-evm-opcode-tester → EVM opcode testing framework on L2
```

If `STATUS_NETWORK_CONTRACTS_ENABLED=true`:
```
8. deploy-status-network-contracts → KarmaTiers, Karma, StakeManager, RLN, KarmaNFT on L2
```

### Hoodi Mode (Manual)

No L1 nodes are run. All services use the external Hoodi RPC (`network.l1RpcEndpoint`).

```
1. Set RPC_URL to Hoodi endpoint (Infura/Alchemy)
2. Set PRIVATE_KEY to Hoodi deployer key (has testnet ETH)
3. Run deploy-linea-rollup with Hoodi RPC
4. Run deploy-l2messageservice with L2 RPC
5. Update values-internal-testnet.yaml network.contracts with new addresses
6. helm upgrade to propagate addresses to coordinator, postman, maru
7. Deploy Status Network contracts on L2
8. Update RLN prover addresses in values
9. helm upgrade again
```

---

## File Locations

| Path | Contents |
|------|----------|
| `contracts/local-deployments-artifacts/L1RollupAddress.txt` | Last deployed LineaRollup address |
| `contracts/local-deployments-artifacts/L2MessageServiceAddress.txt` | Last deployed L2MessageService address |
| `contracts/local-deployments-artifacts/TokenBridgeL1Address.txt` | Last deployed L1 TokenBridge address |
| `contracts/local-deployments-artifacts/TokenBridgeL2Address.txt` | Last deployed L2 TokenBridge address |
| `status-network-contracts/deployments/karma_address.txt` | Last deployed Karma address |
| `status-network-contracts/deployments/karma_tiers_address.txt` | Last deployed KarmaTiers address |
| `status-network-contracts/deployments/rln_address.txt` | Last deployed RLN address |
| `status-network-contracts/deployments/stake_manager_address.txt` | Last deployed StakeManager address |
| `status-network-contracts/deployments/karma_nft_address.txt` | Last deployed KarmaNFT address |
| `docker/config/l2-genesis-initialization/genesis-besu.json` | L2 genesis with pre-deployed system contracts |
| `k8s/helm/status-network/values.yaml` | Default contract addresses for Helm |
| `k8s/helm/status-network/values-internal-testnet.yaml` | Hoodi testnet overrides |
