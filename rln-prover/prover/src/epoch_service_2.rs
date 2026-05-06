use std::{ops::Add, sync::Arc, time::Duration};
// third-party
use chrono::{DateTime, NaiveDate, NaiveDateTime, OutOfRangeError, TimeDelta, Utc};
use metrics::{gauge, histogram};
use parking_lot::RwLock;
use tokio::sync::Notify;
use tracing::{debug, error};
// Common?
use crate::epoch_service::{DEFAULT_EPOCH_DURATION, Epoch, EpochServiceInitError, WaitUntilError};
// Internal
use crate::error::AppError2;
use crate::metrics::{
    EPOCH_SERVICE_CURRENT_EPOCH, EPOCH_SERVICE_CURRENT_EPOCH_SLICE, EPOCH_SERVICE_DRIFT_MILLIS,
};

/// EpochService2::compute_wait_until() can return a low duration (see Notes in listen_for_new_epoch)
/// So we need a minimum threshold to accept or not this duration
const WAIT_UNTIL_MIN_DURATION: Duration = Duration::from_secs(10);
/// EpochService::compute_wait_until() can return an error like TooLow (see WAIT_UNTIL_MIN_DURATION)
/// so the epoch service will retry X many times.
const WAIT_UNTIL_MAX_RETRY: usize = 5;

pub struct EpochService2 {
    /// Duration of an epoch (quotas reset every epoch)
    epoch_duration: Duration,
    /// Current epoch
    pub(crate) current_epoch: Arc<RwLock<Epoch>>,
    /// Genesis time (aka when the service has been started at the first time)
    genesis: DateTime<Utc>,
    /// Channel to notify when an epoch has just changed
    pub(crate) epoch_changes: Arc<Notify>, // assume single subscriber (see tokio Notify doc)
}

impl EpochService2 {
    // Note: listen_for_new_epoch never ends so no log will happen with #[instrument]
    //       + metrics already tracks the current epoch
    // #[instrument(skip(self), fields(self.epoch_duration, self.genesis, self.current_epoch))]
    pub(crate) async fn listen_for_new_epoch(&self) -> Result<(), AppError2> {
        let mut retry_counter = 0;
        // Note: compute_wait_until return the duration to wait (from now until the next epoch)
        //       this can be very low (if we start the program near the end of an epoch)
        //       so we retry a few times if necessary
        let (mut current_epoch, mut wait_until) = loop {
            match EpochService2::compute_wait_until(
                &self.genesis,
                &self.epoch_duration,
                &|| Utc::now(),
                &|| tokio::time::Instant::now(),
            ) {
                Ok((current_epoch, wait_until)) => break (current_epoch, wait_until),
                Err(WaitUntilError::TooLow(d1, d2)) => {
                    // Wait until is too low (according to const WAIT_UNTIL_MIN_DURATION)
                    // so we will retry (WAIT_UNTIL_MAX_RETRY many times) after a short sleep
                    debug!("compute_wait_until return TooLow, will retry after a sleep...");
                    tokio::time::sleep(WAIT_UNTIL_MIN_DURATION).await;
                    retry_counter += 1;
                    if retry_counter > WAIT_UNTIL_MAX_RETRY {
                        error!(
                            "Too many errors while computing the initial wait until time, aborting..."
                        );
                        return Err(AppError2::EpochError(WaitUntilError::TooLow(d1, d2)));
                    }
                }
                Err(e) => {
                    // Another error (like OutOfRange) - exiting...
                    error!("Error computing the initial wait until: {}", e);
                    return Err(AppError2::EpochError(e));
                }
            }
        };

        *self.current_epoch.write() = current_epoch.into();
        debug!("Initial epoch: {}", current_epoch);

        loop {
            debug!("wait until: {:?}", wait_until);
            // XXX: Should we check the drift between now() and wait_until ?
            tokio::time::sleep_until(wait_until).await;
            {
                let now_ = tokio::time::Instant::now();
                debug!("awake at: {:?}, drift by: {:?}", now_, now_ - wait_until);
                histogram!(EPOCH_SERVICE_DRIFT_MILLIS.name, "prover" => "epoch service")
                    .record(now_ - wait_until);

                // Note: checked_add() is used here but this is very unlikely an overflow can occur here
                wait_until = if let Some(wait_until) = wait_until.checked_add(self.epoch_duration) {
                    wait_until
                } else {
                    error!(
                        "wait_until overflows, previous value: {:?}, epoch_duration: {:?}",
                        wait_until, self.epoch_duration
                    );
                    return Err(AppError2::EpochServiceOverflow(
                        wait_until,
                        self.epoch_duration,
                    ));
                };

                current_epoch += 1;
                *self.current_epoch.write() = current_epoch.into();

                // Note: based on this link https://doc.rust-lang.org/reference/expressions/operator-expr.html#type-cast-expressions
                //       "Casting from an integer to float will produce the closest possible float *"
                gauge!(EPOCH_SERVICE_CURRENT_EPOCH.name, "prover" => "epoch service")
                    .set(current_epoch as f64);

                //
                self.epoch_changes.notify_one();
            }
        }
    }

    fn compute_wait_until<T, F: Fn() -> DateTime<Utc>, TF: Fn() -> T>(
        genesis: &DateTime<Utc>,
        epoch_duration: &Duration,
        wall_clock_now: &F,
        monotonic_now: &TF,
    ) -> Result<(i64, T), WaitUntilError>
    where
        T: std::fmt::Debug + Add<Duration, Output = T>,
    {
        // Note: we need to now() & now2() function as in production (see listen_for_new_epoch)
        //       now() -> DateTime<Utc> && now2() returns tokio::time::Instant
        //       but in unit test, we use now() == now2() as it is easier to manipulate DateTime<Utc>

        let (current_epoch, _current_epoch_start_ts, current_epoch_end_ts) =
            Self::compute_current_epoch_0(genesis, wall_clock_now, epoch_duration);

        // time to wait to next epoch
        let now_ = wall_clock_now();
        let now_2_ = monotonic_now();
        // let wait_until_0_ = current_epoch_end_ts - now_.timestamp();
        let wait_until_0_ms = current_epoch_end_ts.saturating_mul(1000) - now_.timestamp_millis();
        // Note: wait_until_0_ms can be < 0: system clock skew, NTP correction, suspend/resume
        if wait_until_0_ms < 0 {
            return Err(WaitUntilError::TooLow(
                Duration::ZERO,
                WAIT_UNTIL_MIN_DURATION,
            ));
        }
        let wait_until_0 = Duration::from_millis(wait_until_0_ms as u64);
        if wait_until_0 < WAIT_UNTIL_MIN_DURATION {
            return Err(WaitUntilError::TooLow(
                wait_until_0,
                WAIT_UNTIL_MIN_DURATION,
            ));
        }

        let wait_until = now_2_ + wait_until_0;
        Ok((current_epoch, wait_until))
    }

    /// Compute current epoch since genesis
    /// Return current_epoch AND DateTime of the start & end of this current epoch (as timestamps)
    fn compute_current_epoch_0<F: Fn() -> DateTime<Utc>>(
        genesis: &DateTime<Utc>,
        now: &F,
        epoch_duration: &Duration,
    ) -> (i64, i64, i64) {
        debug_assert!(now() >= *genesis);
        debug_assert!(i64::try_from(epoch_duration.as_secs()).is_ok());

        let epoch_dur = epoch_duration.as_secs() as i64;

        let genesis_timestamp = genesis.timestamp();
        let now_timestamp = now().timestamp();
        // Get time delta between genesis
        let diff = now_timestamp - genesis_timestamp;
        // Unwrap safe: epoch_duration > 0 (Duration type ~= u64)
        let current_epoch = diff.checked_div(epoch_dur).unwrap();

        let epoch_start_ts = genesis_timestamp + (current_epoch.checked_mul(epoch_dur).unwrap()); // unwrap safe: should not overflow (as epoch_duration is small)
        let epoch_end_ts =
            genesis_timestamp + ((current_epoch + 1).checked_mul(epoch_dur).unwrap()); // unwrap safe: should not overflow (as epoch_duration is small)

        (current_epoch, epoch_start_ts, epoch_end_ts)
    }
}

/// Configuration for EpochService2
/// (epoch_duration, genesis)
pub struct EpochService2Config {
    /// Duration of an epoch (quotas reset every epoch). Default: 24 hours.
    pub epoch_duration: Duration,
    /// Genesis timestamp
    pub genesis: DateTime<Utc>,
}

impl EpochService2Config {
    /// Create config with custom epoch duration
    pub fn new(epoch_duration: Duration, genesis: DateTime<Utc>) -> Self {
        Self {
            epoch_duration,
            genesis,
        }
    }
}

impl TryFrom<EpochService2Config> for EpochService2 {
    type Error = EpochServiceInitError;

    fn try_from(config: EpochService2Config) -> Result<Self, Self::Error> {
        if config.genesis >= Utc::now() {
            return Err(EpochServiceInitError::FutureGenesis);
        }

        if config.epoch_duration.as_secs() == 0 {
            return Err(EpochServiceInitError::EpochDuration);
        }

        // TODO: add more check?

        Ok(Self {
            epoch_duration: config.epoch_duration,
            current_epoch: Arc::new(Default::default()),
            genesis: config.genesis,
            epoch_changes: Arc::new(Default::default()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ARGS_DEFAULT_GENESIS;
    use chrono::{NaiveDate, NaiveDateTime, TimeDelta};
    use serial_test::serial;

    #[test]
    fn test_compute_current_epoch() {
        let epoch_duration = Duration::from_hours(1);

        let day_ = 14;
        let genesis_0_date = NaiveDate::from_ymd_opt(2025, 5, day_).unwrap();
        let genesis_0: NaiveDateTime = genesis_0_date.and_hms_opt(4, 0, 0).unwrap();
        let genesis: DateTime<Utc> = DateTime::from_naive_utc_and_offset(genesis_0, Utc);

        let now_f = move || {
            let now_0: NaiveDateTime = genesis_0_date.and_hms_opt(4 + 2, 0, 0).unwrap();
            let now: DateTime<Utc> = DateTime::from_naive_utc_and_offset(now_0, Utc);
            now
        };

        // With epoch duration = 1 hour
        let (current_epoch, current_epoch_start_ts, current_epoch_end_ts) =
            EpochService2::compute_current_epoch_0(&genesis, &now_f, &Duration::from_hours(1));
        assert_eq!(current_epoch, 2);
        assert_eq!(
            current_epoch_start_ts,
            make_utc_datetime(genesis_0_date, 4 + 2, 0, 0).timestamp()
        );
        assert_eq!(
            current_epoch_end_ts,
            make_utc_datetime(genesis_0_date, 4 + 3, 0, 0).timestamp()
        );

        // Same but with epoch duration = 2 hours
        let (current_epoch, current_epoch_start_ts, current_epoch_end_ts) =
            EpochService2::compute_current_epoch_0(&genesis, &now_f, &Duration::from_hours(2));
        assert_eq!(current_epoch, 1);
        assert_eq!(
            current_epoch_start_ts,
            make_utc_datetime(genesis_0_date, 4 + 2, 0, 0).timestamp()
        );
        assert_eq!(
            current_epoch_end_ts,
            make_utc_datetime(genesis_0_date, 4 + 4, 0, 0).timestamp()
        );

        // Same but with epoch duration = 15 minutes
        let (current_epoch, current_epoch_start_ts, current_epoch_end_ts) =
            EpochService2::compute_current_epoch_0(&genesis, &now_f, &Duration::from_mins(15));
        assert_eq!(current_epoch, 8);
        assert_eq!(
            current_epoch_start_ts,
            make_utc_datetime(genesis_0_date, 6, 0, 0).timestamp()
        );
        assert_eq!(
            current_epoch_end_ts,
            make_utc_datetime(genesis_0_date, 6, 15, 0).timestamp()
        );
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

        let now_f = move || {
            genesis
                .checked_add_signed(TimeDelta::new(1, 0).unwrap())
                .unwrap()
        };

        let (current_epoch, current_epoch_start_ts, current_epoch_end_ts) =
            EpochService2::compute_current_epoch_0(&genesis, &now_f, &Duration::from_hours(1));

        assert_eq!(current_epoch, 0);
        assert_eq!(current_epoch_start_ts, genesis.timestamp());
        assert_eq!(
            current_epoch_end_ts,
            genesis.timestamp() + epoch_duration.as_secs() as i64
        );
    }

    #[test]
    fn test_wait_until() {
        // Check wait_until is correctly computed

        let date_0 = NaiveDate::from_ymd_opt(2025, 5, 14).unwrap();
        let datetime_0 = date_0.and_hms_opt(0, 0, 0).unwrap();

        let genesis: DateTime<Utc> = DateTime::from_naive_utc_and_offset(datetime_0, Utc);

        {
            // Check wait_until close to genesis
            let epoch_duration_ = 45;
            let epoch_duration = Duration::from_mins(epoch_duration_);
            let now = || {
                let t_delta = TimeDelta::minutes(15);
                genesis.checked_add_signed(t_delta).unwrap()
            };

            let (current_epoch, wait_until) =
                EpochService2::compute_wait_until(&genesis, &epoch_duration, &now, &now).unwrap();
            assert_eq!(current_epoch, 0);
            assert_eq!(
                wait_until,
                make_utc_datetime(date_0, 0, epoch_duration_ as u32, 0)
            );
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

            let (current_epoch, wait_until) =
                EpochService2::compute_wait_until(&genesis, &epoch_duration, &now, &now).unwrap();
            assert_eq!(current_epoch, 1);
            assert_eq!(
                wait_until,
                make_utc_datetime(date_0, 0, (epoch_duration_ * 2) as u32, 0)
            );
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

    #[tokio::test]
    #[serial]
    async fn test_service() {
        let epoch_duration = Duration::from_secs(30);
        let cfg = EpochService2Config {
            epoch_duration,
            genesis: ARGS_DEFAULT_GENESIS,
        };
        let epoch_service = EpochService2::try_from(cfg).unwrap();
        let notifier = epoch_service.epoch_changes.clone();

        let epoch_store = epoch_service.current_epoch.clone();
        let mut current_epoch = *epoch_service.current_epoch.read();

        // Start epoch service
        tokio::task::spawn(async move { epoch_service.listen_for_new_epoch().await });

        let counter_max = 3;
        let res = tokio::task::spawn(tokio::time::timeout(
            epoch_duration.checked_mul(counter_max + 1).unwrap(),
            async move {
                let mut counter = 0;
                loop {
                    let res = notifier.notified().await;
                    counter += 1;
                    if counter >= counter_max {
                        break;
                    }

                    let epoch = *epoch_store.read();
                    if epoch != current_epoch + 1 && current_epoch != Epoch::from(0) {
                        panic!(
                            "Start with epoch: {:?}, and now in epoch: {:?}, aborting unit test...",
                            current_epoch, epoch
                        );
                    }
                    current_epoch = epoch;
                }
            },
        ));

        res.await.unwrap().unwrap();
    }

    // Helpers

    fn make_utc_datetime(date: NaiveDate, hours: u32, mins: u32, secs: u32) -> DateTime<Utc> {
        let (mins, time_delta) = if mins > 60 {
            (0, Some(TimeDelta::minutes(mins as i64)))
        } else {
            (mins, None)
        };

        let naive_date = date.and_hms_opt(hours, mins, secs).unwrap();

        let naive_date = if let Some(time_delta) = time_delta {
            naive_date.checked_add_signed(time_delta).unwrap()
        } else {
            naive_date
        };

        DateTime::from_naive_utc_and_offset(naive_date, Utc)
    }
}
