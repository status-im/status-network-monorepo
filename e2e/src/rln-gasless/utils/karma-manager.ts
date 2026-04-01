import { ethers } from "ethers";
import { createTestLogger } from "../../config/logger";
import { RlnTestClient } from "./rln-test-client";
import { RLN_CONFIG, TierName } from "../config/rln-config";

const logger = createTestLogger();

export interface TierConfig {
  name: string;
  minKarma: bigint;
  maxKarma: bigint;
  quota: number;
}

/**
 * Global nonce tracker for the admin wallet to prevent nonce collisions
 * when multiple tests setup users concurrently.
 */
class NonceManager {
  private nonce: number = -1;
  private lock: Promise<void> = Promise.resolve();

  async getNextNonce(provider: ethers.Provider, address: string): Promise<number> {
    // Serialize nonce access
    const unlock = this.lock;
    let resolve: () => void;
    this.lock = new Promise((r) => (resolve = r));

    await unlock;

    try {
      if (this.nonce === -1) {
        // Wait for any pending TXs from previous runs to clear before starting
        let latest = await provider.getTransactionCount(address, "latest");
        let pending = await provider.getTransactionCount(address, "pending");
        let attempts = 0;
        while (latest !== pending && attempts < 15) {
          logger.debug("Waiting for pending TXs to clear", { latest, pending, attempt: attempts + 1 });
          await new Promise((r) => setTimeout(r, 2000));
          latest = await provider.getTransactionCount(address, "latest");
          pending = await provider.getTransactionCount(address, "pending");
          attempts++;
        }
        this.nonce = pending;
        logger.debug("Nonce manager initialized", { nonce: this.nonce });
      }
      const nextNonce = this.nonce;
      this.nonce++;
      return nextNonce;
    } finally {
      resolve!();
    }
  }

  reset(): void {
    this.nonce = -1;
    this.lock = Promise.resolve();
  }
}

// Global nonce manager for admin transactions
const adminNonceManager = new NonceManager();

/**
 * Reset the admin nonce manager (call at start of test suite)
 * This ensures fresh nonce tracking for each test run.
 */
export function resetAdminNonceManager(): void {
  adminNonceManager.reset();
}

/**
 * Get the next nonce for admin transactions.
 * Use this function for any transaction sent from the admin wallet
 * to prevent nonce collisions across the test suite.
 */
export async function getAdminNonce(provider: ethers.Provider, adminAddress: string): Promise<number> {
  return adminNonceManager.getNextNonce(provider, adminAddress);
}

/**
 * Karma Manager for testing Karma-related functionality
 * Handles Karma minting, tier assignment, and quota management
 */
export class KarmaTestManager {
  // Production tier configuration (matches deployed KarmaTiers contract)
  // Users can send exactly 'quota' gasless transactions per epoch (no grace transactions)
  // After using quota, they're added to deny list and must pay premium gas
  // Karma is an ERC20 with 18 decimals (1 Karma = 1e18 wei)
  // Tier 0 ("none") has txPerEpoch=0, users with <1 Karma cannot send gasless transactions
  static readonly ETHER = 10n ** 18n;
  static readonly TIERS: TierConfig[] = [
    { name: "entry", minKarma: 1n * KarmaTestManager.ETHER, maxKarma: 1n * KarmaTestManager.ETHER, quota: 2 },
    {
      name: "newbie",
      minKarma: 1n * KarmaTestManager.ETHER + 1n,
      maxKarma: 50n * KarmaTestManager.ETHER - 1n,
      quota: 6,
    },
    { name: "basic", minKarma: 50n * KarmaTestManager.ETHER, maxKarma: 500n * KarmaTestManager.ETHER - 1n, quota: 16 },
    {
      name: "active",
      minKarma: 500n * KarmaTestManager.ETHER,
      maxKarma: 5000n * KarmaTestManager.ETHER - 1n,
      quota: 96,
    },
    {
      name: "regular",
      minKarma: 5000n * KarmaTestManager.ETHER,
      maxKarma: 20000n * KarmaTestManager.ETHER - 1n,
      quota: 480,
    },
    {
      name: "power",
      minKarma: 20000n * KarmaTestManager.ETHER,
      maxKarma: 100000n * KarmaTestManager.ETHER - 1n,
      quota: 960,
    },
    {
      name: "pro",
      minKarma: 100000n * KarmaTestManager.ETHER,
      maxKarma: 500000n * KarmaTestManager.ETHER - 1n,
      quota: 10080,
    },
    {
      name: "high-throughput",
      minKarma: 500000n * KarmaTestManager.ETHER,
      maxKarma: 5000000n * KarmaTestManager.ETHER - 1n,
      quota: 108000,
    },
    {
      name: "s-tier",
      minKarma: 5000000n * KarmaTestManager.ETHER,
      maxKarma: 10000000n * KarmaTestManager.ETHER - 1n,
      quota: 240000,
    },
    { name: "legendary", minKarma: 10000000n * KarmaTestManager.ETHER, maxKarma: ethers.MaxUint256, quota: 480000 },
  ];

  constructor(
    private karmaContract: ethers.Contract,
    private rlnContract: ethers.Contract,
    private admin: ethers.Signer,
    private rlnClient: RlnTestClient,
  ) {}

  /**
   * Mint a specific amount of Karma to a user
   */
  async mintKarma(userAddress: string, amount: bigint): Promise<ethers.TransactionReceipt> {
    const t0 = Date.now();
    logger.debug("Minting Karma", {
      user: userAddress,
      amount: amount.toString(),
    });

    const adminAddress = await this.admin.getAddress();
    const provider = this.admin.provider;
    if (!provider) {
      throw new Error("Admin signer has no provider");
    }

    // Use nonce manager to avoid collisions with concurrent operations
    const nonce = await adminNonceManager.getNextNonce(provider, adminAddress);
    logger.debug("Mint: got nonce", { nonce, elapsed: Date.now() - t0 });

    const t1 = Date.now();
    const connectedContract = this.karmaContract.connect(this.admin) as ethers.Contract;
    const tx = await connectedContract.getFunction("mint")(userAddress, amount, {
      gasLimit: 200000,
      gasPrice: ethers.parseUnits(String(RLN_CONFIG.test.premiumGasThresholdGwei + 3), "gwei"), // Premium gas to bypass RLN
      nonce: nonce,
    });
    logger.debug("Mint TX sent", { hash: tx.hash, elapsed: Date.now() - t1 });

    const t2 = Date.now();
    const receipt = await tx.wait(1, RLN_CONFIG.test.transactionTimeoutMs);
    logger.debug("Mint TX confirmed", { elapsed: Date.now() - t2 });

    // Prover processes karma events immediately via WebSocket subscription
    // No artificial delay needed - just verify registration worked

    logger.debug("Karma mint complete", {
      user: userAddress,
      totalElapsed: Date.now() - t0,
    });

    return receipt;
  }

  /**
   * Mint Karma to a user to achieve a specific tier
   * Returns the amount minted
   *
   * IMPORTANT: For Entry tier, we ALWAYS mint exactly 1 karma to ensure the user
   * stays in Entry tier. This prevents issues where users might accumulate karma
   * from multiple sources and get bumped to Newbie tier.
   */
  async mintKarmaToTier(userAddress: string, tierName: TierName): Promise<bigint> {
    const currentBalance = await this.getKarmaBalance(userAddress);
    const targetAmount = this.getTierKarmaAmount(tierName);
    const tierBoundary = RLN_CONFIG.tierBoundaries[tierName];

    // For Entry tier, verify the user doesn't already have karma that would
    // put them in a different tier. Entry tier is very sensitive - even 2 karma
    // bumps them to Newbie tier.
    if (tierName === "entry" && currentBalance > 0n) {
      if (currentBalance > tierBoundary.max) {
        throw new Error(
          `Cannot setup Entry tier user ${userAddress}: already has ${currentBalance} karma (max for entry is ${tierBoundary.max})`,
        );
      }
      // User already has karma in Entry tier range, no need to mint more
      logger.debug("User already has Entry tier karma", {
        user: userAddress,
        currentBalance: currentBalance.toString(),
      });
      return 0n;
    }

    if (currentBalance >= targetAmount) {
      logger.debug("User already has sufficient Karma for tier", {
        user: userAddress,
        tier: tierName,
        currentBalance: currentBalance.toString(),
        targetAmount: targetAmount.toString(),
      });
      return 0n;
    }

    const amountToMint = targetAmount - currentBalance;

    logger.debug("Minting Karma to tier", {
      user: userAddress,
      tier: tierName,
      currentBalance: currentBalance.toString(),
      amountToMint: amountToMint.toString(),
    });

    await this.mintKarma(userAddress, amountToMint);

    return amountToMint;
  }

  /**
   * Get the Karma amount for a specific tier
   * Returns minKarma to hit exactly the tier threshold
   */
  getTierKarmaAmount(tierName: TierName): bigint {
    const tierConfig = RLN_CONFIG.tiers[tierName];
    if (!tierConfig) {
      throw new Error(`Unknown tier: ${tierName}`);
    }
    return tierConfig.karma;
  }

  /**
   * Get the quota for a specific tier
   */
  getTierQuota(tierName: TierName): number {
    const tierConfig = RLN_CONFIG.tiers[tierName];
    if (!tierConfig) {
      throw new Error(`Unknown tier: ${tierName}`);
    }
    return tierConfig.quota;
  }

  /**
   * Get the expected tier for a Karma balance
   */
  getExpectedTier(karmaBalance: bigint): TierConfig | null {
    for (const tier of KarmaTestManager.TIERS) {
      if (karmaBalance >= tier.minKarma && karmaBalance <= tier.maxKarma) {
        return tier;
      }
    }
    return null;
  }

  /**
   * Exhaust a user's quota by submitting transactions.
   * The expectedQuota parameter is required since there is no REST API to query tier info
   * (the RLN prover is gRPC-only).
   * Returns the number of transactions sent.
   */
  async exhaustUserQuota(user: ethers.Signer, recipientAddress: string, expectedQuota: number): Promise<number> {
    const userAddress = await user.getAddress();

    logger.debug("Exhausting user quota", { user: userAddress, quota: expectedQuota });

    if (expectedQuota <= 0) {
      logger.debug("User quota already exhausted or zero", { user: userAddress });
      return 0;
    }

    // Submit transactions to exhaust quota
    const timestamp = Date.now();
    for (let i = 0; i < expectedQuota; i++) {
      const uniqueData = ethers.hexlify(ethers.toUtf8Bytes(`exhaust-quota-${i}-${timestamp}`));

      const receipt = await this.rlnClient.sendGaslessTransaction(user, {
        to: recipientAddress,
        value: 0n,
        data: uniqueData,
        gasLimit: 30000,
      });

      logger.debug("Quota consumption transaction", {
        index: i + 1,
        total: expectedQuota,
        txHash: receipt.hash,
      });
    }

    logger.debug("User quota exhausted", {
      user: userAddress,
      transactionsSent: expectedQuota,
    });

    return expectedQuota;
  }

  /**
   * Wait for user to be registered to RLN after Karma mint.
   *
   * The RLN prover automatically registers users when it sees Karma Transfer events.
   * Based on observed behavior:
   * - Prover sees Karma Transfer event within ~1 block (2s)
   * - Prover sends RLN registration TX within ~1s
   * - Registration TX mined within ~1 block (2s)
   * Total: ~5-6 seconds for reliable registration
   *
   * We use a fixed wait time instead of polling because:
   * 1. The prover's HTTP API doesn't work (gRPC only)
   * 2. Polling contract events is O(n) and slow
   * 3. A fixed wait is simple and reliable
   */
  async waitForRlnRegistration(
    userAddress: string,
    timeout: number = RLN_CONFIG.test.registrationTimeoutMs,
  ): Promise<void> {
    const pollIntervalMs = 500;

    logger.debug("Waiting for RLN registration", {
      user: userAddress,
      timeout,
    });

    if (userAddress === "batch-all") {
      // For batch waits, just do a fixed sleep
      await new Promise((resolve) => setTimeout(resolve, timeout));
      logger.debug("RLN registration batch wait complete");
      return;
    }

    // Poll isUserRegistered until it returns true or timeout
    const start = Date.now();
    while (Date.now() - start < timeout) {
      const isRegistered = await this.isUserRegistered(userAddress);
      if (isRegistered) {
        logger.debug("RLN registration confirmed", {
          user: userAddress,
          elapsed: Date.now() - start,
        });
        return;
      }
      await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
    }

    // Timeout - log warning but don't throw (let the test assertion handle it)
    logger.warn("RLN registration wait timed out", {
      user: userAddress,
      timeout,
    });
  }

  /**
   * Get user's Karma balance
   */
  async getKarmaBalance(userAddress: string): Promise<bigint> {
    return await this.karmaContract.balanceOf(userAddress);
  }

  /**
   * Verify user is registered to RLN by checking MemberRegistered events
   * The RLN contract uses members(uint256 commitment) -> (address, uint256), not users(address)
   * Queries only recent blocks (last 200) for performance on testnets with many registrations.
   */
  async isUserRegistered(userAddress: string): Promise<boolean> {
    try {
      const normalizedAddress = userAddress.toLowerCase();
      const provider = this.rlnContract.runner?.provider;
      const currentBlock = provider ? await provider.getBlockNumber() : 0;
      const fromBlock = Math.max(0, currentBlock - 200);

      const filter = this.rlnContract.filters.MemberRegistered();
      const events = await this.rlnContract.queryFilter(filter, fromBlock);

      // Search newest events first (most likely to find recent registrations)
      for (let i = events.length - 1; i >= 0; i--) {
        const event = events[i];
        if ("args" in event && event.args) {
          const commitment = event.args[0];
          try {
            const rlnContractWithMembers = this.rlnContract as ethers.Contract;
            const result = await rlnContractWithMembers.getFunction("members")(commitment);
            const memberAddress = result[0] || result.userAddress;
            if (memberAddress && memberAddress.toLowerCase() === normalizedAddress) {
              return true;
            }
          } catch {
            // Continue to next event
          }
        }
      }
      return false;
    } catch {
      return false;
    }
  }

  /**
   * Get user's RLN identity commitment by checking MemberRegistered events
   * Queries only recent blocks (last 200) for performance.
   */
  async getUserIdentityCommitment(userAddress: string): Promise<string | null> {
    try {
      const normalizedAddress = userAddress.toLowerCase();
      const provider = this.rlnContract.runner?.provider;
      const currentBlock = provider ? await provider.getBlockNumber() : 0;
      const fromBlock = Math.max(0, currentBlock - 200);

      const filter = this.rlnContract.filters.MemberRegistered();
      const events = await this.rlnContract.queryFilter(filter, fromBlock);

      // Search newest events first
      for (let i = events.length - 1; i >= 0; i--) {
        const event = events[i];
        if ("args" in event && event.args) {
          const commitment = event.args[0];
          try {
            const rlnContractWithMembers = this.rlnContract as ethers.Contract;
            const result = await rlnContractWithMembers.getFunction("members")(commitment);
            const memberAddress = result[0] || result.userAddress;
            if (memberAddress && memberAddress.toLowerCase() === normalizedAddress) {
              return commitment.toString();
            }
          } catch {
            // Continue to next event
          }
        }
      }
      return null;
    } catch {
      return null;
    }
  }

  /**
   * Setup a user for gasless transactions
   * - Creates funded wallet
   * - Mints Karma to specified tier
   * - Waits for RLN registration
   */
  async setupUserForGasless(
    provider: ethers.Provider,
    tierName: TierName = "entry",
    fundAmount: bigint = ethers.parseEther("1"),
    options?: { skipRegistrationWait?: boolean },
  ): Promise<ethers.HDNodeWallet> {
    // Create funded wallet
    const wallet = ethers.Wallet.createRandom().connect(provider);
    const adminAddress = await this.admin.getAddress();

    logger.info("Setting up user for gasless transactions", {
      address: wallet.address,
      tier: tierName,
    });

    // Fund the wallet - use nonce manager to avoid collisions
    // Admin uses premium gas to bypass RLN (this is an admin operation, not a user gasless tx)
    const t0 = Date.now();
    const fundNonce = await adminNonceManager.getNextNonce(provider, adminAddress);
    logger.debug("Got nonce", { nonce: fundNonce, elapsed: Date.now() - t0 });

    const t1 = Date.now();
    const fundTx = await this.admin.sendTransaction({
      to: wallet.address,
      value: fundAmount,
      gasLimit: 21000,
      gasPrice: ethers.parseUnits(String(RLN_CONFIG.test.premiumGasThresholdGwei + 3), "gwei"), // Premium gas - admin op
      nonce: fundNonce,
    });
    logger.debug("TX sent", { hash: fundTx.hash, elapsed: Date.now() - t1 });

    const t2 = Date.now();
    await fundTx.wait(1, RLN_CONFIG.test.transactionTimeoutMs);
    logger.debug("Wallet funded", { address: wallet.address, nonce: fundNonce, waitElapsed: Date.now() - t2 });

    // Mint Karma
    await this.mintKarmaToTier(wallet.address, tierName);

    // Wait for RLN registration (skip when batching multiple user setups)
    if (!options?.skipRegistrationWait) {
      await this.waitForRlnRegistration(wallet.address);
    }

    // Verify karma balance matches expected tier
    const actualKarma = await this.getKarmaBalance(wallet.address);
    const expectedKarma = this.getTierKarmaAmount(tierName);
    const tierBoundary = RLN_CONFIG.tierBoundaries[tierName];

    if (actualKarma < tierBoundary.min || actualKarma > tierBoundary.max) {
      logger.error("User karma out of tier bounds!", {
        address: wallet.address,
        tier: tierName,
        actualKarma: actualKarma.toString(),
        expectedKarma: expectedKarma.toString(),
        tierMin: tierBoundary.min.toString(),
        tierMax: tierBoundary.max.toString(),
      });
      throw new Error(
        `User ${wallet.address} has karma ${actualKarma} which is outside ${tierName} tier bounds [${tierBoundary.min}, ${tierBoundary.max}]`,
      );
    }

    logger.info("User setup complete", {
      address: wallet.address,
      tier: tierName,
      karma: actualKarma.toString(),
    });

    return wallet;
  }

  /**
   * Setup multiple users for concurrent testing
   */
  async setupMultipleUsers(
    provider: ethers.Provider,
    count: number,
    tierName: TierName = "entry",
  ): Promise<ethers.HDNodeWallet[]> {
    const users: ethers.HDNodeWallet[] = [];

    for (let i = 0; i < count; i++) {
      const user = await this.setupUserForGasless(provider, tierName);
      users.push(user);
    }

    return users;
  }

  /**
   * Get all tier configurations
   */
  static getAllTiers(): TierConfig[] {
    return [...KarmaTestManager.TIERS];
  }

  /**
   * Get tier by name
   */
  static getTierByName(tierName: string): TierConfig | undefined {
    return KarmaTestManager.TIERS.find((t) => t.name.toLowerCase() === tierName.toLowerCase());
  }
}
