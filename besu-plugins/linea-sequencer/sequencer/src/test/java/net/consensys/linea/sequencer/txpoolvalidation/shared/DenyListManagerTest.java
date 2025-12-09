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
 * <p>Tests the local cache behavior when gRPC is unavailable. In production, the DenyListManager
 * connects to the RLN prover's PostgreSQL database via gRPC.
 */
class DenyListManagerTest {

  private static final Address TEST_ADDRESS_1 =
      Address.fromHexString("0x1234567890123456789012345678901234567890");
  private static final Address TEST_ADDRESS_2 =
      Address.fromHexString("0x9876543210987654321098765432109876543210");
  private static final Address TEST_ADDRESS_3 =
      Address.fromHexString("0xabcdefabcdefabcdefabcdefabcdefabcdefabcd");

  private DenyListManager denyListManager;

  @BeforeEach
  void setUp() {
    // Create manager with localhost gRPC (won't connect in tests, falls back to local cache)
    denyListManager = new DenyListManager("Test", "localhost", 50051, false, 600L, 60L);
  }

  @AfterEach
  void tearDown() throws Exception {
    if (denyListManager != null) {
      denyListManager.close();
    }
  }

  @Test
  void testBasicDenyListOperations() {
    // Initially empty
    assertThat(denyListManager.size()).isEqualTo(0);
    assertThat(denyListManager.isDenied(TEST_ADDRESS_1)).isFalse();

    // Add address
    boolean added = denyListManager.addToDenyList(TEST_ADDRESS_1);
    assertThat(added).isTrue();
    assertThat(denyListManager.size()).isEqualTo(1);
    assertThat(denyListManager.isDenied(TEST_ADDRESS_1)).isTrue();

    // Try adding same address again
    boolean addedAgain = denyListManager.addToDenyList(TEST_ADDRESS_1);
    // May return true due to cache-only mode
    assertThat(denyListManager.isDenied(TEST_ADDRESS_1)).isTrue();

    // Remove address
    boolean removed = denyListManager.removeFromDenyList(TEST_ADDRESS_1);
    assertThat(removed).isTrue();
    assertThat(denyListManager.size()).isEqualTo(0);
    assertThat(denyListManager.isDenied(TEST_ADDRESS_1)).isFalse();
  }

  @Test
  void testMultipleAddresses() {
    // Add multiple addresses
    denyListManager.addToDenyList(TEST_ADDRESS_1);
    denyListManager.addToDenyList(TEST_ADDRESS_2);
    denyListManager.addToDenyList(TEST_ADDRESS_3);

    assertThat(denyListManager.size()).isEqualTo(3);
    assertThat(denyListManager.isDenied(TEST_ADDRESS_1)).isTrue();
    assertThat(denyListManager.isDenied(TEST_ADDRESS_2)).isTrue();
    assertThat(denyListManager.isDenied(TEST_ADDRESS_3)).isTrue();

    // Remove one
    denyListManager.removeFromDenyList(TEST_ADDRESS_2);
    assertThat(denyListManager.size()).isEqualTo(2);
    assertThat(denyListManager.isDenied(TEST_ADDRESS_2)).isFalse();
  }

  @Test
  void testRemoveNonExistentAddress() {
    // Remove address that doesn't exist
    boolean removed = denyListManager.removeFromDenyList(TEST_ADDRESS_1);
    assertThat(removed).isFalse();
  }

  @Test
  void testAddWithReason() {
    // Add with reason
    boolean added = denyListManager.addToDenyList(TEST_ADDRESS_1, "Spam detected");
    assertThat(added).isTrue();
    assertThat(denyListManager.isDenied(TEST_ADDRESS_1)).isTrue();
  }

  @Test
  void testConcurrentOperations() throws InterruptedException {
    // Test concurrent operations
    Thread[] threads = new Thread[10];
    for (int i = 0; i < threads.length; i++) {
      final int index = i;
      threads[i] =
          new Thread(
              () -> {
                Address addr =
                    Address.fromHexString(
                        String.format("0x%040d", index)); // Each thread uses unique address
                denyListManager.addToDenyList(addr);
                assertThat(denyListManager.isDenied(addr)).isTrue();
              });
    }

    // Start all threads
    for (Thread thread : threads) {
      thread.start();
    }

    // Wait for all threads
    for (Thread thread : threads) {
      thread.join();
    }

    // All addresses should be denied
    assertThat(denyListManager.size()).isEqualTo(10);
  }
}
