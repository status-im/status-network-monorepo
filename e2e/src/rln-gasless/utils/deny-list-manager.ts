import fs from "fs/promises";
import { exec } from "child_process";
import { promisify } from "util";
import { createTestLogger } from "../../config/logger";
import { RLN_CONFIG } from "../config/rln-config";

const execAsync = promisify(exec);
const logger = createTestLogger();

export interface DenyListEntry {
  address: string;
  timestamp: Date;
}

/**
 * Deny List Manager for testing deny list functionality
 * Supports both file-based and API-based deny list access
 * Can access deny list inside Docker container via docker exec
 */
export class DenyListTestManager {
  private containerName: string = "sequencer";
  private containerDenyListPath: string = "/data/gasless-deny-list.txt";

  constructor(
    private denyListFilePath: string = RLN_CONFIG.test.denyListPath,
    private karmaServiceUrl: string = RLN_CONFIG.services.karmaServiceUrl,
  ) {}

  /**
   * Check if an address is on the deny list via file (local or Docker container)
   */
  async isDeniedViaFile(address: string): Promise<boolean> {
    try {
      const entries = await this.readDenyListFromContainer();
      return entries.some((e) => e.address.toLowerCase() === address.toLowerCase());
    } catch (error) {
      logger.warn("Failed to read deny list from container", { error });
      // Fall back to local file
      try {
        const entries = await this.readDenyListFromFile();
        return entries.some((e) => e.address.toLowerCase() === address.toLowerCase());
      } catch {
        return false;
      }
    }
  }

  /**
   * Read deny list from Docker container
   */
  async readDenyListFromContainer(): Promise<DenyListEntry[]> {
    try {
      const { stdout } = await execAsync(
        `docker exec ${this.containerName} cat ${this.containerDenyListPath} 2>/dev/null || echo ""`,
      );

      if (!stdout.trim()) {
        return [];
      }

      return stdout
        .split("\n")
        .filter((line) => line.trim() && !line.startsWith("#"))
        .map((line) => {
          const [address, timestamp] = line.split(",");
          return {
            address: address?.trim() || "",
            timestamp: timestamp ? new Date(timestamp.trim()) : new Date(),
          };
        })
        .filter((entry) => entry.address);
    } catch (error) {
      logger.debug("Could not read deny list from container", { error });
      throw error;
    }
  }

  /**
   * Check if an address is on the deny list via API
   */
  async isDeniedViaApi(address: string): Promise<boolean> {
    try {
      const response = await fetch(`${this.karmaServiceUrl}/v1/karma/${address}`, {
        method: "GET",
        headers: { "Content-Type": "application/json" },
      });

      if (!response.ok) {
        return false;
      }

      const data = await response.json();
      return data.is_denied === true;
    } catch (error) {
      logger.warn("Failed to check deny status via API", {
        address,
        error: error instanceof Error ? error.message : String(error),
      });
      return false;
    }
  }

  /**
   * Check if an address is on the deny list (tries container file first, then API)
   */
  async isDenied(address: string): Promise<boolean> {
    // Try container file first (most reliable)
    try {
      const entries = await this.readDenyListFromContainer();
      const isDenied = entries.some((e) => e.address.toLowerCase() === address.toLowerCase());
      if (isDenied) {
        logger.debug("Address found in container deny list", { address });
        return true;
      }
    } catch {
      // Container access failed, continue to other methods
    }

    // Try API
    try {
      return await this.isDeniedViaApi(address);
    } catch {
      // Fall back to local file
      return await this.isDeniedViaFile(address);
    }
  }

  /**
   * Read all deny list entries from file
   */
  async readDenyListFromFile(): Promise<DenyListEntry[]> {
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
        return [];
      }
      throw error;
    }
  }

  /**
   * Wait for an address to be added to the deny list
   */
  async waitForDenied(address: string, timeout: number = 30000): Promise<void> {
    logger.debug("Waiting for address to be denied", { address, timeout });

    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      if (await this.isDenied(address)) {
        logger.debug("Address is now denied", { address });
        return;
      }

      await this.sleep(1000);
    }

    throw new Error(`Address ${address} not added to deny list after ${timeout}ms`);
  }

  /**
   * Wait for an address to be removed from the deny list
   */
  async waitForNotDenied(address: string, timeout: number = 30000): Promise<void> {
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

      await this.sleep(1000);
    }

    throw new Error(`Address ${address} still on deny list after ${timeout}ms`);
  }

  /**
   * Get deny list entry for an address
   */
  async getDenyListEntry(address: string): Promise<DenyListEntry | null> {
    const entries = await this.readDenyListFromFile();
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
   * Clear the deny list file (for test cleanup)
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
   * Manually add an address to the deny list file (for testing)
   */
  async addToDenyListFile(address: string): Promise<void> {
    const timestamp = new Date().toISOString();
    const entry = `${address.toLowerCase()},${timestamp}\n`;

    try {
      await fs.appendFile(this.denyListFilePath, entry, { encoding: "utf-8", mode: 0o600 });
      logger.debug("Address added to deny list file", { address });
    } catch (error) {
      // Create file if it doesn't exist
      await fs.writeFile(this.denyListFilePath, entry, { encoding: "utf-8", mode: 0o600 });
      logger.debug("Created deny list file and added address", { address });
    }
  }

  /**
   * Remove an address from the deny list file (for testing)
   */
  async removeFromDenyListFile(address: string): Promise<void> {
    const entries = await this.readDenyListFromFile();
    const filteredEntries = entries.filter((e) => e.address.toLowerCase() !== address.toLowerCase());

    const content = filteredEntries.map((e) => `${e.address},${e.timestamp.toISOString()}`).join("\n");

    await fs.writeFile(this.denyListFilePath, content ? content + "\n" : "", {
      encoding: "utf-8",
      mode: 0o600,
    });
    logger.debug("Address removed from deny list file", { address });
  }

  /**
   * Get the total number of entries in the deny list
   */
  async getEntryCount(): Promise<number> {
    const entries = await this.readDenyListFromFile();
    return entries.length;
  }

  /**
   * Get all denied addresses
   */
  async getAllDeniedAddresses(): Promise<string[]> {
    const entries = await this.readDenyListFromFile();
    return entries.map((e) => e.address);
  }

  private sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}
