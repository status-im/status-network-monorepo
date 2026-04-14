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
package net.consensys.linea.sequencer.txpoolvalidation.validators;

import com.google.common.annotations.VisibleForTesting;
import com.google.protobuf.ByteString;
import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import io.grpc.Status;
import io.grpc.StatusRuntimeException;
import java.io.Closeable;
import java.io.IOException;
import java.math.BigInteger;
import java.util.Optional;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;
import net.consensys.linea.config.GasKillSwitchMonitor;
import net.consensys.linea.config.LineaRlnValidatorConfiguration;
import net.consensys.linea.config.LineaTracerConfiguration;
import net.consensys.linea.plugins.config.LineaL1L2BridgeSharedConfiguration;
import net.vac.prover.Address;
import net.vac.prover.RemoveFromDenyListRequest;
import net.vac.prover.RlnProverGrpc;
import net.vac.prover.SendTransactionReply;
import net.vac.prover.SendTransactionRequest;
import net.vac.prover.U256;
import net.vac.prover.Wei;
import org.hyperledger.besu.datatypes.Transaction;
import org.hyperledger.besu.plugin.services.BlockchainService;
import org.hyperledger.besu.plugin.services.TransactionSimulationService;
import org.hyperledger.besu.plugin.services.txvalidator.PluginTransactionPoolValidator;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * RLN Prover Forwarder Validator for sending transactions to RLN prover service.
 *
 * <p>This validator implements a transaction forwarding system that:
 *
 * <ul>
 *   <li>Forwards local transactions to the RLN prover service for proof generation
 *   <li>Only validates transactions when running in RPC node mode (not sequencer mode)
 *   <li>Gracefully handles gRPC failures by falling back to default validation
 *   <li>Provides transaction statistics for monitoring and debugging
 * </ul>
 *
 * <p><strong>Core Validation Flow:</strong>
 *
 * <ol>
 *   <li>Check if transaction is local (only local transactions are forwarded)
 *   <li>Send transaction data to RLN prover service via gRPC
 *   <li>Wait for response from prover service
 *   <li>Allow or reject transaction based on prover response
 *   <li>Fall back to allowing transaction if gRPC service fails
 * </ol>
 *
 * <p><strong>gRPC Integration:</strong> This validator maintains a gRPC connection to the RLN
 * Prover Service for sending transaction data and receiving validation responses.
 *
 * <p><strong>Thread Safety:</strong> All operations are thread-safe using atomic counters for
 * statistics tracking.
 *
 * @see PluginTransactionPoolValidator
 * @see LineaRlnValidatorConfiguration
 * @author Status Network Development Team
 * @since 1.0
 */
@SuppressWarnings(
    "deprecation") // BytesHolder.toHexString() deprecated in besu 26.3; migration pending
public class RlnProverForwarderValidator implements PluginTransactionPoolValidator, Closeable {
  private static final Logger LOG = LoggerFactory.getLogger(RlnProverForwarderValidator.class);

  private final LineaRlnValidatorConfiguration rlnConfig;
  private final boolean enabled;

  // Circuit breaker threshold: reject after this many consecutive gRPC failures
  private static final int CIRCUIT_BREAKER_THRESHOLD = 5;
  // Circuit breaker recovery: allow a probe request after this many ms since last failure
  private final long circuitBreakerRecoveryMs;

  // Statistics tracking
  private final AtomicInteger validationCallCount = new AtomicInteger(0);
  private final AtomicInteger localTransactionCount = new AtomicInteger(0);
  private final AtomicInteger peerTransactionCount = new AtomicInteger(0);
  private final AtomicInteger grpcSuccessCount = new AtomicInteger(0);
  private final AtomicInteger grpcFailureCount = new AtomicInteger(0);
  private final AtomicInteger consecutiveGrpcFailures = new AtomicInteger(0);
  private final AtomicLong lastGrpcFailureTime = new AtomicLong(0);

  // gRPC client components
  private final ManagedChannel channel;
  private final RlnProverGrpc.RlnProverBlockingStub blockingStub;

  // Gas kill switch monitor
  private final GasKillSwitchMonitor gasKillSwitchMonitor;

  // Simulation dependencies for estimating gas used
  private final TransactionSimulationService transactionSimulationService;
  private final BlockchainService blockchainService;
  private final org.hyperledger.besu.plugin.services.WorldStateService worldStateService;
  private final LineaTracerConfiguration tracerConfiguration;
  private final LineaL1L2BridgeSharedConfiguration l1L2BridgeConfiguration;

  /**
   * Creates a new RLN Prover Forwarder Validator with default gRPC channel management.
   *
   * @param rlnConfig Configuration for RLN validation including prover service endpoint
   * @param enabled Whether the validator is enabled (should be false in sequencer mode)
   */
  public RlnProverForwarderValidator(
      LineaRlnValidatorConfiguration rlnConfig,
      boolean enabled,
      TransactionSimulationService transactionSimulationService,
      BlockchainService blockchainService,
      org.hyperledger.besu.plugin.services.WorldStateService worldStateService,
      LineaTracerConfiguration tracerConfiguration,
      LineaL1L2BridgeSharedConfiguration l1L2BridgeSharedConfiguration,
      GasKillSwitchMonitor gasKillSwitchMonitor) {
    this(
        rlnConfig,
        enabled,
        transactionSimulationService,
        blockchainService,
        worldStateService,
        tracerConfiguration,
        l1L2BridgeSharedConfiguration,
        gasKillSwitchMonitor,
        null);
  }

  /**
   * Creates a new RLN Prover Forwarder Validator with default gRPC channel management (legacy
   * constructor for backward compatibility).
   *
   * @param rlnConfig Configuration for RLN validation including prover service endpoint
   * @param enabled Whether the validator is enabled (should be false in sequencer mode)
   */
  public RlnProverForwarderValidator(LineaRlnValidatorConfiguration rlnConfig, boolean enabled) {
    this(rlnConfig, enabled, null, null, null, null, null, null, null);
  }

  /**
   * Creates a new RLN Prover Forwarder Validator with optional pre-configured channel.
   *
   * <p>This constructor is primarily intended for testing scenarios where a mock gRPC channel needs
   * to be injected.
   *
   * @param rlnConfig Configuration for RLN validation
   * @param enabled Whether the validator is enabled
   * @param providedChannel Optional pre-configured gRPC channel for testing
   */
  @VisibleForTesting
  RlnProverForwarderValidator(
      LineaRlnValidatorConfiguration rlnConfig,
      boolean enabled,
      TransactionSimulationService transactionSimulationService,
      BlockchainService blockchainService,
      org.hyperledger.besu.plugin.services.WorldStateService worldStateService,
      LineaTracerConfiguration tracerConfiguration,
      LineaL1L2BridgeSharedConfiguration l1L2BridgeSharedConfiguration,
      GasKillSwitchMonitor gasKillSwitchMonitor,
      ManagedChannel providedChannel) {
    this.rlnConfig = rlnConfig;
    this.enabled = enabled;
    this.gasKillSwitchMonitor = gasKillSwitchMonitor;
    this.circuitBreakerRecoveryMs =
        rlnConfig != null ? rlnConfig.circuitBreakerRecoveryMs() : 30_000L;
    this.transactionSimulationService = transactionSimulationService;
    this.blockchainService = blockchainService;
    this.worldStateService = worldStateService;
    this.tracerConfiguration = tracerConfiguration;
    this.l1L2BridgeConfiguration = l1L2BridgeSharedConfiguration;

    if (enabled) {
      if (providedChannel != null) {
        this.channel = providedChannel;
        LOG.info("Using pre-configured ManagedChannel for RLN Prover Forwarder.");
      } else {
        this.channel = createGrpcChannel();
        LOG.info(
            "RLN Prover Forwarder initialized for endpoint: {}:{}",
            rlnConfig.rlnProofServiceHost(),
            rlnConfig.rlnProofServicePort());
      }
      this.blockingStub = RlnProverGrpc.newBlockingStub(this.channel);
      LOG.info("RLN Prover Forwarder Validator is ENABLED.");
    } else {
      this.channel = null;
      this.blockingStub = null;
      LOG.info("RLN Prover Forwarder Validator is DISABLED (sequencer mode).");
    }
  }

  /**
   * Creates a gRPC channel based on configuration.
   *
   * @return The configured ManagedChannel
   */
  private ManagedChannel createGrpcChannel() {
    ManagedChannelBuilder<?> channelBuilder =
        ManagedChannelBuilder.forAddress(
            rlnConfig.rlnProofServiceHost(), rlnConfig.rlnProofServicePort());

    if (rlnConfig.rlnProofServiceUseTls()) {
      channelBuilder.useTransportSecurity();
    } else {
      channelBuilder.usePlaintext();
    }

    // HTTP/2 keepalive: detect dead connections and trigger automatic reconnection
    channelBuilder
        .keepAliveTime(30, TimeUnit.SECONDS)
        .keepAliveTimeout(10, TimeUnit.SECONDS)
        .keepAliveWithoutCalls(true);

    return channelBuilder.build();
  }

  /**
   * Validates a transaction by forwarding it to the RLN prover service.
   *
   * <p>This is the main validation entry point that forwards local transactions to the RLN prover
   * service for proof generation.
   *
   * <p><strong>Validation Logic:</strong>
   *
   * <ol>
   *   <li>Skip validation if validator is disabled (sequencer mode)
   *   <li>Only forward local transactions, allow peer transactions without forwarding
   *   <li>Send transaction data to RLN prover service via gRPC
   *   <li>Return validation result based on prover response
   *   <li>Fall back to allowing transaction if gRPC fails
   * </ol>
   *
   * @param transaction The transaction to validate
   * @param isLocal Whether this is a local transaction
   * @param hasPriority Whether this transaction has priority status
   * @return Optional error message if validation fails, empty if valid
   */
  @Override
  public Optional<String> validateTransaction(
      Transaction transaction, boolean isLocal, boolean hasPriority) {

    int callCount = validationCallCount.incrementAndGet();

    LOG.debug("*** RLN PROVER FORWARDER VALIDATION #{} ***", callCount);
    LOG.debug("Transaction Hash: {}", transaction.getHash().toHexString());
    LOG.debug("Transaction Sender: {}", transaction.getSender().toHexString());
    LOG.debug("Is Local: {}", isLocal);
    LOG.debug("Has Priority: {}", hasPriority);
    LOG.debug("Validator Enabled: {}", enabled);

    // Skip validation if disabled (sequencer mode)
    if (!enabled) {
      LOG.debug("RLN Prover Forwarder is disabled, skipping validation");
      return Optional.empty();
    }

    // When gas kill switch is active, allow premium transactions but reject gasless
    if (gasKillSwitchMonitor != null && gasKillSwitchMonitor.isActive()) {
      BigInteger killSwitchGasPrice = computeEffectiveGasPrice(transaction);
      long premiumThreshold = rlnConfig != null ? rlnConfig.premiumGasPriceThresholdWei() : 0L;
      if (killSwitchGasPrice.compareTo(BigInteger.valueOf(premiumThreshold)) >= 0) {
        LOG.info(
            "Gas kill switch ACTIVE but tx {} pays premium gas. Allowing.",
            transaction.getHash().toHexString());
        return Optional.empty();
      }
      LOG.warn(
          "Gas kill switch ACTIVE - rejecting gasless tx {} from {} (gas price {} Wei < premium threshold {} Wei)",
          transaction.getHash().toHexString(),
          transaction.getSender().toHexString(),
          killSwitchGasPrice,
          premiumThreshold);
      return Optional.of(
          "Gas kill switch is active. Gasless transactions are temporarily disabled.");
    }

    // Only validate local transactions via gRPC
    if (!isLocal) {
      peerTransactionCount.incrementAndGet();
      LOG.debug("Skipping gRPC forwarding for peer transaction");
      return Optional.empty(); // Accept peer transactions without gRPC forwarding
    }

    // Skip RLN forwarding for PREMIUM transactions (gas price >= premium threshold)
    // Premium transactions bypass RLN entirely - they pay high gas fees
    // Transactions with 0 < gas < premium still need RLN proof
    if (rlnConfig != null) {
      // Compute effective gas price correctly for both legacy and EIP-1559 transactions
      // Legacy: use gasPrice directly
      // EIP-1559: min(maxFeePerGas, baseFee + maxPriorityFeePerGas)
      BigInteger effectiveGasPrice = computeEffectiveGasPrice(transaction);
      long premiumThresholdWei = rlnConfig.premiumGasPriceThresholdWei();
      if (effectiveGasPrice.compareTo(BigInteger.valueOf(premiumThresholdWei)) >= 0) {
        LOG.debug(
            "Skipping gRPC forwarding for premium transaction {} with effectiveGasPrice={} Wei >= threshold {} Wei",
            transaction.getHash().toHexString(),
            effectiveGasPrice,
            premiumThresholdWei);

        // Premium gas payment: remove from deny list and reset epoch counter on the prover.
        // This ensures users who pay premium gas get their gasless quota restored.
        try {
          RemoveFromDenyListRequest removeRequest =
              RemoveFromDenyListRequest.newBuilder()
                  .setAddress(
                      Address.newBuilder()
                          .setValue(
                              ByteString.copyFrom(transaction.getSender().getBytes().toArray()))
                          .build())
                  .setResetEpochCounter(true)
                  .build();
          blockingStub
              .withDeadlineAfter(5000, TimeUnit.MILLISECONDS)
              .removeFromDenyList(removeRequest);
          LOG.info(
              "Premium gas TX {}: removed sender {} from deny list and reset epoch counter",
              transaction.getHash().toHexString(),
              transaction.getSender().toHexString());
        } catch (Exception e) {
          LOG.debug(
              "Failed to remove sender {} from deny list (may not be denied): {}",
              transaction.getSender().toHexString(),
              e.getMessage());
        }

        return Optional.empty(); // Accept premium transactions without RLN forwarding
      }
    }

    localTransactionCount.incrementAndGet();
    LOG.debug(
        "Forwarding local transaction to RLN prover: {} from {} (legacyGasPrice={}, maxFee={}, maxPrio={}, chainId={})",
        transaction.getHash().toHexString(),
        transaction.getSender().toHexString(),
        transaction.getGasPrice().map(Object::toString).orElse("-"),
        transaction.getMaxFeePerGas().map(Object::toString).orElse("-"),
        transaction.getMaxPriorityFeePerGas().map(Object::toString).orElse("-"),
        transaction.getChainId().map(Object::toString).orElse("-"));

    // Circuit breaker: if too many consecutive gRPC failures, reject instead of fail-open
    if (consecutiveGrpcFailures.get() >= CIRCUIT_BREAKER_THRESHOLD) {
      long timeSinceLastFailure = System.currentTimeMillis() - lastGrpcFailureTime.get();
      if (timeSinceLastFailure > circuitBreakerRecoveryMs) {
        LOG.info(
            "Circuit breaker recovery window passed ({} ms since last failure), resetting and allowing probe request for tx {}",
            timeSinceLastFailure,
            transaction.getHash().toHexString());
        consecutiveGrpcFailures.set(0);
        // Fall through to attempt the gRPC call as a probe
      } else {
        LOG.error(
            "Circuit breaker OPEN: {} consecutive gRPC failures (last failure {} ms ago). Rejecting tx {} to prevent fail-open.",
            consecutiveGrpcFailures.get(),
            timeSinceLastFailure,
            transaction.getHash().toHexString());
        return Optional.of(
            "RLN prover service unavailable (circuit breaker open). Please try again later.");
      }
    }

    // Forward to RLN prover
    try {
      SendTransactionRequest.Builder requestBuilder = SendTransactionRequest.newBuilder();

      // Set transaction hash
      requestBuilder.setTransactionHash(
          ByteString.copyFrom(transaction.getHash().getBytes().toArray()));

      // Set sender address
      requestBuilder.setSender(
          Address.newBuilder()
              .setValue(ByteString.copyFrom(transaction.getSender().getBytes().toArray()))
              .build());

      // Set gas price if available
      transaction
          .getGasPrice()
          .ifPresent(
              gasPrice ->
                  requestBuilder.setGasPrice(
                      Wei.newBuilder()
                          .setValue(ByteString.copyFrom(gasPrice.getAsBigInteger().toByteArray()))
                          .build()));

      // Set chain ID if available
      transaction
          .getChainId()
          .ifPresent(
              chainId ->
                  requestBuilder.setChainId(
                      U256.newBuilder()
                          .setValue(ByteString.copyFrom(chainId.toByteArray()))
                          .build()));

      // Provide an estimated gas units value. As an initial implementation,
      // simulate execution to estimate gas used when possible; fallback to tx gas limit.
      long estimatedGasUsed = estimateGasUsed(transaction);
      LOG.debug(
          "Estimated gas used for tx {}: {}",
          transaction.getHash().toHexString(),
          estimatedGasUsed);
      requestBuilder.setEstimatedGasUsed(estimatedGasUsed);

      SendTransactionRequest request = requestBuilder.build();

      LOG.debug(
          "Sending transaction to RLN prover: txHash={}, sender={}, chainId={}",
          transaction.getHash().toHexString(),
          transaction.getSender().toHexString(),
          transaction.getChainId().map(Object::toString).orElse("-"));
      long grpcTimeoutMs = rlnConfig != null ? rlnConfig.karmaServiceTimeoutMs() : 5000L;
      SendTransactionReply reply =
          blockingStub
              .withDeadlineAfter(grpcTimeoutMs, TimeUnit.MILLISECONDS)
              .sendTransaction(request);

      if (reply.getResult()) {
        grpcSuccessCount.incrementAndGet();
        consecutiveGrpcFailures.set(0); // Reset circuit breaker on success
        LOG.debug("RLN prover accepted transaction {}", transaction.getHash());
        return Optional.empty(); // Transaction is valid
      } else {
        grpcFailureCount.incrementAndGet();
        consecutiveGrpcFailures.set(0); // Rejection is a successful response, reset breaker
        LOG.warn("RLN prover rejected transaction {}", transaction.getHash());
        return Optional.of("RLN prover rejected transaction");
      }

    } catch (final StatusRuntimeException sre) {
      // gRPC status-based responses: distinguish intentional rejections from transient errors
      Status.Code code = sre.getStatus().getCode();
      if (code == Status.Code.RESOURCE_EXHAUSTED
          || code == Status.Code.NOT_FOUND
          || code == Status.Code.INVALID_ARGUMENT
          || code == Status.Code.PERMISSION_DENIED
          || code == Status.Code.ALREADY_EXISTS) {
        // Intentional rejection by the prover (quota exceeded, unregistered user, bad input,
        // duplicate tx)
        consecutiveGrpcFailures.set(0); // Not a failure, reset circuit breaker
        LOG.warn(
            "RLN prover rejected transaction {} ({}): {}",
            transaction.getHash(),
            code,
            sre.getStatus().getDescription());
        return Optional.of(
            "RLN prover rejected transaction (" + code + "): " + sre.getStatus().getDescription());
      }
      // Transient gRPC error (UNAVAILABLE, DEADLINE_EXCEEDED, etc.)
      grpcFailureCount.incrementAndGet();
      int failures = consecutiveGrpcFailures.incrementAndGet();
      lastGrpcFailureTime.set(System.currentTimeMillis());
      LOG.warn(
          "gRPC forwarding failed for transaction {} (consecutive failures: {}, status: {}): {}",
          transaction.getHash(),
          failures,
          code,
          sre.getMessage());
      if (failures >= CIRCUIT_BREAKER_THRESHOLD) {
        LOG.error(
            "Circuit breaker OPENING after {} consecutive gRPC failures. Will auto-recover after {} ms.",
            failures,
            circuitBreakerRecoveryMs);
        return Optional.of("RLN prover service unavailable. Please try again later.");
      }
      // Below threshold: graceful fallback - accept the transaction
      return Optional.empty();
    } catch (final Exception e) {
      // Non-gRPC exceptions (unexpected errors)
      grpcFailureCount.incrementAndGet();
      int failures = consecutiveGrpcFailures.incrementAndGet();
      lastGrpcFailureTime.set(System.currentTimeMillis());
      LOG.warn(
          "gRPC forwarding failed for transaction {} (consecutive failures: {}): {}",
          transaction.getHash(),
          failures,
          e.getMessage());
      if (failures >= CIRCUIT_BREAKER_THRESHOLD) {
        LOG.error(
            "Circuit breaker OPENING after {} consecutive gRPC failures. Will auto-recover after {} ms.",
            failures,
            circuitBreakerRecoveryMs);
        return Optional.of("RLN prover service unavailable. Please try again later.");
      }
      // Below threshold: graceful fallback - accept the transaction
      return Optional.empty();
    }
  }

  /**
   * Estimates gas used for a transaction. Simple ETH transfers get 21k. For contract interactions,
   * uses TransactionSimulationService to get actual gas usage (needed for accurate rate limiting by
   * the prover). Falls back to gas limit if simulation is unavailable.
   */
  private long estimateGasUsed(final Transaction transaction) {
    // Fast-path: simple ETH transfer with empty calldata
    if (transaction.getTo().isPresent()
        && transaction.getPayload().isEmpty()
        && transaction.getValue().getAsBigInteger().signum() > 0) {
      return 21_000L;
    }

    // Use simulation to get actual gas usage — the prover uses this value to compute
    // how many "standard transactions" this tx costs for rate limiting (tx_gas_quota).
    // Using gasLimit would over-count since users often set generous limits.
    if (transactionSimulationService != null) {
      try {
        var callParam =
            org.hyperledger.besu.ethereum.transaction.CallParameter.fromTransaction(transaction);
        var resp =
            transactionSimulationService.simulate(
                callParam,
                Optional.empty(),
                transactionSimulationService.simulatePendingBlockHeader(),
                org.hyperledger.besu.evm.tracing.OperationTracer.NO_TRACING,
                java.util.EnumSet.of(
                    TransactionSimulationService.SimulationParameters.ALLOW_FUTURE_NONCE));

        if (resp.isPresent() && resp.get().isSuccessful()) {
          long gasUsed = resp.get().result().getEstimateGasUsedByTransaction();
          LOG.debug(
              "Simulated gas estimate for tx {}: {} (vs gasLimit {})",
              transaction.getHash().toHexString(),
              gasUsed,
              transaction.getGasLimit());
          return gasUsed;
        }
      } catch (Exception e) {
        LOG.debug(
            "Gas simulation failed for tx {}, falling back to gasLimit: {}",
            transaction.getHash().toHexString(),
            e.getMessage());
      }
    }

    // Fallback: use gas limit when simulation is unavailable
    return transaction.getGasLimit();
  }

  /**
   * Computes the effective gas price correctly for both legacy and EIP-1559 transactions. For
   * legacy: gasPrice. For EIP-1559: min(maxFeePerGas, baseFee + maxPriorityFeePerGas).
   */
  private BigInteger computeEffectiveGasPrice(final Transaction transaction) {
    // Legacy transaction
    if (transaction.getGasPrice().isPresent()) {
      return transaction.getGasPrice().get().getAsBigInteger();
    }
    // EIP-1559 transaction
    if (transaction.getMaxFeePerGas().isPresent()) {
      BigInteger maxFeePerGas = transaction.getMaxFeePerGas().get().getAsBigInteger();
      BigInteger maxPriorityFeePerGas =
          transaction
              .getMaxPriorityFeePerGas()
              .map(q -> q.getAsBigInteger())
              .orElse(BigInteger.ZERO);
      BigInteger baseFee = BigInteger.ZERO;
      if (blockchainService != null) {
        try {
          baseFee =
              blockchainService
                  .getChainHeadHeader()
                  .getBaseFee()
                  .map(bf -> bf.getAsBigInteger())
                  .orElse(BigInteger.ZERO);
        } catch (Exception e) {
          LOG.debug("Failed to get baseFee for effective gas price: {}", e.getMessage());
        }
      }
      BigInteger basePlusPriority = baseFee.add(maxPriorityFeePerGas);
      return maxFeePerGas.min(basePlusPriority);
    }
    return BigInteger.ZERO;
  }

  /**
   * Closes the gRPC channel and cleans up resources.
   *
   * @throws IOException if there are issues during resource cleanup
   */
  @Override
  public void close() throws IOException {
    if (channel != null && !channel.isShutdown()) {
      LOG.info("Shutting down RLN Prover Forwarder gRPC channel...");
      channel.shutdown();
      try {
        if (!channel.awaitTermination(5, TimeUnit.SECONDS)) {
          channel.shutdownNow();
        }
        LOG.info("RLN Prover Forwarder gRPC channel shutdown complete.");
      } catch (InterruptedException e) {
        channel.shutdownNow();
        Thread.currentThread().interrupt();
        LOG.warn("Interrupted while shutting down gRPC channel", e);
      }
    }
  }

  // Statistics methods for monitoring and testing

  /**
   * Get the total number of validation calls.
   *
   * @return Total validation call count
   */
  public int getValidationCallCount() {
    return validationCallCount.get();
  }

  /**
   * Get the number of local transactions processed.
   *
   * @return Local transaction count
   */
  public int getLocalTransactionCount() {
    return localTransactionCount.get();
  }

  /**
   * Get the number of peer transactions processed.
   *
   * @return Peer transaction count
   */
  public int getPeerTransactionCount() {
    return peerTransactionCount.get();
  }

  /**
   * Get the number of successful gRPC calls.
   *
   * @return gRPC success count
   */
  public int getGrpcSuccessCount() {
    return grpcSuccessCount.get();
  }

  /**
   * Get the number of failed gRPC calls.
   *
   * @return gRPC failure count
   */
  public int getGrpcFailureCount() {
    return grpcFailureCount.get();
  }

  /**
   * Get the number of consecutive gRPC failures (circuit breaker state).
   *
   * @return Consecutive failure count
   */
  public int getConsecutiveGrpcFailures() {
    return consecutiveGrpcFailures.get();
  }

  /**
   * Get the gRPC service endpoint.
   *
   * @return Endpoint in format "host:port"
   */
  public String getEndpoint() {
    if (rlnConfig != null) {
      return rlnConfig.rlnProofServiceHost() + ":" + rlnConfig.rlnProofServicePort();
    }
    return "unknown";
  }

  /**
   * Check if the validator is enabled.
   *
   * @return true if enabled, false otherwise
   */
  public boolean isEnabled() {
    return enabled;
  }
}
