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
import io.grpc.StatusRuntimeException;
import java.io.Closeable;
import java.io.IOException;
import java.time.Instant;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import net.vac.prover.AddToDenyListReply;
import net.vac.prover.AddToDenyListRequest;
import net.vac.prover.GetDenyListEntryReply;
import net.vac.prover.GetDenyListEntryRequest;
import net.vac.prover.IsDeniedReply;
import net.vac.prover.IsDeniedRequest;
import net.vac.prover.RemoveFromDenyListReply;
import net.vac.prover.RemoveFromDenyListRequest;
import net.vac.prover.RlnProverGrpc;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Shared deny list manager that uses gRPC to communicate with the RLN prover's database.
 *
 * <p>This manager provides a unified deny list that is shared between the sequencer and RLN prover,
 * backed by the prover's PostgreSQL database.
 *
 * <p><strong>Features:</strong>
 *
 * <ul>
 *   <li>gRPC-based communication with the RLN prover service
 *   <li>Local in-memory cache for read performance
 *   <li>Automatic cache refresh from database
 *   <li>Graceful fallback to cache if gRPC is unavailable
 *   <li>TTL-based entry expiration (handled by the database)
 * </ul>
 *
 * <p><strong>Thread Safety:</strong> All operations are thread-safe using ConcurrentHashMap for the
 * local cache and gRPC's thread-safe stubs.
 *
 * @author Status Network Development Team
 * @since 1.0
 */
public class DenyListManager implements Closeable {
  private static final Logger LOG = LoggerFactory.getLogger(DenyListManager.class);

  private final String serviceName;
  private final String grpcHost;
  private final int grpcPort;
  private final boolean useTls;
  private final long ttlSeconds;

  // gRPC client components
  private ManagedChannel channel;
  private RlnProverGrpc.RlnProverBlockingStub blockingStub;

  // Local in-memory cache for read performance
  private final Map<org.hyperledger.besu.datatypes.Address, CachedDenyEntry> localCache =
      new ConcurrentHashMap<>();

  // Track if gRPC is available
  private final AtomicBoolean grpcAvailable = new AtomicBoolean(false);

  // Scheduler for cache refresh
  private ScheduledExecutorService cacheRefreshScheduler;

  /** Cached deny list entry with timestamp for local TTL checks. */
  private record CachedDenyEntry(long deniedAtSeconds, Long expiresAtSeconds) {
    boolean isExpired() {
      if (expiresAtSeconds == null) {
        return false; // No expiry
      }
      return Instant.now().getEpochSecond() >= expiresAtSeconds;
    }
  }

  /**
   * Creates a new DenyListManager with gRPC backend.
   *
   * @param serviceName Name for logging and identification purposes
   * @param grpcHost Host of the RLN prover gRPC service
   * @param grpcPort Port of the RLN prover gRPC service
   * @param useTls Whether to use TLS for gRPC connection
   * @param ttlSeconds Default TTL for deny list entries in seconds (0 means no expiry)
   * @param cacheRefreshIntervalSeconds How often to refresh local cache (0 to disable)
   */
  public DenyListManager(
      String serviceName,
      String grpcHost,
      int grpcPort,
      boolean useTls,
      long ttlSeconds,
      long cacheRefreshIntervalSeconds) {
    this.serviceName = serviceName;
    this.grpcHost = grpcHost;
    this.grpcPort = grpcPort;
    this.useTls = useTls;
    this.ttlSeconds = ttlSeconds;

    // Initialize gRPC connection
    initializeGrpcClient();

    // Start cache refresh scheduler if enabled
    if (cacheRefreshIntervalSeconds > 0) {
      startCacheRefreshScheduler(cacheRefreshIntervalSeconds);
    }

    LOG.info(
        "{}: DenyListManager initialized with gRPC backend at {}:{}, TTL: {}s, CacheRefresh: {}s",
        serviceName,
        grpcHost,
        grpcPort,
        ttlSeconds,
        cacheRefreshIntervalSeconds);
  }

  /** Initializes the gRPC client connection. */
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

      LOG.info("{}: gRPC client initialized for {}:{}", serviceName, grpcHost, grpcPort);
    } catch (Exception e) {
      LOG.error("{}: Failed to initialize gRPC client: {}", serviceName, e.getMessage(), e);
      this.grpcAvailable.set(false);
    }
  }

  /**
   * Checks if an address is currently on the deny list.
   *
   * <p>First checks local cache, then queries gRPC if needed. Falls back to cache-only if gRPC is
   * unavailable.
   *
   * @param address The address to check
   * @return true if the address is denied and not expired, false otherwise
   */
  public boolean isDenied(org.hyperledger.besu.datatypes.Address address) {
    // First check local cache
    CachedDenyEntry cached = localCache.get(address);
    if (cached != null) {
      if (cached.isExpired()) {
        localCache.remove(address);
        return false;
      }
      return true;
    }

    // Query gRPC if available
    if (grpcAvailable.get() && blockingStub != null) {
      try {
        IsDeniedRequest request =
            IsDeniedRequest.newBuilder().setAddress(address.toHexString().toLowerCase()).build();

        IsDeniedReply reply = blockingStub.isDenied(request);

        // Update local cache if denied
        if (reply.getIsDenied()) {
          // Fetch full entry to get expiry info
          fetchAndCacheEntry(address);
        }

        return reply.getIsDenied();
      } catch (StatusRuntimeException e) {
        LOG.warn(
            "{}: gRPC isDenied call failed for {}: {}. Using cache only.",
            serviceName,
            address.toHexString(),
            e.getStatus());
        grpcAvailable.set(false);
        scheduleGrpcReconnect();
      }
    }

    return false;
  }

  /**
   * Adds an address to the deny list.
   *
   * <p>Immediately persists to the database via gRPC and updates local cache.
   *
   * @param address The address to add to the deny list
   * @return true if the address was newly added, false if it was already present
   */
  public boolean addToDenyList(org.hyperledger.besu.datatypes.Address address) {
    return addToDenyList(address, null);
  }

  /**
   * Adds an address to the deny list with an optional reason.
   *
   * @param address The address to add to the deny list
   * @param reason Optional reason for denial
   * @return true if the address was newly added, false if it was already present
   */
  public boolean addToDenyList(org.hyperledger.besu.datatypes.Address address, String reason) {
    long now = Instant.now().getEpochSecond();
    Long expiresAt = ttlSeconds > 0 ? now + ttlSeconds : null;

    // Update local cache immediately
    localCache.put(address, new CachedDenyEntry(now, expiresAt));

    // Persist via gRPC if available
    if (grpcAvailable.get() && blockingStub != null) {
      try {
        AddToDenyListRequest.Builder requestBuilder =
            AddToDenyListRequest.newBuilder().setAddress(address.toHexString().toLowerCase());

        if (reason != null) {
          requestBuilder.setReason(reason);
        }

        if (ttlSeconds > 0) {
          requestBuilder.setTtlSeconds(ttlSeconds);
        }

        AddToDenyListReply reply = blockingStub.addToDenyList(requestBuilder.build());

        LOG.info(
            "{}: Address {} {} deny list via gRPC (reason: {})",
            serviceName,
            address.toHexString(),
            reply.getWasNew() ? "added to" : "updated in",
            reason != null ? reason : "none");

        return reply.getWasNew();
      } catch (StatusRuntimeException e) {
        LOG.warn(
            "{}: gRPC addToDenyList call failed for {}: {}. Entry cached locally.",
            serviceName,
            address.toHexString(),
            e.getStatus());
        grpcAvailable.set(false);
        scheduleGrpcReconnect();
      }
    } else {
      LOG.warn(
          "{}: gRPC unavailable. Address {} added to local cache only.",
          serviceName,
          address.toHexString());
    }

    return true; // Assume new when we can't verify
  }

  /**
   * Removes an address from the deny list.
   *
   * @param address The address to remove from the deny list
   * @return true if the address was removed, false if it wasn't on the list
   */
  public boolean removeFromDenyList(org.hyperledger.besu.datatypes.Address address) {
    // Remove from local cache immediately
    CachedDenyEntry removed = localCache.remove(address);

    // Persist via gRPC if available
    if (grpcAvailable.get() && blockingStub != null) {
      try {
        RemoveFromDenyListRequest request =
            RemoveFromDenyListRequest.newBuilder()
                .setAddress(address.toHexString().toLowerCase())
                .build();

        RemoveFromDenyListReply reply = blockingStub.removeFromDenyList(request);

        LOG.info(
            "{}: Address {} {} from deny list via gRPC",
            serviceName,
            address.toHexString(),
            reply.getWasPresent() ? "removed" : "was not");

        return reply.getWasPresent();
      } catch (StatusRuntimeException e) {
        LOG.warn(
            "{}: gRPC removeFromDenyList call failed for {}: {}",
            serviceName,
            address.toHexString(),
            e.getStatus());
        grpcAvailable.set(false);
        scheduleGrpcReconnect();
      }
    }

    return removed != null;
  }

  /**
   * Gets the current size of the local deny list cache (for monitoring/debugging).
   *
   * @return Number of addresses currently in the local cache
   */
  public int size() {
    return localCache.size();
  }

  /**
   * Checks if the gRPC connection to the prover is available.
   *
   * @return true if gRPC is available, false otherwise
   */
  public boolean isGrpcAvailable() {
    return grpcAvailable.get();
  }

  /** Fetches a deny list entry from gRPC and caches it locally. */
  private void fetchAndCacheEntry(org.hyperledger.besu.datatypes.Address address) {
    if (!grpcAvailable.get() || blockingStub == null) {
      return;
    }

    try {
      GetDenyListEntryRequest request =
          GetDenyListEntryRequest.newBuilder()
              .setAddress(address.toHexString().toLowerCase())
              .build();

      GetDenyListEntryReply reply = blockingStub.getDenyListEntry(request);

      if (reply.hasEntry()) {
        var entry = reply.getEntry();
        Long expiresAt = entry.hasExpiresAt() ? entry.getExpiresAt() : null;
        localCache.put(address, new CachedDenyEntry(entry.getDeniedAt(), expiresAt));
      }
    } catch (StatusRuntimeException e) {
      LOG.debug(
          "{}: Failed to fetch deny list entry for {}: {}",
          serviceName,
          address.toHexString(),
          e.getStatus());
    }
  }

  /** Starts the scheduled task for local cache refresh. */
  private void startCacheRefreshScheduler(long refreshIntervalSeconds) {
    cacheRefreshScheduler =
        Executors.newSingleThreadScheduledExecutor(
            r -> {
              Thread t = Executors.defaultThreadFactory().newThread(r);
              t.setName(serviceName + "-DenyListCacheRefresh");
              t.setDaemon(true);
              return t;
            });

    cacheRefreshScheduler.scheduleAtFixedRate(
        this::cleanupExpiredEntries,
        refreshIntervalSeconds,
        refreshIntervalSeconds,
        TimeUnit.SECONDS);

    LOG.info(
        "{}: Scheduled deny list cache cleanup every {} seconds",
        serviceName,
        refreshIntervalSeconds);
  }

  /** Cleans up expired entries from the local cache. */
  private void cleanupExpiredEntries() {
    int removedCount = 0;
    for (var entry : localCache.entrySet()) {
      if (entry.getValue().isExpired()) {
        localCache.remove(entry.getKey());
        removedCount++;
      }
    }
    if (removedCount > 0) {
      LOG.debug("{}: Cleaned up {} expired entries from local cache", serviceName, removedCount);
    }
  }

  /** Schedules a gRPC reconnection attempt. */
  private void scheduleGrpcReconnect() {
    if (cacheRefreshScheduler != null && !cacheRefreshScheduler.isShutdown()) {
      cacheRefreshScheduler.schedule(
          () -> {
            LOG.info("{}: Attempting gRPC reconnection...", serviceName);
            initializeGrpcClient();
          },
          30,
          TimeUnit.SECONDS);
    }
  }

  /**
   * Closes all resources including gRPC channel and scheduled executors.
   *
   * @throws IOException if there are issues during resource cleanup
   */
  @Override
  public void close() throws IOException {
    LOG.info("{}: Shutting down DenyListManager...", serviceName);

    if (cacheRefreshScheduler != null && !cacheRefreshScheduler.isShutdown()) {
      cacheRefreshScheduler.shutdown();
      try {
        if (!cacheRefreshScheduler.awaitTermination(5, TimeUnit.SECONDS)) {
          cacheRefreshScheduler.shutdownNow();
        }
      } catch (InterruptedException e) {
        cacheRefreshScheduler.shutdownNow();
        Thread.currentThread().interrupt();
      }
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

    LOG.info("{}: DenyListManager closed", serviceName);
  }
}
