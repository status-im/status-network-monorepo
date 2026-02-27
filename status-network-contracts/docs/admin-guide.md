# Admin Guide

This guide provides step-by-step instructions for common administrative tasks in the Karma reward and reputation system.
For an overview of the system architecture, see the [System Overview](system-overview.md).

## Table of Contents

- [Managing Roles and Ownership](#managing-roles-and-ownership)
- [Managing Reward Distributors](#managing-reward-distributors)
- [Distributing Rewards](#distributing-rewards)
- [Updating Tiers](#updating-tiers)
- [Whitelisting Transfers](#whitelisting-transfers)
- [Configuring Slashing](#configuring-slashing)
- [Upgrading Contracts](#upgrading-contracts)
- [Pausing the System](#pausing-the-system)
- [Enabling Emergency Mode](#enabling-emergency-mode)
- [Updating NFT Metadata](#updating-nft-metadata)
- [Managing the Airdrop](#managing-the-airdrop)
- [Troubleshooting](#troubleshooting)

---

## Managing Roles and Ownership

### Granting a role

Requires `DEFAULT_ADMIN_ROLE` on the target contract. Applies to StakeManager, Karma, SimpleKarmaDistributor, and RLN.

```bash
cast send <CONTRACT_ADDRESS> \
  "grantRole(bytes32,address)" \
  <ROLE_HASH> <ACCOUNT_ADDRESS> \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>
```

### Revoking a role

Requires `DEFAULT_ADMIN_ROLE` on the target contract.

```bash
cast send <CONTRACT_ADDRESS> \
  "revokeRole(bytes32,address)" \
  <ROLE_HASH> <ACCOUNT_ADDRESS> \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>
```

### Checking if an account has a role

```bash
cast call <CONTRACT_ADDRESS> \
  "hasRole(bytes32,address)(bool)" \
  <ROLE_HASH> <ACCOUNT_ADDRESS> \
  --rpc-url <RPC_URL>
```

### Role reference

| Role                 | Hash                                                                 | Used on                                          |
| -------------------- | -------------------------------------------------------------------- | ------------------------------------------------ |
| `DEFAULT_ADMIN_ROLE` | `0x0000000000000000000000000000000000000000000000000000000000000000` | StakeManager, Karma, SimpleKarmaDistributor, RLN |
| `GUARDIAN_ROLE`      | `0x55435dd261a4b9b3364963f7738a7a662ad9c84396d64be3365284bb7f0a5041` | StakeManager                                     |
| `OPERATOR_ROLE`      | `0x97667070c54ef182b0f5858b034beac1b6f3089aa2d3188bb1e8929f4fa9b929` | Karma, SimpleKarmaDistributor                    |
| `SLASHER_ROLE`       | `0x12b42e8a160f6064dc959c6f251e3af0750ad213dbecf573b4710d67d6c28e39` | Karma                                            |
| `REGISTER_ROLE`      | `keccak256("REGISTER_ROLE")`                                         | RLN                                              |

### Transferring ownership

Applies to VaultFactory, KarmaTiers, KarmaNFT, and metadata generators. Requires the current owner.

```bash
cast send <CONTRACT_ADDRESS> \
  "transferOwnership(address)" \
  <NEW_OWNER_ADDRESS> \
  --rpc-url <RPC_URL> \
  --account <CURRENT_OWNER_ACCOUNT>
```

KarmaAirdrop uses two-step ownership transfer. The new owner must accept:

```bash
# Step 1: Initiate (current owner)
cast send <KARMA_AIRDROP_ADDRESS> \
  "transferOwnership(address)" \
  <NEW_OWNER_ADDRESS> \
  --rpc-url <RPC_URL> \
  --account <CURRENT_OWNER_ACCOUNT>

# Step 2: Accept (new owner)
cast send <KARMA_AIRDROP_ADDRESS> \
  "acceptOwnership()" \
  --rpc-url <RPC_URL> \
  --account <NEW_OWNER_ACCOUNT>
```

---

## Managing Reward Distributors

For background, see [Reward Distributors](reward-distributors.md).

### Adding a new distributor

Requires `DEFAULT_ADMIN_ROLE` on Karma.

Three steps are needed to fully integrate a new distributor:

```bash
# 1. Register the distributor with Karma
cast send <KARMA_ADDRESS> \
  "addRewardDistributor(address)" \
  <DISTRIBUTOR_ADDRESS> \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>

# 2. Allow the distributor to transfer Karma (needed for redeemRewards)
cast send <KARMA_ADDRESS> \
  "setAllowedToTransfer(address,bool)" \
  <DISTRIBUTOR_ADDRESS> true \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>

# 3. Set Karma as the rewards supplier on the distributor
cast send <DISTRIBUTOR_ADDRESS> \
  "setRewardsSupplier(address)" \
  <KARMA_ADDRESS> \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>
```

Once registered, the distributor's virtual balances are included in `karma.balanceOf()` for all accounts.

### Removing a distributor

Requires `DEFAULT_ADMIN_ROLE` on Karma.

> **Warning:** This **burns all Karma tokens held by the distributor**. If users have unredeemed virtual rewards, they
> will lose them. Ensure users redeem before removing.

```bash
cast send <KARMA_ADDRESS> \
  "removeRewardDistributor(address)" \
  <DISTRIBUTOR_ADDRESS> \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>
```

---

## Distributing Rewards

For background on how rewards work, see [Rewards](staking-reward-distributor/rewards.md) and
[Reward Distributors](reward-distributors.md).

### Funding staking rewards via StakeManager

Requires `DEFAULT_ADMIN_ROLE` or `OPERATOR_ROLE` on Karma.

This mints Karma to the StakeManager and starts a reward period. Rewards are distributed linearly over the duration to
stakers based on their share of staked tokens and multiplier points.

```bash
cast send <KARMA_ADDRESS> \
  "setReward(address,uint256,uint256)" \
  <STAKE_MANAGER_ADDRESS> <AMOUNT> <DURATION_SECONDS> \
  --rpc-url <RPC_URL> \
  --account <ADMIN_OR_OPERATOR_ACCOUNT>
```

- `AMOUNT`: Total reward in wei (e.g., `1000000000000000000000` for 1000 Karma)
- `DURATION_SECONDS`: Distribution period (e.g., `604800` for 1 week)

If there is an active reward period, remaining undistributed rewards carry over and are added to the new amount.

### Funding the SimpleKarmaDistributor

Requires `DEFAULT_ADMIN_ROLE` or `OPERATOR_ROLE` on Karma.

This mints Karma to the SimpleKarmaDistributor, increasing its available supply. For details, see
[Simple Reward Distributor](simple-reward-distributor.md).

```bash
cast send <KARMA_ADDRESS> \
  "setReward(address,uint256,uint256)" \
  <SIMPLE_KARMA_DISTRIBUTOR_ADDRESS> <AMOUNT> 0 \
  --rpc-url <RPC_URL> \
  --account <ADMIN_OR_OPERATOR_ACCOUNT>
```

The `duration` parameter is ignored by SimpleKarmaDistributor.

### Minting Karma directly to an account

Requires `DEFAULT_ADMIN_ROLE` or `OPERATOR_ROLE` on Karma.

Mints actual Karma tokens directly, bypassing distributors.

```bash
cast send <KARMA_ADDRESS> \
  "mint(address,uint256)" \
  <ACCOUNT_ADDRESS> <AMOUNT> \
  --rpc-url <RPC_URL> \
  --account <ADMIN_OR_OPERATOR_ACCOUNT>
```

---

## Updating Tiers

Requires the owner of KarmaTiers.

Tiers define how many transactions per epoch an account can send for free on the L2, based on Karma balance. To update
tiers, edit the tier definitions in `script/UpdateTiers.s.sol` and run:

```bash
KARMA_TIERS_ADDRESS=<KARMA_TIERS_ADDRESS> \
  forge script script/UpdateTiers.s.sol:UpdateTiersScript \
  --rpc-url <RPC_URL> \
  --account <OWNER_ACCOUNT> \
  --broadcast
```

The current tier configuration is:

| Tier            | Min Karma  | Max Karma   | Tx/Epoch |
| --------------- | ---------- | ----------- | -------- |
| none            | 0          | <1          | 0        |
| entry           | 1          | 1           | 2        |
| newbie          | >1         | <50         | 6        |
| basic           | 50         | <500        | 16       |
| active          | 500        | <5,000      | 96       |
| regular         | 5,000      | <20,000     | 480      |
| power           | 20,000     | <100,000    | 960      |
| pro             | 100,000    | <500,000    | 10,080   |
| high-throughput | 500,000    | <5,000,000  | 108,000  |
| s-tier          | 5,000,000  | <10,000,000 | 240,000  |
| legendary       | 10,000,000 | ∞           | 480,000  |

Tiers must be contiguous (each tier's `minKarma` must equal the previous tier's `maxKarma + 1`) and the first tier must
start at `minKarma = 0`.

---

## Whitelisting Transfers

Requires `DEFAULT_ADMIN_ROLE` on Karma.

Karma is non-transferable by default. To allow a specific address to transfer Karma tokens (e.g., a new integration
contract or distributor):

```bash
cast send <KARMA_ADDRESS> \
  "setAllowedToTransfer(address,bool)" \
  <ACCOUNT_ADDRESS> true \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>
```

To revoke transfer permission:

```bash
cast send <KARMA_ADDRESS> \
  "setAllowedToTransfer(address,bool)" \
  <ACCOUNT_ADDRESS> false \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>
```

---

## Configuring Slashing

For background on how slashing integrates with RLN, see [RLN](rln.md).

### Setting the slash percentage

Requires `DEFAULT_ADMIN_ROLE` on Karma. Controls what percentage of an account's Karma balance is slashed. Value is in
basis points (100 = 1%, 10000 = 100%). Default: 5000 (50%).

```bash
cast send <KARMA_ADDRESS> \
  "setSlashPercentage(uint256)" \
  <PERCENTAGE_BPS> \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>
```

### Setting the slash reward percentage

Requires `DEFAULT_ADMIN_ROLE` on Karma. Controls what percentage of the slashed amount goes to the reward recipient.
Value is in basis points. Default: 1000 (10%).

```bash
cast send <KARMA_ADDRESS> \
  "setSlashRewardPercentage(uint256)" \
  <PERCENTAGE_BPS> \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>
```

### Setting the RLN reveal window

Requires `DEFAULT_ADMIN_ROLE` on RLN. Controls how long a caller must wait between committing and revealing a slash.
Default: 1 hour. Must be between 1 second and 86400 seconds (1 day).

```bash
cast send <RLN_ADDRESS> \
  "setSlashRevealWindowTime(uint256)" \
  <WINDOW_SECONDS> \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>
```

---

## Upgrading Contracts

### Pre-upgrade checklist

1. Ensure the new implementation compiles and all tests pass
2. New storage variables must be added **after** existing ones and **before** the `__gap` variable — reduce the gap size
   accordingly
3. Test on a fork before broadcasting:
   ```bash
   forge script <UPGRADE_SCRIPT> \
     --rpc-url <RPC_URL> \
     --account <ADMIN_ACCOUNT>
   ```
4. Verify existing state is preserved after the fork test

### Upgrading StakeManager

Requires `DEFAULT_ADMIN_ROLE` on StakeManager.

```bash
STAKE_MANAGER_PROXY_ADDRESS=<STAKE_MANAGER_PROXY_ADDRESS> \
  forge script script/UpgradeStakeManager.s.sol:UpgradeStakeManagerScript \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT> \
  --broadcast
```

Verify after upgrade:

```bash
cast call <STAKE_MANAGER_PROXY_ADDRESS> "totalStaked()(uint256)" --rpc-url <RPC_URL>
cast call <STAKE_MANAGER_PROXY_ADDRESS> "rewardsSupplier()(address)" --rpc-url <RPC_URL>
```

### Upgrading Karma

Requires `DEFAULT_ADMIN_ROLE` on Karma.

```bash
KARMA_PROXY_ADDRESS=<KARMA_PROXY_ADDRESS> \
  forge script script/UpgradeKarma.s.sol:UpgradeKarmaScript \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT> \
  --broadcast
```

### Upgrading the StakeVault implementation

StakeVault instances are minimal proxy clones and cannot be upgraded in place. Instead, deploy a new implementation and
let users migrate.

**Step 1:** Deploy the new implementation:

```bash
forge create src/StakeVault.sol:StakeVault \
  --constructor-args <STAKING_TOKEN_ADDRESS> \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>
```

**Step 2:** Update the VaultFactory (requires VaultFactory owner):

```bash
cast send <VAULT_FACTORY_ADDRESS> \
  "setVaultImplementation(address)" \
  <NEW_VAULT_IMPLEMENTATION_ADDRESS> \
  --rpc-url <RPC_URL> \
  --account <OWNER_ACCOUNT>
```

**Step 3:** Whitelist the new clone codehash on StakeManager (requires `DEFAULT_ADMIN_ROLE`):

```bash
cast send <STAKE_MANAGER_ADDRESS> \
  "setTrustedCodehash(bytes32,bool)" \
  <NEW_CODEHASH> true \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>
```

**Step 4 (optional):** After all users have migrated, remove the old codehash:

```bash
cast send <STAKE_MANAGER_ADDRESS> \
  "setTrustedCodehash(bytes32,bool)" \
  <OLD_CODEHASH> false \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>
```

Users migrate by calling `replaceVault()` on their old vault (creates a new vault and transfers everything), or by
creating a new vault and calling `migrateToVault(newVaultAddress)`.

---

## Pausing the System

Requires `DEFAULT_ADMIN_ROLE` or `GUARDIAN_ROLE` on StakeManager.

Pausing temporarily halts all staking operations (stake, unstake, lock, reward distribution, vault registration,
migration). It does **not** affect Karma, airdrops, or other contracts. Use it for temporary issues like suspected bugs
or ongoing investigations.

```bash
# Pause
cast send <STAKE_MANAGER_ADDRESS> "pause()" \
  --rpc-url <RPC_URL> --account <ADMIN_OR_GUARDIAN_ACCOUNT>

# Unpause
cast send <STAKE_MANAGER_ADDRESS> "unpause()" \
  --rpc-url <RPC_URL> --account <ADMIN_OR_GUARDIAN_ACCOUNT>
```

---

## Enabling Emergency Mode

Requires `DEFAULT_ADMIN_ROLE` or `GUARDIAN_ROLE` on StakeManager. For additional background, see
[Emergency Mode](staking-reward-distributor/emergency-mode.md).

> **Warning:** Emergency mode is **irreversible**. Once enabled, it cannot be disabled. Use only as a last resort.

Emergency mode permanently disables all StakeManager operations. The only remaining action is for users to call
`emergencyExit()` on their vaults to withdraw funds.

Use it when:

- A critical vulnerability cannot be fixed via upgrade
- The StakeManager state is irrecoverably corrupted
- A malicious upgrade has occurred

```bash
cast send <STAKE_MANAGER_ADDRESS> "enableEmergencyMode()" \
  --rpc-url <RPC_URL> --account <ADMIN_OR_GUARDIAN_ACCOUNT>
```

After enabling, users recover funds by calling `emergencyExit(destination)` on their StakeVault. This bypasses all
StakeManager accounting and transfers the vault's entire SNT balance.

If the StakeManager is bricked or unreachable, `emergencyExit()` still works — the vault uses a safe low-level call and
allows exit when the call fails.

The `GUARDIAN_ROLE` is intended for a multisig or monitoring system that can act quickly. Guardians can pause and
trigger emergency mode but cannot upgrade contracts or change configuration.

---

## Updating NFT Metadata

Requires the owner of KarmaNFT.

```bash
cast send <KARMA_NFT_ADDRESS> \
  "setMetadataGenerator(address)" \
  <NEW_GENERATOR_ADDRESS> \
  --rpc-url <RPC_URL> \
  --account <OWNER_ACCOUNT>
```

---

## Managing the Airdrop

For background, see [Karma Airdrop](karma-airdrop.md).

### Setting the merkle root for the first time

Requires the owner of KarmaAirdrop.

```bash
cast send <KARMA_AIRDROP_ADDRESS> \
  "setMerkleRoot(bytes32)" \
  <MERKLE_ROOT> \
  --rpc-url <RPC_URL> \
  --account <OWNER_ACCOUNT>
```

### Updating the merkle root

Only possible if `ALLOW_MERKLE_ROOT_UPDATE` was set to `true` at deployment. The contract must be paused before
updating. Each update increments the epoch, resetting the claimed bitmap so users can claim again under the new root.

```bash
# 1. Pause claims
cast send <KARMA_AIRDROP_ADDRESS> "pause()" \
  --rpc-url <RPC_URL> --account <OWNER_ACCOUNT>

# 2. Set the new merkle root
cast send <KARMA_AIRDROP_ADDRESS> \
  "setMerkleRoot(bytes32)" <NEW_MERKLE_ROOT> \
  --rpc-url <RPC_URL> --account <OWNER_ACCOUNT>

# 3. Resume claims
cast send <KARMA_AIRDROP_ADDRESS> "unpause()" \
  --rpc-url <RPC_URL> --account <OWNER_ACCOUNT>
```

### Pausing and unpausing claims

Requires the owner of KarmaAirdrop.

```bash
# Pause
cast send <KARMA_AIRDROP_ADDRESS> "pause()" \
  --rpc-url <RPC_URL> --account <OWNER_ACCOUNT>

# Unpause
cast send <KARMA_AIRDROP_ADDRESS> "unpause()" \
  --rpc-url <RPC_URL> --account <OWNER_ACCOUNT>
```

---

## Troubleshooting

### Broken reward distributor

**Symptom:** `karma.balanceOf(account)` or `karma.slash(account, recipient)` reverts.

**Cause:** These functions iterate over all registered distributors. If any distributor's `rewardsBalanceOfAccount()` or
`redeemRewards()` reverts, the entire call fails.

**Options:**

1. **Upgrade the broken distributor** to fix the issue (if it is upgradeable)
2. **Remove the distributor** as a last resort (requires `DEFAULT_ADMIN_ROLE` on Karma). This burns all Karma held by it
   — users lose unredeemed virtual rewards:
   ```bash
   cast send <KARMA_ADDRESS> \
     "removeRewardDistributor(address)" \
     <BROKEN_DISTRIBUTOR_ADDRESS> \
     --rpc-url <RPC_URL> \
     --account <ADMIN_ACCOUNT>
   ```

> **Note:** During slashing, paused distributors are skipped. However, `balanceOf()` does **not** skip paused
> distributors.

### Unreachable or bricked StakeManager

**Symptom:** Users cannot stake, unstake, or interact with the StakeManager.

**User impact:** User funds are not locked. StakeVault uses safe low-level calls for `leave()` and `emergencyExit()`:

- `leave()` transfers funds if the lock period has expired, even if the StakeManager is unreachable
- `emergencyExit()` transfers funds if the StakeManager call fails or returns that emergency mode is enabled

### Stuck funds in a vault

**Excess tokens sent accidentally:** The user calls `withdraw(token, amount)` to withdraw tokens above their deposited
balance.

**Lock period hasn't expired:** The user must wait until the `lockUntil` timestamp:

```bash
cast call <VAULT_ADDRESS> "lockUntil()(uint256)" --rpc-url <RPC_URL>
```

**User wants to exit entirely:** The user calls `leave(destination)` on their vault. If the lock period has expired, all
funds transfer immediately. See also [Leave Mechanism](staking-reward-distributor/leave-mechanism.md).

### Codehash mismatch after vault implementation change

**Symptom:** New vaults cannot stake — rejected with `TrustedCodehashAccess__UnauthorizedCodehash`.

**Fix:** Whitelist the new codehash (requires `DEFAULT_ADMIN_ROLE` on StakeManager):

```bash
cast send <STAKE_MANAGER_ADDRESS> \
  "setTrustedCodehash(bytes32,bool)" \
  <NEW_CODEHASH> true \
  --rpc-url <RPC_URL> \
  --account <ADMIN_ACCOUNT>
```

### SimpleKarmaDistributor supply exhausted

**Symptom:** `mint()` reverts with `SimpleKarmaDistributor__InsufficientAvailableSupply`.

**Fix:** Fund the distributor with more Karma (requires `DEFAULT_ADMIN_ROLE` or `OPERATOR_ROLE` on Karma):

```bash
cast send <KARMA_ADDRESS> \
  "setReward(address,uint256,uint256)" \
  <SIMPLE_KARMA_DISTRIBUTOR_ADDRESS> <AMOUNT> 0 \
  --rpc-url <RPC_URL> \
  --account <ADMIN_OR_OPERATOR_ACCOUNT>
```
