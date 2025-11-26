import { exec } from "child_process";
import { promisify } from "util";
import { createTestLogger } from "../../config/logger";

const logger = createTestLogger();

const execAsync = promisify(exec);

export interface LogSearchOptions {
  since?: string; // Docker logs --since format (e.g., "30s", "5m", "1h")
  filter?: string; // String to filter log lines by
  tail?: number; // Number of lines to tail
}

/**
 * Docker Log Monitor for testing
 */
export class DockerLogMonitor {
  /**
   * Get logs from a Docker container
   */
  async getLogs(
    container: string,
    options: LogSearchOptions = {},
  ): Promise<string[]> {
    try {
      const cmdParts = ["docker", "logs", container];

      if (options.since) {
        cmdParts.push("--since", options.since);
      }

      if (options.tail) {
        cmdParts.push("--tail", options.tail.toString());
      }

      const cmd = cmdParts.join(" ");
      logger.debug("Executing log command", { cmd });

      const { stdout, stderr } = await execAsync(cmd);
      const allOutput = stdout + stderr; // Docker logs can output to stderr
      const lines = allOutput.split("\n").filter((line) => line.trim());

      if (options.filter) {
        return lines.filter((line) =>
          line.toLowerCase().includes(options.filter!.toLowerCase()),
        );
      }

      return lines;
    } catch (error: any) {
      logger.warn("Failed to get Docker logs", {
        container,
        error: error.message,
      });
      return [];
    }
  }

  /**
   * Assert that logs contain a specific pattern
   */
  async assertLogContains(
    container: string,
    pattern: string,
    options: LogSearchOptions = { since: "30s" },
  ): Promise<void> {
    const logs = await this.getLogs(container, {
      ...options,
      filter: pattern,
    });

    if (logs.length === 0) {
      throw new Error(
        `Expected logs from ${container} to contain "${pattern}", but found no matches`,
      );
    }

    logger.debug("Log assertion passed", {
      container,
      pattern,
      matchCount: logs.length,
    });
  }

  /**
   * Assert that logs do NOT contain a specific pattern
   */
  async assertLogDoesNotContain(
    container: string,
    pattern: string,
    options: LogSearchOptions = { since: "30s" },
  ): Promise<void> {
    const logs = await this.getLogs(container, {
      ...options,
      filter: pattern,
    });

    if (logs.length > 0) {
      throw new Error(
        `Expected logs from ${container} to NOT contain "${pattern}", but found ${logs.length} matches`,
      );
    }

    logger.debug("Log assertion passed (negative)", {
      container,
      pattern,
    });
  }

  /**
   * Get log lines matching a pattern
   */
  async getMatchingLogs(
    container: string,
    pattern: string,
    options: LogSearchOptions = { since: "30s" },
  ): Promise<string[]> {
    return await this.getLogs(container, {
      ...options,
      filter: pattern,
    });
  }

  /**
   * Wait for a log pattern to appear
   */
  async waitForLogPattern(
    container: string,
    pattern: string,
    timeout: number = 30000,
  ): Promise<void> {
    logger.debug("Waiting for log pattern", {
      container,
      pattern,
      timeout,
    });

    const startTime = Date.now();
    const checkInterval = 1000;

    while (Date.now() - startTime < timeout) {
      const logs = await this.getLogs(container, {
        since: "10s",
        filter: pattern,
      });

      if (logs.length > 0) {
        logger.debug("Log pattern found", {
          container,
          pattern,
          matchCount: logs.length,
        });
        return;
      }

      await this.sleep(checkInterval);
    }

    throw new Error(
      `Log pattern "${pattern}" not found in ${container} after ${timeout}ms`,
    );
  }

  /**
   * Check if a Docker container is running
   */
  async isContainerRunning(containerName: string): Promise<boolean> {
    try {
      const cmd = `docker ps --filter name=${containerName} --format '{{.Status}}'`;
      const { stdout } = await execAsync(cmd);
      return stdout.trim().length > 0;
    } catch (error) {
      return false;
    }
  }

  /**
   * Get container health status
   */
  async getContainerHealth(containerName: string): Promise<string> {
    try {
      const cmd = `docker inspect --format='{{.State.Health.Status}}' ${containerName}`;
      const { stdout } = await execAsync(cmd);
      return stdout.trim();
    } catch (error: any) {
      logger.warn("Failed to get container health", {
        container: containerName,
        error: error.message,
      });
      return "unknown";
    }
  }

  /**
   * Wait for container to be healthy
   */
  async waitForHealthy(
    containerName: string,
    timeout: number = 60000,
  ): Promise<void> {
    logger.debug("Waiting for container to be healthy", {
      container: containerName,
      timeout,
    });

    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      const health = await this.getContainerHealth(containerName);

      if (health === "healthy") {
        logger.debug("Container is healthy", { container: containerName });
        return;
      }

      await this.sleep(2000);
    }

    throw new Error(
      `Container ${containerName} not healthy after ${timeout}ms`,
    );
  }

  private sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}

