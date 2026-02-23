/*
 * Copyright Consensys Software Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on
 * an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the
 * specific language governing permissions and limitations under the License.
 *
 * SPDX-License-Identifier: Apache-2.0
 */
package net.consensys.linea.sequencer.txpoolvalidation.shared;

import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import java.io.Closeable;
import java.io.IOException;
import java.util.concurrent.TimeUnit;
import net.consensys.linea.config.GasKillSwitchMonitor;
import net.consensys.linea.config.LineaRlnValidatorConfiguration;
import net.consensys.linea.config.LineaRpcConfiguration;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Centralized manager for shared services used by gasless transaction functionality.
 *
 * <p>This manager ensures proper lifecycle management of shared services:
 *
 * <ul>
 *   <li>DenyListManager: Single source of truth for deny list state
 *   <li>KarmaServiceClient: Shared gRPC client for karma service
 *   <li>NullifierTracker: Prevents nullifier reuse in RLN proofs
 * </ul>
 *
 * <p>The manager handles initialization, configuration, and proper cleanup of all shared resources
 * to prevent resource leaks and ensure consistency.
 *
 * @author Status Network Development Team
 * @since 1.0
 */
public class SharedServiceManager implements Closeable {
  private static final Logger LOG = LoggerFactory.getLogger(SharedServiceManager.class);

  private DenyListManager denyListManager;
  private KarmaServiceClient karmaServiceClient;
  private NullifierTracker nullifierTracker;
  private GasKillSwitchMonitor gasKillSwitchMonitor;
  private ManagedChannel sharedProverChannel; // Shared gRPC channel for prover endpoint (P5)
  private ManagedChannel sharedKarmaChannel; // Shared gRPC channel for karma endpoint (P5)
  private final boolean gaslessEnabled;

  /**
   * Creates a new SharedServiceManager with the specified configuration.
   *
   * @param rlnConfig RLN validator configuration
   * @param rpcConfig RPC configuration (may be null if RPC features disabled)
   */
  public SharedServiceManager(
      LineaRlnValidatorConfiguration rlnConfig, LineaRpcConfiguration rpcConfig) {
    // Initialize karma service if either:
    // 1. RLN validation is enabled (Sequencer mode)
    // 2. RPC gasless is enabled (RPC node mode)
    // 3. RLN prover forwarder is enabled (RPC node mode needs karma for estimates)
    this.gaslessEnabled =
        rlnConfig.rlnValidationEnabled()
            || (rpcConfig != null && rpcConfig.gaslessTransactionsEnabled())
            || (rpcConfig != null && rpcConfig.rlnProverForwarderEnabled());

    if (gaslessEnabled) {
      initializeSharedServices(rlnConfig, rpcConfig);
    } else {
      LOG.info("Gasless transactions and RLN features disabled - shared services not initialized");
    }

    // Initialize gas kill switch monitor (independent of gasless being enabled)
    String killSwitchFile = rlnConfig.gasKillSwitchFilePath();
    if (killSwitchFile != null && !killSwitchFile.isEmpty()) {
      this.gasKillSwitchMonitor =
          new GasKillSwitchMonitor(killSwitchFile, rlnConfig.gasKillSwitchPollSeconds());
      LOG.info(
          "Gas kill switch monitor initialized: file={}, pollInterval={}s",
          killSwitchFile,
          rlnConfig.gasKillSwitchPollSeconds());
    } else {
      LOG.info("Gas kill switch not configured (no file path provided)");
    }
  }

  /** Initializes the shared services based on configuration. */
  private void initializeSharedServices(
      LineaRlnValidatorConfiguration rlnConfig, LineaRpcConfiguration rpcConfig) {
    try {
      // Common gRPC configuration for all services (RLN prover endpoint)
      String grpcHost = rlnConfig.rlnProofServiceHost();
      int grpcPort = rlnConfig.rlnProofServicePort();
      boolean useTls = rlnConfig.rlnProofServiceUseTls();

      // Initialize DenyListManager with gRPC backend (connected to RLN prover's database)
      if (rlnConfig.sharedGaslessConfig() != null) {
        // Convert max age from minutes to seconds for TTL
        long ttlSeconds = rlnConfig.denyListEntryMaxAgeMinutes() * 60;
        long cacheRefreshSeconds = rlnConfig.sharedGaslessConfig().denyListRefreshSeconds();

        this.denyListManager =
            new DenyListManager(
                "SharedServiceManager",
                grpcHost,
                grpcPort,
                useTls,
                ttlSeconds,
                cacheRefreshSeconds);
        LOG.info(
            "DenyListManager initialized with gRPC backend at {}:{} (TLS: {})",
            grpcHost,
            grpcPort,
            useTls);
      } else {
        LOG.warn("Cannot initialize DenyListManager: sharedGaslessConfig is null");
      }

      // Create shared gRPC channel for karma service (P5: channel sharing)
      ManagedChannelBuilder<?> karmaChannelBuilder =
          ManagedChannelBuilder.forAddress(
              rlnConfig.karmaServiceHost(), rlnConfig.karmaServicePort());
      if (rlnConfig.karmaServiceUseTls()) {
        karmaChannelBuilder.useTransportSecurity();
      } else {
        karmaChannelBuilder.usePlaintext();
      }
      this.sharedKarmaChannel = karmaChannelBuilder.build();

      // Initialize KarmaServiceClient with shared channel
      try {
        this.karmaServiceClient =
            new KarmaServiceClient(
                "SharedServiceManager",
                rlnConfig.karmaServiceHost(),
                rlnConfig.karmaServicePort(),
                rlnConfig.karmaServiceUseTls(),
                rlnConfig.karmaServiceTimeoutMs(),
                this.sharedKarmaChannel);
        LOG.info(
            "KarmaServiceClient initialized successfully for {}:{} (TLS: {})",
            rlnConfig.karmaServiceHost(),
            rlnConfig.karmaServicePort(),
            rlnConfig.karmaServiceUseTls());
      } catch (Exception e) {
        LOG.warn(
            "Failed to initialize KarmaServiceClient for {}:{} - continuing without karma service. Error: {}",
            rlnConfig.karmaServiceHost(),
            rlnConfig.karmaServicePort(),
            e.getMessage());
        this.karmaServiceClient = null;
      }

      // Initialize NullifierTracker with gRPC backend (same endpoint as deny list)
      if (rlnConfig.sharedGaslessConfig() != null) {
        // Cache TTL should match epoch duration for proper cleanup
        long cacheTtlMinutes = Math.max(60, rlnConfig.denyListEntryMaxAgeMinutes() * 2);

        this.nullifierTracker =
            new NullifierTracker(
                "SharedServiceManager",
                grpcHost,
                grpcPort,
                useTls,
                1_000_000L, // 1M cache capacity for 500+ TPS
                cacheTtlMinutes);
        LOG.info(
            "NullifierTracker initialized with gRPC backend at {}:{}, cache TTL: {} min",
            grpcHost,
            grpcPort,
            cacheTtlMinutes);
      } else {
        LOG.warn("Cannot initialize NullifierTracker: sharedGaslessConfig is null");
      }

    } catch (Exception e) {
      LOG.error("Failed to initialize shared services: {}", e.getMessage(), e);
      // Clean up any partially initialized services
      closeQuietly();
      throw new IllegalStateException("Failed to initialize shared services", e);
    }
  }

  /**
   * Gets the shared DenyListManager instance.
   *
   * @return DenyListManager instance, or null if gasless features are disabled
   */
  public DenyListManager getDenyListManager() {
    return denyListManager;
  }

  /**
   * Gets the shared KarmaServiceClient instance.
   *
   * @return KarmaServiceClient instance, or null if gasless features are disabled
   */
  public KarmaServiceClient getKarmaServiceClient() {
    return karmaServiceClient;
  }

  /**
   * Gets the shared NullifierTracker instance.
   *
   * @return NullifierTracker instance, or null if gasless features are disabled
   */
  public NullifierTracker getNullifierTracker() {
    return nullifierTracker;
  }

  /**
   * Gets the gas kill switch monitor instance.
   *
   * @return GasKillSwitchMonitor instance, or null if not configured
   */
  public GasKillSwitchMonitor getGasKillSwitchMonitor() {
    return gasKillSwitchMonitor;
  }

  /**
   * Checks if gasless features are enabled and shared services are available.
   *
   * @return true if gasless features are enabled, false otherwise
   */
  public boolean isGaslessEnabled() {
    return gaslessEnabled;
  }

  /**
   * Closes all shared services and releases resources.
   *
   * @throws IOException if there are issues during resource cleanup
   */
  @Override
  public void close() throws IOException {
    LOG.info("Closing shared services...");

    IOException firstException = null;

    if (karmaServiceClient != null) {
      try {
        karmaServiceClient.close();
        LOG.info("KarmaServiceClient closed successfully");
      } catch (IOException e) {
        LOG.error("Error closing KarmaServiceClient: {}", e.getMessage(), e);
        firstException = e;
      }
    }

    if (denyListManager != null) {
      try {
        denyListManager.close();
        LOG.info("DenyListManager closed successfully");
      } catch (IOException e) {
        LOG.error("Error closing DenyListManager: {}", e.getMessage(), e);
        if (firstException == null) {
          firstException = e;
        }
      }
    }

    if (nullifierTracker != null) {
      try {
        nullifierTracker.close();
        LOG.info("NullifierTracker closed successfully");
      } catch (IOException e) {
        LOG.error("Error closing NullifierTracker: {}", e.getMessage(), e);
        if (firstException == null) {
          firstException = e;
        }
      }
    }

    if (gasKillSwitchMonitor != null) {
      try {
        gasKillSwitchMonitor.close();
        LOG.info("GasKillSwitchMonitor closed successfully");
      } catch (IOException e) {
        LOG.error("Error closing GasKillSwitchMonitor: {}", e.getMessage(), e);
        if (firstException == null) {
          firstException = e;
        }
      }
    }

    // Shutdown shared gRPC channels (P5)
    shutdownChannel(sharedProverChannel, "sharedProverChannel");
    shutdownChannel(sharedKarmaChannel, "sharedKarmaChannel");

    LOG.info("Shared services closed");

    // Throw the first exception if any occurred
    if (firstException != null) {
      throw firstException;
    }
  }

  /** Shuts down a gRPC channel gracefully. */
  private void shutdownChannel(ManagedChannel channel, String name) {
    if (channel != null && !channel.isShutdown()) {
      channel.shutdown();
      try {
        if (!channel.awaitTermination(5, TimeUnit.SECONDS)) {
          channel.shutdownNow();
        }
      } catch (InterruptedException e) {
        channel.shutdownNow();
        Thread.currentThread().interrupt();
      }
      LOG.info("{} shut down successfully", name);
    }
  }

  /**
   * Closes services quietly without throwing exceptions. Used for cleanup during initialization
   * failures.
   */
  private void closeQuietly() {
    try {
      close();
    } catch (IOException e) {
      LOG.warn("Error during quiet close: {}", e.getMessage(), e);
    }
  }
}
