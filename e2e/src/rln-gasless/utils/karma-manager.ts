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
 * Karma Manager for testing Karma-related functionality
 * Handles Karma minting, tier assignment, and quota management
 */
export class KarmaTestManager {
  // Production tier configuration (matches initialize-karma-tiers.ts)
  // Note: No "none" tier - users with 0 karma are not registered in RLN
  static readonly TIERS: TierConfig[] = [
    { name: "entry", minKarma: 0n, maxKarma: 1n, quota: 2 },
    { name: "newbie", minKarma: 2n, maxKarma: 49n, quota: 6 },
    { name: "basic", minKarma: 50n, maxKarma: 499n, quota: 16 },
    { name: "active", minKarma: 500n, maxKarma: 4999n, quota: 96 },
    { name: "regular", minKarma: 5000n, maxKarma: 19999n, quota: 480 },
    { name: "power", minKarma: 20000n, maxKarma: 99999n, quota: 960 },
    { name: "pro", minKarma: 100000n, maxKarma: 499999n, quota: 10080 },
    { name: "high-throughput", minKarma: 500000n, maxKarma: 4999999n, quota: 108000 },
    { name: "s-tier", minKarma: 5000000n, maxKarma: 9999999n, quota: 240000 },
    { name: "legendary", minKarma: 10000000n, maxKarma: ethers.MaxUint256, quota: 480000 },
  ];

  constructor(
    private karmaContract: ethers.Contract,
    private rlnContract: ethers.Contract,
    private admin: ethers.Signer,
    private rlnClient: RlnTestClient,
  ) {}

  /**
   * Mint a specific amount of Karma to a user
   * NOTE: After minting, waits 3 seconds for the RLN prover to process
   * the Transfer event and register the user
   */
  async mintKarma(userAddress: string, amount: bigint): Promise<ethers.TransactionReceipt> {
    logger.debug("Minting Karma", {
      user: userAddress,
      amount: amount.toString(),
    });

    const tx = await (this.karmaContract as ethers.Contract).connect(this.admin).mint(userAddress, amount, {
      gasLimit: 200000,
      gasPrice: ethers.parseUnits("15", "gwei"), // Premium gas to bypass RLN
    });

    const receipt = await tx.wait(1, RLN_CONFIG.test.transactionTimeoutMs);

    logger.debug("Karma minted - waiting for prover to process event", {
      user: userAddress,
      txHash: tx.hash,
      amount: amount.toString(),
    });

    // Give the prover time to see the Transfer event and register the user
    // Reduced from 10s since prover now uses its own private key (no nonce conflicts)
    await new Promise((resolve) => setTimeout(resolve, 3000));

    logger.debug("Karma mint complete", {
      user: userAddress,
    });

    return receipt;
  }

  /**
   * Mint Karma to a user to achieve a specific tier
   * Returns the amount minted
   */
  async mintKarmaToTier(userAddress: string, tierName: TierName): Promise<bigint> {
    const currentBalance = await this.getKarmaBalance(userAddress);
    const targetAmount = this.getTierKarmaAmount(tierName);

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
   * Exhaust a user's quota by submitting transactions
   * Returns the number of transactions sent
   */
  async exhaustUserQuota(user: ethers.Signer, recipientAddress: string, expectedQuota?: number): Promise<number> {
    const userAddress = await user.getAddress();

    logger.debug("Exhausting user quota", { user: userAddress });

    // Get user's tier info from karma service
    let quota: number;
    let alreadyUsed: number;

    try {
      const tierInfo = await this.rlnClient.getUserTierInfo(userAddress);
      quota = expectedQuota ?? tierInfo.dailyQuota;
      alreadyUsed = tierInfo.epochTxCount;
    } catch {
      // If karma service is unavailable, use expected quota
      if (!expectedQuota) {
        throw new Error("Cannot determine quota - karma service unavailable and no expectedQuota provided");
      }
      quota = expectedQuota;
      alreadyUsed = 0;
    }

    const remainingQuota = quota - alreadyUsed;

    logger.debug("User quota info", {
      user: userAddress,
      totalQuota: quota,
      used: alreadyUsed,
      remaining: remainingQuota,
    });

    if (remainingQuota <= 0) {
      logger.debug("User quota already exhausted", { user: userAddress });
      return 0;
    }

    // Submit transactions to exhaust quota
    const timestamp = Date.now();
    for (let i = 0; i < remainingQuota; i++) {
      const uniqueData = ethers.hexlify(ethers.toUtf8Bytes(`exhaust-quota-${i}-${timestamp}`));

      const receipt = await this.rlnClient.sendGaslessTransaction(user, {
        to: recipientAddress,
        value: 0n,
        data: uniqueData,
        gasLimit: 30000,
      });

      logger.debug("Quota consumption transaction", {
        index: i + 1,
        total: remainingQuota,
        txHash: receipt.hash,
      });
    }

    logger.debug("User quota exhausted", {
      user: userAddress,
      transactionsSent: remainingQuota,
    });

    return remainingQuota;
  }

  /**
   * Wait for user to be registered to RLN after Karma mint
   */
  async waitForRlnRegistration(
    userAddress: string,
    timeout: number = RLN_CONFIG.test.registrationTimeoutMs,
  ): Promise<void> {
    logger.debug("Waiting for RLN registration", {
      user: userAddress,
      timeout,
    });

    await this.rlnClient.waitForRegistration(this.rlnContract, userAddress, timeout);

    logger.debug("User registered to RLN", { user: userAddress });
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
   */
  async isUserRegistered(userAddress: string): Promise<boolean> {
    try {
      const normalizedAddress = userAddress.toLowerCase();

      // Query MemberRegistered events to find this user's commitment
      const filter = this.rlnContract.filters.MemberRegistered();
      const events = await this.rlnContract.queryFilter(filter);

      for (const event of events) {
        if (event.args) {
          const commitment = event.args[0];
          try {
            const result = await this.rlnContract.members(commitment);
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
   */
  async getUserIdentityCommitment(userAddress: string): Promise<string | null> {
    try {
      const normalizedAddress = userAddress.toLowerCase();

      // Query MemberRegistered events to find this user's commitment
      const filter = this.rlnContract.filters.MemberRegistered();
      const events = await this.rlnContract.queryFilter(filter);

      for (const event of events) {
        if (event.args) {
          const commitment = event.args[0];
          try {
            const result = await this.rlnContract.members(commitment);
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
  ): Promise<ethers.HDNodeWallet> {
    // Create funded wallet
    const wallet = ethers.Wallet.createRandom().connect(provider);

    logger.info("Setting up user for gasless transactions", {
      address: wallet.address,
      tier: tierName,
    });

    // Fund the wallet
    const adminNonce = await provider.getTransactionCount(await this.admin.getAddress(), "latest");
    const fundTx = await this.admin.sendTransaction({
      to: wallet.address,
      value: fundAmount,
      gasLimit: 21000,
      gasPrice: ethers.parseUnits("15", "gwei"),
      nonce: adminNonce,
    });
    await fundTx.wait(1, RLN_CONFIG.test.transactionTimeoutMs);

    logger.debug("Wallet funded", { address: wallet.address });

    // Mint Karma
    await this.mintKarmaToTier(wallet.address, tierName);

    // Wait for RLN registration
    await this.waitForRlnRegistration(wallet.address);

    logger.info("User setup complete", {
      address: wallet.address,
      tier: tierName,
      karma: (await this.getKarmaBalance(wallet.address)).toString(),
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
