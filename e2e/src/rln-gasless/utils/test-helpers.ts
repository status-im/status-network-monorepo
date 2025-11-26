import { ethers } from "ethers";
import { createTestLogger } from "../../config/logger";

const logger = createTestLogger();

/**
 * Sleep for specified milliseconds
 */
export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Retry a function with exponential backoff
 */
export async function retryWithBackoff<T>(
  fn: () => Promise<T>,
  maxRetries: number = 3,
  initialDelayMs: number = 1000,
): Promise<T> {
  let lastError: Error | undefined;

  for (let i = 0; i < maxRetries; i++) {
    try {
      return await fn();
    } catch (error: any) {
      lastError = error;
      const delay = initialDelayMs * Math.pow(2, i);

      logger.debug("Retry attempt failed", {
        attempt: i + 1,
        maxRetries,
        nextDelayMs: delay,
        error: error.message,
      });

      if (i < maxRetries - 1) {
        await sleep(delay);
      }
    }
  }

  throw lastError ?? new Error("Retry failed with unknown error");
}

/**
 * Wait for a condition to be true
 */
export async function waitFor(
  condition: () => Promise<boolean>,
  options: {
    timeout?: number;
    interval?: number;
    description?: string;
  } = {},
): Promise<void> {
  const {
    timeout = 30000,
    interval = 1000,
    description = "condition",
  } = options;

  logger.debug("Waiting for condition", { description, timeout, interval });

  const startTime = Date.now();

  while (Date.now() - startTime < timeout) {
    try {
      if (await condition()) {
        logger.debug("Condition met", { description });
        return;
      }
    } catch (error) {
      // Condition check failed, continue waiting
    }

    await sleep(interval);
  }

  throw new Error(
    `Condition "${description}" not met after ${timeout}ms`,
  );
}

/**
 * Generate random hex data
 */
export function randomHexData(length: number): string {
  return "0x" + Buffer.from(ethers.randomBytes(length)).toString("hex");
}

/**
 * Parse gas price from Gwei string
 */
export function parseGwei(gwei: string): bigint {
  return ethers.parseUnits(gwei, "gwei");
}

/**
 * Format gas price to Gwei string
 */
export function formatGwei(wei: bigint): string {
  return ethers.formatUnits(wei, "gwei");
}

/**
 * Get current epoch (TEST mode: 60s epochs)
 */
export function getCurrentEpoch(): number {
  return Math.floor(Date.now() / 1000 / 60);
}

/**
 * Get current epoch slice (TEST mode: 10s slices)
 */
export function getCurrentEpochSlice(): number {
  return Math.floor(((Date.now() / 1000) % 60) / 10);
}

/**
 * Calculate time until next epoch
 */
export function timeUntilNextEpoch(): number {
  const now = Date.now() / 1000;
  const currentEpoch = Math.floor(now / 60);
  const nextEpochStart = (currentEpoch + 1) * 60;
  return Math.ceil((nextEpochStart - now) * 1000); // ms
}

/**
 * Assert that a value is defined (not null or undefined)
 */
export function assertDefined<T>(
  value: T | null | undefined,
  message?: string,
): asserts value is T {
  if (value === null || value === undefined) {
    throw new Error(message ?? "Expected value to be defined");
  }
}

/**
 * Assert that an error is thrown with a specific message pattern
 */
export async function assertThrows(
  fn: () => Promise<any>,
  messagePattern?: RegExp | string,
): Promise<Error> {
  try {
    await fn();
    throw new Error("Expected function to throw, but it didn't");
  } catch (error: any) {
    if (messagePattern) {
      const pattern =
        typeof messagePattern === "string"
          ? new RegExp(messagePattern, "i")
          : messagePattern;

      if (!pattern.test(error.message)) {
        throw new Error(
          `Expected error message to match ${pattern}, but got: ${error.message}`,
        );
      }
    }

    return error;
  }
}

/**
 * Create a funded test wallet
 */
export async function createFundedWallet(
  provider: ethers.Provider,
  funder: ethers.Signer,
  fundAmount: bigint = ethers.parseEther("1"),
): Promise<ethers.HDNodeWallet> {
  const wallet = ethers.Wallet.createRandom().connect(provider);
  const funderAddress = await funder.getAddress();

  logger.debug("Creating funded wallet", {
    address: wallet.address,
    fundAmount: fundAmount.toString(),
  });

  // Get fresh nonce from latest block to avoid stale nonce issues
  const nonce = await provider.getTransactionCount(funderAddress, "latest");

  // Fund the wallet with premium gas price to bypass RLN verification
  // Premium gas threshold is 10 gwei, so we use 15 gwei
  const tx = await funder.sendTransaction({
    to: wallet.address,
    value: fundAmount,
    gasLimit: 21000,
    gasPrice: ethers.parseUnits("15", "gwei"), // Use 15 gwei (premium) to bypass RLN
    nonce, // Explicit nonce from latest block
  });

  // Wait with timeout
  await tx.wait(1, 30000); // 1 confirmation, 30s timeout

  logger.debug("Wallet funded", {
    address: wallet.address,
    txHash: tx.hash,
  });

  return wallet;
}

/**
 * Get balance in ETH as a number (for logging)
 */
export async function getBalanceInEth(
  provider: ethers.Provider,
  address: string,
): Promise<string> {
  const balance = await provider.getBalance(address);
  return ethers.formatEther(balance);
}

/**
 * Log transaction details
 */
export function logTransaction(
  tx: ethers.TransactionResponse | ethers.TransactionReceipt,
  label: string = "Transaction",
): void {
  logger.debug(label, {
    hash: tx.hash,
    from: tx.from,
    to: tx.to,
    blockNumber: "blockNumber" in tx ? tx.blockNumber : "pending",
    gasUsed: "gasUsed" in tx ? tx.gasUsed?.toString() : "N/A",
    status: "status" in tx ? tx.status : "pending",
  });
}

