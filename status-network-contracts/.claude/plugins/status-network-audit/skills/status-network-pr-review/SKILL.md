---
name: status-network-pr-review
description: "Security-focused review of PR changes in Status Network contracts. Orchestrates Trail of Bits plugins scoped to changed files only. Use when reviewing pull requests or before merging."
allowed-tools:
  - Read
  - Grep
  - Glob
  - Bash
  - Write
  - Task
  - Skill
---

# PR Security Review

Performs a focused security review of changes in a pull request by orchestrating Trail of Bits plugins scoped to only the modified Solidity files.

## When to Use

- Reviewing pull requests before merge
- Checking your own changes before pushing
- Quick security validation of a feature branch

## When NOT to Use

- For full codebase audits (use `/status-network-full-audit`)
- When there are no Solidity file changes

## Step 1: Identify Changed Files

First, get the list of changed Solidity files:

```bash
# Get changed .sol files compared to develop branch
git diff --name-only develop...HEAD -- '*.sol' | grep '^src/'

# Or for unstaged changes
git diff --name-only -- '*.sol' | grep '^src/'
```

If no Solidity files in `src/` changed, report "No security-relevant changes" and exit.

## Step 2: Get the Diff

```bash
# Show the actual changes with context
git diff -U10 develop...HEAD -- src/
```

## Step 3: Run Targeted Audits

For each changed file, run the relevant Trail of Bits plugins:

### Entry Point Changes

**Invoke:** `/entry-point-analyzer` (scoped to changed files)

Check for:
- New external/public functions added without access control
- Access modifiers removed or changed
- New state-changing functions

**Status Network access patterns:**
- `onlyRole(...)` - Role-based access
- `onlyOwner` - Owner-only
- `onlyTrustedCodehash` - Vault implementations only
- `onlyVaultFactory`, `onlyRewardsSupplier`, `onlyRegisteredVault`

### Vulnerability Patterns

**Invoke:** `/solidity-vulnerability-scanner` (scoped to changed files)

Focus on changes that might introduce:
- Reentrancy (external calls before state updates)
- Access control gaps
- Arithmetic issues in new calculations
- Unvalidated inputs

### Configuration Changes

**Invoke:** `/sharp-edges` (if config-related files changed)
**Invoke:** `/insecure-defaults` (if initialization logic changed)

Check if changes affect:
- Default values
- Admin-configurable parameters
- Initialization logic

## Step 4: Cross-Reference Impact

For each changed contract, identify:

1. **What depends on it?**
   - StakeManager ← StakeVault, VaultFactory
   - Karma ← StakeManager, SimpleKarmaDistributor
   - IRewardDistributor ← Karma, StakeManager

2. **Could this break integrations?**
   - Function signature changes
   - Return value changes
   - Event changes

3. **Are there corresponding test changes?**
   - Check `test/` for matching test updates

## Step 5: Generate Report

Output format for PR comment or `local-audit/pr-review-{branch}.md`:

```markdown
# PR Security Review

**Branch:** {branch-name}
**Base:** develop
**Changed Files:** {count} Solidity files

## Changed Files Summary

| File | Lines Changed | Risk Level |
|------|---------------|------------|
| src/StakeManager.sol | +45, -12 | High |
| src/StakeVault.sol | +10, -5 | Medium |

## Entry Point Analysis
[Results from /entry-point-analyzer on changed files]

### New Entry Points
- `newFunction()` in StakeManager.sol - **No access control!**

### Modified Entry Points
- `stake()` - Added parameter validation

## Vulnerability Check
[Results from /solidity-vulnerability-scanner on changed files]

## Configuration Review
[Results from /sharp-edges if applicable]

## Findings

### [HIGH] Missing Access Control
**File:** `src/StakeManager.sol:234`
**Change:** New function `newFunction()` added without modifier
**Recommendation:** Add `onlyRole(OPERATOR_ROLE)` or appropriate access control

### [MEDIUM] Storage Layout Change
**File:** `src/StakeManager.sol:45`
**Change:** New storage variable added
**Recommendation:** Verify storage gap is adjusted for upgrade safety

### [LOW] Missing Event
**File:** `src/StakeVault.sol:156`
**Change:** State change without event emission
**Recommendation:** Add event for off-chain tracking

## Checklist

- [ ] New functions have appropriate access control
- [ ] No external calls before state updates
- [ ] New parameters are validated
- [ ] Storage layout safe for upgrades
- [ ] Events emitted for state changes
- [ ] Tests added for new functionality

## Recommendation

[ ] Safe to merge
[ ] Requires changes (see findings above)
[ ] Needs further review
```

## Status Network-Specific Checks

When reviewing PRs, pay special attention to:

### Staking Logic (StakeManager.sol, StakeVault.sol)
- Lock period manipulation
- Reward calculation changes
- MP accrual logic
- Migration safety

### Token Logic (Karma.sol)
- Transfer restrictions
- Slashing calculations
- Distributor interactions

### Access Control (TrustedCodehashAccess.sol)
- Codehash whitelist changes
- Role assignments

### Upgradeability
- Storage gaps maintained
- Initializer protection
- No constructor state
