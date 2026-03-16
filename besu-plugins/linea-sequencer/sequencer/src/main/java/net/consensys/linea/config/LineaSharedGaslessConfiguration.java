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
 * <p>The deny list is stored in the RLN Prover's PostgreSQL database and accessed via gRPC. Deny
 * list entries are epoch-aligned — they are automatically cleared when a new epoch starts. No TTL
 * or local caching is needed.
 *
 * @param premiumGasPriceThresholdGWei Minimum gas price (in GWei) for a transaction to be
 *     considered premium. Users on the deny list can bypass restrictions by paying this amount.
 * @param nullifierStoragePath Path to the file for storing nullifier tracking data.
 */
public record LineaSharedGaslessConfiguration(
    long premiumGasPriceThresholdGWei, String nullifierStoragePath)
    implements LineaOptionsConfiguration {

  public static final long DEFAULT_PREMIUM_GAS_PRICE_THRESHOLD_GWEI = 12L; // 12 Gwei
  public static final String DEFAULT_NULLIFIER_STORAGE_PATH = "/var/lib/besu/nullifiers.txt";

  public static LineaSharedGaslessConfiguration V1_DEFAULT =
      new LineaSharedGaslessConfiguration(
          DEFAULT_PREMIUM_GAS_PRICE_THRESHOLD_GWEI, DEFAULT_NULLIFIER_STORAGE_PATH);

  public LineaSharedGaslessConfiguration {
    if (premiumGasPriceThresholdGWei <= 0) {
      throw new IllegalArgumentException("Premium gas price threshold GWei must be positive.");
    }
    if (nullifierStoragePath == null || nullifierStoragePath.isBlank()) {
      throw new IllegalArgumentException("Nullifier storage path cannot be null or blank.");
    }
  }
}
