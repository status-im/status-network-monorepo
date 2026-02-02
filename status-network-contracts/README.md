# Status Network Contracts

Smart contracts for Status Network's Karma reputation system, which enables gasless transactions.

## Overview

This repository contains the core smart contracts that power the Status Network's reputation and reward system. The system is built with modularity and upgradability in mind.

### Main Components

- **Karma Token**: An ERC20 reputation token used throughout the Status Network ecosystem. Karma enables free transactions and access to network features.
- **Karma NFT**: A non-transferable ERC721 token that visually represents an account's Karma level.
- **Reward Distributors**: Contracts that implement custom reward distribution logic, including the staking reward distributor and simple distributor variants.
- **Staking System**: Allows users to stake SNT tokens to earn Karma rewards.
- **Karma Airdrop**: Merkle tree-based airdrop mechanism for distributing Karma to accounts.

For a comprehensive overview of the system architecture, see [System Overview](docs/system-overview.md).

## Documentation

Detailed documentation is available in the [`docs`](docs) folder:

- [System Overview](docs/system-overview.md) - Architecture and component interactions
- [Karma Token](docs/karma.md) - Token mechanics and features
- [Staking Reward Distributor](docs/staking-reward-distributor/overview.md) - How staking and rewards work
- [Reward Distributors](docs/reward-distributors.md) - Custom reward distribution logic
- [Deployment Guide](docs/deployment.md) - How to deploy the contracts
- [Security](SECURITY.md) - Security considerations and audit information

## Getting Started

### Prerequisites

- [Foundry](https://getfoundry.sh/) - Ethereum development toolkit
- [pnpm](https://pnpm.io/) - Package manager

### Installation

```sh
pnpm install
```

### Build

```sh
forge build
```

### Test

```sh
forge test
```

### Coverage

```sh
forge coverage
```

## Development

### Linting

```sh
pnpm lint
```

### Formatting

```sh
forge fmt
```

### Gas Reports

```sh
pnpm gas-report
forge snapshot
```


## Deployment

The recommended way to deploy the full protocol is using the `DeployProtocol` script:

```sh
MNEMONIC=$YOUR_MNEMONIC forge script script/DeployProtocol.s.sol --rpc-url $RPC_URL --broadcast
```

For detailed deployment instructions, including verification and network configuration, see the [Deployment Guide](docs/deployment.md).

## Claude Code (AI-Assisted Development)

This project includes Claude Code integration for AI-assisted security auditing.

### Setup

```bash
# Install local Status Network audit plugin
/plugin marketplace add ./.claude/plugins
/plugin install status-network-audit

# Install Trail of Bits plugins
/plugin marketplace add trailofbits/skills
/plugin install building-secure-contracts entry-point-analyzer property-based-testing insecure-defaults audit-context-building sharp-edges
```

### Available Audit Commands

**Local orchestration skills (this repo):**

| Command | Description |
|---------|-------------|
| `/status-network-audit-help` | List all audit commands and workflow |
| `/status-network-full-audit` | Comprehensive audit using all ToB plugins |
| `/status-network-pr-review` | Security review of PR changes |

**Trail of Bits plugins (direct use):**

| Command | Description |
|---------|-------------|
| `/entry-point-analyzer` | Map state-changing entry points |
| `/solidity-vulnerability-scanner` | 30+ Solidity vulnerability patterns |
| `/sharp-edges` | Find API footguns |
| `/insecure-defaults` | Detect dangerous configs |
| `/property-based-testing` | Generate Echidna/Medusa tests |

Audit reports are saved to `local-audit/`.

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details.
