use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::fs;
use tracing::{debug, info, warn};

/// File-based gas kill switch for disabling gasless transactions at runtime.
///
/// Polls a file at a configurable interval. When the file contains "true" or "enabled"
/// (case-insensitive, trimmed), the kill switch is active and gasless transactions are disabled.
#[derive(Clone, Debug)]
pub struct GasKillSwitch {
    active: Arc<AtomicBool>,
}

impl GasKillSwitch {
    /// Creates a new kill switch (initially inactive).
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns whether the kill switch is currently active.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Creates a new kill switch that is always inactive (no-op).
    /// Used when kill switch is not configured.
    pub fn noop() -> Self {
        Self::new()
    }

    /// Spawns a tokio task that polls the file at the given interval.
    pub fn start_watcher(&self, file_path: PathBuf, poll_interval: Duration) {
        let active = self.active.clone();
        info!(
            "Gas kill switch watcher started: file={}, poll_interval={}s",
            file_path.display(),
            poll_interval.as_secs()
        );

        tokio::spawn(async move {
            loop {
                let new_state = match fs::read_to_string(&file_path).await {
                    Ok(content) => {
                        let trimmed = content.trim().to_lowercase();
                        trimmed == "true" || trimmed == "enabled"
                    }
                    Err(_) => {
                        debug!(
                            "Kill switch file not readable: {}, preserving current state ({})",
                            file_path.display(),
                            active.load(Ordering::Relaxed)
                        );
                        // Preserve previous state instead of defaulting to false
                        active.load(Ordering::Relaxed)
                    }
                };

                let previous = active.swap(new_state, Ordering::Relaxed);
                if new_state && !previous {
                    warn!("Gas kill switch ACTIVATED - all gasless transactions are now disabled");
                } else if !new_state && previous {
                    info!("Gas kill switch DEACTIVATED - gasless transactions are now enabled");
                }

                tokio::time::sleep(poll_interval).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn new_kill_switch_is_inactive() {
        let ks = GasKillSwitch::new();
        assert!(!ks.is_active());
    }

    #[test]
    fn noop_kill_switch_is_inactive() {
        let ks = GasKillSwitch::noop();
        assert!(!ks.is_active());
    }

    #[test]
    fn clone_shares_state() {
        let ks = GasKillSwitch::new();
        let ks2 = ks.clone();
        assert!(!ks2.is_active());
        ks.active.store(true, Ordering::Relaxed);
        assert!(ks2.is_active());
    }

    #[tokio::test]
    async fn activates_when_file_contains_true() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "true").unwrap();

        let ks = GasKillSwitch::new();
        ks.start_watcher(file.path().to_path_buf(), Duration::from_millis(50));

        // Wait for at least one poll
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(ks.is_active());
    }

    #[tokio::test]
    async fn activates_when_file_contains_enabled() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "enabled").unwrap();

        let ks = GasKillSwitch::new();
        ks.start_watcher(file.path().to_path_buf(), Duration::from_millis(50));

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(ks.is_active());
    }

    #[tokio::test]
    async fn activates_case_insensitive() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "TRUE").unwrap();

        let ks = GasKillSwitch::new();
        ks.start_watcher(file.path().to_path_buf(), Duration::from_millis(50));

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(ks.is_active());
    }

    #[tokio::test]
    async fn inactive_when_file_contains_false() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "false").unwrap();

        let ks = GasKillSwitch::new();
        ks.start_watcher(file.path().to_path_buf(), Duration::from_millis(50));

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!ks.is_active());
    }

    #[tokio::test]
    async fn inactive_when_file_missing() {
        let ks = GasKillSwitch::new();
        ks.start_watcher(
            PathBuf::from("/tmp/nonexistent-kill-switch-test-file"),
            Duration::from_millis(50),
        );

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!ks.is_active());
    }

    #[tokio::test]
    async fn deactivates_when_file_changes_to_false() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();

        // Start with "true"
        std::fs::write(&path, "true\n").unwrap();

        let ks = GasKillSwitch::new();
        ks.start_watcher(path.clone(), Duration::from_millis(50));

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(ks.is_active());

        // Change to "false"
        std::fs::write(&path, "false\n").unwrap();

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!ks.is_active());
    }

    #[tokio::test]
    async fn activates_when_file_changes_to_true() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();

        // Start with "false"
        std::fs::write(&path, "false\n").unwrap();

        let ks = GasKillSwitch::new();
        ks.start_watcher(path.clone(), Duration::from_millis(50));

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!ks.is_active());

        // Change to "true"
        std::fs::write(&path, "true\n").unwrap();

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(ks.is_active());
    }

    #[tokio::test]
    async fn handles_whitespace_in_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "  true  ").unwrap();

        let ks = GasKillSwitch::new();
        ks.start_watcher(file.path().to_path_buf(), Duration::from_millis(50));

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(ks.is_active());
    }

    #[tokio::test]
    async fn inactive_for_unexpected_content() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "yes").unwrap();

        let ks = GasKillSwitch::new();
        ks.start_watcher(file.path().to_path_buf(), Duration::from_millis(50));

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!ks.is_active());
    }

    #[tokio::test]
    async fn preserves_state_on_file_read_error() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();

        // Start with "true"
        std::fs::write(&path, "true\n").unwrap();

        let ks = GasKillSwitch::new();
        ks.start_watcher(path.clone(), Duration::from_millis(50));

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(ks.is_active());

        // Delete the file to simulate read error
        std::fs::remove_file(&path).unwrap();

        tokio::time::sleep(Duration::from_millis(150)).await;
        // Should preserve previous state (true), not default to false
        assert!(ks.is_active());
    }
}
