# Emergency Mode

## Table Of Contents

- [Overview](#overview)
- [Key Characteristics](#key-characteristics)
  - [One-Way Activation](#one-way-activation)
  - [Bypasses All Lock-Ups](#bypasses-all-lock-ups)
  - [Minimal Accounting Updates](#minimal-accounting-updates)
  - [Restricted Operations](#restricted-operations)
- [When Emergency Mode Is Used](#when-emergency-mode-is-used)
- [Relationship with Leave Mechanism](#relationship-with-leave-mechanism)

## Overview

Emergency mode is a critical safety mechanism built into the staking system that allows users to immediately exit and
recover their funds in exceptional circumstances. When enabled, it bypasses normal operational constraints including
lock-up periods, providing an unconditional exit path for all stakers.

This mechanism exists as a last-resort protection against catastrophic scenarios such as security vulnerabilities,
malicious contract upgrades, or critical system failures.

## Key Characteristics

### One-Way Activation

Emergency mode is **irreversible**. Once enabled by an admin or guardian, it cannot be disabled. This ensures that if
the system is compromised, malicious actors cannot prevent users from exiting by toggling emergency mode off.

### Bypasses All Lock-Ups

Unlike the user's capablity to [leave](leave-mechanism.md) the system, which respects lock-up periods, emergency exit
allows users to withdraw their funds immediately regardless of when their lock period expires. This ensures that users
can always access their funds in a true emergency.

### Minimal Accounting Updates

When users perform an emergency exit, the StakeManager's internal accounting is **not updated**. This design choice
ensures that:

- Users can exit even if the StakeManager is broken or malicious
- Exits cannot be blocked by reverting transactions
- The function remains simple and reliable under all circumstances

The trade-off is that the StakeManager's state becomes stale, but this is acceptable since emergency mode is terminal
for the contract.

### Restricted Operations

Once emergency mode is enabled, most staking operations are blocked to prevent further interactions with a potentially
compromised system. The allowed operations are [formally verified](../../PROPERTIES.md) to ensure user safety.

## When Emergency Mode Is Used

- If a critical security vulnerability is discovered that puts user funds at risk, emergency mode provides an immediate
  exit path before the vulnerability can be exploited.
- If the StakeManager is upgraded to a malicious or broken implementation, guardians can enable emergency mode to allow
  users to exit before interacting with the compromised contract.
- If the staking system experiences a failure that makes normal operations unsafe or impossible, emergency mode ensures
  users can still recover their funds.

## Relationship with Leave Mechanism

Emergency mode is distinct from the normal [leave mechanism](leave-mechanism.md):

| Feature            | Leave                           | Emergency Exit                   |
| ------------------ | ------------------------------- | -------------------------------- |
| Respects locks     | Yes, locked funds stay in vault | No, bypasses all locks           |
| Updates accounting | Yes, full accounting update     | No, accounting not updated       |
| Claims rewards     | Yes, transfers accrued rewards  | No, only staking tokens          |
| When available     | Always                          | Only when emergency mode enabled |
| Purpose            | Graceful exit from system       | Emergency fund recovery          |

Users should use `leave()` for normal exits (or unstake) and `emergencyExit()` only when emergency mode has been
enabled.
