# RLN (Rate-Limiting Nullifier)

## Overview

The RLN (Rate-Limiting Nullifier) contract is a privacy-preserving identity registry that manages a set of identity
commitments for the protocol participants. It enables users to register their identities, and allows authorized accounts
to slash misbehaving users through a commit-reveal scheme that prevents front-running attacks.

## Features

- **Identity Registry:**

  - Maintains a fixed-size registry of identity commitments determined by merkle tree depth.
  - Registrations are sequential and tracked with unique indices.
  - Prevents duplicate registrations of the same identity commitment.
  - Supports privacy by storing only cryptographic commitments, not actual identities.

- **Cryptographic Foundation:**

  - Uses the Poseidon hash function to derive identity commitments from private keys.

- **Commit-Reveal Slashing:**

  - Two-step slashing mechanism prevents front-running attacks.
  - Slashers commit to a slash operation with a cryptographic hash.
  - Reveals must occur after a configurable reveal window, enforcing time-lock constraints.
  - Subsequent slashes on the same account are queued and scheduled for future reveal windows.

- **Reveal Window Management:**

  - Configurable reveal window duration (default 1 hour, range 1 second to 1 day).
  - Multiple commits for the same account are automatically queued.
  - System tracks when the last reveal window started for each account.

- **Karma Token Integration:**

  - Integrates with the Karma contract to enforce economic penalties for violations.
  - Slashing burns a percentage of the user's total Karma balance (both actual and virtual tokens).
  - Slashing rewards are minted to a recipient specified as parameter.

- **Role-Based Access Control:**

  - Supports multiple roles: DEFAULT_ADMIN_ROLE, REGISTER_ROLE, and SLASHER_ROLE.
  - Ensures only authorized accounts can register identities and execute slashes.

- **Upgradeable Design:**
  - Uses UUPS (Universal Upgradeable Proxy Standard) for secure contract upgrades.
  - Initialized with owner, slasher, register, merkle tree depth, Karma token addresses, Poseidon hasher contract
    address.

## Identity Registration

The RLN contract maintains a registry of identity commitments that represent encrypted representations of user
identities.

### Registration Process

When an authorized account with REGISTER_ROLE registers an identity commitment:

1. **Commitment Validation**: The contract verifies that the identity commitment hasn't been previously registered.
2. **Registry Space**: The contract checks that the registry has capacity (not full based on merkle tree depth).
3. **Commitment Storage**: The identity commitment is stored with the registrant's address and an incrementing index.
4. **Event Emission**: A `MemberRegistered` event is emitted containing the commitment and its registry index.

### Identity Commitment Generation

Identity commitments are generated off-chain.

### Registry Capacity

The registry size is fixed based on the merkle tree depth parameter:

- **Depth 16**: 2^16 = 65,536 identities
- **Depth 20**: 2^20 = 1,048,576 identities

Once the registry reaches capacity, new registrations are rejected until space is available.

## Commit-Reveal Slashing Mechanism

The RLN contract implements a two-step commit-reveal scheme to prevent front-running attacks and ensure orderly
processing of slashing operations.

### Why Commit-Reveal?

Without a commit-reveal scheme, a slasher could observe an on-chain transaction that commits to slashing an account and
front-run it with their own slash transaction before the original slash executes. The commit-reveal scheme prevents this
by:

1. Separating the decision to slash (commit) from the execution (reveal)
2. Enforcing a time delay between commitment and revelation
3. Ensuring all slashes on an account are processed in order

### Step 1: Commit Phase

When a slasher initiates a slash operation, they call `slashCommit()`:

1. **Hash Generation**: The slasher creates a keccak256 hash of the private key and reward recipient address.
2. **Commitment Recording**: The contract stores this hash along with the timestamp in a mapping.
3. **Queue Management**: If this is not the first slash commit on this account, the reveal start time is scheduled for
   after the current reveal window closes.
4. **Event Emission**: The system records the commitment for later verification.

The hash ensures the slasher commits to specific values without revealing them until the reveal phase.

### Step 2: Reveal Phase

After the reveal window has passed, the slasher calls `slashReveal()` to execute the slash:

1. **Data Provision**: The slasher provides the actual private key and reward recipient address.
2. **Hash Verification**: The contract verifies that the hash of these values matches the previously committed hash.
3. **Window Validation**: The contract checks that the current time is within or after the reveal window for this
   commit.
4. **Slash Execution**: If all validations pass, the `slash()` function is called to execute the actual penalty.

### Reveal Window Timing

The reveal window ensures orderly processing:

- **First Commit**: The reveal window starts immediately.
- **Subsequent Commits**: Each new commit on the same account is scheduled to start after the previous window ends.
- **Window Duration**: Configurable by the admin (default 1 hour).
- **Window Range**: Minimum 1 second, maximum 1 day.

This queuing mechanism ensures that even if multiple slashes are committed for the same account, they are processed
sequentially in the order they were committed.

### Direct Slash (Emergency)

The contract also supports a direct `slash()` function for emergency situations or when the slasher wishes to bypass the
commit-reveal mechanism. However, this requires explicit authorization and is typically reserved for privileged roles.

### Access Control

Only accounts with the `SLASHER_ROLE` or the admin can execute slashing operations.

### Configurable Parameters

- **Reveal Window Duration**: The amount of time that must pass between a commit and a reveal.

## Data Structures and State

The RLN contract maintains several key data structures:

### Members Registry

```
mapping(uint256 commitment => User user) public members
```

Maps identity commitments to user information:

```
struct User {
    address userAddress;    // Address of the registrant
    uint256 index;         // Position in the merkle tree
}
```
