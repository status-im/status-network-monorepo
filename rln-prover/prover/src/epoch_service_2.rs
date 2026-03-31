use std::ops::Add;
use std::sync::Arc;
use std::time::Duration;
use alloy::consensus::BlockHeader;
use parking_lot::RwLock;
use tokio::sync::Notify;
use tracing::{debug, error};
use chrono::{DateTime, NaiveDate, NaiveDateTime, OutOfRangeError, TimeDelta, Utc};
use metrics::{gauge, histogram};
use crate::error::AppError2;
// Common?
use crate::epoch_service::{Epoch, WaitUntilError};
// Internal
use crate::metrics::{
    EPOCH_SERVICE_CURRENT_EPOCH, EPOCH_SERVICE_CURRENT_EPOCH_SLICE, EPOCH_SERVICE_DRIFT_MILLIS,
};

/// Minimum duration returned by EpochService2::compute_wait_until()
const WAIT_UNTIL_MIN_DURATION: Duration = Duration::from_secs(10);
/// EpochService::compute_wait_until() can return an error like TooLow (see WAIT_UNTIL_MIN_DURATION)
/// so the epoch service will retry X many times.
const WAIT_UNTIL_MAX_RETRY: usize = 10;

pub struct EpochService2 {
    /// Duration of an epoch (quotas reset every epoch)
    epoch_duration: Duration,
    /// Current epoch and epoch slice
    pub current_epoch: Arc<RwLock<Epoch>>,
    /// Genesis time (aka when the service has been started at the first time)
    genesis: DateTime<Utc>,
    /// Channel to notify when an epoch / epoch slice has just changed
    pub epoch_changes: Arc<Notify>,
}

impl EpochService2 {
    // Note: listen_for_new_epoch never ends so no log will happen with #[instrument]
    //       + metrics already tracks the current epoch / epoch_slice
    // #[instrument(skip(self), fields(self.epoch_duration, self.genesis, self.current_epoch))]
    pub(crate) async fn listen_for_new_epoch(&self) -> Result<(), AppError2> {

        let mut retry_counter = 0;
        let (current_epoch, mut wait_until) = loop {
            match EpochService2::compute_wait_until(
                &self.genesis,
                &self.epoch_duration,
                &|| Utc::now(),
                &|| tokio::time::Instant::now()
            ) {
                Ok((current_epoch, wait_until)) => break (current_epoch, wait_until),
                Err(WaitUntilError::TooLow(d1, d2)) => {
                    // Wait until is too low (according to const WAIT_UNTIL_MIN_DURATION)
                    // so we will retry (WAIT_UNTIL_MAX_COMPUTE_ERROR many times) after a short sleep
                    debug!("compute_wait_until return TooLow, will retry after a sleep...");
                    tokio::time::sleep(WAIT_UNTIL_MIN_DURATION).await;
                    retry_counter += 1;
                    if retry_counter > WAIT_UNTIL_MAX_RETRY {
                        error!(
                            "Too many errors while computing the initial wait until time, aborting..."
                        );
                        return Err(AppError2::EpochError(WaitUntilError::TooLow(d1, d2)));
                    }
                },
                Err(e) => {
                    // Another error (like OutOfRange) - exiting...
                    error!("Error computing the initial wait until: {}", e);
                    return Err(AppError2::EpochError(e));
                }
            }
        };
        // Debug
        // let current_epoch = 0;
        // let mut wait_until = tokio::time::Instant::now();

        *self.current_epoch.write() = current_epoch.into();
        debug!(
            "Initial epoch: {}",
            current_epoch
        );

        loop {
            debug!("wait until: {:?}", wait_until);
            // XXX: Should we check the drift between now() and wait_until ?
            tokio::time::sleep_until(wait_until).await;
            {
                let now_ = tokio::time::Instant::now();
                debug!("awake at: {:?}, drift by: {:?}", now_, now_ - wait_until);
                histogram!(EPOCH_SERVICE_DRIFT_MILLIS.name, "prover" => "epoch service")
                    .record(now_ - wait_until);

                // Note: could use checked_add() here, but it's quite impossible to have an overflow here
                //       it would mean that the epoch_slice_duration is insanely large and wait_until
                //       overflows as a timestamp
                wait_until += self.epoch_duration;

                *self.current_epoch.write() = current_epoch.into();

                // Note: based on this link https://doc.rust-lang.org/reference/expressions/operator-expr.html#type-cast-expressions
                //       "Casting from an integer to float will produce the closest possible float *"
                gauge!(EPOCH_SERVICE_CURRENT_EPOCH.name, "prover" => "epoch service")
                    .set(current_epoch as f64);

                self.epoch_changes.notify_one();
            }
        }
    }

    fn compute_wait_until<T: std::fmt::Debug, F: Fn() -> DateTime<Utc>, TF: Fn() -> T>(
        genesis: &DateTime<Utc>,
        epoch_duration: &Duration,
        now: &F,
        now2: &TF,
    ) -> Result<(i64, T), WaitUntilError>
    where
        T: Add<Duration, Output = T>,
    {
        let (current_epoch, current_epoch_start_ts, current_epoch_end_ts) = Self::compute_current_epoch_0(genesis, now, epoch_duration);

        // TODO / FIXME - Troubleshot: weird to use now() & now2() at different time here...
        // time to wait to next epoch
        let wait_until_0_ = current_epoch_end_ts - now().timestamp();
        let wait_until_0 = Duration::from_secs(wait_until_0_ as u64);
        if wait_until_0 < WAIT_UNTIL_MIN_DURATION {
            return Err(WaitUntilError::TooLow(wait_until_0, WAIT_UNTIL_MIN_DURATION));
        }

        let wait_until = now2() + wait_until_0;
        Ok((current_epoch, wait_until))
    }

    /// Compute current epoch since genesis
    /// Return current_epoch AND DateTime of the start & end of this current epoch (as timestamps)
    fn compute_current_epoch_0<F: Fn() -> DateTime<Utc>>(
        genesis: &DateTime<Utc>,
        now: &F,
        epoch_duration: &Duration,
    ) -> (i64, i64, i64) {

        debug_assert!(now() > *genesis);
        debug_assert!(epoch_duration.as_secs() > 0);
        debug_assert!(i64::try_from(epoch_duration.as_secs()).is_ok());

        let genesis_timestamp = genesis.timestamp();
        let now_timestamp = now().timestamp();
        // Get time delta between genesis
        let diff = now_timestamp - genesis_timestamp;
        let current_epoch = diff.checked_div(epoch_duration.as_secs() as i64).unwrap();

        let epoch_start = genesis_timestamp + (current_epoch.checked_mul(epoch_duration.as_secs() as i64).unwrap());
        let epoch_end = genesis_timestamp + ((current_epoch + 1).checked_mul(epoch_duration.as_secs() as i64).unwrap());

        (current_epoch, epoch_start, epoch_end)

    }

    /*
    /// Compute current epoch since genesis
    /// Return current_epoch AND DateTime of the start & end of this current epoch
    fn compute_current_epoch<F: Fn() -> DateTime<Utc>>(
        genesis: DateTime<Utc>,
        now: &F,
        epoch_duration: &Duration,
    ) -> (i64, DateTime<Utc>, DateTime<Utc>) {

        debug_assert!(now() > genesis);
        debug_assert!(epoch_duration.as_secs() > 0);
        debug_assert!(i64::try_from(epoch_duration.as_secs()).is_ok());


        let diff = now() - genesis;
        // Unwrap safe: epoch_duration > 0 + safe to convert to i64
        let current_epoch = diff.checked_div(epoch_duration.as_secs() as i32).unwrap();
        let epoch_start = genesis + (current_epoch.checked_mul(epoch_duration.as_secs() as i32).unwrap());
        let epoch_end = genesis + (current_epoch.checked_mul(epoch_duration.as_secs() as i32 + 1).unwrap());

        (0, epoch_start, epoch_end)
    }
    */
}

#[cfg(test)]
mod tests {
    use alloy::providers::WatchTxError::Timeout;
    use super::*;
    use chrono::{NaiveDate, NaiveDateTime, TimeDelta};

    #[test]
    fn test_compute_current_epoch() {

        let epoch_duration = Duration::from_hours(1);

        let day_ = 14;
        let genesis_0_date = NaiveDate::from_ymd_opt(2025, 5, day_).unwrap();
        let genesis_0: NaiveDateTime = genesis_0_date
            .and_hms_opt(4, 0, 0)
            .unwrap();
        let genesis: DateTime<Utc> =
            chrono::DateTime::from_naive_utc_and_offset(genesis_0, chrono::Utc);

        let now_f = move || {
            let now_0: NaiveDateTime = genesis_0_date
                .and_hms_opt(4+2, 0, 0)
                .unwrap();
            let now: DateTime<Utc> =
                chrono::DateTime::from_naive_utc_and_offset(now_0, chrono::Utc);
            now
        };

        // With epoch duration = 1 hour
        let (current_epoch, current_epoch_start_ts, current_epoch_end_ts) = EpochService2::compute_current_epoch_0(&genesis, &now_f, &Duration::from_hours(1));
        assert_eq!(current_epoch, 2);
        assert_eq!(current_epoch_start_ts, make_utc_datetime(genesis_0_date, 4+2, 0, 0).timestamp());
        assert_eq!(current_epoch_end_ts, make_utc_datetime(genesis_0_date, 4+3, 0, 0).timestamp());

        // Same but with epoch duration = 2 hours
        let (current_epoch, current_epoch_start_ts, current_epoch_end_ts) = EpochService2::compute_current_epoch_0(&genesis, &now_f, &Duration::from_hours(2));
        assert_eq!(current_epoch, 1);
        assert_eq!(current_epoch_start_ts, make_utc_datetime(genesis_0_date, 4+2, 0, 0).timestamp());
        assert_eq!(current_epoch_end_ts, make_utc_datetime(genesis_0_date, 4+4, 0, 0).timestamp());

        // Same but with epoch duration = 15 minutes
        let (current_epoch, current_epoch_start_ts, current_epoch_end_ts) = EpochService2::compute_current_epoch_0(&genesis, &now_f, &Duration::from_mins(15));
        assert_eq!(current_epoch, 8);
        assert_eq!(current_epoch_start_ts, make_utc_datetime(genesis_0_date, 6, 0, 0).timestamp());
        assert_eq!(current_epoch_end_ts, make_utc_datetime(genesis_0_date, 6, 15, 0).timestamp());
    }

    #[test]
    fn test_compute_current_epoch_2() {

        // Test compute_current_epoch when now() is close to genesis time (expect current_epoch == 0)

        let epoch_duration = Duration::from_hours(1);

        let day_ = 14;
        let genesis_0: NaiveDateTime = NaiveDate::from_ymd_opt(2025, 5, day_)
            .unwrap()
            .and_hms_opt(4, 0, 0)
            .unwrap();
        let genesis: DateTime<Utc> =
            chrono::DateTime::from_naive_utc_and_offset(genesis_0, chrono::Utc);

        let now_f = move || { genesis.checked_add_signed(TimeDelta::new(1, 0).unwrap()).unwrap()  };

        let (current_epoch, current_epoch_start_ts, current_epoch_end_ts) = EpochService2::compute_current_epoch_0(
            &genesis, &now_f, &Duration::from_hours(1));

        assert_eq!(current_epoch, 0);
        assert_eq!(current_epoch_start_ts, genesis.timestamp());
        assert_eq!(current_epoch_end_ts, genesis.timestamp() + epoch_duration.as_secs() as i64);
    }

    #[test]
    fn test_wait_until() {
        // Check wait_until is correctly computed

        let date_0 = NaiveDate::from_ymd_opt(2025, 5, 14).unwrap();
        let datetime_0 = date_0.and_hms_opt(0, 0, 0).unwrap();

        let genesis: DateTime<Utc> =
            chrono::DateTime::from_naive_utc_and_offset(datetime_0, Utc);

        {
            // Check wait_until close to genesis
            let epoch_duration_ = 45;
            let epoch_duration = Duration::from_mins(epoch_duration_);
            let now = || {
                let t_delta = TimeDelta::minutes(15);
                genesis.checked_add_signed(t_delta).unwrap()
            };

            let (current_epoch, wait_until) = EpochService2::compute_wait_until(&genesis, &epoch_duration, &now, &now).unwrap();
            assert_eq!(current_epoch, 0);
            assert_eq!(wait_until, make_utc_datetime(date_0, 0, epoch_duration_ as u32, 0));
        }

        {
            // Check wait_until in epoch 1

            let epoch_duration_ = 45;
            let epoch_duration = Duration::from_mins(epoch_duration_);
            let now = || {
                // Now is in epoch 1 + 1s
                let t_delta = TimeDelta::minutes((epoch_duration_ + 1) as i64);
                genesis.checked_add_signed(t_delta).unwrap()
            };

            let (current_epoch, wait_until) = EpochService2::compute_wait_until(&genesis, &epoch_duration, &now, &now).unwrap();
            assert_eq!(current_epoch, 1);
            assert_eq!(wait_until, make_utc_datetime(date_0, 0, (epoch_duration_ * 2) as u32, 0));
        }


        {
            // Check for WaitUntilError::TooLow

            let epoch_duration = Duration::from_mins(15);
            let now = || {

                let epoch_duration_minus_1 =
                    epoch_duration - WAIT_UNTIL_MIN_DURATION + Duration::from_secs(1);

                let as_tdelta = TimeDelta::new(epoch_duration_minus_1.as_secs() as i64, 0).unwrap();
                genesis.checked_add_signed(as_tdelta).unwrap()
            };

            let res = EpochService2::compute_wait_until(&genesis, &epoch_duration, &now, &now);
            assert!(matches!(res, Err(WaitUntilError::TooLow(_, _))));
        }
    }

    fn make_utc_datetime(date: NaiveDate, hours: u32, mins: u32, secs: u32) -> DateTime<Utc> {

        let (mins, time_delta) = if mins > 60 {
            (0, Some(TimeDelta::minutes(mins as i64)))
        } else {
            (mins, None)
        };

        println!("make_utc_datetime: {} - {:?}", mins, time_delta);

        let naive_date = date
            .and_hms_opt(hours, mins, secs)
            .unwrap();

        let naive_date = if let Some(time_delta) = time_delta {
            naive_date.checked_add_signed(time_delta).unwrap()
        } else {
            naive_date
        };

        chrono::DateTime::from_naive_utc_and_offset(naive_date, chrono::Utc)
    }

}