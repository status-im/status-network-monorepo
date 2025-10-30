use std::ops::Add;
use std::sync::Arc;
use std::time::Duration;
// third-party
use chrono::{DateTime, NaiveDate, NaiveDateTime, OutOfRangeError, TimeDelta, Utc};
use derive_more::{Deref, From, Into};
use metrics::{gauge, histogram};
use parking_lot::RwLock;
use tokio::sync::Notify;
use tracing::{debug, error};
// internal
use crate::error::AppError;
use crate::metrics::{
    EPOCH_SERVICE_CURRENT_EPOCH, EPOCH_SERVICE_CURRENT_EPOCH_SLICE, EPOCH_SERVICE_DRIFT_MILLIS,
};

/// Duration of an epoch (1 day)
const EPOCH_DURATION: Duration = Duration::from_secs(TimeDelta::days(1).num_seconds() as u64);
/// Minimum duration returned by EpochService::compute_wait_until()
pub(crate) const WAIT_UNTIL_MIN_DURATION: Duration = Duration::from_secs(2);
/// EpochService::compute_wait_until() can return an error like TooLow (see WAIT_UNTIL_MIN_DURATION)
/// so the epoch service will retry X many times.
const WAIT_UNTIL_MAX_COMPUTE_ERROR: usize = 10;

/// An Epoch tracking service
///
/// The service keeps track of the current epoch (duration: 1 day) and the current epoch slice
/// (duration: configurable, < 1 day, usually in minutes)
///
/// Use TryFrom impl to initialize an EpochService. Note that initial epoch & epoch slice is
/// initialized to Default values. Calling listen_for_new_epoch will initialize these fields.
pub struct EpochService {
    /// A subdivision of an epoch (in minutes or seconds)
    epoch_slice_duration: Duration,
    /// Current epoch and epoch slice
    pub current_epoch: Arc<RwLock<(Epoch, EpochSlice)>>,
    /// Genesis time (aka when the service has been started at the first time)
    genesis: DateTime<Utc>,
    /// Channel to notify when an epoch / epoch slice has just changed
    pub epoch_changes: Arc<Notify>,
}

impl EpochService {
    // Note: listen_for_new_epoch never ends so no log will happen with #[instrument]
    //       + metrics already tracks the current epoch / epoch_slice
    // #[instrument(skip(self), fields(self.epoch_slice_duration, self.genesis, self.current_epoch))]
    pub(crate) async fn listen_for_new_epoch(&self) -> Result<(), AppError> {
        let epoch_slice_count =
            Self::compute_epoch_slice_count(EPOCH_DURATION, self.epoch_slice_duration);
        debug!("epoch slice in an epoch: {}", epoch_slice_count);

        let mut retry_counter = 0;
        let (mut current_epoch, mut current_epoch_slice, mut wait_until): (
            i64,
            i64,
            tokio::time::Instant,
        ) = loop {
            match self.compute_wait_until(&|| Utc::now(), &|| tokio::time::Instant::now()) {
                Ok((current_epoch, current_epoch_slice, wait_until)) => {
                    break (current_epoch, current_epoch_slice, wait_until);
                }
                Err(WaitUntilError::TooLow(d1, d2)) => {
                    // Wait until is too low (according to const WAIT_UNTIL_MIN_DURATION)
                    // so we will retry (WAIT_UNTIL_MAX_COMPUTE_ERROR many times) after a short sleep

                    debug!("compute_wait_until return TooLow, will retry after a sleep...");
                    tokio::time::sleep(WAIT_UNTIL_MIN_DURATION).await;
                    retry_counter += 1;
                    if retry_counter > WAIT_UNTIL_MAX_COMPUTE_ERROR {
                        error!(
                            "Too many errors while computing the initial wait until, aborting..."
                        );
                        return Err(AppError::EpochError(WaitUntilError::TooLow(d1, d2)));
                    }
                }
                Err(e) => {
                    // Another error (like OutOfRange) - exiting...

                    error!("Error computing the initial wait until: {}", e);
                    return Err(AppError::EpochError(e));
                }
            };
        };

        // debug!("wait until: {:?}", wait_until);
        *self.current_epoch.write() = (current_epoch.into(), current_epoch_slice.into());
        debug!(
            "Initial epoch: {}, epoch slice: {}",
            current_epoch, current_epoch_slice
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
            }

            // Note: could use checked_add() here, but it's quite impossible to have an overflow here
            //       it would mean that the epoch_slice_duration is insanely large and wait_until
            //       overflows as a timestamp
            wait_until += self.epoch_slice_duration;

            current_epoch_slice += 1;
            if current_epoch_slice == epoch_slice_count {
                current_epoch_slice = 0;
                current_epoch += 1;
            }
            *self.current_epoch.write() = (current_epoch.into(), current_epoch_slice.into());
            debug!(
                "epoch: {}, epoch slice: {}",
                current_epoch, current_epoch_slice
            );

            // Note: based on this link https://doc.rust-lang.org/reference/expressions/operator-expr.html#type-cast-expressions
            //       "Casting from an integer to float will produce the closest possible float *"
            gauge!(EPOCH_SERVICE_CURRENT_EPOCH.name, "prover" => "epoch service")
                .set(current_epoch as f64);
            gauge!(EPOCH_SERVICE_CURRENT_EPOCH_SLICE.name, "prover" => "epoch service")
                .set(current_epoch_slice as f64);

            // println!("Epoch changed: {}", current_epoch);
            self.epoch_changes.notify_one();
        }

        // Ok(())
    }

    fn compute_wait_until<T, F: Fn() -> DateTime<Utc>, TF: Fn() -> T>(
        &self,
        now: &F,
        now2: &TF,
    ) -> Result<(i64, i64, T), WaitUntilError>
    where
        T: Add<Duration, Output = T>,
    {
        let (current_epoch, now_date) = EpochService::compute_current_epoch(self.genesis, now);
        debug!("current_epoch: {}", current_epoch);
        // self.current_epoch.store(current_epoch, Ordering::SeqCst);

        let current_epoch_slice =
            EpochService::compute_current_epoch_slice(now_date, self.epoch_slice_duration, now);
        debug!("current epoch slice: {}", current_epoch_slice);

        // time to wait to next epoch slice
        let day_start = DateTime::from_naive_utc_and_offset(now_date.into(), Utc);
        // Note:
        // unwrap safe -> epoch_slice_duration < epoch duration checked in constructor
        let epoch_slice_next: DateTime<Utc> = day_start.add(
            self.epoch_slice_duration
                .checked_mul(current_epoch_slice as u32 + 1)
                .unwrap(),
        );

        debug!("epoch slice next: {}", epoch_slice_next);

        // Note: to_std() will return an Error if now > epoch_slice_next
        //       This can happen if epoch_slice_next is very close to now()
        let wait_until = (epoch_slice_next - now())
            .to_std()
            .map_err(WaitUntilError::OutOfRange)?;
        if wait_until < WAIT_UNTIL_MIN_DURATION {
            return Err(WaitUntilError::TooLow(wait_until, WAIT_UNTIL_MIN_DURATION));
        }

        let wait_until = now2() + wait_until;
        Ok((current_epoch, current_epoch_slice, wait_until))
    }

    /// Number of epoch slices in an epoch
    fn compute_epoch_slice_count(epoch_duration: Duration, epoch_slice_duration: Duration) -> i64 {
        (epoch_duration.as_secs() / epoch_slice_duration.as_secs()) as i64
    }

    /// Compute current epoch (since genesis)
    fn compute_current_epoch<F: Fn() -> DateTime<Utc>>(
        genesis: DateTime<Utc>,
        now: &F,
    ) -> (i64, NaiveDate) {
        debug_assert!(now().date_naive() >= genesis.date_naive());

        let genesis_date = genesis.date_naive();
        let now_date = now().date_naive();
        let diff = now_date - genesis_date;
        (diff.num_days(), now_date)
    }

    /// Compute current epoch slice
    /// now_date: today's date (usually returned by: compute_current_epoch)
    fn compute_current_epoch_slice<F: Fn() -> DateTime<Utc>>(
        now_date: NaiveDate,
        epoch_slice_duration: Duration,
        now: &F,
    ) -> i64 {
        debug_assert!(epoch_slice_duration.as_secs() > 0);
        debug_assert!(i32::try_from(epoch_slice_duration.as_secs()).is_ok());

        let day_start_ = NaiveDateTime::from(now_date);
        let day_start = DateTime::from_naive_utc_and_offset(day_start_, chrono::Utc);
        // debug!("start of day: {}", day_start);

        // Note:
        // cannot unwrap -> epoch_slice_duration.as_secs() as i32 is checked
        // + epoch_slice_duration is checked > 0
        let elapsed_for_epoch = (now() - day_start)
            .checked_div(epoch_slice_duration.as_secs() as i32)
            .unwrap();
        // debug!("elapsed for epoch: {}", elapsed_for_epoch.num_seconds());
        elapsed_for_epoch.num_seconds()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum EpochServiceInitError {
    #[error("epoch slice duration is too large (cannot fit in i32) or == 0")]
    InvalidEpochSliceDuration,
    #[error("genesis is in the future")]
    InvalidGenesis,
}

impl TryFrom<(Duration, DateTime<Utc>)> for EpochService {
    type Error = EpochServiceInitError;

    fn try_from(
        (epoch_slice_duration, genesis): (Duration, DateTime<Utc>),
    ) -> Result<Self, Self::Error> {
        if genesis >= Utc::now() {
            return Err(EpochServiceInitError::InvalidGenesis);
        }

        if epoch_slice_duration.as_secs() == 0
            || i32::try_from(epoch_slice_duration.as_secs()).is_err()
            || epoch_slice_duration < WAIT_UNTIL_MIN_DURATION
            || epoch_slice_duration >= (EPOCH_DURATION / 2)
        {
            return Err(EpochServiceInitError::InvalidEpochSliceDuration);
        }

        // TODO: should we check the division: epoch_duration / epoch_slice_duration ?

        Ok(Self {
            epoch_slice_duration,
            current_epoch: Arc::new(Default::default()),
            genesis,
            epoch_changes: Arc::new(Default::default()),
        })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum WaitUntilError {
    #[error("Computation error: {0}")]
    OutOfRange(#[from] OutOfRangeError),
    #[error("Wait until is too low: {0:?} (min value: {1:?}")]
    TooLow(Duration, Duration),
}

/// An Epoch (wrapper type over i64)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, From, Into, Deref)]
pub(crate) struct Epoch(i64);

impl Add<i64> for Epoch {
    type Output = Self;

    fn add(self, rhs: i64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

/// An Epoch slice (wrapper type over i64)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, From, Into, Deref)]
pub(crate) struct EpochSlice(i64);

impl Add<i64> for EpochSlice {
    type Output = Self;

    fn add(self, rhs: i64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveDateTime, TimeDelta};

    #[test]
    fn test_wait_until() {
        // Check wait_until is correctly computed

        let date_0 = NaiveDate::from_ymd_opt(2025, 5, 14).unwrap();
        let datetime_0 = date_0.and_hms_opt(0, 0, 0).unwrap();

        {
            // standard wait until in epoch 0, epoch slice 1

            let genesis: DateTime<Utc> = DateTime::from_naive_utc_and_offset(datetime_0, Utc);
            let epoch_slice_duration = Duration::from_secs(60);
            let epoch_service = EpochService::try_from((epoch_slice_duration, genesis)).unwrap();

            let now = || {
                let mut now_0: NaiveDateTime = date_0.and_hms_opt(0, 0, 0).unwrap();
                // Set now_0 to be in epoch slice 1
                now_0 += epoch_slice_duration;
                DateTime::from_naive_utc_and_offset(now_0, chrono::Utc)
            };

            let (epoch, epoch_slice, wait_until): (_, _, DateTime<Utc>) =
                epoch_service.compute_wait_until(&now, &now).unwrap();

            assert_eq!(epoch, 0);
            assert_eq!(epoch_slice, 1);
            assert_eq!(
                wait_until,
                DateTime::<Utc>::from_naive_utc_and_offset(datetime_0, Utc)
                    + 2 * epoch_slice_duration
            );
        }

        {
            // standard wait until (but in epoch 1)

            let genesis: DateTime<Utc> = DateTime::from_naive_utc_and_offset(datetime_0, Utc);
            let epoch_slice_duration = Duration::from_secs(60);
            let epoch_service = EpochService::try_from((epoch_slice_duration, genesis)).unwrap();

            let now = || {
                let mut now_0: NaiveDateTime = date_0.and_hms_opt(0, 0, 0).unwrap();
                // Set now_0 to be in epoch 1
                now_0 += EPOCH_DURATION;
                // Set now_0 to be in epoch 1, epoch slice 1
                now_0 += epoch_slice_duration;
                // Add 30 secs (but should still wait until epoch slice 2 starts)
                now_0 += epoch_slice_duration / 2;
                chrono::DateTime::from_naive_utc_and_offset(now_0, Utc)
            };

            let (epoch, epoch_slice, wait_until): (_, _, DateTime<Utc>) =
                epoch_service.compute_wait_until(&now, &now).unwrap();

            assert_eq!(epoch, 1);
            assert_eq!(epoch_slice, 1);
            assert_eq!(
                wait_until,
                DateTime::<Utc>::from_naive_utc_and_offset(datetime_0, Utc)
                    + EPOCH_DURATION
                    + 2 * epoch_slice_duration
            );
        }

        {
            // Check for WaitUntilError::TooLow

            let genesis: DateTime<Utc> =
                chrono::DateTime::from_naive_utc_and_offset(datetime_0, Utc);
            let epoch_slice_duration = Duration::from_secs(60);
            let epoch_service = EpochService::try_from((epoch_slice_duration, genesis)).unwrap();
            let epoch_slice_duration_minus_1 =
                epoch_slice_duration - WAIT_UNTIL_MIN_DURATION + Duration::from_secs(1);

            let now = || {
                let mut now_0: NaiveDateTime = date_0
                    .and_hms_opt(0, 0, epoch_slice_duration_minus_1.as_secs() as u32)
                    .unwrap();
                // Set now_0 to be in epoch slice 1
                now_0 += epoch_slice_duration;
                chrono::DateTime::from_naive_utc_and_offset(now_0, chrono::Utc)
            };

            let res = epoch_service.compute_wait_until(&now, &now);

            assert!(matches!(res, Err(WaitUntilError::TooLow(_, _))));
        }
    }

    #[test]
    fn test_compute_epoch_slice_count() {
        // test the computation of the number of epoch slices in an epoch

        assert_eq!(
            EpochService::compute_epoch_slice_count(
                Duration::from_secs(TimeDelta::days(1).num_seconds() as u64),
                Duration::from_secs(TimeDelta::hours(1).num_seconds() as u64)
            ),
            24
        );
        assert_eq!(
            EpochService::compute_epoch_slice_count(
                Duration::from_secs(TimeDelta::days(1).num_seconds() as u64),
                Duration::from_secs(TimeDelta::minutes(30).num_seconds() as u64)
            ),
            48
        );
    }

    #[test]
    fn test_compute_current_epoch() {
        let day_ = 14;
        let genesis_0: NaiveDateTime = NaiveDate::from_ymd_opt(2025, 5, day_)
            .unwrap()
            .and_hms_opt(4, 0, 0)
            .unwrap();
        let genesis: DateTime<Utc> =
            chrono::DateTime::from_naive_utc_and_offset(genesis_0, chrono::Utc);

        let now_f = move || {
            let now_0: NaiveDateTime = NaiveDate::from_ymd_opt(2025, 5, day_ + 2)
                .unwrap()
                .and_hms_opt(4, 0, 0)
                .unwrap();
            // let genesis = DateTime:: new(2025, 05, 18, 4, 0, 0).unwrap();
            let now: DateTime<Utc> =
                chrono::DateTime::from_naive_utc_and_offset(now_0, chrono::Utc);
            now
        };

        assert_eq!(EpochService::compute_current_epoch(genesis, &now_f).0, 2);
    }

    #[test]
    fn test_compute_current_epoch_slice() {
        let day = NaiveDate::from_ymd_opt(2025, 5, 14).unwrap();
        let now_date = day;

        let now_f = move || {
            let now_0: NaiveDateTime = day.and_hms_opt(0, 4, 0).unwrap();
            let now: DateTime<Utc> =
                chrono::DateTime::from_naive_utc_and_offset(now_0, chrono::Utc);
            now
        };
        let now_f_2 = move || {
            let now_0: NaiveDateTime = day.and_hms_opt(0, 5, 59).unwrap();
            let now: DateTime<Utc> =
                chrono::DateTime::from_naive_utc_and_offset(now_0, chrono::Utc);
            now
        };
        let now_f_3 = move || {
            let now_0: NaiveDateTime = day.and_hms_opt(0, 6, 0).unwrap();
            let now: DateTime<Utc> =
                chrono::DateTime::from_naive_utc_and_offset(now_0, chrono::Utc);
            now
        };

        // epoch_slice == 2 minutes
        let epoch_slice_duration = Duration::from_secs(60 * 2);

        // Note: 4-minute diff -> expect == 2
        assert_eq!(
            EpochService::compute_current_epoch_slice(now_date, epoch_slice_duration, &now_f),
            2
        );
        // Note: 5 minutes and 59 seconds diff -> still expect == 2
        assert_eq!(
            EpochService::compute_current_epoch_slice(
                now_date,
                epoch_slice_duration,
                &Box::new(now_f_2)
            ),
            2
        );
        // Note: 6 minutes diff -> expect == 3
        assert_eq!(
            EpochService::compute_current_epoch_slice(
                now_date,
                epoch_slice_duration,
                &Box::new(now_f_3)
            ),
            3
        );
    }
}
