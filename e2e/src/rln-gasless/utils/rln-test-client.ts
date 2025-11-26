import { ethers } from "ethers";
import { Logger } from "winston";
import { createTestLogger } from "../../config/logger";

const logger = createTestLogger();

export interface UserTierInfo {
  currentEpoch: number;
  currentEpochSlice: number;
  txCount: number;
  tier: {
    name: string;
    quota: number;
  } | null;
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

/**
 * RLN Test Client for interacting with the gasless transaction system
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
    
    // Always get fresh nonce from latest block to avoid stale nonces
    const nonce = await this._rpcProvider.getTransactionCount(from, "latest");
    
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
      nonce, // Explicit nonce from latest block
    });

    this.logger.debug("Gasless transaction sent", { txHash: tx.hash });

    // Wait with shorter timeout for L2 speed
    const receipt = await tx.wait(1, 30000); // 30 second timeout for L2
    if (!receipt) {
      throw new Error("Transaction receipt is null");
    }

    this.logger.debug("Gasless transaction mined", {
      txHash: tx.hash,
      blockNumber: receipt.blockNumber,
      status: receipt.status,
    });

    return receipt;
  }

  /**
   * Send a transaction with premium gas
   * Uses 'latest' nonce to avoid stale pending transaction issues
   */
  async sendPremiumGasTransaction(
    signer: ethers.Signer,
    options: PremiumGasTransactionOptions,
  ): Promise<ethers.TransactionReceipt> {
    const from = await signer.getAddress();
    
    // Always get fresh nonce from latest block
    const nonce = await this._rpcProvider.getTransactionCount(from, "latest");
    
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
      nonce, // Explicit nonce from latest block
    });

    this.logger.debug("Premium gas transaction sent", { txHash: tx.hash });

    // Wait with shorter timeout for L2 speed
    const receipt = await tx.wait(1, 30000); // 30 second timeout for L2
    if (!receipt) {
      throw new Error("Transaction receipt is null");
    }

    this.logger.debug("Premium gas transaction mined", {
      txHash: tx.hash,
      blockNumber: receipt.blockNumber,
      status: receipt.status,
    });

    return receipt;
  }

  /**
   * Get user tier info from Karma Service via gRPC
   * Note: This requires the karma-service to be running
   */
  async getUserTierInfo(address: string): Promise<UserTierInfo> {
    if (!this.karmaServiceUrl) {
      throw new Error("Karma service URL not configured");
    }

    // Make HTTP request to karma service (assuming it exposes HTTP endpoint for testing)
    // TODO: Implement actual gRPC client if needed
    try {
      const response = await fetch(`${this.karmaServiceUrl}/user/${address}/tier`, {
        method: "GET",
        headers: {
          "Content-Type": "application/json",
        },
      });

      if (!response.ok) {
        throw new Error(`Karma service returned ${response.status}: ${await response.text()}`);
      }

      const data = await response.json();
      return {
        currentEpoch: data.currentEpoch ?? Math.floor(Date.now() / 1000 / 60),
        currentEpochSlice: data.currentEpochSlice ?? Math.floor((Date.now() / 1000) % 60 / 10),
        txCount: data.txCount ?? 0,
        tier: data.tier ? {
          name: data.tier.name,
          quota: data.tier.quota,
        } : null,
      };
    } catch (error: any) {
      this.logger.error("Failed to get user tier info from Karma service", {
        address,
        error: error.message,
      });
      throw error;
    }
  }

  /**
   * Call linea_estimateGas RPC method
   */
  async lineaEstimateGas(params: {
    from: string;
    to: string;
    value?: string;
    data?: string;
  }): Promise<{
    gasLimit: string;
    baseFeePerGas: string;
    priorityFeePerGas: string;
  }> {
    this.logger.debug("Calling linea_estimateGas", params);

    const response = await this.rpcCall("linea_estimateGas", [params]);

    this.logger.debug("linea_estimateGas response", response);

    return response;
  }

  /**
   * Wait for a transaction to be mined
   */
  async waitForTransaction(
    txHash: string,
    timeout: number = 60000,
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
      } catch (error) {
        this.logger.debug("Transaction not yet mined", { txHash });
      }

      await this.sleep(2000);
    }

    throw new Error(`Transaction ${txHash} not mined after ${timeout}ms`);
  }

  /**
   * Wait for user to be registered to RLN membership
   */
  async waitForRegistration(
    rlnContract: ethers.Contract,
    userAddress: string,
    timeout: number = 30000,
  ): Promise<void> {
    this.logger.debug("Waiting for RLN registration", { userAddress, timeout });

    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      try {
        // Check if user has identity commitment registered
        const userInfo = await rlnContract.users(userAddress);
        if (userInfo && userInfo.identityCommitment !== ethers.ZeroHash) {
          this.logger.debug("User registered to RLN", { userAddress });
          return;
        }
      } catch (error) {
        this.logger.debug("User not yet registered", { userAddress });
      }

      await this.sleep(1000);
    }

    throw new Error(`User ${userAddress} not registered after ${timeout}ms`);
  }

  /**
   * Get transaction pool status
   */
  async getTxPoolStatus(): Promise<{ pending: number; queued: number }> {
    const response = await this.rpcCall("txpool_status", []);

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
  private async rpcCall(method: string, params: unknown[]): Promise<any> {
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
  private sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}

