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
package net.consensys.linea.sequencer.txpoolvalidation;

import static org.assertj.core.api.Assertions.assertThat;

import java.io.IOException;
import java.math.BigInteger;
import java.nio.file.Path;
import java.time.Duration;
import java.time.Instant;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;
import net.consensys.linea.sequencer.txpoolvalidation.shared.DenyListManager;
import net.consensys.linea.sequencer.txpoolvalidation.shared.NullifierTracker;
import net.consensys.linea.sequencer.txpoolvalidation.shared.NullifierTracker.NullifierStats;
import org.bouncycastle.asn1.sec.SECNamedCurves;
import org.bouncycastle.asn1.x9.X9ECParameters;
import org.bouncycastle.crypto.params.ECDomainParameters;
import org.hyperledger.besu.crypto.SECPSignature;
import org.hyperledger.besu.datatypes.Address;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/**
 * Performance and stress tests for RLN validation components.
 *
 * <p>Tests high-throughput scenarios and system behavior under load.
 */
class RlnValidationPerformanceTest {

  @TempDir Path tempDir;

  private static final SECPSignature FAKE_SIGNATURE;

  static {
    final X9ECParameters params = SECNamedCurves.getByName("secp256k1");
    final ECDomainParameters curve =
        new ECDomainParameters(params.getCurve(), params.getG(), params.getN(), params.getH());
    FAKE_SIGNATURE =
        SECPSignature.create(
            new BigInteger(
                "66397251408932042429874251838229702988618145381408295790259650671563847073199"),
            new BigInteger(
                "24729624138373455972486746091821238755870276413282629437244319694880507882088"),
            (byte) 0,
            curve.getN());
  }

  private DenyListManager denyListManager;
  private NullifierTracker nullifierTracker;

  @BeforeEach
  void setUp() throws IOException {
    // Use cache-only managers for testing (no gRPC)
    denyListManager = DenyListManager.createCacheOnly("PerformanceTest");
    nullifierTracker = new NullifierTracker("PerformanceTest", 100_000L, 1L);
  }

  @AfterEach
  void tearDown() throws Exception {
    if (denyListManager != null) {
      denyListManager.close();
    }
    if (nullifierTracker != null) {
      nullifierTracker.close();
    }
  }

  @Test
  void testHighThroughputNullifierTracking() throws InterruptedException {
    int threadCount = 10;
    int operationsPerThread = 1000;
    int totalOperations = threadCount * operationsPerThread;

    ExecutorService executor = Executors.newFixedThreadPool(threadCount);
    CountDownLatch latch = new CountDownLatch(threadCount);
    AtomicInteger successCount = new AtomicInteger(0);
    AtomicLong totalDuration = new AtomicLong(0);

    // Measure performance of nullifier tracking
    Instant startTime = Instant.now();

    for (int t = 0; t < threadCount; t++) {
      final int threadId = t;
      executor.submit(
          () -> {
            try {
              Instant threadStart = Instant.now();

              for (int i = 0; i < operationsPerThread; i++) {
                String nullifier = String.format("0x%064d", threadId * operationsPerThread + i);
                String epoch = "epoch-" + (i % 10); // Use different epochs to test scoping

                boolean isNew = nullifierTracker.checkAndMarkNullifier(nullifier, epoch);
                if (isNew) {
                  successCount.incrementAndGet();
                }
              }

              Instant threadEnd = Instant.now();
              totalDuration.addAndGet(Duration.between(threadStart, threadEnd).toMillis());

            } finally {
              latch.countDown();
            }
          });
    }

    boolean completed = latch.await(30, TimeUnit.SECONDS);
    assertThat(completed).isTrue();

    executor.shutdown();
    executor.awaitTermination(5, TimeUnit.SECONDS);

    Instant endTime = Instant.now();
    long totalWallClockTime = Duration.between(startTime, endTime).toMillis();

    // Verify performance metrics
    assertThat(successCount.get()).isEqualTo(totalOperations);

    NullifierStats stats = nullifierTracker.getStats();
    assertThat(stats.totalChecks()).isEqualTo(totalOperations);
    assertThat(stats.duplicatesDetected()).isEqualTo(0);

    // Log performance results
    double throughput = (double) totalOperations / (totalWallClockTime / 1000.0);
    System.out.printf(
        "Nullifier tracking performance: %d operations in %d ms (%.2f ops/sec)%n",
        totalOperations, totalWallClockTime, throughput);

    // Performance assertion - should handle at least 1000 ops/sec
    assertThat(throughput).isGreaterThan(1000.0);
  }

  @Test
  void testDenyListNoOpBehavior() {
    // Without gRPC, DenyListManager is no-op (fail open)
    Address testAddr = Address.fromHexString("0x1234567890123456789012345678901234567890");

    // addToDenyList returns false (no gRPC)
    boolean added = denyListManager.addToDenyList(testAddr);
    assertThat(added).isFalse();

    // isDenied returns false (fail open)
    boolean isDenied = denyListManager.isDenied(testAddr);
    assertThat(isDenied).isFalse();

    // removeFromDenyList returns false
    boolean removed = denyListManager.removeFromDenyList(testAddr);
    assertThat(removed).isFalse();
  }

  @Test
  void testMemoryUsageUnderLoad() throws InterruptedException {
    // Test memory usage with large number of nullifier entries
    int nullifierCount = 10_000;

    // Add many nullifiers
    for (int i = 0; i < nullifierCount; i++) {
      String nullifier = String.format("0x%064d", i);
      String epoch = "epoch-" + (i % 100);
      nullifierTracker.checkAndMarkNullifier(nullifier, epoch);
    }

    // Verify counts
    NullifierStats stats = nullifierTracker.getStats();
    assertThat(stats.totalChecks()).isEqualTo(nullifierCount);
    assertThat(stats.cacheSize()).isEqualTo(nullifierCount);

    // Test continued operations under load
    String testNullifier = "0x9999999999999999999999999999999999999999999999999999999999999999";
    boolean canStillOperate = nullifierTracker.checkAndMarkNullifier(testNullifier, "test-epoch");
    assertThat(canStillOperate).isTrue();
  }

  @Test
  void testCacheEvictionBehavior() throws InterruptedException, IOException {
    // Create tracker with small size for testing
    nullifierTracker.close();
    nullifierTracker = new NullifierTracker("EvictionTest", 100L, 1L); // 100 max size, 1 hour TTL

    // Add entries to test cache behavior
    for (int i = 0; i < 50; i++) {
      String nullifier = String.format("0x%064d", i);
      nullifierTracker.checkAndMarkNullifier(nullifier, "test-epoch");
    }

    NullifierStats stats = nullifierTracker.getStats();
    // Verify tracker is working and recording entries
    assertThat(stats.cacheSize()).isGreaterThan(0);
    assertThat(stats.totalChecks()).isEqualTo(50);

    // Adding a new unique nullifier should succeed
    String newNullifier = "0x" + "f".repeat(64); // Unique nullifier
    boolean canAdd = nullifierTracker.checkAndMarkNullifier(newNullifier, "test-epoch-new");
    assertThat(canAdd).isTrue();

    // Verify cache size increased
    NullifierStats statsAfter = nullifierTracker.getStats();
    assertThat(statsAfter.totalChecks()).isEqualTo(51);
  }

  @Test
  void testDenyListGrpcCallPerformance() throws InterruptedException {
    // Without gRPC server, all calls return immediately (no-op)
    int operationCount = 100;
    Instant start = Instant.now();

    for (int i = 0; i < operationCount; i++) {
      Address addr = Address.fromHexString(String.format("0x%040d", i));
      denyListManager.addToDenyList(addr);
      denyListManager.isDenied(addr);
    }

    Instant end = Instant.now();
    long totalTime = Duration.between(start, end).toMillis();

    // No-op operations should be very fast
    System.out.printf("No-op deny list operations: %d in %d ms%n", operationCount * 2, totalTime);
    assertThat(totalTime).isLessThan(5000);
  }

  @Test
  void testConcurrentNullifierConflicts() throws InterruptedException {
    // Test behavior when many threads try to use the same nullifiers
    int threadCount = 20;
    String conflictedNullifier = "0xc0ffeec0ffeec0ffeec0ffeec0ffeec0ffeec0ffeec0ffeec0ffeec0ffee00";

    ExecutorService executor = Executors.newFixedThreadPool(threadCount);
    CountDownLatch latch = new CountDownLatch(threadCount);
    AtomicInteger successCount = new AtomicInteger(0);
    AtomicInteger conflictCount = new AtomicInteger(0);

    for (int t = 0; t < threadCount; t++) {
      executor.submit(
          () -> {
            try {
              // All threads try to use the same nullifier
              boolean isNew =
                  nullifierTracker.checkAndMarkNullifier(conflictedNullifier, "conflict-epoch");

              if (isNew) {
                successCount.incrementAndGet();
              } else {
                conflictCount.incrementAndGet();
              }

            } finally {
              latch.countDown();
            }
          });
    }

    boolean completed = latch.await(10, TimeUnit.SECONDS);
    assertThat(completed).isTrue();

    executor.shutdown();
    executor.awaitTermination(5, TimeUnit.SECONDS);

    // Only one thread should succeed, others should detect conflict
    assertThat(successCount.get()).isEqualTo(1);
    assertThat(conflictCount.get()).isEqualTo(threadCount - 1);

    NullifierStats stats = nullifierTracker.getStats();
    assertThat(stats.duplicatesDetected()).isEqualTo(threadCount - 1);
  }

  @Test
  void testSystemResourceUsageUnderLoad() throws InterruptedException {
    // Test system behavior under sustained load
    int duration = 5; // seconds
    AtomicInteger operationCount = new AtomicInteger(0);
    final boolean[] keepRunning = {true};

    ExecutorService executor = Executors.newFixedThreadPool(4);

    // Nullifier operations
    executor.submit(
        () -> {
          int counter = 0;
          while (keepRunning[0]) {
            String nullifier = String.format("0x%064d", counter++);
            String epoch = "load-epoch-" + (counter % 5);
            nullifierTracker.checkAndMarkNullifier(nullifier, epoch);
            operationCount.incrementAndGet();

            if (counter % 100 == 0) {
              try {
                Thread.sleep(1); // Small pause to prevent CPU overload
              } catch (InterruptedException e) {
                break;
              }
            }
          }
        });

    // Deny list operations
    executor.submit(
        () -> {
          int counter = 0;
          while (keepRunning[0]) {
            Address addr = Address.fromHexString(String.format("0x%040d", counter % 1000));
            if (counter % 2 == 0) {
              denyListManager.addToDenyList(addr);
            } else {
              denyListManager.isDenied(addr);
            }
            operationCount.incrementAndGet();
            counter++;

            if (counter % 50 == 0) {
              try {
                Thread.sleep(1);
              } catch (InterruptedException e) {
                break;
              }
            }
          }
        });

    // Run for specified duration
    Thread.sleep(duration * 1000);
    keepRunning[0] = false;

    executor.shutdown();
    executor.awaitTermination(10, TimeUnit.SECONDS);

    // Verify system performed operations without issues
    assertThat(operationCount.get()).isGreaterThan(1000); // Should have done substantial work

    NullifierStats stats = nullifierTracker.getStats();
    assertThat(stats.cacheSize()).isGreaterThan(0);

    System.out.printf(
        "Sustained load test: %d operations in %d seconds (%.2f ops/sec)%n",
        operationCount.get(), duration, (double) operationCount.get() / duration);
  }

  @Test
  void testDenyListNoOpUnderLoad() {
    // Without gRPC, all deny list operations are no-ops
    int addressCount = 1000;

    Instant start = Instant.now();
    for (int i = 0; i < addressCount; i++) {
      Address addr = Address.fromHexString(String.format("0x%040d", i));
      denyListManager.addToDenyList(addr);
      denyListManager.isDenied(addr);
      denyListManager.removeFromDenyList(addr);
    }
    Instant end = Instant.now();

    long totalTime = Duration.between(start, end).toMillis();
    System.out.printf(
        "No-op deny list bulk operations: %d in %d ms%n", addressCount * 3, totalTime);
    assertThat(totalTime).isLessThan(5000);
  }

  @Test
  void testNullifierConflictUnderHighLoad() throws InterruptedException {
    // Test nullifier conflict detection under high concurrent load
    String conflictNullifier = "0xc0ffeec0ffeec0ffeec0ffeec0ffeec0ffeec0ffeec0ffeec0ffeec0ffee00";
    String conflictEpoch = "high-load-epoch";

    int threadCount = 50;
    ExecutorService executor = Executors.newFixedThreadPool(threadCount);
    CountDownLatch latch = new CountDownLatch(threadCount);
    AtomicInteger winners = new AtomicInteger(0);
    AtomicInteger conflicts = new AtomicInteger(0);

    // All threads compete for the same nullifier
    for (int t = 0; t < threadCount; t++) {
      executor.submit(
          () -> {
            try {
              boolean won =
                  nullifierTracker.checkAndMarkNullifier(conflictNullifier, conflictEpoch);
              if (won) {
                winners.incrementAndGet();
              } else {
                conflicts.incrementAndGet();
              }
            } finally {
              latch.countDown();
            }
          });
    }

    boolean completed = latch.await(10, TimeUnit.SECONDS);
    assertThat(completed).isTrue();

    executor.shutdown();
    executor.awaitTermination(5, TimeUnit.SECONDS);

    // Critical security property: exactly one winner
    assertThat(winners.get()).isEqualTo(1);
    assertThat(conflicts.get()).isEqualTo(threadCount - 1);

    System.out.printf(
        "High load conflict test: 1 winner, %d conflicts from %d threads%n",
        conflicts.get(), threadCount);
  }
}
