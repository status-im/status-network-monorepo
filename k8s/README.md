# Status Network Testnet Deployment Guide

This guide covers everything needed to deploy and operate the Status Network testnet -- the first gasless Ethereum L2 rollup -- on AWS EKS. The network uses RLN (Rate Limiting Nullifier) to enable zero-gas-price transactions while preventing spam.

## Table of Contents

- [Architecture](#architecture)
- [Prerequisites](#prerequisites)
- [Deployment Modes](#deployment-modes)
- [Step-by-Step: Deploy Internal Testnet on AWS](#step-by-step-deploy-internal-testnet-on-aws)
  - [1. Provision AWS Infrastructure](#1-provision-aws-infrastructure)
  - [2. Configure kubectl](#2-configure-kubectl)
  - [3. Prepare Secrets](#3-prepare-secrets)
  - [4. Deploy with Helm](#4-deploy-with-helm)
  - [5. Verify Deployment](#5-verify-deployment)
  - [6. Post-Deployment: Initialize Karma Tiers](#6-post-deployment-initialize-karma-tiers)
  - [7. Post-Deployment: Deploy Contracts (if needed)](#7-post-deployment-deploy-contracts-if-needed)
- [Step-by-Step: Local Development with Docker](#step-by-step-local-development-with-docker)
- [Secrets Management](#secrets-management)
- [Configuration Reference](#configuration-reference)
- [Helm Operations](#helm-operations)
- [Monitoring and Debugging](#monitoring-and-debugging)
- [RLN Gasless Transaction System](#rln-gasless-transaction-system)
- [Contract Addresses](#contract-addresses)
- [Cleanup](#cleanup)
- [Troubleshooting](#troubleshooting)
- [Security Considerations](#security-considerations)

## Architecture

Status Network is a Linea-based L2 rollup with a custom gasless transaction layer powered by RLN. The network consists of two layers plus supporting infrastructure.

### L1 Layer

| Component | Description |
|-----------|-------------|
| **l1-el-node** | Besu execution layer node (local mode only) |
| **l1-cl-node** | Teku consensus layer node (local mode only) |

In **Hoodi mode** (internal testnet), no L1 nodes are deployed. All L2 services connect to the Hoodi Ethereum testnet via an external RPC endpoint (e.g., Infura).

### L2 Layer

| Component | Description |
|-----------|-------------|
| **postgres** | PostgreSQL 16 with `pg_merkle_tree` extension (custom image) |
| **sequencer** | Besu sequencer -- primary block producer, RLN plugin host |
| **maru** | QBFT consensus client and payload builder |
| **l2-node-besu** | Besu full node for external RPC access |
| **l2-node-besu-follower** | Follower node (additional RPC capacity) |
| **traces-node** | Block trace generation node (for ZK proofs) |
| **zkbesu-shomei** | ZK state node (Shomei plugin host) |
| **shomei** | State proof service |
| **shomei-frontend** | State proof frontend |
| **coordinator** | L1/L2 coordination: blob submission, proof aggregation, finalization |
| **postman** | Message relay between L1 and L2 |
| **rln-prover** | RLN proof generation and karma verification service |
| **web3signer** | Key signing service (disabled on internal testnet) |
| **transaction-exclusion-api** | Transaction filtering API (disabled on internal testnet) |

### Data Flow

```
Users --> sequencer (RLN plugin verifies gasless proofs) --> maru (consensus)
                                                               |
             +-------------------------------------------------+
             v                                                 v
     l2-node-besu (RPC)                             traces-node (ZK traces)
             |                                                 |
             v                                                 v
         postman (L1<->L2 messages)               coordinator (blob submission)
             |                                                 |
             +-------------------> L1 (Hoodi) <----------------+
```

## Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| AWS CLI | >= 2.0 | AWS authentication and EKS management |
| Terraform | >= 1.0 | EKS cluster provisioning |
| kubectl | >= 1.28 | Kubernetes cluster management |
| Helm | >= 3.0 | Application deployment |
| Node.js | >= 18 | Contract deployment scripts |
| Foundry | latest | Smart contract compilation and deployment |

Ensure your AWS CLI is configured with credentials that have permissions to create EKS clusters, VPCs, IAM roles, and load balancers.

## Deployment Modes

The chart supports two L1 modes, configured via `network.l1Network`:

| Mode | L1 Chain ID | L2 Chain ID | L1 Nodes | Use Case |
|------|-------------|-------------|----------|----------|
| `local` (default) | 1337 | 59141 | Besu + Teku in-cluster | Local dev, private chain, Docker Compose |
| `hoodi` | 560048 | 59141 | None (external RPC) | Internal testnet on AWS EKS |

In **Hoodi mode**, set `l1.enabled: false` and provide an external L1 RPC via secrets. No L1 pods, PVCs, or services are created.

## Step-by-Step: Deploy Internal Testnet on AWS

### 1. Provision AWS Infrastructure

The Terraform configuration creates a production-ready EKS cluster:

- VPC with 3 public + 3 private subnets across 3 AZs
- EKS cluster (Kubernetes 1.35) with managed node group
- IRSA roles for VPC CNI, EBS CSI driver, and AWS Load Balancer Controller
- AWS Load Balancer Controller for NLB provisioning
- gp3 storage class (set as default)

```bash
cd k8s/terraform
terraform init
terraform apply -var="cluster_name=sn-testnet" -var="aws_region=us-east-1"
```

**Terraform Variables:**

| Variable | Default | Description |
|----------|---------|-------------|
| `aws_region` | us-east-1 | AWS region |
| `cluster_name` | sn-testnet | EKS cluster name |
| `kubernetes_version` | 1.35 | Kubernetes version |
| `node_instance_types` | t3.2xlarge | Instance type (8 vCPU, 32GB RAM) |
| `node_min_size` | 1 | Minimum nodes in ASG |
| `node_max_size` | 3 | Maximum nodes in ASG |
| `node_desired_size` | 1 | Desired node count |
| `create_backup_bucket` | false | Create S3 config backup bucket |

### 2. Configure kubectl

```bash
aws eks update-kubeconfig --region us-east-1 --name sn-testnet

# Verify connectivity
kubectl get nodes
```

### 3. Prepare Secrets

Secrets are **not** stored in the Helm values files committed to git. You must create a `values-secrets.yaml` file with your actual credentials.

```bash
cd k8s/helm/status-network

# Copy the example file
cp values-secrets.yaml.example values-secrets.yaml

# Edit with your real values
vim values-secrets.yaml
```

The `values-secrets.yaml` file contains:

```yaml
network:
  # External L1 RPC endpoint (e.g., Infura, Alchemy)
  l1RpcEndpoint: "https://hoodi.infura.io/v3/YOUR_API_KEY"
  # L1 signer keys for coordinator and postman
  signers:
    blobSubmission: "0xYOUR_KEY"
    aggregation: "0xYOUR_KEY"
    messageAnchoring: "0xYOUR_KEY"
    postmanL1: "0xYOUR_KEY"
    postmanL2: "0xYOUR_KEY"

l2:
  rlnProver:
    # Private key with REGISTER_ROLE on RLN contract
    privateKey: "0xYOUR_KEY"
  postgres:
    credentials:
      password: "CHANGE_ME"
```

**What each secret is for:**

| Secret | Used By | Purpose |
|--------|---------|---------|
| `l1RpcEndpoint` | coordinator, postman, maru | Hoodi L1 RPC URL (contains API key) |
| `signers.blobSubmission` | coordinator | Signs blob submission transactions to L1 |
| `signers.aggregation` | coordinator | Signs proof aggregation transactions to L1 |
| `signers.messageAnchoring` | coordinator | Signs message anchoring transactions to L1 |
| `signers.postmanL1` | postman | Signs L1 message claim transactions |
| `signers.postmanL2` | postman | Signs L2 message claim transactions |
| `rlnProver.privateKey` | rln-prover | Has REGISTER_ROLE on RLN contract; registers users |
| `postgres.credentials.password` | all DB consumers | PostgreSQL password |

> **Important:** `values-secrets.yaml` is gitignored. Never commit this file.

### 4. Deploy with Helm

```bash
# From the repo root
helm install status-network k8s/helm/status-network \
  -f k8s/helm/status-network/values.yaml \
  -f k8s/helm/status-network/values-internal-testnet.yaml \
  -f k8s/helm/status-network/values-secrets.yaml \
  -n status-network-internal-testnet --create-namespace
```

Or use the deployment script which handles Terraform + Helm together:

```bash
k8s/scripts/deploy.sh
```

The deployment creates resources in this order:

1. **Namespace** and **PVCs** (persistent storage)
2. **Secrets** (node keys, JWT, postgres credentials, network secrets, coordinator config)
3. **ConfigMaps** (Besu configs, traces limits, maru config)
4. **Jobs** (L2 genesis initialization)
5. **Deployments** with init containers that wait for dependencies:
   - postgres starts first
   - sequencer waits for genesis + RLN prover (in non-production mode)
   - maru waits for sequencer
   - coordinator waits for postgres, sequencer, shomei, and genesis
   - postman waits for postgres, L2 node, and optionally L1
   - all Besu nodes wait for genesis
6. **Services** (ClusterIP for internal, LoadBalancer for external RPC)

### 5. Verify Deployment

**Check pod status:**

```bash
kubectl get pods -n status-network-internal-testnet
```

All pods should reach `Running` status (jobs will show `Completed`). Expected pods for internal testnet:

```
postgres-*                     Running
rln-prover-*                   Running
sequencer-*                    Running
maru-*                         Running
l2-node-besu-*                 Running
l2-node-besu-follower-*        Running
traces-node-*                  Running
zkbesu-shomei-*                Running
shomei-*                       Running
shomei-frontend-*              Running
coordinator-*                  Running
postman-*                      Running
```

**Get the L2 RPC endpoint:**

```bash
kubectl get svc l2-rpc-lb -n status-network-internal-testnet \
  -o jsonpath='{.status.loadBalancer.ingress[0].hostname}'
```

Or use the helper script:

```bash
k8s/scripts/get-rpc-url.sh
```

**Test the RPC connection:**

```bash
RPC_URL=$(kubectl get svc l2-rpc-lb -n status-network-internal-testnet \
  -o jsonpath='{.status.loadBalancer.ingress[0].hostname}')

curl http://${RPC_URL}:8545 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}'
# Expected: {"jsonrpc":"2.0","id":1,"result":"0xe70d"} (59141 in hex)

curl http://${RPC_URL}:8545 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
# Block number should be increasing
```

**Verify RLN prover is connected:**

```bash
kubectl logs deployment/rln-prover -n status-network-internal-testnet | tail -20
kubectl logs deployment/sequencer -n status-network-internal-testnet | grep -i rln | tail -10
```

### 6. Post-Deployment: Initialize Karma Tiers

The karma tier system controls how many gasless transactions users can send per epoch based on their karma balance. This must be initialized after the RLN contracts are deployed.

```bash
# Port-forward to the sequencer
kubectl port-forward svc/sequencer 9045:8545 -n status-network-internal-testnet &

# Run the initialization script
cd e2e
KARMA_TIERS_ADDRESS=0x5FbDB2315678afecb367f032d93F642f64180aa3 \
  RPC_URL=http://localhost:9045 \
  npx ts-node ../scripts/initialize-karma-tiers.ts
```

The script creates 11 tiers:

| Tier | Name | Karma Required | TX per Epoch |
|------|------|---------------|--------------|
| 0 | none | 0 | 0 |
| 1 | entry | 1 | 2 |
| 2 | newbie | >1 | 6 |
| 3 | basic | 50 | 16 |
| 4 | active | 500 | 96 |
| 5 | regular | 5,000 | 480 |
| 6 | power | 20,000 | 960 |
| 7 | pro | 100,000 | 10,080 |
| 8 | high-throughput | 500,000 | 108,000 |
| 9 | s-tier | 5,000,000 | 240,000 |
| 10 | legendary | 10,000,000 | 480,000 |

### 7. Post-Deployment: Deploy Contracts (if needed)

If deploying to a fresh chain (not using pre-deployed genesis contracts), deploy the Status Network contracts:

```bash
# Port-forward to sequencer
kubectl port-forward svc/sequencer 9045:8545 -n status-network-internal-testnet &

cd status-network-contracts

# Deploy contracts (use --with-gas-price to bypass RLN proof validation)
forge script script/DeployKarmaTiers.s.sol --rpc-url http://localhost:9045 --broadcast --with-gas-price 13gwei
forge script script/DeployKarma.s.sol --rpc-url http://localhost:9045 --broadcast --with-gas-price 13gwei
forge script script/DeployStakeManager.s.sol --rpc-url http://localhost:9045 --broadcast --with-gas-price 13gwei
forge script script/RLN.s.sol --rpc-url http://localhost:9045 --broadcast --with-gas-price 13gwei
forge script script/DeployKarmaNFT.s.sol --rpc-url http://localhost:9045 --broadcast --with-gas-price 13gwei
```

**Contract deployment order** (respects dependencies):

1. KarmaTiers (no dependencies)
2. Karma (no dependencies)
3. StakeManager (depends on Karma)
4. RLN (depends on Karma)
5. KarmaNFT (depends on Karma)

> **Note:** Use `--with-gas-price 13gwei` for all contract deployments. This sets the gas price above the `premiumGasThresholdGwei` (12 gwei), bypassing RLN proof validation which is not possible during deployment.

After deploying contracts, update `values-internal-testnet.yaml` with the new contract addresses and run a Helm upgrade.

## Step-by-Step: Local Development with Docker

For local development, the network runs entirely in Docker Compose with a private L1 chain.

### Quick Start

```bash
# Full environment with RLN in production mode (recommended)
make start-env-with-rln-production
```

This command:
1. Starts the L1 + L2 network with mock RLN
2. Deploys all contracts (Linea protocol + Status Network)
3. Initializes karma tiers
4. Sets up RLN prover account permissions
5. Restarts the RLN prover in production mode (connected to real contracts)

### Other Make Targets

| Command | Description |
|---------|-------------|
| `make start-env` | Base environment (L1 + L2, no RLN) |
| `make start-env-with-rln` | Environment with RLN in mock mode |
| `make start-env-with-rln-and-contracts` | RLN + contract deployment |
| `make start-env-with-rln-production` | Full production mode (recommended) |
| `make start-l2-blockchain-only` | L2 blockchain only (no L1, no contracts) |
| `make clean-environment` | Stop and clean all containers + volumes |

### Local RPC Endpoints

| Service | Endpoint |
|---------|----------|
| L1 EL (Besu) | `http://localhost:8445` |
| L2 Sequencer | `http://localhost:8545` |
| L2 RPC Node | `http://localhost:8645` |
| RLN Prover gRPC | `localhost:50051` |

## Secrets Management

All sensitive values flow through Kubernetes Secrets, never ConfigMaps. The chart uses a layered approach:

### Secret Architecture

| K8s Secret | Contains | Created By |
|------------|----------|------------|
| `network-secrets` | Signer private keys, L1 RPC endpoint, RLN prover key | Helm (from `values-secrets.yaml`) |
| `coordinator-config-secret` | Coordinator TOML config (embeds signer keys + DB credentials) | Helm template |
| `postgres-credentials` | PostgreSQL username, password, connection strings | Helm (auto-generated) |
| `sequencer-keys` | Sequencer node private key | Helm (auto-generated) |
| `node-keys` | L1 node keys, JWT secret, maru keys | Helm (auto-generated) |
| `tx-exclusion-api-config-secret` | TX exclusion API config (embeds DB credentials) | Helm template |

### Values File Layering

```bash
helm install ... \
  -f values.yaml                    # Base defaults (hardhat test keys for local dev)
  -f values-internal-testnet.yaml   # Testnet overrides (contract addresses, images, resource sizing)
  -f values-secrets.yaml            # Real secrets (NEVER committed to git)
```

- `values.yaml` contains safe defaults (hardhat test account keys) for local development
- `values-internal-testnet.yaml` contains non-secret testnet config (contract addresses, images, resource limits)
- `values-secrets.yaml` contains real private keys and API keys (gitignored)

### Providing Secrets Without a File

Alternatively, provide secrets inline:

```bash
helm install status-network k8s/helm/status-network \
  -f k8s/helm/status-network/values.yaml \
  -f k8s/helm/status-network/values-internal-testnet.yaml \
  --set network.l1RpcEndpoint="https://hoodi.infura.io/v3/YOUR_KEY" \
  --set network.signers.blobSubmission="0xYOUR_KEY" \
  --set network.signers.aggregation="0xYOUR_KEY" \
  --set network.signers.messageAnchoring="0xYOUR_KEY" \
  --set network.signers.postmanL1="0xYOUR_KEY" \
  --set network.signers.postmanL2="0xYOUR_KEY" \
  --set l2.rlnProver.privateKey="0xYOUR_KEY" \
  -n status-network-internal-testnet --create-namespace
```

## Configuration Reference

### Values Files

| File | Purpose |
|------|---------|
| `values.yaml` | Base defaults for all environments. Local L1 mode, hardhat test keys, default resource limits. |
| `values-internal-testnet.yaml` | Internal testnet overrides. Hoodi L1, custom ECR images, contract addresses, resource sizing. |
| `values-secrets.yaml` | Real secrets (gitignored). L1 RPC URL, signer keys, RLN prover key, DB password. |
| `values-secrets.yaml.example` | Template for `values-secrets.yaml`. Copy and fill in real values. |

### Key Configuration Sections

```yaml
# Network mode and contracts
network:
  l1Network: "hoodi"           # "local" or "hoodi"
  l1ChainId: 560048            # L1 chain ID
  l2ChainId: 59141             # L2 chain ID
  contracts:
    l1Rollup: "0x..."          # LineaRollup proxy on L1
    l2MessageService: "0x..."  # L2MessageService on L2

# RLN gasless transaction settings
rln:
  enabled: true
  contractAddress: "0x..."     # RLN contract on L2
  epochMode: "TEST"            # TEST (30s) or TIMESTAMP_1H (1 hour)
  premiumGasThresholdGwei: 12  # TX above this bypass RLN
  gasless:
    enabled: true

# RLN prover settings
l2:
  rlnProver:
    productionMode: true       # Connect to real smart contracts
    karmaContract: "0x..."
    rlnContract: "0x..."
    tiersContract: "0x..."
    epochDurationSecs: 300     # 5 min for testing, 86400 for production
```

### Custom Images

The deployment uses custom images. These are published on Docker Hub (`0xnadeem/`) and on ECR for the internal testnet:

| Image | Tag | Description |
|-------|-----|-------------|
| `status-network-besu` | v1.0.1 | Custom Besu sequencer with gasless block fix |
| `status-network-rln-prover` | v1.0.3 | RLN proof generation service |
| `status-network-postgres` | v1.0.2 | PostgreSQL 18 with `pg_merkle_tree` extension |

ECR registry: `720430261111.dkr.ecr.us-east-1.amazonaws.com`

## Helm Operations

### Install

```bash
helm install status-network k8s/helm/status-network \
  -f k8s/helm/status-network/values.yaml \
  -f k8s/helm/status-network/values-internal-testnet.yaml \
  -f k8s/helm/status-network/values-secrets.yaml \
  -n status-network-internal-testnet --create-namespace
```

### Upgrade

```bash
helm upgrade status-network k8s/helm/status-network \
  -f k8s/helm/status-network/values.yaml \
  -f k8s/helm/status-network/values-internal-testnet.yaml \
  -f k8s/helm/status-network/values-secrets.yaml \
  -n status-network-internal-testnet
```

### Template (Dry Run)

Render templates locally without deploying:

```bash
helm template status-network k8s/helm/status-network \
  -f k8s/helm/status-network/values.yaml \
  -f k8s/helm/status-network/values-internal-testnet.yaml \
  -f k8s/helm/status-network/values-secrets.yaml
```

### Lint

```bash
helm lint k8s/helm/status-network \
  -f k8s/helm/status-network/values.yaml \
  -f k8s/helm/status-network/values-internal-testnet.yaml
```

### Rollback

```bash
helm rollback status-network <revision> -n status-network-internal-testnet
```

## Monitoring and Debugging

### Pod Status

```bash
kubectl get pods -n status-network-internal-testnet
kubectl get pods -n status-network-internal-testnet -w  # Watch mode
```

### Logs

```bash
# Core services
kubectl logs -f deployment/sequencer -n status-network-internal-testnet
kubectl logs -f deployment/rln-prover -n status-network-internal-testnet
kubectl logs -f deployment/coordinator -n status-network-internal-testnet
kubectl logs -f deployment/postman -n status-network-internal-testnet
kubectl logs -f deployment/maru -n status-network-internal-testnet

# Init container logs (useful for debugging startup failures)
kubectl logs <pod-name> -c wait-for-postgres -n status-network-internal-testnet
kubectl logs <pod-name> -c wait-for-sequencer -n status-network-internal-testnet
```

### Port Forwarding

```bash
# L2 Sequencer RPC (for contract deployment and direct access)
kubectl port-forward svc/sequencer 9045:8545 -n status-network-internal-testnet

# L2 RPC Node (external-facing node)
kubectl port-forward svc/l2-node-besu 8545:8545 -n status-network-internal-testnet

# RLN Prover gRPC
kubectl port-forward svc/rln-prover 50051:50051 -n status-network-internal-testnet

# PostgreSQL
kubectl port-forward svc/postgres 5432:5432 -n status-network-internal-testnet
```

### Resource Usage

```bash
kubectl top pods -n status-network-internal-testnet
kubectl top nodes
```

### Service Endpoints

```bash
kubectl get svc -n status-network-internal-testnet
kubectl get endpoints -n status-network-internal-testnet
```

## RLN Gasless Transaction System

The RLN (Rate Limiting Nullifier) system enables gasless transactions on the L2. Here is how it works:

1. **Users acquire Karma tokens** -- the ERC20 reputation token on L2
2. **Karma balance determines tier** -- higher karma = more transactions per epoch
3. **Gasless transactions** (gas price = 0) are submitted to the sequencer
4. **Sequencer forwards to RLN prover** for proof verification
5. **RLN prover checks** user's karma, tier, and epoch rate limit
6. **If valid**, proof is attached and the transaction is included in a block
7. **Premium transactions** (gas price > `premiumGasThresholdGwei`) bypass RLN entirely

### Key RLN Configuration

| Parameter | Description | Testnet Value |
|-----------|-------------|---------------|
| `rln.epochMode` | Epoch duration mode | `TEST` (30s) |
| `rln.premiumGasThresholdGwei` | Gas price threshold for premium TX | 12 |
| `l2.rlnProver.epochDurationSecs` | Epoch length for prover | 300 (5 min) |

## Contract Addresses

### Internal Testnet (Chain ID 59141)

| Contract | Address | Purpose |
|----------|---------|---------|
| KarmaTiers | `0x5FbDB2315678afecb367f032d93F642f64180aa3` | Tier management (11 tiers) |
| Karma | `0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0` | ERC20 reputation token |
| RLN | `0xa513E6E4b8f2a923D98304ec87F64353C4D5C853` | Rate Limiting Nullifier |
| StakeManager | `0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9` | Staking management |
| KarmaNFT | `0x2279B7A0a67DB372996a5FaB50D91eAA73d2eBe6` | NFT reputation representation |

### L1 Contracts (Hoodi, Chain ID 560048)

| Contract | Address | Purpose |
|----------|---------|---------|
| LineaRollup | `0x64269d08c795d66A542903AA35B6e775B031872A` | Blob submission, proof finalization |
| L2MessageService | `0x2f6dAaF8A81AB675fbD37Ca6Ed5b72cf86237453` | L2 message service |

See [CONTRACTS.md](./CONTRACTS.md) for the complete contract reference including local mode addresses, deployer accounts, and deployment flow.

## Cleanup

### Delete Helm Release

```bash
helm uninstall status-network -n status-network-internal-testnet
```

### Delete Namespace (removes all resources including PVCs)

```bash
kubectl delete namespace status-network-internal-testnet
```

### Destroy EKS Cluster

```bash
cd k8s/terraform
terraform destroy
```

## Troubleshooting

### Pods Stuck in Init

Most pods use init containers that wait for dependencies. Check which init container is waiting:

```bash
kubectl describe pod <pod-name> -n status-network-internal-testnet

# Common init containers:
kubectl logs <pod-name> -c wait-for-postgres -n status-network-internal-testnet
kubectl logs <pod-name> -c wait-for-sequencer -n status-network-internal-testnet
kubectl logs <pod-name> -c wait-for-genesis -n status-network-internal-testnet
kubectl logs <pod-name> -c wait-for-l1 -n status-network-internal-testnet
```

**Startup dependency chain:**
```
postgres --> rln-prover --> sequencer --> maru --> l2-node-besu
                                     --> coordinator
                                     --> traces-node
                                     --> zkbesu-shomei
```

### PostgreSQL Issues

```bash
# Check if postgres is running and databases exist
kubectl exec -it deployment/postgres -n status-network-internal-testnet -- \
  psql -U postgres -c "\l"

# Verify pg_merkle_tree extension
kubectl exec -it deployment/postgres -n status-network-internal-testnet -- \
  psql -U postgres -d prover_db -c "SELECT * FROM pg_extension WHERE extname='pg_merkle_tree';"
```

### RLN Prover Issues

```bash
# Check if prover is connected to contracts
kubectl logs deployment/rln-prover -n status-network-internal-testnet | grep -i "contract\|tier\|epoch"

# Check if sequencer can reach prover
kubectl logs deployment/sequencer -n status-network-internal-testnet | grep -i "rln\|proof"

# Verify gRPC connectivity
kubectl run --rm -it debug --image=busybox --restart=Never \
  -n status-network-internal-testnet -- nc -zv rln-prover 50051
```

### Coordinator Issues

The coordinator is the most complex service. Common issues:

```bash
# Check L1 connectivity (Infura/Alchemy)
kubectl logs deployment/coordinator -n status-network-internal-testnet | grep -i "l1\|endpoint\|connect"

# Check blob submission
kubectl logs deployment/coordinator -n status-network-internal-testnet | grep -i "blob\|submission"

# Check for smart contract errors
kubectl logs deployment/coordinator -n status-network-internal-testnet | grep -i "error\|revert"
```

### Network Connectivity

```bash
# Check all service endpoints are populated
kubectl get endpoints -n status-network-internal-testnet

# Test internal connectivity from a debug pod
kubectl run --rm -it debug --image=busybox --restart=Never \
  -n status-network-internal-testnet -- sh

# Inside the debug pod:
nc -zv sequencer 8545
nc -zv postgres 5432
nc -zv rln-prover 50051
nc -zv l2-node-besu 8545
```

### LoadBalancer Not Getting External IP

```bash
# Check LoadBalancer service status
kubectl describe svc l2-rpc-lb -n status-network-internal-testnet

# Verify AWS Load Balancer Controller is running
kubectl get pods -n kube-system | grep aws-load-balancer

# Check controller logs
kubectl logs -f deployment/aws-load-balancer-controller -n kube-system
```

### PVC Issues

```bash
# Check PVC status
kubectl get pvc -n status-network-internal-testnet

# If PVCs are stuck in Pending, check storage class
kubectl get storageclass
kubectl describe pvc <pvc-name> -n status-network-internal-testnet
```

## Security Considerations

- **Secrets management:** All private keys and API keys flow through K8s Secrets, never ConfigMaps. The `values-secrets.yaml` file is gitignored. For production, consider using an external secrets manager (AWS Secrets Manager, HashiCorp Vault).
- **Network policies:** Disabled for testnet. Enable `networkPolicies.enabled: true` for production to restrict pod-to-pod communication.
- **L2 RPC LoadBalancer:** Internet-facing by default. For production, restrict access via security groups or use a private NLB.
- **Test keys in values.yaml:** The default `values.yaml` contains hardhat test account keys for local development. These are well-known keys with no real value. Real keys are only in `values-secrets.yaml`.
- **Contract deployment gas price:** Use `--with-gas-price 13gwei` to bypass RLN proof validation during deployment. This is only needed for deployer transactions.

## Directory Structure

```
k8s/
├── terraform/                  # EKS cluster infrastructure (Terraform)
│   ├── main.tf                 # VPC, EKS, IRSA roles, ALB controller, gp3 storage class
│   ├── variables.tf            # Configurable variables
│   └── outputs.tf              # Cluster endpoint, kubeconfig command
├── helm/
│   └── status-network/         # Helm chart
│       ├── Chart.yaml
│       ├── values.yaml                     # Base defaults (local dev)
│       ├── values-internal-testnet.yaml    # Internal testnet overrides
│       ├── values-secrets.yaml.example     # Secrets template (copy to values-secrets.yaml)
│       └── templates/
│           ├── _helpers.tpl        # Template helpers
│           ├── namespace.yaml      # Namespace
│           ├── configmaps/         # ConfigMaps (Besu, maru, node configs)
│           ├── secrets/            # Secrets (keys, credentials, coordinator config)
│           ├── storage/            # PersistentVolumeClaims (gp3)
│           ├── jobs/               # Genesis initialization, contract deployment
│           ├── l1/                 # L1 node deployments + services
│           ├── l2/                 # L2 service deployments + services
│           ├── services/           # LoadBalancer services
│           └── networkpolicies/    # Network policies (disabled for testnet)
├── scripts/
│   ├── deploy.sh               # Main deployment script (terraform + helm)
│   ├── deploy-contracts.sh     # Contract deployment helper
│   └── get-rpc-url.sh          # Get L2 RPC LoadBalancer URL
├── CONTRACTS.md                # Full contract address reference
└── README.md                   # This file
```
