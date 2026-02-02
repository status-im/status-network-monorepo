# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Required Plugins Setup

**IMPORTANT FOR CLAUDE:** At the start of any security-related conversation (audits, vulnerability checks, security reviews), remind the user to install the required plugins if they haven't already:

```bash
# Install local Status Network audit plugin
/plugin marketplace add ./.claude/plugins
/plugin install status-network-audit

# Install Trail of Bits plugins
/plugin marketplace add trailofbits/skills
/plugin install building-secure-contracts entry-point-analyzer property-based-testing insecure-defaults audit-context-building sharp-edges
```

These plugins are required for the `/status-network-*` audit commands to work properly.

## Build & Test Commands

```bash
# Install dependencies
pnpm install

# Build contracts
forge build

# Run all tests
forge test

# Run a single test file
forge test --match-path test/stake-manager/Stake.t.sol

# Run a single test function
forge test --match-test test_StakeAmount

# Run tests with verbosity (show stack traces)
forge test -vvv

# Run tests with gas report
forge test --gas-report

# Code coverage
forge coverage

# Linting (Solidity + Prettier)
pnpm lint

# Format Solidity
forge fmt

# Format all (prettier + forge fmt + gas report)
pnpm adorno

# Formal verification (Certora)
pnpm verify                    # Run all specs
pnpm verify:stake_manager      # Run specific spec
```

## Architecture

This is a Foundry-based Solidity project implementing Status Network's Karma reputation system for gasless transactions.

### Core Components

**Karma Token (`src/Karma.sol`)** - UUPS-upgradeable ERC20-Votes token
- Non-transferable by default (whitelist controls transfers)
- Aggregates balances from multiple reward distributors via `IRewardDistributor`
- Supports slashing mechanism for misbehaving accounts

**Staking System**
- `StakeManager.sol` - UUPS-upgradeable orchestrator managing all user vaults
- `StakeVault.sol` - Per-user vault created as proxy clones via `VaultFactory`
- `TrustedCodehashAccess.sol` - Security boundary allowing only whitelisted vault implementations
- **Multiplier Points (MP)** - Time-weighted stake boost (max 4x over 4 years)

**Reward Distribution**
- `SimpleKarmaDistributor.sol` - Tracks virtual Karma balances, mints actual tokens on redemption
- Virtual rewards allow real-time accrual without constant transfers

**Other Components**
- `KarmaNFT.sol` - Non-transferable NFT representing Karma tier
- `KarmaTiers.sol` - Tier management with rate-limiting (txPerEpoch)
- `KarmaAirdrop.sol` - Merkle tree-based distribution
- `RLN.sol` - Rate-Limiting Nullifier for spam prevention with Poseidon hashing

### Data Flow

```
User → VaultFactory.createVault() → StakeVault (clone)
     → StakeVault.stake(SNT) → StakeManager tracks balance + locks + MP
     → StakeManager accrues rewards → IRewardDistributor
     → User.redeemRewards() → actual Karma tokens minted
```

### Key Patterns

- **UUPS Upgradeable**: Karma, StakeManager, SimpleKarmaDistributor, RLN
- **Proxy Clones**: StakeVault instances for gas efficiency
- **Interface-based**: `IRewardDistributor`, `IStakeManager`, `IStakeVault` for decoupling
- **Codehash Whitelisting**: Prevents unauthorized vault implementations

### Test Structure

Tests are organized by contract in `test/` with function-scoped files:
- `test/stake-manager/` - 24 test files covering Stake, Unstake, Lock, MP, etc.
- `test/karma/` - Token behavior tests
- `test/mocks/` - MockStakeManager, MockToken for isolated testing

### Deployment

```bash
# Deploy full protocol
MNEMONIC=$YOUR_MNEMONIC forge script script/DeployProtocol.s.sol --rpc-url $RPC_URL --broadcast
```

Individual deployment scripts in `script/Deploy*.s.sol`, upgrade scripts in `script/Upgrade*.s.sol`.

## Solidity Version & Config

- Solidity 0.8.26, Paris EVM
- Optimizer: 10,000 runs
- Fuzz testing: 1,000 runs (10,000 in CI)

## Security Audit Tools

This project includes a local Claude Code plugin for security auditing. Use `/status-network-audit-help` to see all available commands.

### Local Orchestration Skills

These skills orchestrate Trail of Bits plugins with Status Network-specific context:

| Command | Description |
|---------|-------------|
| `/status-network-audit-help` | Lists all audit commands and recommended workflow |
| `/status-network-full-audit` | Comprehensive audit using all ToB plugins |
| `/status-network-pr-review` | Security review of PR changes |

### Trail of Bits Plugins (Direct Use)

| Command | Description |
|---------|-------------|
| `/entry-point-analyzer` | Map state-changing entry points by access level |
| `/solidity-vulnerability-scanner` | Scan for 30+ Solidity vulnerability patterns |
| `/sharp-edges` | Find API footguns and dangerous defaults |
| `/insecure-defaults` | Detect fail-open configurations |
| `/property-based-testing` | Generate Echidna/Medusa invariant tests |
| `/code-maturity-assessor` | Assess code quality across 9 categories |
| `/audit-context-building` | Deep line-by-line code analysis |

### Recommended Workflows

**Full Audit (before deployment):**
```
/status-network-full-audit
```

**PR Review (before merge):**
```
/status-network-pr-review
```

**Quick Checks (individual plugins):**
```bash
/entry-point-analyzer           # Map attack surface
/solidity-vulnerability-scanner # Check for vulnerabilities
/sharp-edges                    # Review configurations
/insecure-defaults
/property-based-testing         # Generate fuzz tests
```

### Audit Output

All audit reports are saved to `local-audit/`:
- `full-audit-YYYY-MM-DD.md` - Comprehensive audit report
- `pr-review-{branch}.md` - PR-specific review
- `sharp-edges.md` - Configuration issues
- `entry-points.md` - Attack surface map
