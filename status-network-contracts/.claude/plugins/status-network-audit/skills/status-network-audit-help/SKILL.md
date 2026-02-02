---
name: status-network-audit-help
description: "Lists all available security audit commands for Status Network contracts. Use when developers need to discover audit tools, understand audit workflow, or get help with security analysis options."
allowed-tools:
  - Read
---

# Status Network Audit Help

Overview of security audit commands for Status Network smart contracts.

## First Time Setup

Before using audit commands, install the required plugins:

```bash
# Install local Status Network audit plugin
/plugin marketplace add ./.claude/plugins
/plugin install status-network-audit

# Install Trail of Bits plugins
/plugin marketplace add trailofbits/skills
/plugin install building-secure-contracts entry-point-analyzer property-based-testing insecure-defaults audit-context-building sharp-edges
```

## Available Commands

### Local Orchestration Skills

These skills orchestrate Trail of Bits plugins with Status Network-specific context:

| Command | Description |
|---------|-------------|
| `/status-network-audit-help` | This help |
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

## Recommended Workflows

### Full Audit (before deployment)

```
/status-network-full-audit
```

This orchestrates all ToB plugins and generates a comprehensive report in `local-audit/full-audit-YYYY-MM-DD.md`.

### PR Review (before merge)

```
/status-network-pr-review
```

This scopes the audit to changed files only and generates a focused report.

### Quick Checks (individual plugins)

```bash
# Map attack surface
/entry-point-analyzer

# Check for vulnerabilities
/solidity-vulnerability-scanner

# Review configurations
/sharp-edges
/insecure-defaults

# Generate fuzz tests
/property-based-testing
```

## Contract Architecture

```
User → VaultFactory.createVault() → StakeVault (clone)
     → StakeVault.stake(SNT) → StakeManager tracks balance + locks + MP
     → StakeManager accrues rewards → IRewardDistributor
     → User.redeemRewards() → actual Karma tokens minted
```

### Core Contracts

| Contract | Purpose | Priority |
|----------|---------|----------|
| `Karma.sol` | ERC20-Votes token with slashing | Critical |
| `StakeManager.sol` | Staking orchestrator (UUPS) | Critical |
| `StakeVault.sol` | Per-user vault (proxy clones) | Critical |
| `VaultFactory.sol` | Vault creation | High |
| `SimpleKarmaDistributor.sol` | Virtual reward tracking | High |

### Access Control Roles

| Role | Permissions |
|------|-------------|
| `DEFAULT_ADMIN_ROLE` | Upgrades, configuration, distributors |
| `OPERATOR_ROLE` | Set rewards, mint Karma |
| `GUARDIAN_ROLE` | Pause, emergency mode |
| `SLASHER_ROLE` | Slash misbehaving accounts |

## Audit Output

All reports are saved to `local-audit/`:
- `full-audit-YYYY-MM-DD.md` - Comprehensive audit
- `pr-review-{branch}.md` - PR-specific review
- `sharp-edges.md` - Configuration issues
- `entry-points.md` - Attack surface map
