import { ethers } from "ethers";
import { createTestLogger } from "../../config/logger";
import { RLN_CONFIG } from "../config/rln-config";
import { getAdminNonce } from "./karma-manager";

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
    } catch (error: unknown) {
      lastError = error instanceof Error ? error : new Error(String(error));
      const delay = initialDelayMs * Math.pow(2, i);

      logger.debug("Retry attempt failed", {
        attempt: i + 1,
        maxRetries,
        nextDelayMs: delay,
        error: lastError.message,
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
  const { timeout = 30000, interval = 1000, description = "condition" } = options;

  logger.debug("Waiting for condition", { description, timeout, interval });

  const startTime = Date.now();

  while (Date.now() - startTime < timeout) {
    try {
      if (await condition()) {
        logger.debug("Condition met", { description });
        return;
      }
    } catch {
      // Condition check failed, continue waiting
    }

    await sleep(interval);
  }

  throw new Error(`Condition "${description}" not met after ${timeout}ms`);
}

/**
 * Generate random hex data
 */
export function randomHexData(length: number): string {
  return "0x" + Buffer.from(ethers.randomBytes(length)).toString("hex");
}

/**
 * Generate unique transaction data to avoid "Known transaction" errors
 */
export function uniqueTxData(prefix: string = "test"): string {
  const timestamp = Date.now();
  const random = Math.random().toString(36).substring(7);
  return ethers.hexlify(ethers.toUtf8Bytes(`${prefix}-${timestamp}-${random}`));
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
 * Get current epoch (based on config epoch duration)
 */
export function getCurrentEpoch(): number {
  return Math.floor(Date.now() / 1000 / RLN_CONFIG.test.epochDurationSeconds);
}

/**
 * Get time until next epoch in milliseconds
 */
export function timeUntilNextEpoch(): number {
  const epochDuration = RLN_CONFIG.test.epochDurationSeconds * 1000;
  const now = Date.now();
  const currentEpoch = Math.floor(now / epochDuration);
  const nextEpochStart = (currentEpoch + 1) * epochDuration;
  return nextEpochStart - now;
}

/**
 * Wait until next epoch starts
 */
export async function waitForNextEpoch(maxWaitMs: number = 120000): Promise<number> {
  const waitTime = timeUntilNextEpoch() + 1000; // +1s buffer

  if (waitTime > maxWaitMs) {
    throw new Error(`Next epoch starts in ${waitTime}ms, exceeds max wait of ${maxWaitMs}ms`);
  }

  logger.debug("Waiting for next epoch", { waitTime });
  await sleep(waitTime);

  return getCurrentEpoch();
}

/**
 * Assert that a value is defined (not null or undefined)
 */
export function assertDefined<T>(value: T | null | undefined, message?: string): asserts value is T {
  if (value === null || value === undefined) {
    throw new Error(message ?? "Expected value to be defined");
  }
}

/**
 * Assert that an error is thrown with a specific message pattern
 */
export async function assertThrows(fn: () => Promise<unknown>, messagePattern?: RegExp | string): Promise<Error> {
  try {
    await fn();
    throw new Error("Expected function to throw, but it didn't");
  } catch (error: unknown) {
    const err = error instanceof Error ? error : new Error(String(error));
    if (err.message === "Expected function to throw, but it didn't") {
      throw err;
    }
    if (messagePattern) {
      const pattern = typeof messagePattern === "string" ? new RegExp(messagePattern, "i") : messagePattern;

      if (!pattern.test(err.message)) {
        throw new Error(`Expected error message to match ${pattern}, but got: ${err.message}`);
      }
    }

    return err;
  }
}

/**
 * Create a funded test wallet
 * Uses shared nonce manager to prevent collisions when multiple wallets are created concurrently
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

  // Use shared nonce manager from karma-manager to prevent collisions
  const nonce = await getAdminNonce(provider, funderAddress);

  const tx = await funder.sendTransaction({
    to: wallet.address,
    value: fundAmount,
    gasLimit: 21000,
    gasPrice: ethers.parseUnits("15", "gwei"),
    nonce,
  });

  await tx.wait(1, RLN_CONFIG.test.transactionTimeoutMs);

  logger.debug("Wallet funded", {
    address: wallet.address,
    txHash: tx.hash,
    nonce,
  });

  return wallet;
}

/**
 * Create multiple funded wallets
 */
export async function createMultipleFundedWallets(
  provider: ethers.Provider,
  funder: ethers.Signer,
  count: number,
  fundAmount: bigint = ethers.parseEther("1"),
): Promise<ethers.HDNodeWallet[]> {
  const wallets: ethers.HDNodeWallet[] = [];

  for (let i = 0; i < count; i++) {
    const wallet = await createFundedWallet(provider, funder, fundAmount);
    wallets.push(wallet);
  }

  return wallets;
}

/**
 * Get balance in ETH as a formatted string
 */
export async function getBalanceInEth(provider: ethers.Provider, address: string): Promise<string> {
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

/**
 * Generate test case ID with description
 */
export function testId(category: string, number: number, description: string): string {
  return `${category}-${String(number).padStart(3, "0")}: ${description}`;
}

/**
 * Premium gas price (above threshold) for bypassing RLN
 */
export const PREMIUM_GAS_PRICE = parseGwei("15");

/**
 * Sub-threshold gas price (requires RLN)
 */
export const SUB_THRESHOLD_GAS_PRICE = parseGwei("9");

/**
 * Exactly at threshold gas price
 */
export const THRESHOLD_GAS_PRICE = parseGwei("10");

/**
 * Recipient address for test transactions
 */
export const TEST_RECIPIENT = RLN_CONFIG.accounts.recipient;
