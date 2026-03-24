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

import static org.assertj.core.api.Assertions.assertThat;

import org.hyperledger.besu.datatypes.Address;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

/**
 * Tests for DenyListManager functionality.
 *
 * <p>Tests the no-op behavior when gRPC is unavailable. In production, the DenyListManager connects
 * to the RLN prover's PostgreSQL database via gRPC.
 */
class DenyListManagerTest {

  private static final Address TEST_ADDRESS_1 =
      Address.fromHexString("0x1234567890123456789012345678901234567890");
  private static final Address TEST_ADDRESS_2 =
      Address.fromHexString("0x9876543210987654321098765432109876543210");

  private DenyListManager denyListManager;

  @BeforeEach
  void setUp() {
    // Create cache-only (no-op) manager for testing (no gRPC)
    denyListManager = DenyListManager.createCacheOnly("Test");
  }

  @AfterEach
  void tearDown() throws Exception {
    if (denyListManager != null) {
      denyListManager.close();
    }
  }

  @Test
  void testCacheOnlyIsDeniedAlwaysFalse() {
    // Without gRPC, isDenied always returns false (fail open)
    assertThat(denyListManager.isDenied(TEST_ADDRESS_1)).isFalse();
  }

  @Test
  void testCacheOnlyAddReturnsFalse() {
    // Without gRPC, addToDenyList returns false
    boolean added = denyListManager.addToDenyList(TEST_ADDRESS_1);
    assertThat(added).isFalse();
    // Still not denied (no local cache)
    assertThat(denyListManager.isDenied(TEST_ADDRESS_1)).isFalse();
  }

  @Test
  void testCacheOnlyRemoveReturnsFalse() {
    // Without gRPC, removeFromDenyList returns false
    boolean removed = denyListManager.removeFromDenyList(TEST_ADDRESS_1);
    assertThat(removed).isFalse();
  }

  @Test
  void testAddWithReason() {
    // Add with reason — returns false without gRPC
    boolean added = denyListManager.addToDenyList(TEST_ADDRESS_1, "Spam detected");
    assertThat(added).isFalse();
  }

  @Test
  void testGrpcNotAvailable() {
    // Cache-only mode should report gRPC as unavailable
    assertThat(denyListManager.isGrpcAvailable()).isFalse();
  }
}
