/*
 * Copyright Consensys Software Inc.
 *
 * This file is dual-licensed under either the MIT license or Apache License 2.0.
 * See the LICENSE-MIT and LICENSE-APACHE files in the repository root for details.
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */
package net.consensys.linea.config;

import static org.assertj.core.api.Assertions.assertThat;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Unit tests for {@link GasKillSwitchMonitor}. */
class GasKillSwitchMonitorTest {

  @TempDir Path tempDir;

  private GasKillSwitchMonitor monitor;

  @AfterEach
  void tearDown() throws IOException {
    if (monitor != null) {
      monitor.close();
    }
  }

  @Test
  void initiallyInactiveWhenFileContainsFalse() throws Exception {
    Path file = tempDir.resolve("kill-switch");
    Files.writeString(file, "false");

    monitor = new GasKillSwitchMonitor(file.toString(), 1);
    // Scheduler runs immediately (initialDelay=0), give it time to execute
    Thread.sleep(200);

    assertThat(monitor.isActive()).isFalse();
  }

  @Test
  void activatesWhenFileContainsTrue() throws Exception {
    Path file = tempDir.resolve("kill-switch");
    Files.writeString(file, "true");

    monitor = new GasKillSwitchMonitor(file.toString(), 1);
    Thread.sleep(200);

    assertThat(monitor.isActive()).isTrue();
  }

  @Test
  void activatesWhenFileContainsEnabled() throws Exception {
    Path file = tempDir.resolve("kill-switch");
    Files.writeString(file, "enabled");

    monitor = new GasKillSwitchMonitor(file.toString(), 1);
    Thread.sleep(200);

    assertThat(monitor.isActive()).isTrue();
  }

  @Test
  void activatesCaseInsensitive() throws Exception {
    Path file = tempDir.resolve("kill-switch");
    Files.writeString(file, "TRUE");

    monitor = new GasKillSwitchMonitor(file.toString(), 1);
    Thread.sleep(200);

    assertThat(monitor.isActive()).isTrue();
  }

  @Test
  void handlesWhitespace() throws Exception {
    Path file = tempDir.resolve("kill-switch");
    Files.writeString(file, "  true  \n");

    monitor = new GasKillSwitchMonitor(file.toString(), 1);
    Thread.sleep(200);

    assertThat(monitor.isActive()).isTrue();
  }

  @Test
  void inactiveWhenFileMissing() throws Exception {
    Path file = tempDir.resolve("nonexistent-kill-switch");

    monitor = new GasKillSwitchMonitor(file.toString(), 1);
    Thread.sleep(200);

    assertThat(monitor.isActive()).isFalse();
  }

  @Test
  void inactiveForUnexpectedContent() throws Exception {
    Path file = tempDir.resolve("kill-switch");
    Files.writeString(file, "yes");

    monitor = new GasKillSwitchMonitor(file.toString(), 1);
    Thread.sleep(200);

    assertThat(monitor.isActive()).isFalse();
  }

  @Test
  void deactivatesWhenFileChangesToFalse() throws Exception {
    Path file = tempDir.resolve("kill-switch");
    Files.writeString(file, "true");

    monitor = new GasKillSwitchMonitor(file.toString(), 1);
    Thread.sleep(200);
    assertThat(monitor.isActive()).isTrue();

    Files.writeString(file, "false");
    Thread.sleep(1500); // Wait for next poll cycle
    assertThat(monitor.isActive()).isFalse();
  }

  @Test
  void activatesWhenFileChangesToTrue() throws Exception {
    Path file = tempDir.resolve("kill-switch");
    Files.writeString(file, "false");

    monitor = new GasKillSwitchMonitor(file.toString(), 1);
    Thread.sleep(200);
    assertThat(monitor.isActive()).isFalse();

    Files.writeString(file, "true");
    Thread.sleep(1500); // Wait for next poll cycle
    assertThat(monitor.isActive()).isTrue();
  }

  @Test
  void closeShutsDownScheduler() throws Exception {
    Path file = tempDir.resolve("kill-switch");
    Files.writeString(file, "true");

    monitor = new GasKillSwitchMonitor(file.toString(), 1);
    Thread.sleep(200);
    assertThat(monitor.isActive()).isTrue();

    monitor.close();

    // State persists after close (no reset), but no more polling
    assertThat(monitor.isActive()).isTrue();

    // Change file - state should NOT update after close
    Files.writeString(file, "false");
    Thread.sleep(1500);
    assertThat(monitor.isActive()).isTrue(); // Still true because scheduler is stopped
    monitor = null; // Prevent double close in tearDown
  }

  @Test
  void inactiveForEmptyFile() throws Exception {
    Path file = tempDir.resolve("kill-switch");
    Files.writeString(file, "");

    monitor = new GasKillSwitchMonitor(file.toString(), 1);
    Thread.sleep(200);

    assertThat(monitor.isActive()).isFalse();
  }
}
