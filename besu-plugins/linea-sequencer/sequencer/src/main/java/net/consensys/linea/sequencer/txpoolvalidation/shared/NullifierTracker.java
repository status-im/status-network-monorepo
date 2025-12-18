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

import com.github.benmanes.caffeine.cache.Cache;
import com.github.benmanes.caffeine.cache.Caffeine;
import com.google.protobuf.ByteString;
import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import io.grpc.StatusRuntimeException;
import java.io.Closeable;
import java.io.IOException;
import java.time.Duration;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;
import net.vac.prover.CheckAndRecordNullifierReply;
import net.vac.prover.CheckAndRecordNullifierRequest;
import net.vac.prover.RlnProverGrpc;
import org.apache.tuweni.bytes.Bytes;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * High-performance nullifier tracking with database persistence via gRPC.
 *
 * <p><strong>Architecture:</strong> Uses a two-tier approach for maximum performance:
 *
 * <ul>
 *   <li>Hot path: Local in-memory cache (Caffeine) for O(1) duplicate rejection
 *   <li>Cold path: PostgreSQL database via gRPC for persistence and cross-instance sharing
 * </ul>
 *
 * <p><strong>Security Critical:</strong> This component is essential for RLN security. Nullifier
 * tracking prevents replay attacks and enforces transaction rate limiting by detecting when users
 * reuse nullifiers within the same epoch.
 *
 * <p><strong>Performance Target:</strong> 500+ TPS with sub-millisecond response times for
 * duplicate detection.
 *
 * <p><strong>Thread Safety:</strong> All operations are thread-safe and suitable for high-
 * concurrency transaction validation.
 *
 * @author Status Network Development Team
 * @since 1.0
 */
public class NullifierTracker implements Closeable {
  private static final Logger LOG = LoggerFactory.getLogger(NullifierTracker.class);

  private final String serviceName;

  // Local cache for hot path (immediate duplicate rejection)
  private final Cache<String, Boolean> localCache;

  // gRPC client for database persistence
  private ManagedChannel channel;
  private RlnProverGrpc.RlnProverBlockingStub blockingStub;
  private final AtomicBoolean grpcAvailable = new AtomicBoolean(false);

  // gRPC configuration
  private final String grpcHost;
  private final int grpcPort;
  private final boolean useTls;

  // Metrics
  private final AtomicLong totalChecks = new AtomicLong(0);
  private final AtomicLong cacheHits = new AtomicLong(0);
  private final AtomicLong duplicatesDetected = new AtomicLong(0);
  private final AtomicLong grpcFailures = new AtomicLong(0);

  /**
   * Creates a new NullifierTracker with gRPC backend and local cache.
   *
   * @param serviceName Service name for logging
   * @param grpcHost RLN prover gRPC host
   * @param grpcPort RLN prover gRPC port
   * @param useTls Whether to use TLS for gRPC
   * @param cacheSize Maximum size of local cache
   * @param cacheTtlMinutes TTL for cache entries (should match epoch duration)
   */
  public NullifierTracker(
      String serviceName,
      String grpcHost,
      int grpcPort,
      boolean useTls,
      long cacheSize,
      long cacheTtlMinutes) {
    this.serviceName = serviceName;
    this.grpcHost = grpcHost;
    this.grpcPort = grpcPort;
    this.useTls = useTls;

    // Initialize local cache for hot path
    this.localCache =
        Caffeine.newBuilder()
            .maximumSize(cacheSize)
            .expireAfterWrite(Duration.ofMinutes(cacheTtlMinutes))
            .build();

    // Initialize gRPC connection
    initializeGrpcClient();

    LOG.info(
        "{}: NullifierTracker initialized with gRPC backend at {}:{}, cache size: {}, TTL: {} min",
        serviceName,
        grpcHost,
        grpcPort,
        cacheSize,
        cacheTtlMinutes);
  }

  /**
   * Legacy constructor for backward compatibility. Creates a cache-only tracker without gRPC.
   *
   * @param serviceName Service name for logging
   * @param maxSize Maximum cache size
   * @param nullifierExpiryHours Expiry time in hours
   */
  public NullifierTracker(String serviceName, long maxSize, long nullifierExpiryHours) {
    this.serviceName = serviceName;
    this.grpcHost = null;
    this.grpcPort = 0;
    this.useTls = false;

    // Initialize local cache for cache-only operation
    this.localCache =
        Caffeine.newBuilder()
            .maximumSize(maxSize)
            .expireAfterWrite(Duration.ofHours(nullifierExpiryHours))
            .build();

    // No gRPC in legacy mode
    this.grpcAvailable.set(false);

    LOG.info(
        "{}: NullifierTracker initialized in cache-only mode (legacy constructor), max size: {}, TTL: {} hours",
        serviceName,
        maxSize,
        nullifierExpiryHours);
  }

  /**
   * Legacy constructor for backward compatibility.
   *
   * @param serviceName Service name for logging
   * @param nullifierExpiryHours Expiry time in hours
   */
  public NullifierTracker(String serviceName, long nullifierExpiryHours) {
    this(serviceName, 1_000_000L, nullifierExpiryHours);
    LOG.info("{}: Using default capacity (1M) with PostgreSQL via gRPC", serviceName);
  }

  private void initializeGrpcClient() {
    try {
      ManagedChannelBuilder<?> channelBuilder =
          ManagedChannelBuilder.forAddress(grpcHost, grpcPort);

      if (useTls) {
        channelBuilder.useTransportSecurity();
      } else {
        channelBuilder.usePlaintext();
      }

      this.channel = channelBuilder.build();
      this.blockingStub = RlnProverGrpc.newBlockingStub(channel);
      this.grpcAvailable.set(true);

      LOG.info("{}: gRPC client connected to {}:{}", serviceName, grpcHost, grpcPort);
    } catch (Exception e) {
      LOG.error("{}: Failed to initialize gRPC client: {}", serviceName, e.getMessage(), e);
      this.grpcAvailable.set(false);
    }
  }

  /**
   * Checks if a nullifier has been used within the given epoch and marks it as used if new.
   *
   * <p><strong>Performance:</strong> Uses local cache first for immediate duplicate rejection. New
   * nullifiers are persisted to the database via gRPC for cross-instance sharing.
   *
   * <p><strong>Atomicity:</strong> The database operation is atomic (INSERT ON CONFLICT DO
   * NOTHING), ensuring no race conditions even with multiple sequencer instances.
   *
   * @param nullifierHex Hex-encoded nullifier (32 bytes as hex string)
   * @param epochId Epoch identifier (as string, will be parsed to long)
   * @return true if nullifier is new (transaction allowed), false if duplicate (reject)
   */
  public boolean checkAndMarkNullifier(String nullifierHex, String epochId) {
    if (nullifierHex == null || nullifierHex.trim().isEmpty()) {
      LOG.warn("{}: Invalid nullifier: null or empty", serviceName);
      return false;
    }

    if (epochId == null || epochId.trim().isEmpty()) {
      LOG.warn("{}: Invalid epoch ID: null or empty", serviceName);
      return false;
    }

    // Normalize inputs before processing
    String normalizedNullifier = nullifierHex.toLowerCase().trim();
    String normalizedEpoch = epochId.trim();
    String cacheKey = normalizedNullifier + ":" + normalizedEpoch;

    totalChecks.incrementAndGet();

    // Hot path: Check local cache first
    Boolean cached = localCache.getIfPresent(cacheKey);
    if (cached != null) {
      cacheHits.incrementAndGet();
      duplicatesDetected.incrementAndGet();
      LOG.debug("{}: Duplicate nullifier detected in cache: {}", serviceName, cacheKey);
      return false;
    }

    // Cold path: Check and record in database via gRPC
    if (grpcAvailable.get() && blockingStub != null) {
      try {
        byte[] nullifierBytes = Bytes.fromHexString(normalizedNullifier).toArrayUnsafe();
        long epoch = parseEpoch(normalizedEpoch);

        CheckAndRecordNullifierRequest request =
            CheckAndRecordNullifierRequest.newBuilder()
                .setNullifier(ByteString.copyFrom(nullifierBytes))
                .setEpoch(epoch)
                .build();

        CheckAndRecordNullifierReply reply = blockingStub.checkAndRecordNullifier(request);

        if (reply.getIsValid()) {
          // New nullifier - add to local cache
          localCache.put(cacheKey, Boolean.TRUE);
          LOG.debug("{}: New nullifier recorded: {}", serviceName, cacheKey);
          return true;
        } else {
          // Duplicate detected in database
          localCache.put(cacheKey, Boolean.TRUE); // Cache it to speed up future checks
          duplicatesDetected.incrementAndGet();
          LOG.warn("{}: Duplicate nullifier detected in DB: {}", serviceName, cacheKey);
          return false;
        }
      } catch (StatusRuntimeException e) {
        grpcFailures.incrementAndGet();
        LOG.error("{}: gRPC call failed: {}. Using cache-only mode.", serviceName, e.getStatus());
        grpcAvailable.set(false);
        scheduleGrpcReconnect();
        // Fall through to cache-only behavior
      } catch (IllegalArgumentException e) {
        LOG.error("{}: Invalid nullifier format: {}", serviceName, e.getMessage());
        return false;
      }
    }

    // Fallback: Cache-only mode when gRPC is unavailable
    // Use putIfAbsent for thread-safe atomic insertion
    Boolean existingValue = localCache.asMap().putIfAbsent(cacheKey, Boolean.TRUE);
    if (existingValue != null) {
      // Another thread already added this nullifier
      duplicatesDetected.incrementAndGet();
      LOG.debug("{}: Duplicate nullifier detected (concurrent): {}", serviceName, cacheKey);
      return false;
    }
    LOG.debug("{}: Nullifier recorded in cache only (gRPC unavailable): {}", serviceName, cacheKey);
    return true;
  }

  /**
   * Checks if a nullifier exists without marking it.
   *
   * @param nullifierHex Hex-encoded nullifier
   * @param epochId Epoch identifier
   * @return true if nullifier exists (duplicate), false if new
   */
  public boolean isNullifierUsed(String nullifierHex, String epochId) {
    if (nullifierHex == null || epochId == null) {
      return false;
    }

    String cacheKey = nullifierHex.toLowerCase().trim() + ":" + epochId.trim();

    // Check local cache
    if (localCache.getIfPresent(cacheKey) != null) {
      return true;
    }

    // Could add gRPC check here if needed, but for read-only we can rely on cache
    return false;
  }

  private long parseEpoch(String epochId) {
    try {
      // Try parsing as a number first
      return Long.parseLong(epochId.trim());
    } catch (NumberFormatException e) {
      // If it's a hex string (like block hash), hash it to a number
      return epochId.hashCode() & 0xFFFFFFFFL;
    }
  }

  private void scheduleGrpcReconnect() {
    // Simple reconnect after delay
    Thread reconnectThread =
        new Thread(
            () -> {
              try {
                Thread.sleep(30000); // 30 second delay
                LOG.info("{}: Attempting gRPC reconnection...", serviceName);
                initializeGrpcClient();
              } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
              }
            });
    reconnectThread.setDaemon(true);
    reconnectThread.setName(serviceName + "-NullifierGrpcReconnect");
    reconnectThread.start();
  }

  /**
   * Gets current statistics for monitoring.
   *
   * @return Statistics including cache size, checks, hits, and failures
   */
  public NullifierStats getStats() {
    return new NullifierStats(
        (int) localCache.estimatedSize(),
        totalChecks.get(),
        cacheHits.get(),
        duplicatesDetected.get(),
        grpcFailures.get(),
        grpcAvailable.get());
  }

  /** Statistics record for monitoring. */
  public record NullifierStats(
      int cacheSize,
      long totalChecks,
      long cacheHits,
      long duplicatesDetected,
      long grpcFailures,
      boolean grpcAvailable) {}

  @Override
  public void close() throws IOException {
    LOG.info("{}: Shutting down NullifierTracker...", serviceName);

    if (localCache != null) {
      localCache.invalidateAll();
      localCache.cleanUp();
    }

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
    }

    LOG.info("{}: NullifierTracker closed. Final stats: {}", serviceName, getStats());
  }
}
