# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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
