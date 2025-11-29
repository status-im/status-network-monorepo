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
package net.consensys.linea.config;

import net.consensys.linea.plugins.LineaOptionsConfiguration;

/**
 * Shared configuration parameters for gasless transaction features (RLN, RPC modifications).
 *
 * <p>The deny list is stored in the RLN Prover's PostgreSQL database and accessed via gRPC. The
 * sequencer connects to the prover service using the RLN proof service host/port configuration.
 *
 * @param denyListCacheRefreshSeconds Interval in seconds for local cache cleanup of expired
 *     entries.
 * @param premiumGasPriceThresholdGWei Minimum gas price (in GWei) for a transaction to be
 *     considered premium. Users on the deny list can bypass restrictions by paying this amount.
 * @param denyListEntryMaxAgeMinutes Maximum age in minutes for an entry on the deny list before it
 *     expires. This TTL is enforced by the prover's database.
 * @param nullifierStoragePath Path to the file for storing nullifier tracking data.
 */
public record LineaSharedGaslessConfiguration(
    long denyListCacheRefreshSeconds,
    long premiumGasPriceThresholdGWei,
    long denyListEntryMaxAgeMinutes,
    String nullifierStoragePath)
    implements LineaOptionsConfiguration {

  public static final long DEFAULT_DENY_LIST_CACHE_REFRESH_SECONDS = 60L; // 1 minute
  public static final long DEFAULT_PREMIUM_GAS_PRICE_THRESHOLD_GWEI = 100L; // 100 Gwei
  public static final long DEFAULT_DENY_LIST_ENTRY_MAX_AGE_MINUTES = 10L; // 10 minutes
  public static final String DEFAULT_NULLIFIER_STORAGE_PATH = "/var/lib/besu/nullifiers.txt";

  public static LineaSharedGaslessConfiguration V1_DEFAULT =
      new LineaSharedGaslessConfiguration(
          DEFAULT_DENY_LIST_CACHE_REFRESH_SECONDS,
          DEFAULT_PREMIUM_GAS_PRICE_THRESHOLD_GWEI,
          DEFAULT_DENY_LIST_ENTRY_MAX_AGE_MINUTES,
          DEFAULT_NULLIFIER_STORAGE_PATH);

  public LineaSharedGaslessConfiguration {
    if (denyListCacheRefreshSeconds <= 0) {
      throw new IllegalArgumentException("Deny list cache refresh seconds must be positive.");
    }
    if (premiumGasPriceThresholdGWei <= 0) {
      throw new IllegalArgumentException("Premium gas price threshold GWei must be positive.");
    }
    if (denyListEntryMaxAgeMinutes <= 0) {
      throw new IllegalArgumentException("Deny list entry max age minutes must be positive.");
    }
    if (nullifierStoragePath == null || nullifierStoragePath.isBlank()) {
      throw new IllegalArgumentException("Nullifier storage path cannot be null or blank.");
    }
  }

  // Backward compatibility getter for code still using the old name
  public long denyListRefreshSeconds() {
    return denyListCacheRefreshSeconds;
  }
}
