# Status Network - Production Configuration Checklist

This document lists every configuration parameter required to run the Status Network in a production setup. Cross-check each section to ensure nothing is missed.

---

## 1. RLN Prover

### CLI Arguments

| Parameter | Production Value | Notes |
|-----------|-----------------|-------|
| `--ip` | `0.0.0.0` | Bind to all interfaces |
| `--port` | `50051` | gRPC service port |
| `--no-config` | (flag) | Skip config file, use CLI args only |
| `--ws-rpc-url` | `ws://sequencer:8546` | **REQUIRED** - WebSocket RPC to sequencer for event subscriptions + tx submission |
| `--ksc` | `<KARMA_CONTRACT>` | **REQUIRED** - Karma token contract address |
| `--rlnsc` | `<RLN_CONTRACT>` | **REQUIRED** - RLN membership contract address |
| `--tsc` | `<TIERS_CONTRACT>` | **REQUIRED** - KarmaTiers contract address |
| `--db` | `postgres://user:pass@host:5432/prover_db` | **REQUIRED** - PostgreSQL connection URL. Also stores nullifiers and deny list. |
| `--registration-min` | `1` | Minimum karma to auto-register a user (in token units) |
| `--registration-gas-price-gwei` | `101` | **CRITICAL** - Must be >= sequencer's premium gas threshold. Without this, registration txs are rejected. |
| `--epoch-duration-secs` | `86400` | 24 hours. Controls quota reset cycle and proof epoch field. |
| `--tx-gas-quota` | `100000` | Gas quota per tx for rate limiting (default) |
| `--spam-limit` | `1000000` | **CRITICAL** - Max `message_id` in RLN ZK proofs. Circuit hard limit: 1048575 (2^20 - 1) due to Num2Bits(20) constraint in custom circuit. Values above 1048575 cause proof verification failure. Must be >= highest tier's `tx_per_epoch`. |
| `--rln-identifier` | `<production-identifier>` | Should be unique per deployment. Default: `test-rln-identifier` |
| `--kill-switch-file` | `/var/lib/besu/kill-switch/gas-kill-switch` | Optional - emergency proof generation disable |
| `--kill-switch-poll-secs` | `5` | Poll interval for kill switch file |
| `--no-grpc-reflection` | (flag) | Recommended for production (disable gRPC reflection) |
| `--metrics-ip` | `0.0.0.0` | For Prometheus scraping |
| `--metrics-port` | `30031` | Prometheus metrics port |

### Environment Variables

| Variable | Required | Notes |
|----------|----------|-------|
| `PRIVATE_KEY` | **YES** | Hex private key for signing registration transactions. Wallet must be funded with ETH and have `REGISTER_ROLE` on the RLN contract. |
| `DATABASE_URL` | Yes (or use `--db`) | PostgreSQL connection string |
| `RUST_LOG` | No | Log level: `info` for production, `debug` for troubleshooting |

### Common Gotchas

- **Missing `--registration-gas-price-gwei`**: Registration txs fail silently. Users never get registered, they get `NOT_FOUND: Sender not registered` errors.
- **`PRIVATE_KEY` wallet not funded**: Registration txs fail with insufficient gas.
- **`PRIVATE_KEY` wallet missing `REGISTER_ROLE`**: Registration txs revert on-chain.
- **Wrong contract addresses in `--ksc`/`--rlnsc`/`--tsc`**: Prover subscribes to wrong events, never sees mints.
- **`BackendGone` errors**: WebSocket connection to sequencer drops. Check network stability between prover and sequencer pods.
- **Deny list epoch alignment**: Deny list entries are automatically cleared on epoch boundaries. No TTL configuration needed — entries are scoped to the epoch they were created in.
- **`--spam-limit` too low (default 10,000)**: Proof generation fails with `Message id (N) is not within user_message_limit (10000)` once a user's epoch tx counter exceeds the limit. Set to highest tier's `tx_per_epoch` or desired limit. **Circuit hard limit: 1048575** (custom circuit uses Num2Bits(20)). Values >1048575 generate proofs that fail verification. Existing users need re-registration after changing this (value is baked into Merkle tree leaf).

---

## 2. Sequencer (Besu Node, type: SEQUENCER)

The sequencer loads the RLN **Verifier** (validates proofs). It does NOT need the Forwarder — users submit txs to the RPC node, which forwards them to the prover. Controlled by `--plugin-linea-node-type=SEQUENCER`.

### RLN Plugin Parameters

| Parameter | Production Value | Notes |
|-----------|-----------------|-------|
| `--plugin-linea-node-type` | `SEQUENCER` | **CRITICAL** - Determines which plugins load. Sequencer gets verifier + forwarder. |
| `--plugin-linea-rln-enabled` | `true` | Master switch for RLN verifier |
| `--plugin-linea-rln-proof-service` | `rln-prover:50051` | gRPC endpoint to RLN prover (for verifier proof stream) |
| `--plugin-linea-rln-karma-service` | `rln-prover:50051` | Karma service (same as proof service) |
| `--plugin-linea-rln-timeouts-ms` | `10000` | Service timeout (10s) |
| `--plugin-linea-rln-proof-wait-timeout-ms` | `10000` | Timeout waiting for proof in local cache during validation |
| `--plugin-linea-rln-premium-gas-threshold-gwei` | `100` | Txs with gas >= this bypass RLN proof requirement. Must be <= prover's `--registration-gas-price-gwei` so registration txs pass through. |
### Gasless RPC Parameters

| Parameter | Production Value | Notes |
|-----------|-----------------|-------|
| `--plugin-linea-rpc-gasless-enabled` | `true` | Enable gasless tx support in `linea_estimateGas` |
| `--plugin-linea-rpc-allow-zero-gas-estimation-gasless` | `true` | Return 0 gas for eligible gasless users |

**Note**: The sequencer does NOT need `--plugin-linea-rpc-rln-prover-forwarder-enabled`. The forwarder is only for RPC nodes.

### Gas Kill Switch

| Parameter | Production Value | Notes |
|-----------|-----------------|-------|
| `--plugin-linea-gas-kill-switch-file` | `/var/lib/besu/kill-switch/gas-kill-switch` | Emergency gasless disable. Write `true` to file to kill gasless. |
| `--plugin-linea-gas-kill-switch-poll-seconds` | `5` | Poll interval |

### Common Gotchas

- **Premium gas threshold**: The `--plugin-linea-rln-premium-gas-threshold-gwei` on the sequencer and the `--registration-gas-price-gwei` on the prover must be aligned. Prover's gas price must be >= sequencer's threshold.
- **gRPC reconnect**: If the prover restarts, the sequencer reconnects automatically with max 5s backoff. No manual intervention needed.

---

## 3. L2 RPC Node (Besu Node, type: RPC)

The RPC node loads **only** the Forwarder (sends incoming txs to prover for proof generation). It does **NOT** run the verifier — that's the sequencer's job. Controlled by `--plugin-linea-node-type=RPC`.

### RLN Plugin Parameters

| Parameter | Production Value | Notes |
|-----------|-----------------|-------|
| `--plugin-linea-node-type` | `RPC` | Loads only the forwarder, NOT the verifier |
| `--plugin-linea-rln-enabled` | `true` | Enables shared RLN services (deny list, karma client) |
| `--plugin-linea-rln-proof-service` | `rln-prover:50051` | gRPC endpoint to RLN prover |
| `--plugin-linea-rln-karma-service` | `rln-prover:50051` | Karma service endpoint |
| `--plugin-linea-rln-timeouts-ms` | `5000` | Service timeout (can be lower than sequencer) |
| `--plugin-linea-rln-premium-gas-threshold-gwei` | `100` | Same as sequencer |

### Gasless RPC Parameters (same as sequencer)

| Parameter | Production Value | Notes |
|-----------|-----------------|-------|
| `--plugin-linea-rpc-gasless-enabled` | `true` | Enable gasless tx support |
| `--plugin-linea-rpc-rln-prover-forwarder-enabled` | `true` | Enable forwarder |
| `--plugin-linea-rpc-allow-zero-gas-estimation-gasless` | `true` | Return 0 gas for eligible gasless users |

### What the RPC node does NOT need

- `--plugin-linea-rln-proof-wait-timeout-ms` — not used (verifier not loaded)

---

## 4. Smart Contract Addresses

All services must use the **same** deployed contract addresses. Verify these are consistent across all configs.

| Contract | Used By | Config Parameter |
|----------|---------|-----------------|
| Karma (ERC20) | RLN Prover | `--ksc` |
| RLN | RLN Prover, Sequencer | `--rlnsc`, `rln.contractAddress` |
| KarmaTiers | RLN Prover | `--tsc` |
| L1 Rollup | Coordinator, Postman | `network.contracts.l1Rollup` |
| L2 Message Service | Coordinator, Postman, Sequencer | `network.contracts.l2MessageService` |

---

## 5. Coordinator

### Key Production Settings

| Setting | Production Value | Notes |
|---------|-----------------|-------|
| Genesis State Root | Must match shomei ZK root at block 0 | Use `rollup_getZkEVMStateMerkleProofV0` for blocks 0-0, take `zkEndStateRootHash`. **NOT** the Ethereum stateRoot. |
| Genesis Shnarf | `keccak256(abi.encode(0, 0, zkStateRoot, 0, 0))` | Computed from ZK state root |
| L1 Contract | Deployed L1 Rollup proxy address | Must match actual deployment |
| L2 Message Service | Deployed L2 contract | Must match actual deployment |
| L2 Contract Deploy Block | Block number when L2MessageService was deployed | Used to start event scraping |
| L1 RPC Endpoint | Paid RPC endpoint (e.g., gateway.fm) | Must support `eth_sendRawTransaction` for blob txs |

### Submission Gas Caps

| Submission Type | Max Fee per Gas | Max Priority Fee | Gas Limit |
|----------------|-----------------|-------------------|-----------|
| Blob Submission | `100 Gwei` | `20 Gwei` | `10,000,000` |
| Aggregation | `200 Gwei` | `40 Gwei` | `10,000,000` |
| Message Anchoring | `100 Gwei` | N/A | `10,000,000` |

### Conflation Settings

| Setting | Production Value | Notes |
|---------|-----------------|-------|
| Blocks Limit | `2` | Max blocks per conflation batch |
| Conflation Deadline | `PT6S` | Max time before forced conflation |
| Blob Size Limit | `102400` bytes | Blob compression size limit |
| Proofs Limit | `3` | Max proofs per aggregation |

### Signer Keys

| Signer | Purpose | Storage |
|--------|---------|---------|
| Blob Submission | Signs type-3 blob txs to L1 | K8s secret `network-secrets` |
| Aggregation | Signs proof aggregation txs to L1 | K8s secret `network-secrets` |
| Message Anchoring | Signs L1<>L2 message anchoring txs | K8s secret `network-secrets` |

### Common Gotchas

- **Genesis state root wrong**: Must be shomei ZK root (Poseidon hash), NOT Ethereum stateRoot (keccak256). Finalization fails with `FinalizationStateIncorrect`.
- **Genesis timestamp mismatch**: L1 contract `LINEA_ROLLUP_GENESIS_TIMESTAMP` must equal L2 block 0 timestamp exactly.
- **PVC config stale**: Coordinator reads config from PVC (`l2-genesis-data`), not directly from K8s Secret. After helm upgrade, must also update the PVC file manually.
- **`PendingTxsOfConflictingType`**: L1 rejects type-2 txs when type-3 (blob) txs are pending from same sender. Scale coordinator to 0 before deploying L1 contracts.
- **Coordinator DB is separate**: Uses `linea_coordinator` DATABASE (not schema). Wipe with `DROP DATABASE linea_coordinator; CREATE DATABASE linea_coordinator;`

---

## 6. PostgreSQL

### Required Databases

| Database | Used By | Notes |
|----------|---------|-------|
| `prover_db` | RLN Prover | Needs `pg_merkle_tree` extension. Also stores nullifiers and deny list. |
| `linea_coordinator` | Coordinator | Separate DATABASE, not schema |
| `postman_db` | Postman | Message relaying state |
| `linea_transaction_exclusion` | TX Exclusion API | Optional |

---

## 7. Secrets Checklist

All production secrets must be set. None should be default/test values.

| Secret | Service | Notes |
|--------|---------|-------|
| `RLN_PROVER_PRIVATE_KEY` | RLN Prover | Must have REGISTER_ROLE on RLN contract, funded with ETH |
| `BLOB_SUBMISSION_PRIVATE_KEY` | Coordinator | L1 signer for blob txs |
| `AGGREGATION_PRIVATE_KEY` | Coordinator | L1 signer for aggregation |
| `MESSAGE_ANCHORING_PRIVATE_KEY` | Coordinator | L1 signer for anchoring |
| `POSTMAN_L1_PRIVATE_KEY` | Postman | L1 message claiming |
| `POSTMAN_L2_PRIVATE_KEY` | Postman | L2 message claiming |
| `L1_RPC_ENDPOINT` | Coordinator, Postman | Paid L1 RPC (must support blobs). **NEVER expose API key.** |

---

## 8. Production vs Test - Quick Diff

| Setting | Test | Production |
|---------|------|------------|
| RLN Prover mode | `--mock-sc true` | `--ws-rpc-url ws://sequencer:8546` |
| RLN Prover epoch | `--epoch-duration-secs 60` | `--epoch-duration-secs 86400` |
| RLN Prover registration gas | (not set, default 0) | `--registration-gas-price-gwei 101` |
| Sequencer premium gas | `12` Gwei | `100` Gwei |
| Kill switch | disabled | enabled with file path |
| L1 RPC | local node | paid endpoint (gateway.fm) |
| Signer keys | Hardhat test keys | Production keys from secrets |
| Contract addresses | Local deployment | Testnet/mainnet deployment |
| gRPC reflection | enabled | disabled (`--no-grpc-reflection`) |

---

## 9. Health Checks

After deployment, verify:

1. **RLN Prover**: Check logs for `on_new_user` / `register` entries after minting karma to a test address
2. **Sequencer**: Check logs for `Proof cached for txHash` when gasless txs are submitted
3. **Coordinator**: Check that blob submission, aggregation, and finalization are all processing
4. **WebSocket**: Verify `ws://sequencer:8546` is reachable from RLN prover pod (`websocat` test)
5. **Kill switch**: Verify kill switch file path is mounted and writable (shared volume)
6. **Contract addresses**: Call `balanceOf` on karma contract for the prover's wallet to confirm connectivity
7. **Nullifiers**: Confirm prover_db PostgreSQL is accessible from sequencer (nullifiers tracked via gRPC to prover)
