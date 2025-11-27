import fs from "fs/promises";
import { createTestLogger } from "../../config/logger";

const logger = createTestLogger();

export interface DenyListEntry {
  address: string;
  timestamp: Date;
}

/**
 * Deny List Manager for testing deny list functionality
 */
export class DenyListTestManager {
  constructor(private denyListFilePath: string) {}

  /**
   * Check if an address is on the deny list
   */
  async isDenied(address: string): Promise<boolean> {
    try {
      const entries = await this.readDenyList();
      return entries.some((e) => e.address.toLowerCase() === address.toLowerCase());
    } catch (error) {
      logger.warn("Failed to read deny list", { error });
      return false;
    }
  }

  /**
   * Read all deny list entries
   */
  async readDenyList(): Promise<DenyListEntry[]> {
    try {
      const content = await fs.readFile(this.denyListFilePath, "utf-8");
      return content
        .split("\n")
        .filter((line) => line.trim())
        .map((line) => {
          const [address, timestamp] = line.split(",");
          return {
            address: address.trim(),
            timestamp: new Date(timestamp.trim()),
          };
        });
    } catch (error: unknown) {
      if (error instanceof Error && (error as NodeJS.ErrnoException).code === "ENOENT") {
        // File doesn't exist yet
        return [];
      }
      throw error;
    }
  }

  /**
   * Wait for an address to be added to the deny list
   */
  async waitForDenied(address: string, timeout: number = 10000): Promise<void> {
    logger.debug("Waiting for address to be denied", { address, timeout });

    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      if (await this.isDenied(address)) {
        logger.debug("Address is now denied", { address });
        return;
      }

      await this.sleep(500);
    }

    throw new Error(`Address ${address} not added to deny list after ${timeout}ms`);
  }

  /**
   * Wait for an address to be removed from the deny list
   */
  async waitForNotDenied(address: string, timeout: number = 10000): Promise<void> {
    logger.debug("Waiting for address to be removed from deny list", {
      address,
      timeout,
    });

    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      if (!(await this.isDenied(address))) {
        logger.debug("Address is no longer denied", { address });
        return;
      }

      await this.sleep(500);
    }

    throw new Error(`Address ${address} still on deny list after ${timeout}ms`);
  }

  /**
   * Get deny list entry for an address
   */
  async getDenyListEntry(address: string): Promise<DenyListEntry | null> {
    const entries = await this.readDenyList();
    return entries.find((e) => e.address.toLowerCase() === address.toLowerCase()) ?? null;
  }

  /**
   * Get the age of a deny list entry in milliseconds
   */
  async getEntryAge(address: string): Promise<number | null> {
    const entry = await this.getDenyListEntry(address);
    if (!entry) {
      return null;
    }

    return Date.now() - entry.timestamp.getTime();
  }

  /**
   * Clear the deny list (for test cleanup)
   * Uses secure file permissions (0o600 = owner read/write only) to address CodeQL security concerns
   */
  async clearDenyList(): Promise<void> {
    logger.debug("Clearing deny list", { path: this.denyListFilePath });

    try {
      await fs.writeFile(this.denyListFilePath, "", { encoding: "utf-8", mode: 0o600 });
      logger.debug("Deny list cleared");
    } catch (error) {
      logger.warn("Failed to clear deny list", { error });
    }
  }

  /**
   * Get the total number of entries in the deny list
   */
  async getEntryCount(): Promise<number> {
    const entries = await this.readDenyList();
    return entries.length;
  }

  private sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}
