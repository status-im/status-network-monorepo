import { ethers } from "ethers";
import { createTestLogger } from "../../config/logger";
import { RlnTestClient } from "./rln-test-client";

const logger = createTestLogger();

export interface TierConfig {
  name: string;
  minKarma: bigint;
  maxKarma: bigint;
  quota: number;
}

/**
 * Karma Manager for testing Karma-related functionality
 */
export class KarmaTestManager {
  // Default tier configuration (should match KarmaTiers contract)
  static readonly TIERS: TierConfig[] = [
    {
      name: "Basic",
      minKarma: 0n,
      maxKarma: ethers.parseEther("999"),
      quota: 6,
    },
    {
      name: "Active",
      minKarma: ethers.parseEther("1000"),
      maxKarma: ethers.parseEther("9999"),
      quota: 120,
    },
    {
      name: "Regular",
      minKarma: ethers.parseEther("10000"),
      maxKarma: ethers.parseEther("99999"),
      quota: 720,
    },
    {
      name: "Power User",
      minKarma: ethers.parseEther("100000"),
      maxKarma: ethers.parseEther("999999"),
      quota: 14400,
    },
    {
      name: "High-Throughput",
      minKarma: ethers.parseEther("1000000"),
      maxKarma: ethers.parseEther("9999999"),
      quota: 86400,
    },
    {
      name: "S-Tier",
      minKarma: ethers.parseEther("10000000"),
      maxKarma: ethers.MaxUint256,
      quota: 432000,
    },
  ];

  constructor(
    private karmaContract: ethers.Contract,
    private rlnContract: ethers.Contract,
    private admin: ethers.Signer,
    private rlnClient: RlnTestClient,
  ) {}

  /**
   * Mint Karma to a user to achieve a specific tier
   */
  async mintKarmaToTier(userAddress: string, tierName: string): Promise<void> {
    const karmaAmount = this.getTierKarmaAmount(tierName);

    logger.debug("Minting Karma to tier", {
      user: userAddress,
      tier: tierName,
      amount: karmaAmount.toString(),
    });

    const tx = await (this.karmaContract as ethers.Contract).connect(this.admin).mint(userAddress, karmaAmount, {
      gasLimit: 100000,
      gasPrice: ethers.parseUnits("15", "gwei"), // Premium gas to bypass RLN
    });

    await tx.wait(1, 30000); // 1 confirmation, 30s timeout

    logger.debug("Karma minted", {
      user: userAddress,
      txHash: tx.hash,
    });
  }

  /**
   * Get the Karma amount for a specific tier (uses minimum + buffer)
   */
  getTierKarmaAmount(tierName: string): bigint {
    const tier = KarmaTestManager.TIERS.find((t) => t.name === tierName);

    if (!tier) {
      throw new Error(`Unknown tier: ${tierName}`);
    }

    // Return min + 10% to ensure we're solidly in the tier
    const buffer = (tier.maxKarma - tier.minKarma) / 10n;
    return tier.minKarma + buffer;
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
   */
  async exhaustUserQuota(user: ethers.Signer, recipientAddress: string): Promise<number> {
    const userAddress = await user.getAddress();

    logger.debug("Exhausting user quota", { user: userAddress });

    // Get user's tier info
    const tierInfo = await this.rlnClient.getUserTierInfo(userAddress);

    if (!tierInfo.tier) {
      throw new Error(`User ${userAddress} has no tier assigned`);
    }

    const remainingQuota = tierInfo.tier.quota - tierInfo.txCount;

    logger.debug("User quota info", {
      user: userAddress,
      tier: tierInfo.tier.name,
      totalQuota: tierInfo.tier.quota,
      used: tierInfo.txCount,
      remaining: remainingQuota,
    });

    // Submit transactions to exhaust quota
    for (let i = 0; i < remainingQuota; i++) {
      const receipt = await this.rlnClient.sendGaslessTransaction(user, {
        to: recipientAddress,
        value: 0n,
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
  async waitForRlnRegistration(userAddress: string, timeout: number = 30000): Promise<void> {
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
   * Verify user is registered to RLN
   */
  async isUserRegistered(userAddress: string): Promise<boolean> {
    try {
      const userInfo = await this.rlnContract.users(userAddress);
      return userInfo && userInfo.identityCommitment !== ethers.ZeroHash;
    } catch (error) {
      return false;
    }
  }

  /**
   * Update tiers in KarmaTiers contract
   */
  async updateTiers(karmaTiersContract: ethers.Contract, tiers: TierConfig[]): Promise<void> {
    logger.debug("Updating tiers in contract", {
      tierCount: tiers.length,
    });

    const tierStructs = tiers.map((t) => ({
      name: t.name,
      minKarma: t.minKarma,
      maxKarma: t.maxKarma,
      txPerEpoch: t.quota,
    }));

    const tx = await (karmaTiersContract as ethers.Contract).connect(this.admin).updateTiers(tierStructs);
    await tx.wait();

    logger.debug("Tiers updated", { txHash: tx.hash });
  }
}
