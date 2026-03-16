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

import com.google.protobuf.ByteString;
import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import io.grpc.StatusRuntimeException;
import java.io.Closeable;
import java.io.IOException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import net.vac.prover.AddToDenyListReply;
import net.vac.prover.AddToDenyListRequest;
import net.vac.prover.Address;
import net.vac.prover.IsDeniedReply;
import net.vac.prover.IsDeniedRequest;
import net.vac.prover.RemoveFromDenyListReply;
import net.vac.prover.RemoveFromDenyListRequest;
import net.vac.prover.RlnProverGrpc;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Thin gRPC client for the deny list stored in the RLN prover's PostgreSQL database.
 *
 * <p>Deny list entries are epoch-aligned — they are automatically cleared when a new epoch starts.
 * No local caching or TTL logic is needed; the prover's database is the single source of truth.
 *
 * <p>All queries go directly to the prover via gRPC. When gRPC is unavailable, the manager fails
 * open (isDenied returns false) to avoid blocking transactions.
 */
public class DenyListManager implements Closeable {
  private static final Logger LOG = LoggerFactory.getLogger(DenyListManager.class);

  private final String serviceName;
  private final String grpcHost;
  private final int grpcPort;
  private final boolean useTls;

  // gRPC client components
  private ManagedChannel channel;
  private RlnProverGrpc.RlnProverBlockingStub blockingStub;

  // Track if gRPC is available
  private final AtomicBoolean grpcAvailable = new AtomicBoolean(false);

  /**
   * Creates a new DenyListManager with gRPC backend.
   *
   * @param serviceName Name for logging and identification purposes
   * @param grpcHost Host of the RLN prover gRPC service
   * @param grpcPort Port of the RLN prover gRPC service
   * @param useTls Whether to use TLS for gRPC connection
   */
  public DenyListManager(String serviceName, String grpcHost, int grpcPort, boolean useTls) {
    this.serviceName = serviceName;
    this.grpcHost = grpcHost;
    this.grpcPort = grpcPort;
    this.useTls = useTls;

    // Initialize gRPC connection only if host is provided
    if (grpcHost != null && !grpcHost.isEmpty()) {
      initializeGrpcClient();
    } else {
      this.grpcAvailable.set(false);
    }

    LOG.info(
        "{}: DenyListManager initialized with gRPC backend at {}:{}",
        serviceName,
        grpcHost,
        grpcPort);
  }

  /**
   * Creates a no-op DenyListManager for testing (no gRPC). isDenied always returns false.
   *
   * @param serviceName Name for logging and identification purposes
   * @return A new DenyListManager operating in no-op mode
   */
  public static DenyListManager createCacheOnly(String serviceName) {
    return new DenyListManager(serviceName, null, 0, false);
  }

  /**
   * Converts a Besu Address to a Proto Address.
   *
   * @param address The Besu address to convert
   * @return The Proto Address with the 20-byte value
   */
  private static Address toProtoAddress(org.hyperledger.besu.datatypes.Address address) {
    return Address.newBuilder().setValue(ByteString.copyFrom(address.toArrayUnsafe())).build();
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
   * <p>Queries the prover's database via gRPC. Returns false (fail open) if gRPC is unavailable.
   *
   * @param address The address to check
   * @return true if the address is denied, false otherwise
   */
  public boolean isDenied(org.hyperledger.besu.datatypes.Address address) {
    if (blockingStub != null) {
      try {
        IsDeniedRequest request =
            IsDeniedRequest.newBuilder().setAddress(toProtoAddress(address)).build();

        IsDeniedReply reply = blockingStub.isDenied(request);
        grpcAvailable.set(true); // Mark available on success
        return reply.getIsDenied();
      } catch (StatusRuntimeException e) {
        LOG.warn(
            "{}: gRPC isDenied call failed for {}: {}. Failing open (will retry next call).",
            serviceName,
            address.toHexString(),
            e.getStatus());
        grpcAvailable.set(false);
      }
    }

    // Fail open when gRPC unavailable
    return false;
  }

  /**
   * Adds an address to the deny list.
   *
   * @param address The address to add to the deny list
   * @return true if the address was newly added, false if gRPC unavailable or already present
   */
  public boolean addToDenyList(org.hyperledger.besu.datatypes.Address address) {
    return addToDenyList(address, null);
  }

  /**
   * Adds an address to the deny list with an optional reason.
   *
   * @param address The address to add to the deny list
   * @param reason Optional reason for denial
   * @return true if the address was newly added, false if gRPC unavailable or already present
   */
  public boolean addToDenyList(org.hyperledger.besu.datatypes.Address address, String reason) {
    if (blockingStub != null) {
      try {
        AddToDenyListRequest.Builder requestBuilder =
            AddToDenyListRequest.newBuilder().setAddress(toProtoAddress(address));

        if (reason != null) {
          requestBuilder.setReason(reason);
        }

        AddToDenyListReply reply = blockingStub.addToDenyList(requestBuilder.build());
        grpcAvailable.set(true);

        LOG.info(
            "{}: Address {} {} deny list via gRPC (reason: {})",
            serviceName,
            address.toHexString(),
            reply.getWasNew() ? "added to" : "updated in",
            reason != null ? reason : "none");

        return reply.getWasNew();
      } catch (StatusRuntimeException e) {
        LOG.warn(
            "{}: gRPC addToDenyList call failed for {}: {}",
            serviceName,
            address.toHexString(),
            e.getStatus());
        grpcAvailable.set(false);
      }
    }

    return false;
  }

  /**
   * Removes an address from the deny list.
   *
   * @param address The address to remove from the deny list
   * @return true if the address was removed, false if it wasn't on the list
   */
  public boolean removeFromDenyList(org.hyperledger.besu.datatypes.Address address) {
    if (blockingStub != null) {
      try {
        RemoveFromDenyListRequest request =
            RemoveFromDenyListRequest.newBuilder().setAddress(toProtoAddress(address)).build();

        RemoveFromDenyListReply reply = blockingStub.removeFromDenyList(request);
        grpcAvailable.set(true);

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
      }
    }

    return false;
  }

  /**
   * Removes an address from the deny list AND resets their epoch transaction counter.
   *
   * <p>This is used when a user pays premium gas — they earn a fresh gasless quota for the current
   * epoch.
   *
   * @param address The address to remove and reset quota for
   * @return true if the address was removed, false if it wasn't on the list
   */
  public boolean removeFromDenyListAndResetQuota(org.hyperledger.besu.datatypes.Address address) {
    if (blockingStub != null) {
      try {
        RemoveFromDenyListRequest request =
            RemoveFromDenyListRequest.newBuilder()
                .setAddress(toProtoAddress(address))
                .setResetEpochCounter(true)
                .build();

        RemoveFromDenyListReply reply = blockingStub.removeFromDenyList(request);
        grpcAvailable.set(true);

        LOG.info(
            "{}: Address {} {} from deny list via gRPC (with epoch counter reset)",
            serviceName,
            address.toHexString(),
            reply.getWasPresent() ? "removed" : "was not");

        return reply.getWasPresent();
      } catch (StatusRuntimeException e) {
        LOG.warn(
            "{}: gRPC removeFromDenyListAndResetQuota call failed for {}: {}",
            serviceName,
            address.toHexString(),
            e.getStatus());
        grpcAvailable.set(false);
      }
    }

    return false;
  }

  /**
   * Checks if the gRPC connection to the prover is available.
   *
   * @return true if gRPC is available, false otherwise
   */
  public boolean isGrpcAvailable() {
    return grpcAvailable.get();
  }

  /**
   * Closes all resources including gRPC channel.
   *
   * @throws IOException if there are issues during resource cleanup
   */
  @Override
  public void close() throws IOException {
    LOG.info("{}: Shutting down DenyListManager...", serviceName);

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
