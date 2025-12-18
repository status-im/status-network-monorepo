import { ethers } from "ethers";
import { createTestLogger } from "../../config/logger";
import { RLN_CONFIG } from "../config/rln-config";

const logger = createTestLogger();

export interface DenyListEntry {
  address: string;
  deniedAt: Date;
  expiresAt?: Date | undefined;
  reason?: string | undefined;
}

/**
 * Deny List Manager for testing deny list functionality
 *
 * The deny list is now stored in the RLN prover's PostgreSQL database and accessed via gRPC.
 * This test manager uses multiple approaches to check deny list status:
 *
 * 1. Primary: Uses `linea_estimateGas` RPC - denied users get premium gas multiplier
 * 2. Secondary: Uses gRPC endpoint via JSON-RPC proxy (if available)
 * 3. Fallback: Behavior-based detection (transaction rejection patterns)
 *
 * Note: Direct file-based access is no longer supported since the deny list
 * has been migrated from a text file to the prover's database.
 */
export class DenyListTestManager {
  private provider: ethers.JsonRpcProvider;
  private rlnProverUrl: string;

  constructor(rlnProverUrl: string = RLN_CONFIG.services.rlnProverUrl, rpcUrl: string = RLN_CONFIG.services.rpcUrl) {
    this.rlnProverUrl = rlnProverUrl;
    this.provider = new ethers.JsonRpcProvider(rpcUrl);
  }

  /**
   * Check if an address is on the deny list by comparing gas estimates.
   * Denied users receive inflated gas estimates with premium multiplier (1.5x).
   *
   * For a simple transfer, normal gasLimit is 21000.
   * Denied users get ~31500 (21000 * 1.5).
   *
   * This is the most reliable method since it tests actual system behavior.
   */
  async isDeniedViaGasEstimate(address: string): Promise<boolean> {
    try {
      // Get gas estimate for a simple transfer
      const estimate = await this.provider.send("linea_estimateGas", [
        {
          from: address,
          to: "0x0000000000000000000000000000000000000001",
          value: "0x0",
          data: "0x",
        },
      ]);

      // Check if gasLimit is inflated (premium multiplier applied)
      // Normal simple transfer: 21000
      // Denied user (1.5x multiplier): ~31500
      if (estimate.gasLimit) {
        const gasLimit = BigInt(estimate.gasLimit);
        const normalGasLimit = 21000n;
        const premiumThreshold = 28000n; // > 1.3x normal indicates premium

        if (gasLimit >= premiumThreshold) {
          logger.debug("User appears to be denied (inflated gas limit)", {
            address,
            gasLimit: gasLimit.toString(),
            normalGasLimit: normalGasLimit.toString(),
            threshold: premiumThreshold.toString(),
          });
          return true;
        }
      }

      return false;
    } catch (error) {
      logger.debug("Gas estimate check failed, trying other methods", {
        address,
        error: error instanceof Error ? error.message : String(error),
      });
      return false;
    }
  }

  /**
   * Check if an address is on the deny list via RLN prover gRPC service.
   * Uses HTTP-based JSON-RPC proxy if available.
   */
  async isDeniedViaProver(address: string): Promise<boolean> {
    try {
      // Try to call the deny list endpoint via HTTP
      // The RLN prover may expose a REST API for deny list queries
      const response = await fetch(`${this.rlnProverUrl}/deny-list/check`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ address: address.toLowerCase() }),
      });

      if (response.ok) {
        const data = await response.json();
        return data.isDenied === true;
      }

      return false;
    } catch (error) {
      logger.debug("Prover deny list check failed", {
        address,
        error: error instanceof Error ? error.message : String(error),
      });
      return false;
    }
  }

  /**
   * Check if an address is on the deny list by attempting a gasless transaction.
   * If the transaction is rejected with a deny-related error, the user is denied.
   */
  async isDeniedViaBehavior(address: string, wallet: ethers.Wallet): Promise<boolean> {
    try {
      // Attempt a gasless transaction
      const tx = {
        to: "0x0000000000000000000000000000000000000001",
        value: 0n,
        gasPrice: 0n,
        gasLimit: 21000n,
        data: "0x",
      };

      await wallet.sendTransaction(tx);
      // If transaction succeeds or is pending, user is not denied
      return false;
    } catch (error) {
      const errMsg = error instanceof Error ? error.message : String(error);
      // Check for deny list related error messages
      if (errMsg.match(/denied|deny.?list|blocked|premium.*gas.*required/i)) {
        logger.debug("User is denied (behavior check)", { address, error: errMsg });
        return true;
      }
      // Other errors don't necessarily mean denied
      return false;
    }
  }

  /**
   * Check if an address is on the deny list.
   * Tries multiple methods for reliability.
   */
  async isDenied(address: string): Promise<boolean> {
    // Method 1: Check via gas estimate (most reliable)
    const deniedViaGas = await this.isDeniedViaGasEstimate(address);
    if (deniedViaGas) {
      return true;
    }

    // Method 2: Check via prover API
    const deniedViaProver = await this.isDeniedViaProver(address);
    if (deniedViaProver) {
      return true;
    }

    return false;
  }

  /**
   * Wait for an address to be added to the deny list.
   */
  async waitForDenied(address: string, timeout: number = RLN_CONFIG.test.maxWaitForDenyListMs): Promise<void> {
    logger.debug("Waiting for address to be denied", { address, timeout });

    const startTime = Date.now();
    const pollInterval = RLN_CONFIG.test.denyListPollIntervalMs;

    while (Date.now() - startTime < timeout) {
      if (await this.isDenied(address)) {
        logger.debug("Address is now denied", { address });
        return;
      }

      await this.sleep(pollInterval);
    }

    throw new Error(`Address ${address} not added to deny list after ${timeout}ms`);
  }

  /**
   * Wait for an address to be removed from the deny list.
   */
  async waitForNotDenied(address: string, timeout: number = RLN_CONFIG.test.maxWaitForDenyListMs): Promise<void> {
    logger.debug("Waiting for address to be removed from deny list", {
      address,
      timeout,
    });

    const startTime = Date.now();
    const pollInterval = RLN_CONFIG.test.denyListPollIntervalMs;

    while (Date.now() - startTime < timeout) {
      if (!(await this.isDenied(address))) {
        logger.debug("Address is no longer denied", { address });
        return;
      }

      await this.sleep(pollInterval);
    }

    throw new Error(`Address ${address} still on deny list after ${timeout}ms`);
  }

  /**
   * Get deny list entry details for an address via prover API.
   * Returns null if not on deny list or if API is unavailable.
   */
  async getDenyListEntry(address: string): Promise<DenyListEntry | null> {
    try {
      const response = await fetch(`${this.rlnProverUrl}/deny-list/entry`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ address: address.toLowerCase() }),
      });

      if (response.ok) {
        const data = await response.json();
        if (data.entry) {
          return {
            address: data.entry.address,
            deniedAt: new Date(data.entry.deniedAt * 1000),
            expiresAt: data.entry.expiresAt ? new Date(data.entry.expiresAt * 1000) : undefined,
            reason: data.entry.reason,
          };
        }
      }

      return null;
    } catch (error) {
      logger.debug("Failed to get deny list entry", {
        address,
        error: error instanceof Error ? error.message : String(error),
      });
      return null;
    }
  }

  /**
   * Get the age of a deny list entry in milliseconds.
   * Returns null if not on deny list.
   */
  async getEntryAge(address: string): Promise<number | null> {
    const entry = await this.getDenyListEntry(address);
    if (!entry) {
      return null;
    }

    return Date.now() - entry.deniedAt.getTime();
  }

  /**
   * Clear the deny list for testing purposes.
   * This requires admin access to the prover's database.
   * In most cases, tests should work around existing entries.
   */
  async clearDenyList(): Promise<void> {
    logger.warn("clearDenyList() is not supported with database-backed deny list");
    logger.warn("Tests should use new addresses or wait for TTL expiry");
  }

  /**
   * Manually add an address to the deny list for testing.
   * This requires admin access to the prover's database.
   */
  async addToDenyList(address: string, reason?: string): Promise<void> {
    try {
      const response = await fetch(`${this.rlnProverUrl}/deny-list/add`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          address: address.toLowerCase(),
          reason: reason || "Test addition",
        }),
      });

      if (!response.ok) {
        throw new Error(`Failed to add to deny list: ${response.status}`);
      }

      logger.debug("Address added to deny list via API", { address });
    } catch (error) {
      logger.warn("Failed to add to deny list via API, may need to trigger via quota violation", {
        address,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  }

  /**
   * Remove an address from the deny list for testing.
   * This is typically done by paying premium gas, which the sequencer handles.
   */
  async removeFromDenyList(address: string): Promise<void> {
    try {
      const response = await fetch(`${this.rlnProverUrl}/deny-list/remove`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ address: address.toLowerCase() }),
      });

      if (!response.ok) {
        throw new Error(`Failed to remove from deny list: ${response.status}`);
      }

      logger.debug("Address removed from deny list via API", { address });
    } catch (error) {
      logger.warn("Failed to remove from deny list via API, use premium gas transaction instead", {
        address,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  }

  /**
   * Get the total number of entries in the deny list.
   * Returns -1 if API is unavailable.
   */
  async getEntryCount(): Promise<number> {
    try {
      const response = await fetch(`${this.rlnProverUrl}/deny-list/count`, {
        method: "GET",
        headers: { "Content-Type": "application/json" },
      });

      if (response.ok) {
        const data = await response.json();
        return data.count || 0;
      }

      return -1;
    } catch {
      return -1;
    }
  }

  private sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}
