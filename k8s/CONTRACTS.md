# Status Network - Contract Addresses (Internal Testnet)

Contract addresses for the internal testnet deployment on Hoodi L1 + Status Network L2.

- **L1**: Hoodi (Chain ID 560048)
- **L2**: Status Network (Chain ID 374)
- **L2 RPC**: Via internal load balancer (see `values-secrets.yaml`)

> **Note:** Chain ID changed from 59141 to 374 on 2026-02-26. All contracts were redeployed from scratch.

---

## L1 Contracts (Hoodi)

Deployed on Hoodi L1 via `contracts/local-deployments-artifacts/deployPlonkVerifierAndLineaRollupV6.ts`.

| Contract | Address | Description |
|----------|---------|-------------|
| **LineaRollupV6 (Proxy)** | `0x41C5351c40Ce8097C8a65202ecaAF3282720a93f` | Main rollup contract (TransparentUpgradeableProxy) |
| **LineaRollupV6 (Implementation)** | `0x88DC8be84fBf2DddB58f7b9cC6D5eE4FCa6a23dC` | Implementation behind the proxy |
| **ProxyAdmin** | `0x2621F080D39CF57C1Af15A3f501038EeaA76b84f` | Owner of the TransparentUpgradeableProxy |
| **IntegrationTestTrueVerifier** | `0x1705ACa799be36ffE6ccFD108ca0D16d678ff8C0` | ZK proof verification (test verifier, accepts any proof) |

Deployer/Operator: `0x8E5bA9C1DF138754076FAfaC0DeeDAf3d598ed35`

### Genesis Parameters

These are baked into the L1 contract's `initialize()` call and **must match shomei's ZK state root at L2 block 0**.

| Parameter | Value |
|-----------|-------|
| Genesis state root | `0x1144cbb47dd47380846aab9ade4cd71d1186a62f8ff49f894b08109f99525a79` |
| Genesis shnarf | `0x40e726d0b7e3ec644dcf9239842ffa1449baae4e7f86982a90da77c6dfbefd68` |
| Genesis timestamp | `1683325137` (2023-05-05T22:18:57Z, from L2 block 0) |
| Initial L2 block number | `0` |

> **Important:** The `LINEA_ROLLUP_INITIAL_STATE_ROOT_HASH` MUST be shomei's ZK state root (Poseidon sparse merkle trie), NOT the Ethereum state root from L2 block 0 headers. These are different hash algorithms over the same state. Query shomei: `rollup_getZkEVMStateMerkleProofV0` for blocks 0-0, use `zkEndStateRootHash`. The `LINEA_ROLLUP_GENESIS_TIMESTAMP` MUST match the L2 block 0 timestamp exactly.

---

## L2 System Contracts (Genesis Pre-deploys)

Baked into L2 genesis (`docker/config/l2-genesis-initialization/genesis-besu.json`). Exist at block 0.

| Address | Contract | Purpose |
|---------|----------|---------|
| `0x3a69eD7FA6f6CA1dB2649327c4A2E666130823bE` | Withdrawal Request Queue (ERC1967Proxy) | EIP-6110 withdrawal request processing |
| `0x9AFC00F4CEaadb97A3822FCA2225FaD78839FBde` | Consolidation Request Queue (ERC1967Proxy) | EIP-6110 consolidation request processing |
| `0x0000F90827F1C53a10cb7A02335B175320002935` | MCOPY Precompile | EIP-5656 memory copy opcode support |
| `0x000F3df6D732807Ef1319fB7B8bB8522d0Beac02` | BLOCKHASH Precompile | EIP-2935 historical block hash access |

---

## L2 Linea Protocol Contracts

Deployed on L2 after genesis. Handle L2->L1 messaging.

| Contract | Address | Description |
|----------|---------|-------------|
| **L2MessageService** | `0xeB0b0a14F92e3BA35aEF3a2B6A24D7ED1D11631B` | L2->L1 message dispatching, L1->L2 message claiming |

Deployer: `0x1B9AbEeC3215D8AdE8a33607f2cF0f4F60e5F0D0`

---

## L2 Status Network Contracts

Deployed on L2 via Foundry (forge scripts). These implement the Status Network protocol.

| Contract | Address | Description |
|----------|---------|-------------|
| **KarmaTiers** | `0x5D7F9C0249F82277699DDd94cEFD9b0D1C56BC30` | Tier management for Karma holders |
| **Karma (proxy)** | `0x5C95Bcd50E6D1B4E3CDC478484C9030Ff0a7D493` | ERC20 reputation token |
| **RLN (proxy)** | `0xc407C7Bc2b3C109b8bCDE7C681d84a6a4B600eA5` | Rate Limiting Nullifier for gasless transactions |
| **StakeManager (proxy)** | `0xE4392c8ecC46b304C83cDB5edaf742899b1bda93` | Manages user staking and creates StakeVault clones |
| **KarmaNFT** | `0x37329AFc217D487d1db339F8EfEE8C4eEA8C1648` | NFT representation of Karma holdings |
| **VaultFactory** | `0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0` | Creates StakeVault proxies for users |
| **StakeVault (implementation)** | `0x5FbDB2315678afecb367f032d93F642f64180aa3` | StakeVault logic contract used by VaultFactory |
| **SimpleKarmaDistributor (proxy)** | `0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9` | Distributes Karma rewards to users |
| **SimpleKarmaDistributor (impl)** | `0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9` | Implementation behind the proxy |

Deployer: `0x1B9AbEeC3215D8AdE8a33607f2cF0f4F60e5F0D0`

### Deployment Order

1. **KarmaTiers** (no dependencies)
2. **Karma** (no dependencies)
3. **StakeManager** (depends on Karma address)
4. **RLN** (depends on Karma address)
5. **KarmaNFT** (depends on Karma address)
6. **VaultFactory** (depends on StakeManager + Karma addresses)
7. **SimpleKarmaDistributor** (depends on Karma address)

---

## Helm Values Reference

These addresses are configured in `values-internal-testnet.yaml`:

```yaml
network:
  contracts:
    l1Rollup: "0x41C5351c40Ce8097C8a65202ecaAF3282720a93f"
    l2MessageService: "0xeB0b0a14F92e3BA35aEF3a2B6A24D7ED1D11631B"

l2:
  rlnProver:
    karmaContract: "0x5C95Bcd50E6D1B4E3CDC478484C9030Ff0a7D493"
    rlnContract: "0xc407C7Bc2b3C109b8bCDE7C681d84a6a4B600eA5"
    tiersContract: "0x5D7F9C0249F82277699DDd94cEFD9b0D1C56BC30"
```

### Where Addresses Are Used

| Service | Config Key / Env Var | Which Address |
|---------|---------------------|---------------|
| **Coordinator** | `[protocol.l1] contract-address` | `network.contracts.l1Rollup` |
| **Coordinator** | `[protocol.l2] contract-address` | `network.contracts.l2MessageService` |
| **Coordinator** | `[defaults] l1-endpoint` | `network.l1RpcEndpoint` |
| **Postman** | `L1_CONTRACT_ADDRESS` | `network.contracts.l1Rollup` |
| **Postman** | `L2_CONTRACT_ADDRESS` | `network.contracts.l2MessageService` |
| **Postman** | `L1_RPC_URL` | `network.l1RpcEndpoint` |
| **Maru** | `[linea] contract-address` | `network.contracts.l1Rollup` |
| **Maru** | `[linea] l1-eth-api` | `network.l1RpcEndpoint` |
| **Sequencer** | `plugin-linea-l1l2-bridge-contract` | `network.contracts.l2MessageService` |
| **L2 Node Besu** | `plugin-linea-l1l2-bridge-contract` | `network.contracts.l2MessageService` |
| **Traces Node** | `plugin-linea-l1l2-bridge-contract` | `network.contracts.l2MessageService` |

---

## Deployer & Operator Accounts

| Account | Address | Used For |
|---------|---------|----------|
| Hoodi Deployer/Operator | `0x8E5bA9C1DF138754076FAfaC0DeeDAf3d598ed35` | L1 contract deployment, rollup operations |
| L2 Deployer | `0x1B9AbEeC3215D8AdE8a33607f2cF0f4F60e5F0D0` | L2MessageService, Status Network contracts |

> **Note:** Private keys are stored in `values-secrets.yaml` (gitignored, NEVER commit).

---

## Deployment Flow

```
1. Set RPC_URL to Hoodi endpoint and PRIVATE_KEY to deployer key
2. Run deployPlonkVerifierAndLineaRollupV6.ts with Hoodi RPC
3. Deploy L2MessageService on L2 (port-forward sequencer to 8545)
4. Update values-internal-testnet.yaml network.contracts with new addresses
5. helm upgrade to propagate addresses to coordinator, postman, maru
6. Deploy Status Network contracts on L2 (with --with-gas-price 13gwei)
7. Update RLN prover contract addresses in values
8. helm upgrade again
```

> **Tip:** The deploy script uses `process.env.RPC_URL` (NOT `BLOCKCHAIN_NODE`). If `RPC_URL` is undefined, ethers defaults to `http://localhost:8545`. Scale coordinator to 0 replicas before deploying to L1 (pending blob type-3 txs conflict with deploy type-2 txs).

> **Known issue:** The coordinator reads its config from `/initialization/coordinator-config-v2-hardforks.toml` on the `l2-genesis-data` PVC (baked during genesis init), NOT directly from the K8s Secret. If you update the coordinator Secret after genesis, you must also update the file on the PVC (via `kubectl exec`) and restart the coordinator.
