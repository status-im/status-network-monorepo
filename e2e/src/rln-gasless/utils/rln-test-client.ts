import { ethers } from "ethers";
import { Logger } from "winston";
import { createTestLogger } from "../../config/logger";
import { RLN_CONFIG } from "../config/rln-config";
import { txBenchmarker } from "./tx-benchmarker";

const logger = createTestLogger();

export interface UserTierInfo {
  currentEpoch: number;
  epochTxCount: number;
  dailyQuota: number;
  tier: string;
  karmaBalance: string;
  isDenied: boolean;
}

export interface GaslessTransactionOptions {
  to: string;
  value?: bigint;
  data?: string;
  gasLimit?: number;
}

export interface PremiumGasTransactionOptions extends GaslessTransactionOptions {
  gasPrice: bigint;
}

export interface LineaGasEstimate {
  gasLimit: string;
  baseFeePerGas: string;
  priorityFeePerGas: string;
}

export interface ProofStatus {
  txHash: string;
  proofReceived: boolean;
  proofValid: boolean;
  timestamp: number;
}

/**
 * RLN Test Client for interacting with the gasless transaction system
 * Supports both mock and production mode testing
 */
export class RlnTestClient {
  private logger: Logger;

  constructor(
    private readonly _rpcProvider: ethers.Provider,
    private sequencerProvider: ethers.Provider,
    private rpcUrl: string,
    private karmaServiceUrl?: string,
  ) {
    this.logger = logger;
  }

  get rpcProvider(): ethers.Provider {
    return this._rpcProvider;
  }

  /**
   * Send a gasless transaction (gasPrice: 0)
   * Uses 'latest' nonce to avoid stale pending transaction issues
   */
  async sendGaslessTransaction(
    signer: ethers.Signer,
    options: GaslessTransactionOptions,
  ): Promise<ethers.TransactionReceipt> {
    const from = await signer.getAddress();
    const nonce = await this._rpcProvider.getTransactionCount(from, "latest");
    const sendTime = Date.now();

    this.logger.debug("Sending gasless transaction", {
      from,
      to: options.to,
      nonce,
    });

    const tx = await signer.sendTransaction({
      to: options.to,
      value: options.value ?? 0n,
      data: options.data ?? "0x",
      gasLimit: options.gasLimit ?? 25000,
      gasPrice: 0, // Gasless intent
      nonce,
    });

    this.logger.debug("Gasless transaction sent", { txHash: tx.hash });

    // Wait for tx to be mined (just until included in block, no extra confirmations)
    const receipt = await tx.wait(1, RLN_CONFIG.test.transactionTimeoutMs);
    if (!receipt) {
      throw new Error("Transaction receipt is null");
    }

    const minedTime = Date.now();
    txBenchmarker.recordTx("gasless", tx.hash, sendTime, minedTime, receipt.blockNumber);

    this.logger.debug("Gasless transaction mined", {
      txHash: tx.hash,
      blockNumber: receipt.blockNumber,
      status: receipt.status,
      latency: `${minedTime - sendTime}ms`,
    });

    return receipt;
  }

  /**
   * Send a gasless transaction and expect it to fail
   * Returns the error message if it fails, throws if it succeeds
   */
  async sendGaslessTransactionExpectFailure(
    signer: ethers.Signer,
    options: GaslessTransactionOptions,
    timeoutMs: number = 30000,
  ): Promise<string> {
    const from = await signer.getAddress();
    const nonce = await this._rpcProvider.getTransactionCount(from, "latest");

    this.logger.debug("Sending gasless transaction (expecting failure)", {
      from,
      to: options.to,
      nonce,
    });

    try {
      const timeoutPromise = new Promise<never>((_, reject) => {
        setTimeout(() => reject(new Error("Transaction timeout - no RLN proof generated")), timeoutMs);
      });

      const txPromise = signer.sendTransaction({
        to: options.to,
        value: options.value ?? 0n,
        data: options.data ?? "0x",
        gasLimit: options.gasLimit ?? 25000,
        gasPrice: 0,
        nonce,
      });

      const tx = await Promise.race([txPromise, timeoutPromise]);
      const receipt = await tx.wait(1, timeoutMs);

      if (receipt && receipt.status === 1) {
        throw new Error("Expected transaction to fail but it succeeded");
      }

      return "Transaction reverted";
    } catch (error: unknown) {
      const err = error instanceof Error ? error : new Error(String(error));
      if (err.message.includes("Expected transaction to fail")) {
        throw err;
      }
      this.logger.debug("Transaction failed as expected", { error: err.message });
      return err.message;
    }
  }

  /**
   * Send a transaction with premium gas
   */
  async sendPremiumGasTransaction(
    signer: ethers.Signer,
    options: PremiumGasTransactionOptions,
  ): Promise<ethers.TransactionReceipt> {
    const from = await signer.getAddress();
    const nonce = await this._rpcProvider.getTransactionCount(from, "latest");
    const sendTime = Date.now();

    this.logger.debug("Sending premium gas transaction", {
      from,
      to: options.to,
      gasPrice: options.gasPrice.toString(),
      nonce,
    });

    const tx = await signer.sendTransaction({
      to: options.to,
      value: options.value ?? 0n,
      data: options.data ?? "0x",
      gasLimit: options.gasLimit ?? 25000,
      gasPrice: options.gasPrice,
      nonce,
    });

    this.logger.debug("Premium gas transaction sent", { txHash: tx.hash });

    // Wait for tx to be mined (just until included in block, no extra confirmations)
    const receipt = await tx.wait(1, RLN_CONFIG.test.transactionTimeoutMs);
    if (!receipt) {
      throw new Error("Transaction receipt is null");
    }

    const minedTime = Date.now();
    txBenchmarker.recordTx("premium", tx.hash, sendTime, minedTime, receipt.blockNumber);

    this.logger.debug("Premium gas transaction mined", {
      txHash: tx.hash,
      blockNumber: receipt.blockNumber,
      status: receipt.status,
      latency: `${minedTime - sendTime}ms`,
    });

    return receipt;
  }

  /**
   * Get user tier info from Karma Service via gRPC HTTP gateway
   */
  async getUserTierInfo(address: string): Promise<UserTierInfo> {
    if (!this.karmaServiceUrl) {
      throw new Error("Karma service URL not configured");
    }

    try {
      const response = await fetch(`${this.karmaServiceUrl}/v1/karma/${address}`, {
        method: "GET",
        headers: {
          "Content-Type": "application/json",
        },
      });

      if (!response.ok) {
        const text = await response.text();
        throw new Error(`Karma service returned ${response.status}: ${text}`);
      }

      const data = await response.json();
      return {
        currentEpoch: data.epoch_id ?? Math.floor(Date.now() / 1000 / RLN_CONFIG.test.epochDurationSeconds),
        epochTxCount: data.epoch_tx_count ?? 0,
        dailyQuota: data.daily_quota ?? 0,
        tier: data.tier ?? "none",
        karmaBalance: data.karma_balance ?? "0",
        isDenied: data.is_denied ?? false,
      };
    } catch (error: unknown) {
      this.logger.error("Failed to get user tier info from Karma service", {
        address,
        error: error instanceof Error ? error.message : String(error),
      });
      throw error;
    }
  }

  /**
   * Check if user is on deny list via Karma Service
   */
  async isUserDenied(address: string): Promise<boolean> {
    try {
      const tierInfo = await this.getUserTierInfo(address);
      return tierInfo.isDenied;
    } catch (error) {
      this.logger.warn("Failed to check deny status, assuming not denied", {
        address,
        error: error instanceof Error ? error.message : String(error),
      });
      return false;
    }
  }

  /**
   * Get user's remaining quota for current epoch
   */
  async getRemainingQuota(address: string): Promise<number> {
    const tierInfo = await this.getUserTierInfo(address);
    return Math.max(0, tierInfo.dailyQuota - tierInfo.epochTxCount);
  }

  /**
   * Call linea_estimateGas RPC method
   */
  async lineaEstimateGas(params: {
    from: string;
    to: string;
    value?: string;
    data?: string;
  }): Promise<LineaGasEstimate> {
    this.logger.debug("Calling linea_estimateGas", params);

    const response = await this.rpcCall("linea_estimateGas", [params]);

    this.logger.debug("linea_estimateGas response", response);

    return response as LineaGasEstimate;
  }

  /**
   * Wait for a transaction to be mined
   */
  async waitForTransaction(
    txHash: string,
    timeout: number = RLN_CONFIG.test.transactionTimeoutMs,
  ): Promise<ethers.TransactionReceipt> {
    this.logger.debug("Waiting for transaction", { txHash, timeout });

    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      try {
        const receipt = await this.sequencerProvider.getTransactionReceipt(txHash);
        if (receipt) {
          this.logger.debug("Transaction mined", {
            txHash,
            blockNumber: receipt.blockNumber,
          });
          return receipt;
        }
      } catch {
        this.logger.debug("Transaction not yet mined", { txHash });
      }

      await this.sleep(2000);
    }

    throw new Error(`Transaction ${txHash} not mined after ${timeout}ms`);
  }

  /**
   * Wait for user to be registered to RLN membership
   * The RLN prover automatically registers users when it sees Karma Transfer events.
   * We verify registration by checking MemberRegistered events on the RLN contract.
   */
  async waitForRegistration(
    rlnContract: ethers.Contract,
    userAddress: string,
    timeout: number = RLN_CONFIG.test.registrationTimeoutMs,
  ): Promise<void> {
    this.logger.debug("Waiting for RLN registration via MemberRegistered events", { userAddress, timeout });

    const startTime = Date.now();
    const normalizedAddress = userAddress.toLowerCase();

    while (Date.now() - startTime < timeout) {
      try {
        // Query MemberRegistered events - these emit (identityCommitment, index)
        const filter = rlnContract.filters.MemberRegistered();
        const events = await rlnContract.queryFilter(filter);

        this.logger.debug("Found MemberRegistered events", { count: events.length });

        // Check each commitment to see if it belongs to our user
        // members(commitment) returns (address userAddress, uint256 index)
        for (const event of events) {
          const eventLog = event as ethers.EventLog;
          if (eventLog.args) {
            const identityCommitment = eventLog.args[0]; // uint256
            try {
              // Call members(uint256) which returns (address, uint256)
              const result = await rlnContract.members(identityCommitment);
              const memberAddress = result[0] || result.userAddress;
              const memberIndex = result[1] || result.index;

              if (memberAddress && memberAddress.toLowerCase() === normalizedAddress) {
                this.logger.debug("User registered to RLN", {
                  userAddress,
                  identityCommitment: identityCommitment.toString(),
                  index: memberIndex?.toString(),
                });
                return;
              }
            } catch (err) {
              // Member lookup failed for this commitment - continue to next
              this.logger.debug("Member lookup failed", {
                commitment: identityCommitment.toString(),
                error: err instanceof Error ? err.message : String(err),
              });
            }
          }
        }
      } catch (error) {
        this.logger.debug("Error checking registration events", {
          userAddress,
          error: error instanceof Error ? error.message : String(error),
        });
      }

      await this.sleep(2000);
    }

    throw new Error(`User ${userAddress} not registered after ${timeout}ms`);
  }

  /**
   * Wait for user to appear on deny list
   */
  async waitForDenyList(address: string, timeout: number = 30000): Promise<void> {
    this.logger.debug("Waiting for user to be denied", { address, timeout });

    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      if (await this.isUserDenied(address)) {
        this.logger.debug("User is now denied", { address });
        return;
      }

      await this.sleep(1000);
    }

    throw new Error(`User ${address} not added to deny list after ${timeout}ms`);
  }

  /**
   * Wait for user to be removed from deny list
   */
  async waitForDenyListRemoval(address: string, timeout: number = 30000): Promise<void> {
    this.logger.debug("Waiting for user to be removed from deny list", { address, timeout });

    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      if (!(await this.isUserDenied(address))) {
        this.logger.debug("User is no longer denied", { address });
        return;
      }

      await this.sleep(1000);
    }

    throw new Error(`User ${address} still on deny list after ${timeout}ms`);
  }

  /**
   * Wait for next epoch to start
   */
  async waitForNextEpoch(maxWaitMs: number = 120000): Promise<number> {
    const epochDuration = RLN_CONFIG.test.epochDurationSeconds * 1000;
    const now = Date.now();
    const currentEpoch = Math.floor(now / epochDuration);
    const nextEpochStart = (currentEpoch + 1) * epochDuration;
    const waitTime = nextEpochStart - now + 1000; // +1s buffer

    if (waitTime > maxWaitMs) {
      throw new Error(`Next epoch starts in ${waitTime}ms, exceeds max wait of ${maxWaitMs}ms`);
    }

    this.logger.debug("Waiting for next epoch", {
      currentEpoch,
      waitTime,
    });

    await this.sleep(waitTime);

    return currentEpoch + 1;
  }

  /**
   * Get current epoch ID
   */
  getCurrentEpoch(): number {
    return Math.floor(Date.now() / 1000 / RLN_CONFIG.test.epochDurationSeconds);
  }

  /**
   * Get transaction pool status
   */
  async getTxPoolStatus(): Promise<{ pending: number; queued: number }> {
    const response = (await this.rpcCall("txpool_status", [])) as { pending: string; queued: string };

    return {
      pending: parseInt(response.pending, 16),
      queued: parseInt(response.queued, 16),
    };
  }

  /**
   * Get current block number
   */
  async getBlockNumber(): Promise<number> {
    return await this.sequencerProvider.getBlockNumber();
  }

  /**
   * Make a raw RPC call
   */
  async rpcCall(method: string, params: unknown[]): Promise<unknown> {
    const response = await fetch(this.rpcUrl, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: Date.now(),
        method,
        params,
      }),
    });

    const json = await response.json();

    if (json.error) {
      throw new Error(`RPC error: ${json.error.message}`);
    }

    return json.result;
  }

  /**
   * Sleep for specified milliseconds
   */
  sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}
