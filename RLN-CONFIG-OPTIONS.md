# RLN Configuration Options for Linea Sequencer

This document lists all RLN (Rate Limiting Nullifier) configuration options available in the Linea Sequencer. The sequencer can run in both **Sequencer Mode** (validates proofs locally) and **RPC Mode** (forwards to prover).

---

## Table of Contents
1. [Core RLN Validator Options](#core-rln-validator-options)
2. [RPC/Estimate Gas Options](#rpcestimate-gas-options)
3. [Profitability & Gasless Options](#profitability--gasless-options)
4. [Transaction Pool Validator Options](#transaction-pool-validator-options)
5. [Shared Gasless Configuration](#shared-gasless-configuration)
6. [Mode-Specific Configurations](#mode-specific-configurations)

---

## Core RLN Validator Options

**Source:** `LineaRlnValidatorCliOptions.java`

### Essential Options (Required for Operators)

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--plugin-linea-rln-enabled` | boolean | `false` | Enable RLN validation for gasless transactions |
| `--plugin-linea-rln-verifying-key` | string | `/etc/linea/rln_verifying_key.bin` | Path to the RLN verifying key file (required for Sequencer mode) |
| `--plugin-linea-rln-proof-service` | string | `localhost:50051` | RLN Proof service endpoint (host:port format) |
| `--plugin-linea-rln-karma-service` | string | `localhost:50052` | Karma service endpoint (host:port format) |
| `--plugin-linea-rln-nullifier-storage-path` | string | `/var/lib/besu/nullifiers.txt` | Path to the nullifier storage file |

### Advanced Options (Internal/Tuning)

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--plugin-linea-rln-use-tls` | boolean | auto-detect | Use TLS for gRPC services (auto: false for :505x, true for :443/8443) |
| `--plugin-linea-rln-premium-gas-threshold-gwei` | long | `10` | Premium gas threshold in GWei to bypass deny list |
| `--plugin-linea-rln-timeouts-ms` | long | `5000` | Service timeout in milliseconds (5 seconds) |
| `--plugin-linea-rln-proof-wait-timeout-ms` | long | `1000` | Timeout for waiting for RLN proof in cache during validation (1 second) |
| `--plugin-linea-rln-epoch-mode` | string | `TIMESTAMP_1H` | Epoch mode: `BLOCK`, `TIMESTAMP_1H`, `TEST`, `FIXED_FIELD_ELEMENT` |

### Internal Configuration (Not CLI-Exposed)

These are set programmatically in `LineaRlnValidatorConfiguration.java`:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `rlnProofCacheMaxSize` | long | `10000` | Maximum number of proofs to keep in memory cache |
| `rlnProofCacheExpirySeconds` | long | `300` | Time-to-live for proofs in cache (5 minutes) |
| `rlnProofStreamRetries` | int | `5` | Max retries for establishing/re-establishing gRPC stream |
| `rlnProofStreamRetryIntervalMs` | long | `5000` | Interval between gRPC stream retry attempts (5 seconds) |
| `exponentialBackoffEnabled` | boolean | `true` | Use exponential backoff for gRPC reconnections |
| `maxBackoffDelayMs` | long | `60000` | Maximum backoff delay for gRPC reconnections (60 seconds) |
| `rlnJniLibPath` | Optional<String> | empty | Optional explicit path to the rln_jni native library |

---

## RPC/Estimate Gas Options

**Source:** `LineaRpcCliOptions.java`

These options control how the RPC endpoints (`linea_estimateGas`) behave with RLN and gasless transactions.

### Gasless Transaction Features

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--plugin-linea-rpc-gasless-enabled` | boolean | `false` | Enable gasless transaction features for `linea_estimateGas` |
| `--plugin-linea-rpc-rln-prover-forwarder-enabled` | boolean | `false` | Enable RLN prover forwarder for `linea_estimateGas` (RPC mode) |
| `--plugin-linea-rpc-premium-gas-multiplier` | double | `1.5` | Gas multiplier for denied users in `linea_estimateGas` (1.5 = 50% premium) |
| `--plugin-linea-rpc-allow-zero-gas-estimation-gasless` | boolean | `false` | Allow zero gas estimation for users with karma balance |

### Gas Estimation Compatibility

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--plugin-linea-estimate-gas-compatibility-mode-enabled` | boolean | `false` | Return min mineable gas price × multiplier instead of profitable price |
| `--plugin-linea-estimate-gas-compatibility-mode-multiplier` | BigDecimal | `1.2` | Multiplier to apply to min priority fee per gas in compatibility mode |

---

## Profitability & Gasless Options

**Source:** `LineaProfitabilityCliOptions.java`

These options control transaction profitability checks. For gasless/RLN networks, these are typically **disabled** or set to **zero**.

### Gas Cost Configuration

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--plugin-linea-fixed-gas-cost-wei` | long | `0` | Fixed gas cost in Wei (set to 0 for gasless) |
| `--plugin-linea-variable-gas-cost-wei` | long | `1000000000` | Variable gas cost in Wei (set to 0 for gasless) |

### Margin Configuration

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--plugin-linea-min-margin` | BigDecimal | `1.0` | Minimum margin of a transaction to be selected (set to 0 for gasless) |
| `--plugin-linea-estimate-gas-min-margin` | BigDecimal | `1.0` | Recommend specific gas price when using `linea_estimateGas` |
| `--plugin-linea-tx-pool-min-margin` | BigDecimal | `0.5` | Min margin for incoming tx to be accepted in txpool (set to 0 for gasless) |

### Profitability Check Toggles

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--plugin-linea-tx-pool-profitability-check-api-enabled` | boolean | `true` | Enable profitability check for txs received via API (disable for gasless) |
| `--plugin-linea-tx-pool-profitability-check-p2p-enabled` | boolean | `false` | Enable profitability check for txs received via P2P (disable for gasless) |

### Extra Data Pricing

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--plugin-linea-extra-data-pricing-enabled` | boolean | `false` | Enable setting pricing parameters via extra data field |
| `--plugin-linea-extra-data-set-min-gas-price-enabled` | boolean | `true` | Enable setting min gas price runtime value via extra data field |
| `--plugin-linea-profitability-metrics-buckets` | double[] | `[0.1, 0.3, 0.5, 0.7, 0.9, 1.0, 1.1, 1.2, 1.5, 2.0, 5.0, 10.0]` | Buckets for profitability ratio histogram metrics |

---

## Transaction Pool Validator Options

**Source:** `LineaTransactionPoolValidatorCliOptions.java`

These options control general transaction validation in the pool (separate from RLN-specific deny list).

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--plugin-linea-deny-list-path` | string | `lineaDenyList.txt` | Path to the file containing the deny list (address-based) |
| `--plugin-linea-bundle-overriding-deny-list-path` | string | (uses deny-list-path) | Path to the file containing the deny list for bundles |
| `--plugin-linea-max-tx-gas-limit` | int | `30000000` | Maximum gas limit for a transaction |
| `--plugin-linea-max-tx-calldata-size` | int | `60000` | Maximum size for the calldata of a transaction |
| `--plugin-linea-tx-pool-simulation-check-api-enabled` | boolean | `false` | Enable simulation check for txs received via API |
| `--plugin-linea-tx-pool-simulation-check-p2p-enabled` | boolean | `false` | Enable simulation check for txs received via P2P |

---

## Shared Gasless Configuration

**Source:** `LineaSharedGaslessConfiguration.java`

These are internal configuration parameters used by both RLN validator and RPC components. They are set programmatically from CLI options.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `denyListCacheRefreshSeconds` | long | `60` | Interval in seconds for local cache cleanup of expired entries |
| `premiumGasPriceThresholdGWei` | long | `100` | Minimum gas price (in GWei) for premium transactions to bypass deny list |
| `denyListEntryMaxAgeMinutes` | long | `10` | Maximum age in minutes for a deny list entry before it expires |
| `nullifierStoragePath` | string | `/var/lib/besu/nullifiers.txt` | Path to the file for storing nullifier tracking data |

**Note:** The deny list is now stored in the RLN Prover's PostgreSQL database and accessed via gRPC. These local cache settings control how the sequencer caches deny list information.

---

## Transaction Selector Options

**Source:** `LineaTransactionSelectorCliOptions.java`

These control block building and transaction selection (includes profitability for regular blocks).

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--plugin-linea-max-block-calldata-size` | int | `70000` | Maximum size for the calldata of a block |
| `--plugin-linea-over-line-count-limit-cache-size` | int | `10000` | Max number of txs that go over line count limit to track |
| `--plugin-linea-max-block-gas` | long | `30000000` | Max gas per block |
| `--plugin-linea-max-bundle-pool-size-bytes` | long | `16777216` | Max memory size in bytes for bundle txpool (16 MB) |
| `--plugin-linea-max-bundle-block-gas` | long | `15000000` | Max amount of block gas bundle transactions can use |
| `--plugin-linea-events-deny-list-path` | string | - | Path to file containing events deny list (CSV format) |
| `--plugin-linea-events-bundle-deny-list-path` | string | - | Path to file containing events deny list for bundles |
| `--plugin-linea-profitability-check-enabled` | boolean | `true` | Enable profitability check during block selection (disable for gasless) |

---

## Mode-Specific Configurations

### Sequencer Mode (Validates Proofs Locally)

**Required validators:**
- ✅ RLN Verifier Validator (validates ZK proofs)
- ✅ Linea Estimate Gas
- ❌ RLN Prover Forwarder (disabled, validates locally)

**Key settings:**
```bash
--plugin-linea-rln-enabled=true
--plugin-linea-rln-verifying-key=/var/lib/besu/rln/verifying_key.json  # REQUIRED
--plugin-linea-rln-proof-service=rln-prover:50051
--plugin-linea-rln-karma-service=rln-prover:50051
--plugin-linea-rpc-rln-prover-forwarder-enabled=false  # Validates locally
```

**Gasless configuration:**
```bash
# Disable profitability checks
--plugin-linea-tx-pool-profitability-check-api-enabled=false
--plugin-linea-tx-pool-profitability-check-p2p-enabled=false
--plugin-linea-fixed-gas-cost-wei=0
--plugin-linea-variable-gas-cost-wei=0
--plugin-linea-min-margin=0
--plugin-linea-tx-pool-min-margin=0
--plugin-linea-profitability-check-enabled=false

# Enable gasless features
--plugin-linea-rpc-gasless-enabled=true
--plugin-linea-rpc-allow-zero-gas-estimation-gasless=true
--plugin-linea-rpc-premium-gas-multiplier=1.5
```

### RPC Mode (Forwards to Prover)

**Required validators:**
- ✅ RLN Prover Forwarder (forwards to prover for validation)
- ✅ Linea Estimate Gas
- ❌ RLN Verifier Validator (disabled, no local proof verification)

**Key settings:**
```bash
--plugin-linea-rln-enabled=true
--plugin-linea-rln-proof-service=rln-prover:50051
--plugin-linea-rln-karma-service=rln-prover:50051
--plugin-linea-rpc-rln-prover-forwarder-enabled=true  # Forwards to prover
# No verifying key needed (prover validates)
```

**Gasless configuration:** Same as Sequencer mode

### Follower Mode (RLN Disabled)

**Key settings:**
```bash
--plugin-linea-rln-enabled=false
```

All RLN features are disabled. Standard validators only.

---

## Epoch Modes Explained

The `--plugin-linea-rln-epoch-mode` option determines how quota epochs are calculated:

| Mode | Description | Use Case |
|------|-------------|----------|
| `TEST` | 30-second epochs | Testing and development |
| `TIMESTAMP_1H` | 1-hour epochs | Production (default) |
| `BLOCK` | Per-block epochs | Special cases requiring per-block quotas |
| `FIXED_FIELD_ELEMENT` | Fixed epoch value | Custom/experimental configurations |

---

## Premium Gas Pricing

Users on the deny list can bypass restrictions by paying premium gas prices:

**Formula:** `required_gas_price = base_gas_price × premium_multiplier`

**Example with `--plugin-linea-rpc-premium-gas-multiplier=1.5`:**
- Base gas price: 10 GWei
- Premium gas price: 15 GWei (50% premium)

**Threshold bypass:** If `--plugin-linea-rln-premium-gas-threshold-gwei=100`, any transaction with gas price ≥ 100 GWei bypasses deny list checks entirely.

---

## Events Deny List Format

The events deny list files (`--plugin-linea-events-deny-list-path`) use CSV format:

```
address,topic0,topic1,topic2,topic3
0x1234...abcd,0xddf2...7b1e,,,
0x5678...ef01,,0x0000...0001,,
```

- Empty fields are allowed (no filter for that topic)
- All topics must be 32-byte hex values (0x-prefixed)
- Address must be 20-byte hex value (0x-prefixed)

---

## Configuration Files Hierarchy

1. **TOML Config Files** (`*.config.toml`): Base configuration
   - `sequencer-rln.config.toml`
   - `l2-node-besu-rln.config.toml`

2. **Docker Compose Overrides** (`compose-spec-l2-services-rln.yml`): RLN-specific runtime overrides

3. **Java Configuration Classes**:
   - `LineaRlnValidatorConfiguration.java` - Core RLN settings
   - `LineaSharedGaslessConfiguration.java` - Shared deny list/premium gas
   - `LineaRpcConfiguration.java` - RPC-specific gasless settings
   - `LineaProfitabilityConfiguration.java` - Profitability checks
   - `LineaTransactionPoolValidatorConfiguration.java` - Pool validation

---

## Quick Reference: Common Tasks

### Disable RLN
```bash
--plugin-linea-rln-enabled=false
```

### Change Premium Gas Multiplier (to 2x)
```toml
plugin-linea-rpc-premium-gas-multiplier=2.0
```

### Change Premium Gas Threshold (to 50 GWei)
```bash
--plugin-linea-rln-premium-gas-threshold-gwei=50
```

### Change Epoch Mode (to 1-hour epochs)
```bash
--plugin-linea-rln-epoch-mode=TIMESTAMP_1H
```

### Enable/Disable Gasless Transactions
```toml
plugin-linea-rpc-gasless-enabled=true
plugin-linea-rpc-allow-zero-gas-estimation-gasless=true
```

---

## Related Documentation

- `docker/RLN-CONFIGURATION-GUIDE.md` - Practical guide for Docker deployments
- `besu-plugins/linea-sequencer/sequencer/src/main/java/net/consensys/linea/config/` - Source configuration classes

