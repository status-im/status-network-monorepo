#[cfg(test)]
mod tests {

    // std
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;
    // third-party
    use chrono::Utc;
    use claims::assert_ge;
    use futures::TryFutureExt;
    use tracing::debug;
    use tracing_test::traced_test;
    // internal
    use crate::epoch_service::{EpochService, WAIT_UNTIL_MIN_DURATION};
    use crate::error::AppError;

    #[derive(thiserror::Error, Debug)]
    enum AppErrorExt {
        #[error("AppError: {0}")]
        AppError(#[from] AppError),
        #[error("Future timeout")]
        Elapsed,
    }

    #[tokio::test]
    #[traced_test]
    async fn test_notify() {
        // Test epoch_service is really notifying when an epoch or epoch slice has just changed

        let epoch_slice_duration = Duration::from_secs(10);
        let epoch_service = EpochService::try_from((epoch_slice_duration, Utc::now())).unwrap();
        let notifier = epoch_service.epoch_changes.clone();
        let counter_0 = Arc::new(AtomicU64::new(0));
        let counter = counter_0.clone();

        let start = std::time::Instant::now();
        let res = tokio::try_join!(
            epoch_service
                .listen_for_new_epoch()
                .map_err(AppErrorExt::AppError),
            // Wait for 3 epoch slices
            // + WAIT_UNTIL_MIN_DURATION * 2 (expect a maximum of 2 retry)
            // + 500 ms (to wait to receive notif + counter incr)
            // Note: this might fail if there is more retry (see list_for_new_epoch code)
            tokio::time::timeout(
                epoch_slice_duration * 3 + WAIT_UNTIL_MIN_DURATION * 2 + Duration::from_millis(500),
                async move {
                    loop {
                        notifier.notified().await;
                        // debug!("[Notified] Epoch update...");
                        let _v = counter.fetch_add(1, Ordering::SeqCst);
                    }
                    // Ok::<(), AppErrorExt>(())
                }
            )
            .map_err(|_e| AppErrorExt::Elapsed)
        );

        debug!("Elapsed time: {}", start.elapsed().as_millis());
        // debug!("res: {:?}", res);
        assert!(matches!(res, Err(AppErrorExt::Elapsed)));
        // Because the timeout is quite large - it is expected that sometimes the counter == 4
        assert_ge!(counter_0.load(Ordering::SeqCst), 3);
    }
}
