# Status Network Audit Plugin

Local Claude Code plugin providing security audit commands for the Status Network smart contracts.

## Installation

```bash
# Add the local marketplace
/plugin marketplace add ./.claude/plugins

# Install the plugin
/plugin install status-network-audit
```

To verify it's loaded:
```
/plugin list
```

## Available Commands

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

## Quick Start

```
/status-network-audit-help
```

This shows all available commands and the recommended audit workflow.

## Recommended Workflows

### Full Audit (before deployment)
```
/status-network-full-audit
```
Orchestrates all ToB plugins and generates a comprehensive report in `local-audit/full-audit-YYYY-MM-DD.md`.

### PR Review (before merge)
```
/status-network-pr-review
```
Scopes the audit to changed files only and generates a focused report.

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

## Required Trail of Bits Plugins

For full functionality, install these plugins from the Trail of Bits marketplace:

```
/plugin marketplace add trailofbits/skills

/plugin install building-secure-contracts
/plugin install entry-point-analyzer
/plugin install property-based-testing
/plugin install insecure-defaults
/plugin install audit-context-building
/plugin install sharp-edges
```

### Trail of Bits Plugin Commands

| Plugin | Command | Purpose |
|--------|---------|---------|
| `building-secure-contracts` | `/solidity-vulnerability-scanner` | Scans for 30+ Solidity vulnerability patterns |
| `entry-point-analyzer` | `/entry-point-analyzer` | Maps state-changing functions by access level |
| `property-based-testing` | Auto-triggers | Suggests Echidna/Medusa invariant tests |
| `insecure-defaults` | `/insecure-defaults` | Detects fail-open configurations |
| `audit-context-building` | `/audit-context-building` | Deep line-by-line code analysis |
| `sharp-edges` | `/sharp-edges` | Identifies API footguns and dangerous defaults |

## Output Location

All audit reports are saved to:

```
local-audit/
├── full-audit-YYYY-MM-DD.md # Comprehensive audit report
├── pr-review-{branch}.md    # PR-specific review
├── sharp-edges.md           # Configuration issues
└── entry-points.md          # Attack surface map
```

## Contract Architecture

```
User → VaultFactory.createVault() → StakeVault (clone)
     → StakeVault.stake(SNT) → StakeManager tracks balance + locks + MP
     → StakeManager accrues rewards → IRewardDistributor
     → User.redeemRewards() → actual Karma tokens minted
```

### Core Contracts
- `src/Karma.sol` - ERC20-Votes token with slashing
- `src/StakeManager.sol` - Staking orchestrator (UUPS upgradeable)
- `src/StakeVault.sol` - Per-user vault (proxy clones)
- `src/SimpleKarmaDistributor.sol` - Virtual reward tracking

### Security Patterns
- UUPS Upgradeable contracts
- Codehash whitelisting for vault implementations
- Role-based access control (ADMIN, OPERATOR, SLASHER, GUARDIAN)
- Emergency mode with irreversible activation

## Contributing

To add new audit skills:

1. Create a new folder in `skills/<skill-name>/`
2. Add `SKILL.md` with YAML frontmatter
3. Follow the existing skill templates

## License

MIT
