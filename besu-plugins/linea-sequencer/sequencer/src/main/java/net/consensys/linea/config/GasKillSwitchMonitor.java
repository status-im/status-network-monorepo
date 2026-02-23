/*
 * Copyright Consensys Software Inc.
 *
 * This file is dual-licensed under either the MIT license or Apache License 2.0.
 * See the LICENSE-MIT and LICENSE-APACHE files in the repository root for details.
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

package net.consensys.linea.config;

import java.io.Closeable;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * File-based gas kill switch monitor that polls a file at a configurable interval. When the file
 * contains "true" or "enabled" (case-insensitive, trimmed), gasless transactions are disabled.
 *
 * <p>Thread-safe: state is exposed via {@link AtomicBoolean} for lock-free concurrent reads.
 */
public class GasKillSwitchMonitor implements Closeable {
  private static final Logger LOG = LoggerFactory.getLogger(GasKillSwitchMonitor.class);

  private final Path filePath;
  private final AtomicBoolean active = new AtomicBoolean(false);
  private final ScheduledExecutorService scheduler;

  /**
   * Creates and starts a new kill switch monitor.
   *
   * @param filePath path to the kill switch file
   * @param pollIntervalSeconds how often to poll the file
   */
  public GasKillSwitchMonitor(final String filePath, final long pollIntervalSeconds) {
    this.filePath = Path.of(filePath);
    this.scheduler =
        Executors.newSingleThreadScheduledExecutor(
            r -> {
              Thread t = new Thread(r, "GasKillSwitchMonitor");
              t.setDaemon(true);
              return t;
            });

    LOG.info(
        "Gas kill switch monitor started: file={}, pollInterval={}s",
        filePath,
        pollIntervalSeconds);

    // Poll immediately, then at fixed interval
    scheduler.scheduleAtFixedRate(this::poll, 0, pollIntervalSeconds, TimeUnit.SECONDS);
  }

  /** Polls the kill switch file and updates state. */
  private void poll() {
    try {
      if (!Files.exists(filePath)) {
        updateState(false);
        return;
      }

      String content = Files.readString(filePath).trim().toLowerCase();
      boolean shouldBeActive = "true".equals(content) || "enabled".equals(content);
      updateState(shouldBeActive);
    } catch (Exception e) {
      // On file read error, preserve previous state (fail-safe)
      LOG.debug("Error reading kill switch file {}: {}", filePath, e.getMessage());
    }
  }

  private void updateState(boolean newState) {
    boolean previousState = active.getAndSet(newState);
    if (newState && !previousState) {
      LOG.warn("Gas kill switch ACTIVATED - all gasless transactions are now disabled");
    } else if (!newState && previousState) {
      LOG.info("Gas kill switch DEACTIVATED - gasless transactions are now enabled");
    }
  }

  /**
   * Returns whether the kill switch is currently active.
   *
   * @return true if gasless transactions should be disabled
   */
  public boolean isActive() {
    return active.get();
  }

  @Override
  public void close() throws IOException {
    scheduler.shutdownNow();
    LOG.info("Gas kill switch monitor stopped");
  }
}
