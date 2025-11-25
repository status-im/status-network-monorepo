use std::sync::Arc;
// third-party
use alloy::primitives::Address;
use ark_bn254::Fr;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use parking_lot::RwLock;
use tokio::sync::RwLock as TokioRwLock;
// RLN
use rln::{
    hashers::poseidon_hash,
    protocol::keygen,
};
// db
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, QueryFilter, ColumnTrait, TransactionTrait, IntoActiveModel, ActiveModelTrait, Set, Iden, PaginatorTrait};
use sea_orm::sea_query::OnConflict;
// internal
use prover_db_entity::{tx_counter, user, tier_limits, m_tree_config};
use prover_pmtree::{Hasher, MerkleTree, PmtreeErrorKind, Value};
use prover_merkle_tree::{MemoryDb, MemoryDbConfig, PersistentDb, PersistentDbConfig, PersistentDbError};
use rln_proof::RlnUserIdentity;
use smart_contract::KarmaAmountExt;
use crate::epoch_service::{Epoch, EpochSlice};
use crate::tier::{TierLimit, TierLimits, TierMatch};
use crate::user_db::{UserDb, UserTierInfo};
use crate::user_db_error::{DbError, RegisterError, RegisterError2, SetTierLimitsError2, TxCounterError2, UserTierInfoError2};
use crate::user_db_serialization::U64Deserializer;
use crate::user_db_types::{EpochCounter, EpochSliceCounter, RateLimit};

const TIER_LIMITS_KEY: &str = "CURRENT";
const TIER_LIMITS_NEXT_KEY: &str = "NEXT";

type ProverMerkleTree = MerkleTree<MemoryDb, ProverPoseidonHash, PersistentDb, MerkleTreeError>;

#[derive(Clone)]
pub struct UserDb2Config {
    pub(crate) tree_count: u64,
    pub(crate) max_tree_count: u64,
    pub(crate) tree_depth: u8,
}

#[derive(Clone)]
pub(crate) struct UserDb2 {
    db: DatabaseConnection,
    config: UserDb2Config,
    rate_limit: RateLimit,
    pub(crate) epoch_store: Arc<RwLock<(Epoch, EpochSlice)>>,
    merkle_trees: Arc<TokioRwLock<Vec<ProverMerkleTree>>>,
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

        debug_assert!(config.tree_count <= config.max_tree_count);

        // tier limits
        debug_assert!(tier_limits.validate().is_ok());
        let res_delete = tier_limits::Entity::delete_many()
            .filter(tier_limits::Column::Name.eq(TIER_LIMITS_KEY))
            .exec(&db)
            .await?;

        let tier_limits_value = serde_json::to_value(tier_limits).unwrap();
        let tier_limits_active_model = tier_limits::ActiveModel {
            name: Set(TIER_LIMITS_KEY.to_string()),
            tier_limits: Set(Some(tier_limits_value)),
            ..Default::default()
        };
        tier_limits::Entity::insert(tier_limits_active_model).exec(&db).await?;

        // merkle trees
        let merkle_tree_count = Self::get_merkle_tree_count_from_db(&db).await?;
        let mut merkle_trees = Vec::with_capacity(merkle_tree_count as usize);

        if merkle_tree_count == 0 {

            // FIXME: 'as'
            for i in 0..(config.tree_count as i16) {
                let persistent_db_config = PersistentDbConfig {
                    db_conn: db.clone(),
                    tree_index: i,
                    insert_batch_size: 10_000, // TODO: no hardcoded value
                };

                let mt = ProverMerkleTree::new(
                    config.tree_depth as usize, // FIXME: no 'as'
                    MemoryDbConfig,
                    persistent_db_config.clone()
                ).await.unwrap();

                merkle_trees.push(mt);
            }

        } else {

            for i in 0..(merkle_tree_count as i16) {
                let persistent_db_config = PersistentDbConfig {
                    db_conn: db.clone(),
                    tree_index: i,
                    insert_batch_size: 10_000, // TODO: no hardcoded value
                };

                let mt = ProverMerkleTree::load(
                    MemoryDbConfig,
                    persistent_db_config.clone()
                ).await.unwrap();

                merkle_trees.push(mt);
            }

        }

        Ok(Self {
            db,
            config,
            rate_limit,
            epoch_store,
            merkle_trees: Arc::new(TokioRwLock::new(merkle_trees)),
        })
    }

    // (Internal) Simple Db related methods

    pub(crate) async fn has_user(&self, address: &Address) -> Result<bool, DbErr> {
        let res = user::Entity::find()
            .filter(user::Column::Address.eq(address.to_string()))
            .one(&self.db)
            .await?;
        Ok(res.is_some())
    }

    pub(crate) async fn get_user(&self, address: &Address) -> Result<Option<user::Model>, DbErr> {

        user::Entity::find()
            .filter(user::Column::Address.eq(address.to_string()))
            .one(&self.db)
            .await
    }

    async fn get_user_identity(&self, address: &Address) -> Option<RlnUserIdentity> {

        let res = self.get_user(address).await
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

    async fn get_merkle_tree_count_from_db(db: &DatabaseConnection) -> Result<u64, DbErr> {
        m_tree_config::Entity::find().count(db).await
    }

    // internal methods for tx_counter

    async fn incr_tx_counter(
        &self,
        address: &Address,
        incr_value: Option<i64>,
    ) -> Result<EpochSliceCounter, DbErr> {

        let incr_value = incr_value.unwrap_or(1);
        let (epoch, epoch_slice) = *self.epoch_store.read();

        let txn = self.db.begin().await?;

        let res = tx_counter::Entity::find()
            .filter(tx_counter::Column::Address.eq(address.to_string()))
            .one(&txn)
            .await?;

        let new_tx_counter = if let Some(res) = res {

            let mut res_active = res.into_active_model();

            // unwrap safe: res_active.epoch/epoch_slice cannot be null
            let model_epoch = res_active.epoch.clone().unwrap();
            let model_epoch_slice = res_active.epoch_slice.clone().unwrap();
            let model_epoch_counter = res_active.epoch_counter.clone().unwrap();
            let model_epoch_slice_counter = res_active.epoch_slice_counter.clone().unwrap();

            if model_epoch == 0 && model_epoch_slice == 0 {
                res_active.epoch = Set(epoch.into());
                res_active.epoch_slice = Set(epoch_slice.into());
                res_active.epoch_counter = Set(incr_value);
                res_active.epoch_slice_counter = Set(incr_value);
            } else if epoch != Epoch::from(model_epoch) {
                // New epoch
                res_active.epoch = Set(epoch.into());
                res_active.epoch_slice = Set(0);
                res_active.epoch_counter = Set(incr_value);
                res_active.epoch_slice_counter = Set(incr_value);
            } else if epoch_slice != EpochSlice::from(model_epoch_slice) {
                // New epoch slice
                res_active.epoch = Set(epoch.into());
                res_active.epoch_slice = Set(epoch_slice.into());
                res_active.epoch_counter = Set(model_epoch_counter.saturating_add(incr_value));
                res_active.epoch_slice_counter = Set(incr_value);
            } else {
                // Same epoch & epoch slice
                res_active.epoch_counter = Set(model_epoch_counter.saturating_add(incr_value));
                res_active.epoch_slice_counter = Set(model_epoch_slice_counter.saturating_add(incr_value));
            }

            // res_active.update(&txn).await?;
            tx_counter::Entity::update(res_active).exec(&txn).await?

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

            // new_tx_counter.insert(&txn).await?;
            tx_counter::Entity::insert(new_tx_counter)
                .exec_with_returning(&txn)
                .await?
        };

        txn.commit().await?;
        // FIXME: no 'as'
        Ok((new_tx_counter.epoch_slice_counter as u64).into())
    }

    pub(crate) async fn get_tx_counter(
        &self,
        address: &Address,
    ) -> Result<(EpochCounter, EpochSliceCounter), TxCounterError2> {

        let res = tx_counter::Entity::find()
            .filter(tx_counter::Column::Address.eq(address.to_string()))
            .one(&self.db)
            .await?;

        match res {
            None => Err(TxCounterError2::NotRegistered(address.clone())),
            Some(res) => Ok(self.counters_from_key(address, res))
        }
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

    // user register & delete (with app logic)

    pub(crate) async fn register_user(&self, address: Address) -> Result<Fr, RegisterError2> {

        // Generate RLN identity
        let (identity_secret_hash, id_commitment) = keygen();

        let rln_identity = RlnUserIdentity::from((
            id_commitment,
            identity_secret_hash,
            Fr::from(self.rate_limit),
        ));

        if self.has_user(&address).await? {
            return Err(RegisterError2::AlreadyRegistered(address))
        }

        let rate_commit =
            poseidon_hash(&[id_commitment, Fr::from(u64::from(self.rate_limit))]);

        let mut guard = self.merkle_trees.write().await;

        let found = guard
            .iter_mut()
            .enumerate()
            .find(|(_, tree)| tree.leaves_set() < tree.capacity());

        let (last_tree_index, last_index_in_mt) =
            if let Some((tree_index, tree_to_set)) = found {
                // Found a tree that can accept our new user
                let index_in_mt = tree_to_set.leaves_set();
                tree_to_set
                    .set(index_in_mt, rate_commit)
                    .await
                    .map_err(RegisterError2::TreeError)?;

                (tree_index, index_in_mt)

            } else {

                // All trees are full, let's create a new one that can accept our new user

                // as safe : assume sizeof usize == sizeof 64 (see user_db_types.rs)
                let tree_count = guard.len() as u64;

                if tree_count == self.config.max_tree_count {
                    return Err(RegisterError2::TooManyUsers);
                }

                let persistent_db_config = PersistentDbConfig {
                    db_conn: self.db.clone(),
                    tree_index: tree_count as i16, // FIXME: as
                    insert_batch_size: 10_000, // TODO: no hardcoded value
                };

                let mut mt = ProverMerkleTree::new(
                    self.config.tree_depth as usize,
                    MemoryDbConfig,
                    persistent_db_config.clone()
                ).await.unwrap();

                mt.set(0, rate_commit).await.map_err(RegisterError2::TreeError)?;

                guard.push(mt);

                (tree_count as usize, 0)
            };

        drop(guard);

        let txn = self.db.begin().await?;

        // TODO: unwrap safe?
        let user_active_model = user::ActiveModel {
            address: Set(address.to_string()),
            rln_id: Set(serde_json::to_value(rln_identity).unwrap()),
            tree_index: Set(last_tree_index as i64),
            index_in_merkle_tree: Set(last_index_in_mt as i64), // FIXME
            ..Default::default()
        };

        user::Entity::insert(user_active_model).exec(&txn).await?;

        let tx_counter_active_model = tx_counter::ActiveModel {
            address: Set(address.to_string()),
            ..Default::default()
        };

        tx_counter::Entity::insert(tx_counter_active_model).exec(&txn).await?;

        txn.commit().await?;

        Ok(id_commitment)
    }

    async fn remove_user(&self, address: &Address) -> Result<bool, MerkleTreeError> {

        let user = self.get_user(address).await
            .map_err(|e| MerkleTreeError::PDb(e.into()))?;

        if user.is_none() {
            // User not found (User not registered)
            return Ok(false);
        }

        let user = user.unwrap(); // Unwrap safe: just checked above
        let tree_index = user.tree_index as usize;
        let index_in_merkle_tree = user.index_in_merkle_tree as usize;

        let mut guard = self.merkle_trees.write().await;
        // FIXME: unwrap safe?
        let mt = guard.get_mut(tree_index).unwrap();
        // Only delete it if this is the last index
        // Note: No reuse of index in PmTree (as this is a generic impl and could lead to security issue:
        // like replay attack...)
        if mt.leaves_set().saturating_sub(1) == index_in_merkle_tree {
            mt.delete(index_in_merkle_tree).await?;
        } else {
            // FIXME
            println!("Not the last {} {}", index_in_merkle_tree, mt.leaves_set());
        }

        // TODO: delete in merkle tree in txn
        // FIXME: map_err repetitions?
        let txn = self.db.begin().await
            .map_err(|e| MerkleTreeError::PDb(e.into()))?;
        user::Entity::delete_many()
            .filter(user::Column::Address.eq(address.to_string()))
            .exec(&txn)
            .await
            .map_err(|e| MerkleTreeError::PDb(e.into()))?;
        tx_counter::Entity::delete_many()
            .filter(tx_counter::Column::Address.eq(address.to_string()))
            .exec(&txn)
            .await
            .map_err(|e| MerkleTreeError::PDb(e.into()))?;
        txn.commit().await
            .map_err(|e| MerkleTreeError::PDb(e.into()))?;

        Ok(true)
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
            .await?;

        if has_user {
            let epoch_slice_counter = self.incr_tx_counter(address, incr_value).await?;
            Ok(epoch_slice_counter)
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

// Test only functions
#[cfg(test)]
impl UserDb2 {

    pub(crate) async fn get_db_tree_count(&self) -> Result<u64, DbErr> {
        Self::get_merkle_tree_count_from_db(&self.db).await
    }

    pub(crate) async fn get_vec_tree_count(&self) -> usize {
        self.merkle_trees.read().await.len()
    }

    pub(crate) async fn get_user_indexes(&self, address: &Address) -> (i64, i64) {

        let user_model = self.get_user(address).await
            .unwrap().unwrap();

        (user_model.tree_index, user_model.index_in_merkle_tree)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ProverPoseidonHash;

impl Hasher for ProverPoseidonHash {
    type Fr = Fr;

    fn serialize(value: Self::Fr) -> Value {
        let mut buffer = vec![];
        // FIXME: unwrap safe?
        value.serialize_compressed(&mut buffer).unwrap();
        buffer
    }

    fn deserialize(value: Value) -> Self::Fr {
        // FIXME: unwrap safe?
        CanonicalDeserialize::deserialize_compressed(value.as_slice()).unwrap()
    }

    fn default_leaf() -> Self::Fr {
        Self::Fr::from(0)
    }
    fn hash(inputs: &[Self::Fr]) -> Self::Fr {
        poseidon_hash(inputs)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum MerkleTreeError {
    #[error(transparent)]
    PmtreeError(#[from] PmtreeErrorKind),
    #[error(transparent)]
    PDb(#[from] PersistentDbError),
}

#[cfg(test)]
mod tests {
    use super::*;
    // std
    // third-party
    use alloy::primitives::{address, U256};
    use async_trait::async_trait;
    use claims::assert_matches;
    use derive_more::Display;
    use sea_orm::{ConnectionTrait, Database, Statement};
    use tracing_test::traced_test;
    // internal
    use prover_db_migration::{Migrator as MigratorCreate, MigratorTrait};

    #[derive(Debug, Display, thiserror::Error)]
    struct DummyError();

    struct MockKarmaSc {}

    #[async_trait]
    impl KarmaAmountExt for MockKarmaSc {
        type Error = DummyError;

        async fn karma_amount(&self, _address: &Address) -> Result<U256, Self::Error> {
            Ok(U256::from(10))
        }
    }

    const ADDR_1: Address = address!("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    const ADDR_2: Address = address!("0xb20a608c624Ca5003905aA834De7156C68b2E1d0");
    pub(crate) const MERKLE_TREE_HEIGHT: u8 = 20;

    async fn create_database_connection(db_name: &str) -> Result<DatabaseConnection, DbErr> {

        // Drop / Create db_name then return a connection to it

        let db_url_base = "postgres://myuser:mysecretpassword@localhost";
        let db_url = format!("{}/{}", db_url_base, "mydatabase");
        let db = Database::connect(db_url)
            .await
            .expect("Database connection 0 failed");

        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            format!("DROP DATABASE IF EXISTS \"{}\";", db_name),
        ))
        .await?;
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            format!("CREATE DATABASE \"{}\";", db_name),
        ))
        .await?;

        db.close().await?;

        let db_url_final = format!("{}/{}", db_url_base, db_name);
        let db = Database::connect(db_url_final)
            .await
            .expect("Database connection failed");
        MigratorCreate::up(&db, None).await?;

        Ok(db)
    }

    #[tokio::test]
    // #[traced_test]
    async fn test_user_register() {

        // tracing_subscriber::fmt()
        //     .with_max_level(tracing::Level::DEBUG)
        //     .with_test_writer()
        //     .init();

        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDb2Config {
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };
        let db_conn = create_database_connection("user_db_test_user_register")
            .await
            .unwrap();

        let user_db = UserDb2::new(db_conn, config, epoch_store, Default::default(), Default::default())
            .await
            .expect("Cannot create UserDb");

        let addr = Address::new([0; 20]);
        user_db.register_user(addr).await.unwrap();
        assert_matches!(
            user_db.register_user(addr).await,
            Err(RegisterError2::AlreadyRegistered(_))
        );

        assert!(user_db.get_user_identity(&addr).await.is_some());
        assert_eq!(user_db.get_tx_counter(&addr).await.unwrap(), (0.into(), 0.into()));

        assert!(user_db.get_user_identity(&ADDR_1).await.is_none());
        user_db.register_user(ADDR_1).await.unwrap();
        assert!(user_db.get_user_identity(&ADDR_1).await.is_some());
        assert_eq!(user_db.get_tx_counter(&addr).await.unwrap(), (0.into(), 0.into()));

        user_db.incr_tx_counter(&addr, Some(42)).await.unwrap();
        assert_eq!(
            user_db.get_tx_counter(&addr).await.unwrap(),
            (42.into(), 42.into())
        );
    }

    #[tokio::test]
    async fn test_get_tx_counter() {
        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDb2Config {
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };
        let db_conn = create_database_connection("user_db_test_tx_counter")
            .await
            .unwrap();

        let user_db = UserDb2::new(db_conn, config, epoch_store, Default::default(), Default::default())
            .await
            .expect("Cannot create UserDb");

        let addr = Address::new([0; 20]);

        user_db.register_user(addr).await.unwrap();

        let (ec, ecs) = user_db.get_tx_counter(&addr).await.unwrap();
        assert_eq!(ec, 0u64.into());
        assert_eq!(ecs, EpochSliceCounter::from(0u64));

        let ecs_2 = user_db.incr_tx_counter(&addr, Some(42)).await.unwrap();
        // TODO
        assert_eq!(ecs_2, EpochSliceCounter::from(42));
    }

    #[tokio::test]
    async fn test_incr_tx_counter() {

        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDb2Config {
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };
        let db_conn = create_database_connection("user_db_test_incr_tx_counter")
            .await
            .unwrap();

        let user_db = UserDb2::new(db_conn, config, epoch_store, Default::default(), Default::default())
            .await
            .expect("Cannot create UserDb");

        let addr = Address::new([0; 20]);

        // Try to update tx counter without registering first
        assert_matches!(
            user_db.on_new_tx(&addr, None).await,
            Err(TxCounterError2::NotRegistered(_))
        );

        let tier_info = user_db.user_tier_info(&addr, &MockKarmaSc {}).await;
        // User is not registered -> no tier info
        assert!(matches!(
            tier_info,
            Err(UserTierInfoError2::NotRegistered(_))
        ));
        // Register user
        user_db.register_user(addr).await.unwrap();
        // Now update user tx counter
        assert_eq!(
             user_db.on_new_tx(&addr, None).await,
             Ok(EpochSliceCounter::from(1))
        );
        let tier_info = user_db
            .user_tier_info(&addr, &MockKarmaSc {})
            .await
            .unwrap();
        assert_eq!(tier_info.epoch_tx_count, 1);
        assert_eq!(tier_info.epoch_slice_tx_count, 1);
    }

    #[tokio::test]
    async fn test_user_remove() {

        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDb2Config {
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: crate::user_db::MERKLE_TREE_HEIGHT,
        };
        let db_conn = create_database_connection("user_db_test_user_remove")
            .await
            .unwrap();

        let user_db = UserDb2::new(db_conn, config, epoch_store, Default::default(), Default::default())
            .await
            .expect("Cannot create UserDb");

        user_db.register_user(ADDR_1).await.unwrap();
        let guard = user_db.merkle_trees.read().await;
        let mtree_index_add_addr_1 = guard[0].leaves_set();
        // Note: need to drop read guard before registering user as register_user tries to acquire
        // write lock on merkle trees (and will wait indefinitely if a read lock is held)
        drop(guard);
        user_db.register_user(ADDR_2).await.unwrap();
        let guard = user_db.merkle_trees.read().await;
        let mtree_index_add_addr_2 = guard[0].leaves_set();
        drop(guard);
        assert_ne!(mtree_index_add_addr_1, mtree_index_add_addr_2);
        println!("index addr 1: {}", mtree_index_add_addr_1);
        println!("index addr 2: {}", mtree_index_add_addr_2);

        user_db.remove_user(&ADDR_2).await.unwrap();
        let guard = user_db.merkle_trees.read().await;
        let mtree_index_after_rm_addr_2 = guard[0].leaves_set();
        drop(guard);
        assert_eq!(user_db.has_user(&ADDR_1).await, Ok(true));
        assert_eq!(user_db.has_user(&ADDR_2).await, Ok(false));
        // No reuse of index in PmTree (as this is a generic impl and could lead to security issue:
        // like replay attack...)
        assert_eq!(mtree_index_after_rm_addr_2, mtree_index_add_addr_2);
    }

    #[tokio::test]
    // #[traced_test]
    async fn test_user_reg_merkle_tree_fail() {
        // Try to register some users but init UserDb so the merkle tree write will fail (after 1st register)
        // This tests ensures that the DB and the MerkleTree stays in sync

        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDb2Config {
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: 1,
        };
        let db_conn = create_database_connection("user_db_test_user_reg_merkle_tree_fail")
            .await
            .unwrap();

        let user_db = UserDb2::new(db_conn, config, epoch_store, Default::default(), Default::default())
            .await
            .expect("Cannot create UserDb");

        let addr = Address::new([0; 20]);
        {
            let guard = user_db.merkle_trees.read().await;
            let mt = guard.get(0).unwrap();
            assert_eq!(mt.leaves_set(), 0);
        }
        user_db.register_user(addr).await.unwrap();
        {
            let guard = user_db.merkle_trees.read().await;
            let mt = guard.get(0).unwrap();
            assert_eq!(mt.leaves_set(), 1);
        }
        user_db.register_user(ADDR_1).await.unwrap();
        {
            let guard = user_db.merkle_trees.read().await;
            let mt = guard.get(0).unwrap();
            assert_eq!(mt.leaves_set(), 2);
        }

        let res = user_db.register_user(ADDR_2).await;
        assert_matches!(res, Err(RegisterError2::TooManyUsers));
        assert_eq!(user_db.has_user(&ADDR_1).await, Ok(true));
        assert_eq!(user_db.has_user(&ADDR_2).await, Ok(false));
        {
            let guard = user_db.merkle_trees.read().await;
            let mt = guard.get(0).unwrap();
            assert_eq!(mt.leaves_set(), 2);
        }
    }
}