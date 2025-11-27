import { createTestLogger } from "../../config/logger";

const logger = createTestLogger();

export interface TxTiming {
  type: "gasless" | "premium";
  txHash: string;
  sendTime: number; // When tx was sent
  minedTime: number; // When tx was mined
  duration: number; // Total time in ms
  blockNumber: number;
}

export interface BenchmarkSummary {
  gasless: {
    count: number;
    avgMs: number;
    minMs: number;
    maxMs: number;
    p50Ms: number;
    p95Ms: number;
  };
  premium: {
    count: number;
    avgMs: number;
    minMs: number;
    maxMs: number;
    p50Ms: number;
    p95Ms: number;
  };
}

/**
 * Transaction Benchmarker - tracks timing for gasless and premium transactions
 */
export class TxBenchmarker {
  private static instance: TxBenchmarker;
  private timings: TxTiming[] = [];
  private enabled: boolean = true;

  static getInstance(): TxBenchmarker {
    if (!TxBenchmarker.instance) {
      TxBenchmarker.instance = new TxBenchmarker();
    }
    return TxBenchmarker.instance;
  }

  enable(): void {
    this.enabled = true;
  }

  disable(): void {
    this.enabled = false;
  }

  reset(): void {
    this.timings = [];
  }

  recordTx(
    type: "gasless" | "premium",
    txHash: string,
    sendTime: number,
    minedTime: number,
    blockNumber: number,
  ): void {
    if (!this.enabled) return;

    const duration = minedTime - sendTime;
    this.timings.push({
      type,
      txHash,
      sendTime,
      minedTime,
      duration,
      blockNumber,
    });

    // Log individual transaction timing
    logger.info(`⏱️  ${type.toUpperCase()} TX: ${duration}ms`, {
      txHash: txHash.slice(0, 10) + "...",
      duration: `${duration}ms`,
      block: blockNumber,
    });
  }

  getTimings(): TxTiming[] {
    return [...this.timings];
  }

  getSummary(): BenchmarkSummary {
    const gaslessTimes = this.timings.filter((t) => t.type === "gasless").map((t) => t.duration);
    const premiumTimes = this.timings.filter((t) => t.type === "premium").map((t) => t.duration);

    return {
      gasless: this.calculateStats(gaslessTimes),
      premium: this.calculateStats(premiumTimes),
    };
  }

  private calculateStats(durations: number[]): BenchmarkSummary["gasless"] {
    if (durations.length === 0) {
      return { count: 0, avgMs: 0, minMs: 0, maxMs: 0, p50Ms: 0, p95Ms: 0 };
    }

    const sorted = [...durations].sort((a, b) => a - b);
    const sum = sorted.reduce((a, b) => a + b, 0);

    return {
      count: sorted.length,
      avgMs: Math.round(sum / sorted.length),
      minMs: sorted[0],
      maxMs: sorted[sorted.length - 1],
      p50Ms: sorted[Math.floor(sorted.length * 0.5)],
      p95Ms: sorted[Math.floor(sorted.length * 0.95)],
    };
  }

  printSummary(): void {
    const summary = this.getSummary();

    console.log("\n" + "=".repeat(70));
    console.log("📊 TRANSACTION BENCHMARK SUMMARY");
    console.log("=".repeat(70));

    if (summary.gasless.count > 0) {
      console.log("\n🆓 GASLESS TRANSACTIONS:");
      console.log(`   Count:   ${summary.gasless.count}`);
      console.log(`   Average: ${summary.gasless.avgMs}ms`);
      console.log(`   Min:     ${summary.gasless.minMs}ms`);
      console.log(`   Max:     ${summary.gasless.maxMs}ms`);
      console.log(`   P50:     ${summary.gasless.p50Ms}ms`);
      console.log(`   P95:     ${summary.gasless.p95Ms}ms`);
    } else {
      console.log("\n🆓 GASLESS TRANSACTIONS: None recorded");
    }

    if (summary.premium.count > 0) {
      console.log("\n💰 PREMIUM GAS TRANSACTIONS:");
      console.log(`   Count:   ${summary.premium.count}`);
      console.log(`   Average: ${summary.premium.avgMs}ms`);
      console.log(`   Min:     ${summary.premium.minMs}ms`);
      console.log(`   Max:     ${summary.premium.maxMs}ms`);
      console.log(`   P50:     ${summary.premium.p50Ms}ms`);
      console.log(`   P95:     ${summary.premium.p95Ms}ms`);
    } else {
      console.log("\n💰 PREMIUM GAS TRANSACTIONS: None recorded");
    }

    console.log("\n" + "=".repeat(70) + "\n");

    // Also log via logger for test output
    logger.info("📊 BENCHMARK SUMMARY", {
      gasless: summary.gasless,
      premium: summary.premium,
    });
  }
}

// Export singleton instance
export const txBenchmarker = TxBenchmarker.getInstance();
