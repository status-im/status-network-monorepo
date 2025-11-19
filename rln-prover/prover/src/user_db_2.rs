use std::sync::Arc;
// third-party
use alloy::primitives::Address;
use ark_bn254::Fr;
use parking_lot::RwLock;
// RLN
use rln::{
    hashers::poseidon_hash,
    protocol::keygen,
};
// db
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, QueryFilter, ColumnTrait, TransactionTrait, IntoActiveModel, ActiveModelTrait, Set, Iden};
use sea_orm::sea_query::OnConflict;
// internal
use prover_db_entity::{tx_counter, user, tier_limits};
use rln_proof::RlnUserIdentity;
use smart_contract::KarmaAmountExt;
use crate::epoch_service::{Epoch, EpochSlice};
use crate::tier::{TierLimit, TierLimits, TierMatch};
use crate::user_db::UserTierInfo;
use crate::user_db_error::{RegisterError, RegisterError2, SetTierLimitsError2, TxCounterError, TxCounterError2, UserTierInfoError2};
use crate::user_db_types::{EpochCounter, EpochSliceCounter, IndexInMerkleTree, RateLimit, TreeIndex};

const TIER_LIMITS_KEY: &str = "CURRENT";
const TIER_LIMITS_NEXT_KEY: &str = "NEXT";

#[derive(Clone)]
pub struct UserDb2Config {
    pub(crate) tree_count: u64,
    pub(crate) max_tree_count: u64,
    pub(crate) tree_depth: u8,
}

#[derive(Clone)]
struct UserDb2 {
    db: DatabaseConnection,
    config: UserDb2Config,
    rate_limit: RateLimit,
    pub(crate) epoch_store: Arc<RwLock<(Epoch, EpochSlice)>>,
}


impl UserDb2 {
    /// Returns a new `UserDB` instance
    pub async fn new(
        db: DatabaseConnection,
        config: UserDb2Config,
        epoch_store: Arc<RwLock<(Epoch, EpochSlice)>>,
        tier_limits: TierLimits,
        rate_limit: RateLimit,
    ) -> Result<Self, DbErr> {

        // tier limits
        let res_delete = tier_limits::Entity::delete_many()
            .filter(tier_limits::Column::Name.eq(TIER_LIMITS_KEY))
            .exec(&db)
            .await?;
        debug_assert!(res_delete.rows_affected == 2);

        let tier_limits_value = serde_json::to_value(tier_limits).unwrap();
        let tier_limits_active_model = tier_limits::ActiveModel {
            name: Set(TIER_LIMITS_KEY.to_string()),
            tier_limits: Set(Some(tier_limits_value)),
            ..Default::default()
        };
        tier_limits::Entity::insert(tier_limits_active_model).exec(&db).await?;

        Ok(Self {
            db,
            config,
            rate_limit,
            epoch_store,
        })
    }

    // (Internal) Simple Db related methods

    async fn has_user(&self, address: &Address) -> Result<bool, DbErr> {
        let res = user::Entity::find()
            .filter(user::Column::Address.eq(address.to_string()))
            .one(&self.db)
            .await?;
        Ok(res.is_some())
    }

    async fn get_user(&self, address: &Address) -> Option<RlnUserIdentity> {

        let res = user::Entity::find()
            .filter(user::Column::Address.eq(address.to_string()))
            .one(&self.db)
            .await
            .ok()??;

        // FIXME: deser directly when query with orm?
        serde_json::from_value(res.rln_id).ok()
    }


    async fn get_tier_limits(&self) -> Result<TierLimits, DbErr> {

        let res = tier_limits::Entity::find()
            .filter(tier_limits::Column::Name.eq(TIER_LIMITS_KEY))
            .one(&self.db)
            .await?
            .unwrap() // unwrap safe - db is always initialized with this row
            ;

        // unwrap safe - db is initialized with valid tier limits
        Ok(serde_json::from_value(res.tier_limits.unwrap()).unwrap())
    }

    async fn set_tier_limits(&self, tier_limits: TierLimits) -> Result<(), DbErr> {

        let tier_limits_active_model = tier_limits::ActiveModel {
            name: Set(TIER_LIMITS_NEXT_KEY.to_string()),
            tier_limits: Set(Some(serde_json::to_value(tier_limits).unwrap())),
            ..Default::default()
        };

        // upsert
        tier_limits::Entity::insert(tier_limits_active_model)
            .on_conflict(
                OnConflict::column(tier_limits::Column::Name)
                    .update_column(tier_limits::Column::TierLimits)
                    .to_owned()
            )
            .exec(&self.db)
            .await?;
        Ok(())
    }

    // internal methods for tx_counter

    async fn incr_tx_counter(
        &self,
        address: &Address,
        incr_value: Option<i64>,
    ) -> Result<(), DbErr> {

        let incr_value = incr_value.unwrap_or(1);
        let (epoch, epoch_slice) = *self.epoch_store.read();

        let txn = self.db.begin().await?;

        let res = tx_counter::Entity::find()
            .filter(user::Column::Address.eq(address.to_string()))
            .one(&txn)
            .await?;

        if let Some(res) = res {

            let mut res_active = res.into_active_model();

            // unwrap safe: res_active.epoch/epoch_slice cannot be null
            let model_epoch = res_active.epoch.clone().unwrap();
            let model_epoch_slice = res_active.epoch_slice.clone().unwrap();
            let model_epoch_counter = res_active.epoch_counter.clone().unwrap();
            let model_epoch_slice_counter = res_active.epoch_slice_counter.clone().unwrap();

            if epoch != Epoch::from(model_epoch) {
                // New epoch
                res_active.epoch = Set(epoch.into());
                res_active.epoch_slice = Set(0);
                res_active.epoch_counter = Set(incr_value);
                res_active.epoch_slice_counter = Set(incr_value);
            } else if epoch_slice != EpochSlice::from(model_epoch_slice) {
                // New epoch slice
                res_active.epoch_slice = Set(epoch_slice.into());
                res_active.epoch_counter = Set(model_epoch_counter.saturating_add(incr_value));
                res_active.epoch_slice_counter = Set(incr_value);
            } else {
                // Same epoch & epoch slice
                res_active.epoch_counter = Set(model_epoch_counter.saturating_add(incr_value));
                res_active.epoch_slice_counter = Set(model_epoch_slice_counter.saturating_add(incr_value));
            }

            res_active.update(&txn).await?;

        } else {

            // first time - need to create a new entry
            let new_tx_counter = tx_counter::ActiveModel {
                address: Set(address.to_string()),
                epoch: Set(epoch.into()),
                epoch_slice: Set(epoch_slice.into()),
                epoch_counter: Set(incr_value),
                epoch_slice_counter: Set(incr_value),
                ..Default::default()
            };

            new_tx_counter.insert(&txn).await?;
        }

        txn.commit().await?;
        Ok(())
    }

    async fn get_tx_counter(
        &self,
        address: &Address,
    ) -> Result<(EpochCounter, EpochSliceCounter), DbErr> {

        let res = tx_counter::Entity::find()
            .filter(user::Column::Address.eq(address.to_string()))
            .one(&self.db)
            .await?
            // TODO: return NotRegisteredError
            .unwrap(); // FIXME

        Ok(self.counters_from_key(address, res))
    }

    fn counters_from_key(
        &self,
        address: &Address,
        model: tx_counter::Model
    ) -> (EpochCounter, EpochSliceCounter) {

        let (epoch, epoch_slice) = *self.epoch_store.read();
        let cmp = (model.epoch == i64::from(epoch), model.epoch_slice == i64::from(epoch_slice));

        match cmp {
            (true, true) => {
                // EpochCounter stored in DB == epoch store
                // We query for an epoch / epoch slice and this is what is stored in the Db
                // Return the counters
                (
                    // FIXME: as
                    (model.epoch_counter as u64).into(),
                    // FIXME: as
                    (model.epoch_slice_counter as u64).into(),
                )
            }
            (true, false) => {
                // EpochCounter.epoch_slice (stored in Db) != epoch_store.epoch_slice
                // We query for an epoch slice after what is stored in Db
                // This can happen if no Tx has updated the epoch slice counter (yet)
                // FIXME: as
                ((model.epoch_counter as u64).into(), EpochSliceCounter::from(0))
            }
            (false, true) => {
                // EpochCounter.epoch (stored in DB) != epoch_store.epoch
                // We query for an epoch after what is stored in Db
                // This can happen if no Tx has updated the epoch counter (yet)
                (EpochCounter::from(0), EpochSliceCounter::from(0))
            }
            (false, false) => {
                // EpochCounter (stored in DB) != epoch_store
                // Outdated value (both for epoch & epoch slice)
                (EpochCounter::from(0), EpochSliceCounter::from(0))
            }
        }
    }

    // user register

    async fn register_user(&self, address: Address) -> Result<Fr, RegisterError2> {

        // Generate RLN identity
        let (identity_secret_hash, id_commitment) = keygen();

        let rln_identity = RlnUserIdentity::from((
            id_commitment,
            identity_secret_hash,
            Fr::from(self.rate_limit),
        ));

        if !self.has_user(&address).await? {
            return Err(RegisterError2::AlreadyRegistered(address))
        }

        let rate_commit =
            poseidon_hash(&[id_commitment, Fr::from(u64::from(self.rate_limit))]); 
        
        todo!()
    }

    // external UserDb methods

    pub fn on_new_user(&self, address: &Address) -> Result<Fr, RegisterError> {
        // self.register(*address)
        unimplemented!()
    }

    pub async fn on_new_tx(
        &self,
        address: &Address,
        incr_value: Option<i64>
    ) -> Result<EpochSliceCounter, TxCounterError2> {

        let has_user = self
            .has_user(address)
            .await
            .map_err(TxCounterError2::Db)?;

        if has_user {
            let _ = self.incr_tx_counter(address, incr_value).await?;
            // FIXME: return? should we handle check against rate_limit here?
            Ok(EpochSliceCounter::from(0))
        } else {
            Err(TxCounterError2::NotRegistered(*address))
        }
    }

    pub async fn on_tier_limits_updated(
        &self,
        tier_limits: TierLimits,
    ) -> Result<(), SetTierLimitsError2> {
        tier_limits.validate()?;
        self.set_tier_limits(tier_limits).await.map_err(SetTierLimitsError2::Db)
    }

    /// Get user tier info
    pub(crate) async fn user_tier_info<E: std::error::Error, KSC: KarmaAmountExt<Error = E>>(
        &self,
        address: &Address,
        karma_sc: &KSC,
    ) -> Result<UserTierInfo, UserTierInfoError2<E>> {

        let has_user = self.has_user(address).await.map_err(UserTierInfoError2::Db)?;

        if !has_user {
            return Err(UserTierInfoError2::NotRegistered(*address));
        }

        let karma_amount = karma_sc
            .karma_amount(address)
            .await
            .map_err(|e| UserTierInfoError2::Contract(e))?;

        // TODO
        let (epoch_tx_count, epoch_slice_tx_count) = self.get_tx_counter(address).await?;
        // TODO: avoid db query the tier limits (keep it in memory)
        let tier_limits = self.get_tier_limits().await?;
        let tier_match = tier_limits.get_tier_by_karma(&karma_amount);

        let user_tier_info = {
            let (current_epoch, current_epoch_slice) = *self.epoch_store.read();
            let mut t = UserTierInfo {
                current_epoch,
                current_epoch_slice,
                epoch_tx_count: epoch_tx_count.into(),
                epoch_slice_tx_count: epoch_slice_tx_count.into(),
                karma_amount,
                tier_name: None,
                tier_limit: None,
            };

            if let TierMatch::Matched(tier) = tier_match {
                t.tier_name = Some(tier.name.into());
                t.tier_limit = Some(TierLimit::from(tier.tx_per_epoch));
            }

            t
        };

        Ok(user_tier_info)
    }
}