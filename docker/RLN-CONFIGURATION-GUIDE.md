# RLN Configuration Guide

This guide explains how the RLN (Rate Limiting Nullifier) configuration is structured and how to easily modify settings.

## Configuration Architecture

### Separation of Concerns

1. **TOML Config Files** (`*.config.toml`): Base configuration for node behavior
2. **Docker Compose** (`compose-spec-l2-services-rln.yml`): RLN-specific overrides and service endpoints

This separation ensures:
- Configuration reusability across environments
- Clear distinction between base settings and RLN-specific settings
- Easier maintenance and debugging

---

## Configuration Files

### Sequencer Configuration

**File:** `config/linea-besu-sequencer/sequencer-rln.config.toml`

Key RLN-related settings in the TOML:
```toml
# Gasless network - no profitability checks
plugin-linea-tx-pool-profitability-check-api-enabled=false
plugin-linea-tx-pool-profitability-check-p2p-enabled=false
plugin-linea-fixed-gas-cost-wei=0
plugin-linea-variable-gas-cost-wei=0
plugin-linea-min-margin="0"
plugin-linea-tx-pool-min-margin="0"

# RPC/Estimate gas settings
plugin-linea-rpc-gasless-enabled=true
plugin-linea-rpc-rln-prover-forwarder-enabled=false  # Sequencer validates locally
plugin-linea-rpc-allow-zero-gas-estimation-gasless=true
plugin-linea-estimate-gas-compatibility-mode-enabled=true
plugin-linea-estimate-gas-compatibility-mode-multiplier=1.2
```

**Docker Compose overrides:**
```yaml
--plugin-linea-rln-enabled=true
--plugin-linea-rln-verifying-key=/var/lib/besu/rln/verifying_key.json
--plugin-linea-rln-proof-service=rln-prover:50051
--plugin-linea-rln-karma-service=rln-prover:50051
--plugin-linea-rln-timeouts-ms=5000
--plugin-linea-rln-premium-gas-threshold-gwei=0
```

### RPC Node Configuration

**File:** `config/l2-node-besu/l2-node-besu-rln.config.toml`

Key differences from sequencer:
```toml
# RPC node forwards proof generation to prover
plugin-linea-rpc-rln-prover-forwarder-enabled=true

# Same gasless and premium gas settings as sequencer
plugin-linea-rpc-gasless-enabled=true
```

**Docker Compose overrides:**
```yaml
--plugin-linea-rln-enabled=true
--plugin-linea-rln-proof-service=rln-prover:50051
--plugin-linea-rln-karma-service=rln-prover:50051
--plugin-linea-rln-timeouts-ms=5000
--plugin-linea-rln-premium-gas-threshold-gwei=0
```

---

## Node Mode Configurations

### Sequencer Mode (validates proofs locally)

**Enabled validators:**
- ✅ RLN Verifier Validator (validates ZK proofs)
- ✅ Linea Estimate Gas
- ❌ RLN Prover Forwarder (not needed, validates locally)

**Key settings:**
- `--plugin-linea-rln-enabled=true`
- `--plugin-linea-rln-verifying-key=/path/to/verifying_key.json` (required)
- `--plugin-linea-rpc-rln-prover-forwarder-enabled=false`

### RPC Mode (forwards to prover)

**Enabled validators:**
- ✅ RLN Prover Forwarder (forwards to prover for validation)
- ✅ Linea Estimate Gas
- ❌ RLN Verifier Validator (disabled, no local proof verification)

**Key settings:**
- `--plugin-linea-rln-enabled=true`
- `--plugin-linea-rpc-rln-prover-forwarder-enabled=true`
- No verifying key needed (prover does the validation)

### Follower Mode (RLN disabled)

**Enabled validators:**
- Standard validators only
- ❌ All RLN features disabled

**Key settings:**
- `--plugin-linea-rln-enabled=false`

---

## Common Configuration Tasks

### 1. Change Premium Gas Threshold

**What it does:** Transactions with gas price ≥ threshold bypass deny list checks

**Where to change:**
```yaml
# In compose-spec-l2-services-rln.yml
--plugin-linea-rln-premium-gas-threshold-gwei=0  # Change this value
```

**Examples:**
- `0` = always check deny list (current setting)
- `12` = bypass deny list if gas price ≥ 12 GWei (default)
- `100` = bypass deny list if gas price ≥ 100 GWei

### 3. Enable/Disable RLN

**To disable RLN on a node:**
```yaml
# In compose-spec-l2-services-rln.yml
--plugin-linea-rln-enabled=false
```

**To enable RLN on a node:**
```yaml
--plugin-linea-rln-enabled=true
--plugin-linea-rln-proof-service=rln-prover:50051
--plugin-linea-rln-karma-service=rln-prover:50051
# ... other RLN settings
```

### 4. Adjust RLN Service Timeouts

**What it does:** How long to wait for RLN prover/karma service responses

**Where to change:**
```yaml
# In compose-spec-l2-services-rln.yml
--plugin-linea-rln-timeouts-ms=5000  # 5 seconds
```

### 5. Configure Gasless Transactions

**Enable/disable gasless features:**
```toml
# In config files
plugin-linea-rpc-gasless-enabled=true  # Enable gasless transactions
plugin-linea-rpc-allow-zero-gas-estimation-gasless=true  # Allow 0 gas estimates
```

---

## CLI Options Reference

### RLN Validator Options

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--plugin-linea-rln-enabled` | Enable RLN validation | `false` | Yes |
| `--plugin-linea-rln-verifying-key` | Path to verifying key | - | Sequencer only |
| `--plugin-linea-rln-proof-service` | Proof service endpoint | `localhost:50051` | Yes |
| `--plugin-linea-rln-karma-service` | Karma service endpoint | `localhost:50052` | Yes |
| `--plugin-linea-rln-timeouts-ms` | Service timeout | `5000` | No |
| `--plugin-linea-rln-premium-gas-threshold-gwei` | Premium bypass threshold | `12` | No |

### RPC/Estimate Gas Options

| Option | Description | Default |
|--------|-------------|---------|
| `--plugin-linea-rpc-gasless-enabled` | Enable gasless transactions | `false` |
| `--plugin-linea-rpc-rln-prover-forwarder-enabled` | Forward to prover (RPC mode) | `false` |
| `--plugin-linea-rpc-allow-zero-gas-estimation-gasless` | Allow 0 gas estimates | `false` |
| `--plugin-linea-estimate-gas-compatibility-mode-enabled` | Compatibility mode | `false` |
| `--plugin-linea-estimate-gas-compatibility-mode-multiplier` | Compatibility multiplier | `1.2` |

### Profitability Options (Gasless = all disabled)

| Option | Description | Gasless Setting |
|--------|-------------|-----------------|
| `--plugin-linea-fixed-gas-cost-wei` | Fixed cost per tx | `0` |
| `--plugin-linea-variable-gas-cost-wei` | Variable cost per gas | `0` |
| `--plugin-linea-min-margin` | Min margin for block selection | `0` |
| `--plugin-linea-tx-pool-min-margin` | Min margin for tx pool | `0` |
| `--plugin-linea-tx-pool-profitability-check-api-enabled` | Check profitability (API) | `false` |
| `--plugin-linea-tx-pool-profitability-check-p2p-enabled` | Check profitability (P2P) | `false` |

---

## Troubleshooting

### l2-node-besu fails to start

**Symptoms:** Node exits immediately or fails healthcheck

**Common causes:**
1. Missing RLN native library (`librln_bridge.so`)
2. RLN prover not healthy
3. Missing configuration file

**Solutions:**
```yaml
# Ensure library is mounted:
volumes:
  - ../tmp/libs_fix/librln_bridge.so:/opt/besu/lib/native/librln_bridge.so

# Ensure prover dependency:
depends_on:
  rln-prover:
    condition: service_healthy

# Check environment variables:
environment:
  LD_LIBRARY_PATH: "/opt/besu/lib/native:/usr/local/lib:/usr/lib"
  JAVA_LIBRARY_PATH: "/opt/besu/lib/native"
```

### RLN validation not working

**Check:**
1. RLN prover is running and healthy
2. Verifying key file exists (sequencer only)
3. Service endpoints are correct
4. Network connectivity between containers

### Premium gas not applied

**Check:**
1. `plugin-linea-rln-premium-gas-threshold-gwei` is set appropriately
2. User is actually on the deny list

---

## Production vs Test Configuration

### Test Configuration (Current)
```yaml
--plugin-linea-rln-premium-gas-threshold-gwei=0  # Always check deny list
--plugin-linea-rln-timeouts-ms=5000  # 5 second timeout
```

### Production Configuration (Example)
```yaml
--plugin-linea-rln-premium-gas-threshold-gwei=12  # Bypass at 12 GWei
--plugin-linea-rln-timeouts-ms=10000  # 10 second timeout
```

---

## Summary

**Key Takeaways:**
1. ✅ Base configuration lives in `.toml` files
2. ✅ RLN-specific overrides in docker-compose
3. ✅ Sequencer validates proofs locally (needs verifying key)
4. ✅ RPC nodes forward to prover (no verifying key needed)
5. ✅ Premium gas multiplier configured in `.toml` files
6. ✅ Service endpoints configured in docker-compose

This architecture makes it easy to:
- Switch between RLN and non-RLN modes
- Adjust premium gas settings
- Configure different node types
- Maintain consistent base configuration

