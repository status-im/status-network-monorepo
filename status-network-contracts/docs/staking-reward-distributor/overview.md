# Staking Reward Distributor

The staking system is a core component of the Status Network and a reward distributor of within the greater Karma
system, that allows users to stake SNT tokens to earn Karma token. The illustration below provides an overview of the
main components and their interactions within the staking system.

![System Overview](assets/reward-distributor-staking-overview.png)

1. **Vault Factory**: A smart contract that allows accounts to create stake vaults. It interacts with the stake manager
   proxy to register newly created stake vaults.
2. **Stake vault**: A smart contract that maintains the account's stake. A single account can own multiple vaults,
   allowing them to stake with different configurations. Accounts interface with the staking system through their stake
   vaults. One important characteristic is that staked funds are deposited in the vault and will only leave the vault if
   the owner withdraws them again by unstaking.
3. **Stake manager proxy / Implementation**: The smart contract proxy delegates to the logic/implementation contract of
   the staking system. It maintains its own onchain storage. Stake vaults interact with the staking system through the
   proxy. The logic of the staking system resides in the stake manager contract. This contract is upgradeable, allowing
   for changes of the protocol in the future.

## How the system works

- As discussed in the [system overview](../system-overview.md), reward distributors receive their Karma via the Karma
  contract and so does the staking reward distributor.
- The rewards are then distributed over a certain amount of time to accounts that stake their SNT token.
- Accounts (EOAs or smart accounts) create one or multiple stake vaults to stake their SNT and participate in the Karma
  Programme. In most cases, accounts will have only one stake vault, but nothing prevents them from creating more vaults
  with different configurations.
- The stake vaults interact with the stake manager through the proxy by forwarding calls to the implementation contract.
  When an account stakes funds, their funds are moved into the stake vault and they will stay there until the account
  decides to unstake them.
- While the account is staking, it will accrue Karma based on the amount of SNT staked and the duration of the stake.
  The longer accounts stake, the more Karma they will earn. Another important aspect is the use of
  [Multiplier Points](multiplier-points.md), which ensure that rewards are distributed fairly among participants.
- By locking up their stake, accounts receive multiplier points that increase their initial Karma earnings upon staking.
- At any point in time, accounts can view their Karma token balance in their wallets and how it updates in realtime.
- Eventually, an account will reach the [maximum amount](multiplier-points.md#maximum-mp) of multiplier points they can
  accrue based on their stake amount, at which point their total weight in Karma shares will no longer increase. They
  still earn Karma rewards set by the admin, according to their share.
- Accounts can unstake their SNT at any time (unless locked up). When they do, they will receive their initial stake
  back, along with any Karma rewards they have earned, however, they will loose their multiplier points.
- The Karma in the account's wallet can be used to access exclusive features or services within the Status app or other
  DApps that support the Karma token, like voting in governance proposals or participating in exclusive events.
