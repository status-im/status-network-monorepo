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

import com.github.benmanes.caffeine.cache.Cache;
import com.github.benmanes.caffeine.cache.Caffeine;
import com.google.common.annotations.VisibleForTesting;
import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import io.grpc.stub.StreamObserver;
import java.io.Closeable;
import java.io.IOException;
import java.math.BigInteger;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.time.Instant;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.atomic.AtomicInteger;
import net.consensys.linea.config.GasKillSwitchMonitor;
import net.consensys.linea.config.LineaRlnValidatorConfiguration;
import net.consensys.linea.rln.JniRlnVerificationService;
import net.consensys.linea.rln.RlnVerificationService;
import net.consensys.linea.sequencer.txpoolvalidation.shared.DenyListManager;
import net.consensys.linea.sequencer.txpoolvalidation.shared.KarmaServiceClient;
import net.consensys.linea.sequencer.txpoolvalidation.shared.KarmaServiceClient.KarmaInfo;
import net.consensys.linea.sequencer.txpoolvalidation.shared.NullifierTracker;
import net.vac.prover.RlnProof;
import net.vac.prover.RlnProofFilter;
import net.vac.prover.RlnProofReply;
import net.vac.prover.RlnProverGrpc;
import org.apache.tuweni.bytes.Bytes;
import org.hyperledger.besu.datatypes.Address;
import org.hyperledger.besu.datatypes.Transaction;
import org.hyperledger.besu.datatypes.Wei;
import org.hyperledger.besu.plugin.services.BlockchainService;
import org.hyperledger.besu.plugin.services.txvalidator.PluginTransactionPoolValidator;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * RLN (Rate Limiting Nullifier) Verifier Validator for gasless transaction validation.
 *
 * <p>This validator implements a comprehensive RLN verification system that:
 *
 * <ul>
 *   <li>Maintains a deny list of addresses that have exceeded their quotas
 *   <li>Verifies RLN proofs using JNI calls to Rust implementation
 *   <li>Queries gRPC Karma service for user quota status (service handles all counting internally)
 *   <li>Provides premium gas bypass functionality for deny-listed users
 * </ul>
 *
 * <p><strong>Core Validation Flow:</strong>
 *
 * <ol>
 *   <li>Check if sender is on deny list and validate premium gas if required
 *   <li>Retrieve and verify RLN proof from in-memory cache
 *   <li>Validate proof authenticity using cryptographic verification
 *   <li>Query user's current quota status via gRPC karma service
 *   <li>Add to deny list if quota exceeded, otherwise allow transaction
 * </ol>
 *
 * <p><strong>gRPC Integration:</strong> This validator maintains two gRPC connections:
 *
 * <ul>
 *   <li>RLN Proof Service: Streaming server for receiving RLN proofs
 *   <li>Karma Service: Request-response service for querying user quota status
 * </ul>
 *
 * Both connections feature exponential backoff reconnection strategies.
 *
 * <p><strong>Cache Management:</strong> Implements an LRU cache with TTL expiration for efficient
 * proof storage and retrieval during asynchronous transaction validation.
 *
 * <p><strong>Thread Safety:</strong> All operations are thread-safe using concurrent data
 * structures and proper synchronization for file I/O operations.
 *
 * @see PluginTransactionPoolValidator
 * @see LineaRlnValidatorConfiguration
 * @author Status Network Development Team
 * @since 1.0
 */
@SuppressWarnings(
    "deprecation") // BytesHolder.toHexString() deprecated in besu 26.3; migration pending
public class RlnVerifierValidator implements PluginTransactionPoolValidator, Closeable {
  private static final Logger LOG = LoggerFactory.getLogger(RlnVerifierValidator.class);

  private final LineaRlnValidatorConfiguration rlnConfig;
  private final BlockchainService blockchainService;
  private final byte[] rlnVerifyingKeyBytes;
  private final DenyListManager denyListManager;
  private final RlnVerificationService rlnVerificationService;
  private ScheduledExecutorService proofCacheEvictionScheduler;

  // Instance maps — there is now only one validator instance (created by the factory singleton)
  private final Map<String, CompletableFuture<CachedProof>> pendingProofs =
      new ConcurrentHashMap<>();

  private final AtomicInteger activeProofWaits = new AtomicInteger(0);
  private static final int MAX_CONCURRENT_PROOF_WAITS = 100;

  /**
   * Represents a cached RLN proof with combined format and extracted public inputs.
   *
   * @param combinedProofBytes Combined proof data (proof + proof values serialized together)
   * @param senderBytes Sender address bytes
   * @param shareXHex X-coordinate of the secret share (public input)
   * @param shareYHex Y-coordinate of the secret share (public input)
   * @param epochHex Current epoch identifier (public input)
   * @param rootHex Merkle tree root of the RLN membership tree (public input)
   * @param nullifierHex Unique nullifier for this transaction (public input)
   * @param cachedAt Timestamp when this proof was cached for TTL management
   */
  record CachedProof(
      byte[] combinedProofBytes,
      byte[] senderBytes,
      String shareXHex,
      String shareYHex,
      String epochHex,
      String rootHex,
      String nullifierHex,
      Instant cachedAt) {}

  // Caffeine cache for RLN proofs — instance-level (one validator instance via factory singleton)
  private Cache<String, CachedProof> rlnProofCache;

  // gRPC client members for proof service
  private ManagedChannel proofServiceChannel;
  private RlnProverGrpc.RlnProverStub asyncProofStub;

  // Shared karma service client (injected dependency)
  private final KarmaServiceClient karmaServiceClient;

  // Shared nullifier tracker for preventing proof reuse (injected dependency)
  private final NullifierTracker nullifierTracker;

  // Gas kill switch monitor (injected dependency)
  private final GasKillSwitchMonitor gasKillSwitchMonitor;

  private ScheduledExecutorService grpcReconnectionScheduler;

  // Exponential backoff state — instance-level (one validator instance via factory singleton)
  private final AtomicInteger proofStreamRetryCount = new AtomicInteger(0);
  private volatile long lastProofStreamRetryTime = 0;

  // Subscription management — instance-level (one validator instance via factory singleton)
  private final java.util.concurrent.atomic.AtomicBoolean subscriptionActive =
      new java.util.concurrent.atomic.AtomicBoolean(false);
  private volatile io.grpc.Context.CancellableContext currentStreamContext = null;
  private final Object subscriptionLock = new Object();

  /**
   * Creates a new RLN Verifier Validator with default gRPC channel management.
   *
   * @param rlnConfig Configuration for RLN validation including service endpoints
   * @param blockchainService Blockchain service for accessing chain state
   * @param denyListManager Shared deny list manager for state consistency
   * @param karmaServiceClient Shared karma service client for quota validation
   * @param nullifierTracker Shared nullifier tracker for preventing proof reuse
   */
  public RlnVerifierValidator(
      LineaRlnValidatorConfiguration rlnConfig,
      BlockchainService blockchainService,
      DenyListManager denyListManager,
      KarmaServiceClient karmaServiceClient,
      NullifierTracker nullifierTracker,
      GasKillSwitchMonitor gasKillSwitchMonitor) {
    this(
        rlnConfig,
        blockchainService,
        denyListManager,
        karmaServiceClient,
        nullifierTracker,
        gasKillSwitchMonitor,
        null,
        null);
  }

  /**
   * Creates a new RLN Verifier Validator with shared services and optional pre-configured proof
   * channel.
   *
   * <p>This constructor is primarily intended for testing scenarios where a mock proof gRPC channel
   * or mock RLN verification service needs to be injected.
   *
   * @param rlnConfig Configuration for RLN validation
   * @param blockchainService Blockchain service for accessing chain state
   * @param denyListManager Shared deny list manager for state consistency
   * @param karmaServiceClient Shared karma service client for quota validation
   * @param nullifierTracker Shared nullifier tracker for preventing proof reuse
   * @param providedProofChannel Optional pre-configured proof service channel for testing
   * @param providedRlnService Optional pre-configured RLN verification service for testing
   */
  @VisibleForTesting
  RlnVerifierValidator(
      LineaRlnValidatorConfiguration rlnConfig,
      BlockchainService blockchainService,
      DenyListManager denyListManager,
      KarmaServiceClient karmaServiceClient,
      NullifierTracker nullifierTracker,
      GasKillSwitchMonitor gasKillSwitchMonitor,
      ManagedChannel providedProofChannel,
      RlnVerificationService providedRlnService) {
    this.rlnConfig = rlnConfig;
    this.blockchainService = blockchainService;
    this.denyListManager = denyListManager;
    this.karmaServiceClient = karmaServiceClient;
    this.nullifierTracker = nullifierTracker;
    this.gasKillSwitchMonitor = gasKillSwitchMonitor;
    this.proofServiceChannel = providedProofChannel;

    // Initialize RLN verification service
    if (providedRlnService != null) {
      this.rlnVerificationService = providedRlnService;
    } else {
      this.rlnVerificationService = new JniRlnVerificationService();
    }

    // Initialize LRU cache with TTL support
    this.rlnProofCache =
        Caffeine.newBuilder()
            .expireAfterWrite(rlnConfig.rlnProofCacheExpirySeconds(), TimeUnit.SECONDS)
            .maximumSize(rlnConfig.rlnProofCacheMaxSize())
            .build();
    LOG.info(
        "Initialized RLN proof cache with expiry={}s, maxSize={}",
        rlnConfig.rlnProofCacheExpirySeconds(),
        rlnConfig.rlnProofCacheMaxSize());

    if (rlnConfig.rlnValidationEnabled()) {
      LOG.info("RLN Validator is ENABLED.");

      if (denyListManager == null) {
        throw new IllegalArgumentException(
            "DenyListManager cannot be null when RLN validation is enabled");
      }

      byte[] keyBytes;
      try {
        keyBytes = Files.readAllBytes(Paths.get(rlnConfig.verifyingKeyPath()));
        LOG.info("RLN Verifying Key loaded successfully from {}.", rlnConfig.verifyingKeyPath());
        LOG.info(
            "✅ IMPORTANT: RLN verification uses custom circuit with LIMIT_BIT_SIZE=20 (supports ~1M message limit).");
        LOG.info("   - The loaded external key file is kept for API compatibility");
        LOG.info(
            "   - Actual verification uses custom circuit bundled in rln_bridge (rln_final.arkzkey)");
        LOG.info(
            "   - This ensures correct verification for RLN proofs with limits up to 1,048,575");
      } catch (IOException e) {
        LOG.warn(
            "Failed to load external RLN verifying key from {}: {}. This is acceptable when using zerokit's built-in keys.",
            rlnConfig.verifyingKeyPath(),
            e.getMessage());
        LOG.info("✅ Using custom RLN circuit bundled in rln_bridge (no external key file needed).");
        keyBytes = new byte[0]; // Empty placeholder - zerokit ignores this
      } catch (UnsatisfiedLinkError | RuntimeException e) {
        LOG.error("Failed to initialize RLN JNI RlnBridge: {}", e.getMessage(), e);
        throw new IllegalStateException(
            "Failed to initialize RlnVerifierValidator: JNI linkage error", e);
      }
      this.rlnVerifyingKeyBytes = keyBytes;

      initializeGrpcClients();
      startProofStreamSubscription();
      startProofCacheEvictionScheduler();
      initializeSharedProofWaitExecutor();

    } else {
      this.rlnVerifyingKeyBytes = null;
      LOG.info("RLN Validator is DISABLED.");
    }
  }

  /**
   * Initializes gRPC client connection for proof service.
   *
   * <p>Creates managed channel with appropriate TLS configuration based on the provided
   * configuration. Supports both injected channels (for testing) and dynamically created channels.
   */
  private void initializeGrpcClients() {
    // Initialize proof service client
    initializeProofServiceClient();
  }

  /**
   * Initializes the gRPC client for the RLN Proof Service.
   *
   * <p>Creates a managed channel configured for streaming proof reception with appropriate TLS
   * settings based on configuration.
   */
  private void initializeProofServiceClient() {
    boolean wasChannelProvided =
        (this.proofServiceChannel != null && !this.proofServiceChannel.isShutdown());

    if (wasChannelProvided) {
      LOG.info("Using pre-configured ManagedChannel for RLN Proof Service client.");
    } else {
      LOG.info("Creating new ManagedChannel for RLN Proof Service client based on configuration.");
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

      this.proofServiceChannel = channelBuilder.build();
    }

    this.asyncProofStub = RlnProverGrpc.newStub(this.proofServiceChannel);

    if (wasChannelProvided) {
      LOG.info("RLN Proof Service client initialized with injected ManagedChannel.");
    } else {
      LOG.info(
          "RLN Proof Service client initialized for target: {}:{}",
          rlnConfig.rlnProofServiceHost(),
          rlnConfig.rlnProofServicePort());
    }
  }

  /**
   * Starts the gRPC streaming subscription for receiving RLN proofs.
   *
   * <p>Establishes a persistent streaming connection to receive proofs asynchronously as they are
   * generated by the proof service. Implements automatic reconnection with exponential backoff on
   * failures.
   */
  private void startProofStreamSubscription() {
    synchronized (subscriptionLock) {
      if (this.asyncProofStub == null) {
        LOG.error("Cannot start RLN proof stream: gRPC stub not initialized.");
        return;
      }

      // Ensure only one subscription is active at a time
      if (!subscriptionActive.compareAndSet(false, true)) {
        LOG.debug(
            "RLN proof stream subscription already active, skipping duplicate subscription attempt.");
        return;
      }

      // Cancel any previous context before creating new one
      if (currentStreamContext != null) {
        currentStreamContext.cancel(new Exception("Creating new subscription"));
        currentStreamContext = null;
      }

      LOG.info("Starting RLN proof stream subscription...");
    }

    RlnProofFilter request =
        RlnProofFilter.newBuilder().setAddress("").build(); // Empty address means all proofs

    // Create a cancellable context for this stream
    io.grpc.Context.CancellableContext streamContext = io.grpc.Context.current().withCancellation();
    currentStreamContext = streamContext;

    // Run the subscription within the cancellable context
    streamContext.run(
        () -> {
          asyncProofStub.getProofs(
              request,
              new StreamObserver<>() {
                @Override
                public void onNext(RlnProofReply proofMessage) {
                  if (proofMessage.hasProof()) {
                    RlnProof rlnProofMessage = proofMessage.getProof();
                    String txHashHex =
                        Bytes.wrap(rlnProofMessage.getTxHash().toByteArray()).toHexString();
                    LOG.debug("Received proof from gRPC stream for txHash: {}", txHashHex);

                    // Parse the combined proof and extract public inputs using verification service
                    // Note: epoch is extracted from the proof's public inputs by the native code;
                    // the sequencer does not need to compute its own epoch.
                    try {
                      RlnVerificationService.RlnProofData proofData =
                          rlnVerificationService.parseAndVerifyRlnProof(
                              rlnVerifyingKeyBytes,
                              rlnProofMessage.getProof().toByteArray(),
                              "" /* unused — native code uses epoch from proof */);

                      if (proofData != null && proofData.isValid()) {
                        LOG.debug(
                            "Successfully parsed and verified proof for txHash: {}. Proof data: shareX={}, shareY={}, epoch={}, root={}, nullifier={}",
                            txHashHex,
                            proofData.shareX(),
                            proofData.shareY(),
                            proofData.epoch(),
                            proofData.root(),
                            proofData.nullifier());

                        CachedProof cachedProof =
                            new CachedProof(
                                rlnProofMessage.getProof().toByteArray(),
                                rlnProofMessage.getSender().toByteArray(),
                                proofData.shareX(), // share_x
                                proofData.shareY(), // share_y
                                proofData.epoch(), // epoch
                                proofData.root(), // root
                                proofData.nullifier(), // nullifier
                                Instant.now());

                        rlnProofCache.put(txHashHex, cachedProof);
                        LOG.info(
                            "Proof cached for txHash: {}, cache size: {}, proof epoch: {}",
                            txHashHex,
                            rlnProofCache.estimatedSize(),
                            proofData.epoch());

                        // Complete the future for any waiting threads
                        CompletableFuture<CachedProof> proofFuture =
                            pendingProofs.remove(txHashHex);
                        if (proofFuture != null) {
                          proofFuture.complete(cachedProof);
                        }
                      } else {
                        LOG.warn(
                            "Invalid proof received for txHash: {} (verification failed). ProofData: {}",
                            txHashHex,
                            proofData);
                        // Notify waiters about the failure
                        CompletableFuture<CachedProof> proofFuture =
                            pendingProofs.remove(txHashHex);
                        if (proofFuture != null) {
                          proofFuture.complete(null);
                        }
                      }
                    } catch (Exception e) {
                      LOG.error(
                          "Failed to parse and verify proof for txHash: {}: {}",
                          txHashHex,
                          e.getMessage(),
                          e);
                      // Notify waiters about the failure
                      CompletableFuture<CachedProof> proofFuture = pendingProofs.remove(txHashHex);
                      if (proofFuture != null) {
                        proofFuture.complete(null);
                      }
                    }
                  } else if (proofMessage.hasError()) {
                    LOG.error(
                        "Received error from proof stream: {}", proofMessage.getError().getError());
                  }

                  // Reset retry count on successful message (even if proof was invalid)
                  proofStreamRetryCount.set(0);
                }

                @Override
                public void onError(Throwable t) {
                  // Mark subscription as inactive before scheduling reconnection
                  subscriptionActive.set(false);
                  currentStreamContext = null;
                  LOG.error(
                      "RLN proof stream error: {}. Scheduling reconnection...", t.getMessage(), t);
                  scheduleProofStreamReconnection();
                }

                @Override
                public void onCompleted() {
                  // Mark subscription as inactive before scheduling reconnection
                  subscriptionActive.set(false);
                  currentStreamContext = null;
                  LOG.info("RLN proof stream completed by server. Scheduling reconnection...");
                  scheduleProofStreamReconnection();
                }
              });
        });
  }

  /**
   * Schedules reconnection for the proof stream using exponential backoff strategy.
   *
   * <p>Implements intelligent reconnection with increasing delays to avoid overwhelming a failing
   * service while ensuring eventual connectivity restoration.
   *
   * <p><strong>Backoff Strategy:</strong>
   *
   * <ul>
   *   <li>Base delay from configuration (rlnProofStreamRetryIntervalMs)
   *   <li>Exponential increase: delay = base * 2^(retry_count)
   *   <li>Maximum delay capped by maxBackoffDelayMs configuration
   *   <li>Retry count resets on successful connection
   * </ul>
   */
  private void scheduleProofStreamReconnection() {
    synchronized (subscriptionLock) {
      if (grpcReconnectionScheduler == null || grpcReconnectionScheduler.isShutdown()) {
        grpcReconnectionScheduler =
            Executors.newSingleThreadScheduledExecutor(r -> new Thread(r, "RlnGrpcReconnect"));
      }

      long delay;
      if (rlnConfig.exponentialBackoffEnabled()) {
        int retryCount = proofStreamRetryCount.getAndIncrement();
        // Ensure we don't exceed max retries
        if (retryCount >= rlnConfig.rlnProofStreamRetries()) {
          LOG.error(
              "Maximum proof stream retry attempts ({}) exceeded. Stopping reconnection attempts.",
              rlnConfig.rlnProofStreamRetries());
          return;
        }

        // Calculate exponential backoff: base * 2^retryCount, capped at max
        delay =
            Math.min(
                rlnConfig.rlnProofStreamRetryIntervalMs() * (1L << retryCount),
                rlnConfig.maxBackoffDelayMs());

        LOG.info(
            "Scheduling gRPC proof stream reconnection in {} ms (attempt {}/{})",
            delay,
            retryCount + 1,
            rlnConfig.rlnProofStreamRetries());
      } else {
        // Simple fixed delay reconnection
        delay = rlnConfig.rlnProofStreamRetryIntervalMs();
        LOG.info("Scheduling gRPC proof stream reconnection in {} ms (fixed delay)", delay);
      }

      lastProofStreamRetryTime = System.currentTimeMillis();
      grpcReconnectionScheduler.schedule(
          this::startProofStreamSubscription, delay, TimeUnit.MILLISECONDS);
    }
  }

  /**
   * Starts the scheduled task for proof cache eviction.
   *
   * <p>Note: With Caffeine cache, automatic TTL-based eviction is handled internally. This method
   * is kept for compatibility but now only triggers manual cleanup.
   */
  private void startProofCacheEvictionScheduler() {
    // Caffeine handles TTL automatically, but we can still do periodic cleanup for metrics
    proofCacheEvictionScheduler =
        Executors.newSingleThreadScheduledExecutor(r -> new Thread(r, "RlnProofCacheEviction"));
    proofCacheEvictionScheduler.scheduleAtFixedRate(
        this::evictExpiredProofs,
        this.rlnConfig.rlnProofCacheExpirySeconds(),
        this.rlnConfig.rlnProofCacheExpirySeconds(),
        TimeUnit.SECONDS);
  }

  /** Initializes the shared executor for proof waiting operations. */
  private void initializeSharedProofWaitExecutor() {
    // This executor is no longer needed with the CompletableFuture-based approach
    LOG.info("Shared proof wait executor is no longer used.");
  }

  /**
   * Triggers manual cache cleanup and logs cache statistics.
   *
   * <p>Note: Caffeine automatically evicts expired entries, so this is primarily for logging and
   * manual cleanup triggers.
   */
  private void evictExpiredProofs() {
    LOG.debug("Running RLN proof cache cleanup. Current size: {}", rlnProofCache.estimatedSize());
    rlnProofCache.cleanUp(); // Manual cleanup trigger
    LOG.debug(
        "RLN proof cache cleanup finished. Size after cleanup: {}", rlnProofCache.estimatedSize());
  }

  /**
   * Waits for an RLN proof to appear in cache using an event-driven CompletableFuture.
   *
   * <p>This implementation avoids polling by creating a future that is completed by the gRPC stream
   * thread. Implements proper concurrency limits to prevent resource exhaustion.
   *
   * @param txHashString The transaction hash to wait for
   * @return The cached proof if found within timeout, null otherwise
   */
  private CachedProof waitForProofInCache(String txHashString) {
    // First check if proof is already available
    CachedProof proof = rlnProofCache.getIfPresent(txHashString);
    if (proof != null) {
      return proof;
    }

    // Apply backpressure - reject if too many concurrent waits
    if (activeProofWaits.get() >= MAX_CONCURRENT_PROOF_WAITS) {
      LOG.warn(
          "Too many concurrent proof waits ({}), rejecting wait for tx {}",
          activeProofWaits.get(),
          txHashString);
      return null;
    }

    CompletableFuture<CachedProof> proofFuture =
        pendingProofs.computeIfAbsent(txHashString, k -> new CompletableFuture<>());

    activeProofWaits.incrementAndGet();
    try {
      // Wait for the future to be completed by the gRPC onNext handler
      return proofFuture.get(rlnConfig.rlnProofLocalWaitTimeoutMs(), TimeUnit.MILLISECONDS);
    } catch (TimeoutException e) {
      LOG.warn("Proof wait timed out for tx {}", txHashString);
      return null;
    } catch (Exception e) {
      LOG.warn("Error waiting for proof for tx {}: {}", txHashString, e.getMessage(), e);
      return null;
    } finally {
      // Ensure the future is removed to prevent memory leaks if it timed out
      pendingProofs.remove(txHashString);
      activeProofWaits.decrementAndGet();
    }
  }

  /**
   * Adds an address to the deny list with current timestamp.
   *
   * @param address The address to add to the deny list
   */
  void addToDenyList(final Address address) {
    denyListManager.addToDenyList(address);
  }

  /**
   * Removes an address from the deny list.
   *
   * @param address The address to remove from the deny list
   * @return true if the address was in the list and removed, false otherwise
   */
  boolean removeFromDenyList(final Address address) {
    return denyListManager.removeFromDenyList(address);
  }

  /**
   * Fetches current karma status for a user via shared Karma Service client. The karma service
   * handles all transaction counting internally.
   *
   * @param userAddress The user address to query karma information for
   * @return Optional containing karma info (including current quota status) if successful, empty on
   *     failure
   */
  private Optional<KarmaInfo> fetchKarmaInfoFromService(Address userAddress) {
    if (karmaServiceClient == null || !karmaServiceClient.isAvailable()) {
      LOG.warn("Karma service client not available. Cannot fetch karma info.");
      return Optional.empty();
    }

    return karmaServiceClient.fetchKarmaInfo(userAddress);
  }

  /**
   * Validates a transaction against RLN requirements.
   *
   * <p>This is the main validation entry point that orchestrates the complete RLN validation flow
   * including deny list checks, proof verification, and quota enforcement.
   *
   * <p><strong>Validation Steps:</strong>
   *
   * <ol>
   *   <li>Check deny list status and premium gas bypass
   *   <li>Retrieve and validate RLN proof from cache
   *   <li>Verify cryptographic proof authenticity
   *   <li>Check user karma quota via gRPC service
   *   <li>Apply deny list penalties for quota violations
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
    if (!rlnConfig.rlnValidationEnabled()) {
      return Optional.empty(); // RLN validation is disabled
    }

    // Priority txs (configured via tx-pool-priority-senders) bypass RLN checks.
    // This is required to allow infrastructure/deployment accounts to operate
    // regardless of base-fee configuration.
    if (hasPriority) {
      LOG.info(
          "[RLN] Bypass RLN validation for priority transaction {} from {}",
          transaction.getHash().toHexString(),
          transaction.getSender().toHexString());
      return Optional.empty();
    }

    final Address sender = transaction.getSender();
    final org.hyperledger.besu.datatypes.Hash txHash = transaction.getHash();
    final String txHashString = txHash.toHexString();

    // Compute effective gas price (0 indicates gasless intent)
    // For legacy txs: use gasPrice directly
    // For EIP-1559: min(maxFeePerGas, baseFee + maxPriorityFeePerGas)
    final Wei effectiveGasPrice = computeEffectiveGasPrice(transaction);

    // Gas Kill Switch Check: when active, only allow premium gas transactions
    if (gasKillSwitchMonitor != null && gasKillSwitchMonitor.isActive()) {
      long premiumThresholdWei = rlnConfig.premiumGasPriceThresholdWei();
      if (effectiveGasPrice.getAsBigInteger().compareTo(BigInteger.valueOf(premiumThresholdWei))
          >= 0) {
        LOG.info(
            "[RLN] Gas kill switch active but tx {} pays premium gas ({} Wei >= {} Wei). Allowing.",
            txHashString,
            effectiveGasPrice,
            premiumThresholdWei);
        return Optional.empty();
      } else {
        LOG.warn(
            "[RLN] Gas kill switch ACTIVE - rejecting gasless tx {} from {} (gas price {} Wei < premium threshold {} Wei)",
            txHashString,
            sender.toHexString(),
            effectiveGasPrice,
            premiumThresholdWei);
        return Optional.of(
            "Gas kill switch is active. All gasless transactions are temporarily disabled.");
      }
    }

    // 1. Deny List Check
    if (denyListManager.isDenied(sender)) {
      // User is actively denied. Check for premium gas.
      long premiumThresholdWei = rlnConfig.premiumGasPriceThresholdWei();

      if (effectiveGasPrice.getAsBigInteger().compareTo(BigInteger.valueOf(premiumThresholdWei))
          >= 0) {
        // Remove from deny list AND reset epoch counter — paying premium gas earns a fresh quota
        denyListManager.removeFromDenyListAndResetQuota(sender);
        LOG.info(
            "Sender {} paid premium gas ({} Wei >= {} Wei). Removed from deny list and quota reset.",
            sender.toHexString(),
            effectiveGasPrice,
            premiumThresholdWei);
        return Optional.empty(); // Allow transaction — no RLN proof needed for premium gas
      } else {
        LOG.warn(
            "Sender {} is on deny list. Transaction {} rejected. Effective gas price {} Wei < {} Wei.",
            sender.toHexString(),
            txHashString,
            effectiveGasPrice,
            premiumThresholdWei);
        return Optional.of("Sender on deny list, premium gas not met.");
      }
    }

    // Global premium-gas bypass: if user pays at or above premium threshold, allow without RLN
    long premiumThresholdWei = rlnConfig.premiumGasPriceThresholdWei();
    if (effectiveGasPrice.getAsBigInteger().compareTo(BigInteger.valueOf(premiumThresholdWei))
        >= 0) {
      LOG.info(
          "[RLN] Premium gas payment detected ({} Wei >= {} Wei) for tx {}. Bypassing RLN validation.",
          effectiveGasPrice,
          premiumThresholdWei,
          txHashString);
      return Optional.empty();
    }

    // 2. Pre-check: Query karma service BEFORE waiting for proof to fail fast on quota exceeded
    // This avoids waiting 10 seconds for a proof that will never come when user exceeded quota
    Optional<KarmaInfo> preCheckKarmaOpt = fetchKarmaInfoFromService(sender);
    if (preCheckKarmaOpt.isPresent()) {
      KarmaInfo karmaInfo = preCheckKarmaOpt.get();

      // If user has already exceeded quota, reject immediately
      if (karmaInfo.epochTxCount() > karmaInfo.dailyQuota()) {
        LOG.warn(
            "User {} already exceeded quota (count={}, quota={}). Rejecting immediately. Tx: {}",
            sender.toHexString(),
            karmaInfo.epochTxCount(),
            karmaInfo.dailyQuota(),
            txHashString);
        return Optional.of("User transaction quota exceeded.");
      }

      // If user is at their quota limit (last allowed transaction), add to deny list
      // This ensures linea_estimateGas will show premium gas for subsequent attempts
      if (karmaInfo.epochTxCount() == karmaInfo.dailyQuota()) {
        LOG.info(
            "User {} reached quota limit (count={}, quota={}). Adding to deny list. Tx: {}",
            sender.toHexString(),
            karmaInfo.epochTxCount(),
            karmaInfo.dailyQuota(),
            txHashString);
        addToDenyList(sender);
        // Continue to wait for proof - this is the last allowed transaction
      }
    }

    // 3. RLN Proof Verification (via gRPC Cache) - with non-blocking wait
    LOG.debug(
        "Attempting to fetch RLN proof for txHash: {} from cache. isLocal={}, hasPriority={}",
        txHashString,
        isLocal,
        hasPriority);
    CachedProof proof = waitForProofInCache(txHashString);

    if (proof == null) {
      LOG.warn(
          "RLN proof not found in cache after timeout for txHash: {}. Timeout: {}ms (sender={}, gasPrice={}, maxFee={}, maxPrio={})",
          txHashString,
          rlnConfig.rlnProofLocalWaitTimeoutMs(),
          sender.toHexString(),
          transaction.getGasPrice().map(Object::toString).orElse("-"),
          transaction.getMaxFeePerGas().map(Object::toString).orElse("-"),
          transaction.getMaxPriorityFeePerGas().map(Object::toString).orElse("-"));

      // On-demand reconnection: if the proof stream is not active, try to reconnect
      // This handles the case where the prover started after the sequencer exhausted retries
      if (!subscriptionActive.get()) {
        LOG.info("Proof stream is not active, triggering on-demand reconnection...");
        proofStreamRetryCount.set(0); // Reset retry counter for fresh attempt
        scheduleProofStreamReconnection();
      }

      return Optional.of("RLN proof not found in cache after timeout.");
    }
    LOG.debug("RLN proof found in cache for txHash: {}", txHashString);

    // Verify sender-proof binding: the proof must be for this sender
    if (!java.util.Arrays.equals(proof.senderBytes(), sender.getBytes().toArray())) {
      LOG.error(
          "SECURITY VIOLATION: RLN proof sender mismatch for tx {}. Expected: {}, Proof sender: {}",
          txHashString,
          sender.toHexString(),
          org.apache.tuweni.bytes.Bytes.wrap(proof.senderBytes()).toHexString());
      return Optional.of("RLN proof sender mismatch");
    }

    // Validate proof epoch format first
    if (proof.epochHex() == null || proof.epochHex().trim().isEmpty()) {
      LOG.warn("Invalid proof epoch for tx {}: epoch is null or empty", txHashString);
      return Optional.of("RLN validation failed: Invalid proof epoch");
    }

    // Validate that the proof epoch is a valid hex string
    if (!proof.epochHex().matches("^0x[0-9a-fA-F]+$")) {
      LOG.warn("Invalid proof epoch format for tx {}: {}", txHashString, proof.epochHex());
      return Optional.of("RLN validation failed: Invalid proof epoch format");
    }

    // Nullifier dedup: use the proof's epoch (computed by the prover, not the sequencer)
    String proofEpochId = proof.epochHex();
    if (nullifierTracker != null) {
      boolean isNullifierNew =
          nullifierTracker.checkAndMarkNullifier(proof.nullifierHex(), proofEpochId);
      if (!isNullifierNew) {
        LOG.error(
            "Nullifier reuse detected for tx {}. Nullifier: {}, Proof Epoch: {}",
            txHashString,
            proof.nullifierHex(),
            proofEpochId);
        return Optional.of(
            "RLN validation failed: Nullifier already used in epoch " + proofEpochId);
      }
      LOG.debug(
          "Nullifier {} verified as unique for proof epoch {}", proof.nullifierHex(), proofEpochId);
    } else {
      LOG.error("NullifierTracker not available - cannot prevent nullifier reuse");
      return Optional.of("RLN validation failed: Nullifier tracking unavailable");
    }

    // Since the proof was already verified and public inputs extracted during caching,
    // we can skip the verification step here as the proof is already validated.
    // However, for completeness and double-checking, we can still verify if needed.

    // The proof verification was already done during the onNext() processing when the proof was
    // cached.
    // At this point, we can trust that the cached proof is valid and the public inputs are correct.
    LOG.info("Using cached and pre-verified RLN proof for tx: {}", txHashString);

    // 3. Karma / Quota Check (via gRPC Karma Service) - always fetch fresh data
    // SECURITY FIX (BP-C2): Do NOT reuse pre-check data here. Between the pre-check
    // and this point, other transactions from the same sender may have been processed,
    // making the pre-check data stale (TOCTOU race condition).
    Optional<KarmaInfo> karmaInfoOpt = fetchKarmaInfoFromService(sender);

    if (karmaInfoOpt.isEmpty()) {
      // SECURITY: Reject when karma service is down to prevent DoS attacks
      LOG.warn(
          "Karma service unavailable for sender {} and tx {}. REJECTING transaction for security.",
          sender.toHexString(),
          txHashString);

      return Optional.of(
          "RLN validation failed: Karma service unavailable - transaction rejected for security");
    }

    KarmaInfo karmaInfo = karmaInfoOpt.get();
    LOG.debug(
        "Karma info for sender {}: Tier={}, EpochTxCount={}, DailyQuota={}, EpochId={}, KarmaBalance={}",
        sender.toHexString(),
        karmaInfo.tier(),
        karmaInfo.epochTxCount(),
        karmaInfo.dailyQuota(),
        karmaInfo.epochId(),
        karmaInfo.karmaBalance());

    // Check if user has exceeded their quota (karma service handles all counting internally)
    // No grace transaction - users get exactly their quota, then are denied
    if (karmaInfo.epochTxCount() > karmaInfo.dailyQuota()) {
      LOG.warn(
          "User {} (Tier: {}) has exceeded their quota. Count: {}, Quota: {}. Transaction {} rejected.",
          sender.toHexString(),
          karmaInfo.tier(),
          karmaInfo.epochTxCount(),
          karmaInfo.dailyQuota(),
          txHashString);
      return Optional.of("User transaction quota exceeded. Transaction rejected.");
    }

    // If user is at exactly their quota limit, allow this transaction but add to deny list
    // This ensures linea_estimateGas will show premium gas for subsequent attempts
    if (karmaInfo.epochTxCount() == karmaInfo.dailyQuota()) {
      LOG.info(
          "User {} (Tier: {}) reached quota limit. Count: {}, Quota: {}. Adding to deny list. Transaction {} allowed.",
          sender.toHexString(),
          karmaInfo.tier(),
          karmaInfo.epochTxCount(),
          karmaInfo.dailyQuota(),
          txHashString);
      addToDenyList(sender);
      // Continue to allow this transaction - it's the last one
    }

    // User is within quota - allow transaction (karma service handles transaction counting
    // internally)
    LOG.debug(
        "User {} (Tier: {}) is within transaction quota. Count: {}, Quota: {}. Transaction {} allowed by karma check.",
        sender.toHexString(),
        karmaInfo.tier(),
        karmaInfo.epochTxCount(),
        karmaInfo.dailyQuota(),
        txHashString);

    LOG.info(
        "Transaction {} from sender {} passed all RLN validations.",
        txHashString,
        sender.toHexString());
    return Optional.empty(); // Transaction is valid from RLN perspective
  }

  /**
   * Computes the effective gas price for a transaction, correctly handling both legacy and EIP-1559
   * transaction types.
   *
   * <p>For legacy transactions, returns the gasPrice directly. For EIP-1559 transactions, computes
   * min(maxFeePerGas, baseFee + maxPriorityFeePerGas) to prevent bypass attacks where a high
   * maxFeePerGas with zero maxPriorityFeePerGas would appear as premium.
   *
   * @param transaction The transaction to compute effective gas price for
   * @return The effective gas price as Wei
   */
  private Wei computeEffectiveGasPrice(Transaction transaction) {
    // Legacy transaction: use gasPrice directly
    if (transaction.getGasPrice().isPresent()) {
      return Wei.of(transaction.getGasPrice().get().getAsBigInteger());
    }
    // EIP-1559 transaction: effective = min(maxFeePerGas, baseFee + maxPriorityFeePerGas)
    if (transaction.getMaxFeePerGas().isPresent()) {
      Wei maxFeePerGas = Wei.of(transaction.getMaxFeePerGas().get().getAsBigInteger());
      Wei maxPriorityFeePerGas =
          transaction
              .getMaxPriorityFeePerGas()
              .map(q -> Wei.of(q.getAsBigInteger()))
              .orElse(Wei.ZERO);
      Wei baseFee =
          blockchainService
              .getChainHeadHeader()
              .getBaseFee()
              .map(q -> Wei.of(q.getAsBigInteger()))
              .orElse(Wei.ZERO);
      Wei basePlusPriority =
          Wei.of(baseFee.getAsBigInteger().add(maxPriorityFeePerGas.getAsBigInteger()));
      return maxFeePerGas.getAsBigInteger().compareTo(basePlusPriority.getAsBigInteger()) <= 0
          ? maxFeePerGas
          : basePlusPriority;
    }
    return Wei.ZERO;
  }

  /**
   * Closes all resources including gRPC channels and scheduled executors.
   *
   * <p>Ensures graceful shutdown of all background tasks and network connections. This method
   * should be called when the validator is no longer needed to prevent resource leaks.
   *
   * @throws IOException if there are issues during resource cleanup
   */
  @Override
  public void close() throws IOException {
    LOG.info("Closing RlnVerifierValidator resources...");

    synchronized (subscriptionLock) {
      // Cancel active stream subscription first
      subscriptionActive.set(false);
      if (currentStreamContext != null) {
        currentStreamContext.cancel(new Exception("Validator shutting down"));
        currentStreamContext = null;
      }

      // Shutdown gRPC channel
      if (proofServiceChannel != null && !proofServiceChannel.isShutdown()) {
        proofServiceChannel.shutdown();
        try {
          if (!proofServiceChannel.awaitTermination(5, TimeUnit.SECONDS)) {
            proofServiceChannel.shutdownNow();
          }
        } catch (InterruptedException e) {
          proofServiceChannel.shutdownNow();
          Thread.currentThread().interrupt();
        }
      }

      // Shutdown reconnection scheduler
      if (grpcReconnectionScheduler != null && !grpcReconnectionScheduler.isShutdown()) {
        grpcReconnectionScheduler.shutdownNow();
      }
    }

    if (karmaServiceClient != null) {
      try {
        karmaServiceClient.close();
      } catch (IOException e) {
        LOG.warn("Error closing karma service client: {}", e.getMessage(), e);
      }
    }

    // Shutdown cache eviction scheduler
    if (proofCacheEvictionScheduler != null && !proofCacheEvictionScheduler.isShutdown()) {
      proofCacheEvictionScheduler.shutdownNow();
    }

    // Clear pending proof futures
    pendingProofs.values().forEach(f -> f.cancel(false));
    pendingProofs.clear();

    LOG.info("RlnVerifierValidator resources closed.");
  }

  // Test-only helper methods

  @VisibleForTesting
  void addToDenyListForTest(Address user, Instant addedAt) {
    denyListManager.addToDenyList(user);
  }

  @VisibleForTesting
  boolean isDeniedForTest(Address user) {
    return denyListManager.isDenied(user);
  }

  @VisibleForTesting
  Optional<CachedProof> getProofFromCacheForTest(String txHash) {
    return Optional.ofNullable(rlnProofCache.getIfPresent(txHash));
  }

  @VisibleForTesting
  void addProofToCacheForTest(String txHash, CachedProof proof) {
    rlnProofCache.put(txHash, proof);
  }
}
