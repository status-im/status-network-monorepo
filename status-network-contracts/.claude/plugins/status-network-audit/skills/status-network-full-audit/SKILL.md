---
name: status-network-full-audit
description: "Runs a comprehensive security audit of Status Network contracts by orchestrating Trail of Bits plugins. Combines entry point analysis, vulnerability scanning, configuration review, and invariant testing. Use for thorough security review before deployments or major releases."
allowed-tools:
  - Read
  - Grep
  - Glob
  - Bash
  - Write
  - Task
  - Skill
---

# Full Security Audit

Orchestrates multiple Trail of Bits security plugins to perform a comprehensive audit of the Status Network smart contract codebase.

## When to Use

- Before mainnet deployments
- After significant code changes
- During periodic security reviews
- When onboarding external auditors (as a baseline)

## When NOT to Use

- For quick spot checks (use individual ToB plugins directly)
- For PR reviews (use `/status-network-pr-review`)

## Audit Execution

**IMPORTANT:** Execute each phase by invoking the Trail of Bits plugins using the Skill tool. Collect all findings into a single report.

### Phase 1: Entry Point Analysis

**Invoke:** `/entry-point-analyzer`

Focus the analysis on these Status Network contracts:
- `src/Karma.sol` - Token + slashing (Critical)
- `src/StakeManager.sol` - Core staking logic (Critical)
- `src/StakeVault.sol` - User fund management (Critical)
- `src/VaultFactory.sol` - Vault creation (High)
- `src/SimpleKarmaDistributor.sol` - Reward distribution (High)
- `src/KarmaAirdrop.sol` - Token distribution (Medium)
- `src/KarmaTiers.sol` - Tier management (Medium)
- `src/rln/RLN.sol` - Rate limiting (Medium)
- `src/TrustedCodehashAccess.sol` - Access control base (High)

**Status Network-specific access patterns to look for:**
- `onlyRole(DEFAULT_ADMIN_ROLE)` - Admin functions
- `onlyRole(OPERATOR_ROLE)` - Operational functions
- `onlyRole(GUARDIAN_ROLE)` - Emergency functions
- `onlyRole(SLASHER_ROLE)` - Slashing capability
- `onlyTrustedCodehash` - Vault-only functions
- `onlyVaultFactory` - Factory-only functions
- `onlyRewardsSupplier` - Karma token only
- `onlyRegisteredVault` - Registered vaults only

### Phase 2: Vulnerability Scanning

**Invoke:** `/solidity-vulnerability-scanner`

**Status Network-specific concerns:**
- Staking: lock bypass, reward manipulation, MP gaming
- Slashing: griefing vectors, reward gaming, MIN_SLASH_AMOUNT bypass
- Migration: fund loss during vault migration
- Upgradeability: storage collisions in UUPS contracts
- Reentrancy: check `redeemRewards()`, `leave()`, `emergencyExit()`

### Phase 3: Configuration & Sharp Edges

**Invoke:** `/sharp-edges`
**Invoke:** `/insecure-defaults`

**Status Network-specific configuration concerns:**

| Setting | Dangerous Values | Location |
|---------|------------------|----------|
| `slashPercentage` | 0 (inconsistent behavior) | Karma.sol |
| `slashRewardPercentage` | 10000 (100% to slasher) | Karma.sol |
| `maxVaultsPerUser` | Very high (gas DoS) | StakeManager.sol |
| `emergencyModeEnabled` | Irreversible once set | StakeManager.sol |
| `rewardsSupplier` | Unset after init | StakeManager.sol |
| `vaultFactory` | Unset after init | StakeManager.sol |
| `vaultImplementation` | Zero address | VaultFactory.sol |

### Phase 4: Code Maturity Assessment

**Invoke:** `/code-maturity-assessor`

Focus on:
- Arithmetic safety in reward calculations (StakeMath.sol)
- Access control completeness
- Test coverage for edge cases
- Documentation quality

### Phase 5: Invariant Identification

**Invoke:** `/property-based-testing`

**Status Network invariants to test:**

```solidity
// Staking Invariants
totalStaked == sum(vault.stakedBalance for all vaults)
totalMPStaked == sum(vault.mpAccrued for all vaults)
vault.stakedBalance <= vault.depositedBalance
lockUntil can only increase, never decrease

// Token Invariants
Karma transfers only allowed for whitelisted addresses
Slashing removes at least MIN_SLASH_AMOUNT (1 ether)

// Reward Invariants
totalRewardsAccrued <= rewardAmount
User rewards proportional to (stakedBalance + mpAccrued)
```

### Phase 6: Trust Boundary Mapping

Document the trust hierarchy:

```
ADMIN (highest trust)
  ├─ Upgrade contracts (UUPS)
  ├─ Set trusted codehashes
  ├─ Configure slash parameters
  └─ Add/remove reward distributors

OPERATOR
  ├─ Set rewards
  └─ Mint Karma

GUARDIAN
  ├─ Pause/unpause
  └─ Enable emergency mode (irreversible!)

SLASHER
  └─ Slash accounts (with commit-reveal in RLN)

VAULT OWNER (user)
  ├─ Stake/unstake own funds
  ├─ Lock funds
  └─ Migrate between own vaults
```

## Report Generation

Compile all findings into `local-audit/full-audit-YYYY-MM-DD.md`:

```markdown
# Full Security Audit - Status Network Contracts

**Date:** YYYY-MM-DD
**Auditor:** Claude Code + Trail of Bits Plugins

## Executive Summary

| Severity | Count |
|----------|-------|
| Critical | X |
| High | X |
| Medium | X |
| Low | X |
| Info | X |

## Phase 1: Entry Points
[Results from /entry-point-analyzer]

## Phase 2: Vulnerability Scan
[Results from /solidity-vulnerability-scanner]

## Phase 3: Configuration Issues
[Results from /sharp-edges and /insecure-defaults]

## Phase 4: Code Maturity
[Results from /code-maturity-assessor]

## Phase 5: Invariants for Fuzzing
[Results from /property-based-testing]

## Phase 6: Trust Boundaries
[Trust hierarchy diagram]

## Consolidated Findings

### [SEVERITY] Finding Title
**Location:** `src/Contract.sol:123`
**Description:** ...
**Recommendation:** ...

## Recommendations (Prioritized)
1. ...
2. ...
```

## Rationalizations to Reject

| Rationalization | Why It's Wrong |
|-----------------|----------------|
| "It's admin-only" | Admins get compromised; minimize blast radius |
| "It's not exploitable" | Combine with other issues; defense in depth |
| "Gas costs prevent abuse" | L2 costs are low; don't rely on gas |
| "Tests pass" | Tests don't cover all edge cases |
| "It's upgradeable, we'll fix it" | Upgrades require coordination; fix now |
