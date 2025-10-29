// std
use parking_lot::RwLock;
use std::sync::Arc;
// third-party
use tokio::sync::Notify;
use tracing::debug;
// internal
use crate::epoch_service::{Epoch, EpochSlice};
use crate::error::AppError;
use crate::tier::TierLimits;
use crate::user_db::{UserDb, UserDbConfig};
use crate::user_db_error::UserDbOpenError;
use crate::user_db_types::RateLimit;

/// Async service to update a UserDb on epoch changes
#[derive(Debug)]
pub struct UserDbService {
    user_db: UserDb,
    epoch_changes: Arc<Notify>,
}

impl UserDbService {
    pub fn new(
        config: UserDbConfig,
        epoch_changes_notifier: Arc<Notify>,
        epoch_store: Arc<RwLock<(Epoch, EpochSlice)>>,
        rate_limit: RateLimit,
        tier_limits: TierLimits,
    ) -> Result<Self, UserDbOpenError> {
        let user_db = UserDb::new(config, epoch_store, tier_limits, rate_limit)?;
        Ok(Self {
            user_db,
            epoch_changes: epoch_changes_notifier,
        })
    }

    pub fn get_user_db(&self) -> UserDb {
        self.user_db.clone()
    }

    pub async fn listen_for_epoch_changes(&self) -> Result<(), AppError> {
        let (mut current_epoch, mut current_epoch_slice) = *self.user_db.epoch_store.read();

        loop {
            self.epoch_changes.notified().await;
            let (new_epoch, new_epoch_slice) = *self.user_db.epoch_store.read();
            debug!(
                "new epoch: {:?}, new epoch slice: {:?}",
                new_epoch, new_epoch_slice
            );
            self.update_on_epoch_changes(
                &mut current_epoch,
                new_epoch,
                &mut current_epoch_slice,
                new_epoch_slice,
            );
        }
    }

    /// Internal - used by listen_for_epoch_changes
    fn update_on_epoch_changes(
        &self,
        current_epoch: &mut Epoch,
        new_epoch: Epoch,
        current_epoch_slice: &mut EpochSlice,
        new_epoch_slice: EpochSlice,
    ) {
        if new_epoch > *current_epoch {
            self.user_db.on_new_epoch()
        } else if new_epoch_slice > *current_epoch_slice {
            self.user_db.on_new_epoch_slice()
        }

        *current_epoch = new_epoch;
        *current_epoch_slice = new_epoch_slice;
    }
}
