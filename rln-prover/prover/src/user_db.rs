use std::path::PathBuf;
use std::sync::Arc;
// third-party
use alloy::primitives::{Address, U256};
use ark_bn254::Fr;
use parking_lot::RwLock;
use rocksdb::{
    ColumnFamily, ColumnFamilyDescriptor, DB, Options, ReadOptions, WriteBatch, WriteBatchWithIndex,
};
use tracing::{debug, error};
// Zerokit
use rln::{
    hashers::poseidon_hash,
    pm_tree_adapter::PmtreeConfig,
    poseidon_tree::{MerkleProof, PoseidonTree},
    protocol::keygen,
};
use zerokit_utils::Mode::HighThroughput;
// internal
use crate::epoch_service::{Epoch, EpochSlice};
use crate::error::GetMerkleTreeProofError;
use crate::rocksdb_operands::{
    EpochCounterDeserializer, EpochCounterSerializer, EpochIncr, EpochIncrSerializer,
    epoch_counters_operands, u64_counter_operands,
};
use crate::tier::{TierLimit, TierLimits, TierMatch, TierName};
use crate::user_db_error::{
    DbError, RegisterError, SetTierLimitsError, TxCounterError, UserDbOpenError,
    UserMerkleTreeIndexError, UserTierInfoError,
};
use crate::user_db_serialization::{
    IndexInMerkleTreeDeserializer, IndexInMerkleTreeSerializer, RlnUserIdentityDeserializer,
    RlnUserIdentitySerializer, TierDeserializer, TierLimitsDeserializer, TierLimitsSerializer,
    TreeIndexDeserializer, TreeIndexSerializer, U64Deserializer, U64Serializer,
};
use crate::user_db_types::{
    EpochCounter, EpochSliceCounter, IndexInMerkleTree, RateLimit, TreeIndex,
};
use rln_proof::{RlnUserIdentity, ZerokitMerkleTree};
use smart_contract::KarmaAmountExt;

pub(crate) const MERKLE_TREE_HEIGHT: u8 = 20;
const USER_CF: &str = "user";
// pub const MERKLE_TREE_COUNTER_CF: &str = "mtree";
const INDEX_COUNTERS_CF: &str = "idx";
const TX_COUNTER_CF: &str = "tx_counter";
const TIER_LIMITS_CF: &str = "tier_limits";

// INDEX_COUNTERS_CF keys
const INDEX_COUNTERS_KEY_COUNT: &[u8] = b"COUNT";
// const INDEX_COUNTERS_KEY_TREE_LAST_INDEX: &[u8] = b"LAST";
// const INDEX_COUNTERS_KEY_LAST_INDEX_IN_MT_PREFIX: &[u8] = b"T"; // T0, T1, ..., TN

// TIER_LIMITS_CF keys
const TIER_LIMITS_KEY: &[u8; 7] = b"CURRENT";
const TIER_LIMITS_NEXT_KEY: &[u8; 4] = b"NEXT";

/// TEMP?
// const MERKLE_TREE_COUNT: usize = 4;

#[derive(Debug, PartialEq)]
pub struct UserTierInfo {
    pub(crate) current_epoch: Epoch,
    pub(crate) current_epoch_slice: EpochSlice,
    pub(crate) epoch_tx_count: u64,
    pub(crate) epoch_slice_tx_count: u64,
    pub(crate) karma_amount: U256,
    pub(crate) tier_name: Option<TierName>,
    pub(crate) tier_limit: Option<TierLimit>,
}

#[derive(Clone)]
pub struct UserDbConfig {
    pub(crate) db_path: PathBuf,
    pub(crate) merkle_tree_folder: PathBuf,
    pub(crate) tree_count: u64,
    pub(crate) max_tree_count: u64,
    pub(crate) tree_depth: u8,
}

#[derive(Clone)]
pub(crate) struct UserDb {
    db: Arc<DB>,
    merkle_tree: Arc<RwLock<Vec<PoseidonTree>>>,
    config: UserDbConfig,
    rate_limit: RateLimit,
    pub(crate) epoch_store: Arc<RwLock<(Epoch, EpochSlice)>>,
    rln_identity_serializer: RlnUserIdentitySerializer,
    rln_identity_deserializer: RlnUserIdentityDeserializer,
    // tree_count_deserializer: U64Deserializer,
    tree_index_serializer: TreeIndexSerializer,
    tree_index_deserializer: TreeIndexDeserializer,
    index_in_merkle_tree_serializer: IndexInMerkleTreeSerializer,
    index_in_merkle_tree_deserializer: IndexInMerkleTreeDeserializer,
    epoch_increase_serializer: EpochIncrSerializer,
    epoch_counter_deserializer: EpochCounterDeserializer,
    tier_limits_serializer: TierLimitsSerializer,
    tier_limits_deserializer: TierLimitsDeserializer,
}

impl std::fmt::Debug for UserDb {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("UserDb")
            .field("db", &self.db)
            .field("rate limit", &self.rate_limit)
            .field("epoch store", &self.epoch_store)
            .finish()
    }
}

impl UserDb {
    /// Returns a new `UserDB` instance
    pub fn new(
        config: UserDbConfig,
        epoch_store: Arc<RwLock<(Epoch, EpochSlice)>>,
        tier_limits: TierLimits,
        rate_limit: RateLimit,
    ) -> Result<Self, UserDbOpenError> {
        debug_assert!(config.merkle_tree_folder.is_dir());
        debug_assert!(config.tree_count <= config.max_tree_count);

        let db_options = {
            let mut db_opts = Options::default();
            db_opts.set_max_open_files(820);
            db_opts.create_if_missing(true);
            db_opts.create_missing_column_families(true);
            db_opts
        };

        let mut tx_counter_cf_opts = Options::default();
        tx_counter_cf_opts
            .set_merge_operator_associative("counters operator", epoch_counters_operands);
        let mut user_mtree_cf_opts = Options::default();
        user_mtree_cf_opts.set_merge_operator_associative("counter operator", u64_counter_operands);

        let db = DB::open_cf_descriptors(
            &db_options,
            &config.db_path,
            vec![
                // Db column for users, key: User address, value: RlnUserIdentity + Merkle trees index + Index in Merkle tree
                ColumnFamilyDescriptor::new(USER_CF, Options::default()),
                // Db Column for:
                // * Trees count - key: INDEX_COUNTERS_KEY_COUNT
                ColumnFamilyDescriptor::new(INDEX_COUNTERS_CF, user_mtree_cf_opts),
                // Db column for user tx counters, key: User address, value: EpochCounters
                ColumnFamilyDescriptor::new(TX_COUNTER_CF, tx_counter_cf_opts),
                // Db column for tier limits - key: current && next, value: TierLimits
                // Note: only 2 keys in this column
                ColumnFamilyDescriptor::new(TIER_LIMITS_CF, Options::default()),
            ],
        )?;

        debug_assert!(tier_limits.validate().is_ok());
        let tier_limits_serializer = TierLimitsSerializer::default();
        let mut buffer = Vec::with_capacity(tier_limits_serializer.size_hint(tier_limits.len()));
        tier_limits_serializer.serialize(&tier_limits, &mut buffer)?;

        // unwrap safe - db is always created with this column
        let cf = db.cf_handle(TIER_LIMITS_CF).unwrap();
        db.delete_cf(cf, TIER_LIMITS_NEXT_KEY.as_slice())?;
        db.put_cf(cf, TIER_LIMITS_KEY.as_slice(), buffer)?;

        let db = Arc::new(db);

        // index column

        let cf_index = db.cf_handle(INDEX_COUNTERS_CF).unwrap();
        let tree_count_serializer = U64Serializer {};
        let tree_count_deserializer = U64Deserializer {};
        let tree_count = if let Ok(tree_count) =
            Self::get_tree_count_(db.clone(), cf_index, &tree_count_deserializer)
        {
            tree_count
        } else {
            let mut buffer = Vec::with_capacity(tree_count_serializer.size_hint());
            tree_count_serializer.serialize(&config.tree_count, &mut buffer);
            db.merge_cf(cf_index, INDEX_COUNTERS_KEY_COUNT, &buffer)?;
            config.tree_count
        };

        // merkle tree
        let trees = (0..tree_count)
            .map(|i| {
                let tree_folder = config.merkle_tree_folder.join(format!("tree_{}", i));

                if !tree_folder.exists() {
                    debug!(
                        "New - Creating merkle tree folder: {}",
                        tree_folder.display()
                    );
                    std::fs::create_dir_all(&tree_folder)?;
                }

                let tree_config = PmtreeConfig::builder()
                    .path(tree_folder)
                    .temporary(false)
                    .cache_capacity(100_000)
                    .flush_every_ms(12_000)
                    .mode(HighThroughput)
                    .use_compression(false)
                    .build()?;

                Ok(PoseidonTree::new(
                    usize::from(config.tree_depth),
                    Default::default(),
                    tree_config.clone(),
                )?)
            })
            .collect::<Result<Vec<_>, UserDbOpenError>>()?;

        let tier_limits_deserializer = TierLimitsDeserializer {
            tier_deserializer: TierDeserializer {},
        };

        Ok(Self {
            db,
            merkle_tree: Arc::new(RwLock::new(trees)),
            config,
            rate_limit,
            epoch_store,
            rln_identity_serializer: RlnUserIdentitySerializer {},
            rln_identity_deserializer: RlnUserIdentityDeserializer {},
            // tree_count_deserializer: U64Deserializer {},
            tree_index_serializer: TreeIndexSerializer {},
            tree_index_deserializer: TreeIndexDeserializer {},
            index_in_merkle_tree_serializer: IndexInMerkleTreeSerializer {},
            index_in_merkle_tree_deserializer: IndexInMerkleTreeDeserializer {},
            epoch_increase_serializer: EpochIncrSerializer {},
            epoch_counter_deserializer: EpochCounterDeserializer {},
            tier_limits_serializer,
            tier_limits_deserializer,
        })
    }

    fn get_user_cf(&self) -> &ColumnFamily {
        // unwrap safe - db is always created with this column
        self.db.cf_handle(USER_CF).unwrap()
    }

    fn get_index_cf(&self) -> &ColumnFamily {
        // unwrap safe - db is always created with this column
        self.db.cf_handle(INDEX_COUNTERS_CF).unwrap()
    }

    fn get_counter_cf(&self) -> &ColumnFamily {
        // unwrap safe - db is always created with this column
        self.db.cf_handle(TX_COUNTER_CF).unwrap()
    }

    fn get_tier_limits_cf(&self) -> &ColumnFamily {
        // unwrap safe - db is always created with this column
        self.db.cf_handle(TIER_LIMITS_CF).unwrap()
    }

    pub(crate) fn register(&self, address: Address) -> Result<Fr, RegisterError> {
        // Generate RLN identity
        let (identity_secret_hash, id_commitment) = keygen();

        let rln_identity = RlnUserIdentity::from((
            id_commitment,
            identity_secret_hash,
            Fr::from(self.rate_limit),
        ));

        // Column to store the user
        let cf_user = self.get_user_cf();
        // Setup Key & Value to insert in RocksDB
        let key = address.as_slice();
        let mut buffer = vec![
            0;
            self.rln_identity_serializer.size_hint()
                + self.tree_index_serializer.size_hint()
                + self.index_in_merkle_tree_serializer.size_hint()
        ];

        let _index = match self.db.get_cf(cf_user, key) {
            Ok(Some(_)) => {
                return Err(RegisterError::AlreadyRegistered(address));
            }
            Err(e) => {
                return Err(RegisterError::Db(e));
            }
            Ok(None) => {
                let cf_index = self.get_index_cf();
                let cf_counter = self.get_counter_cf();

                let rate_commit =
                    poseidon_hash(&[id_commitment, Fr::from(u64::from(self.rate_limit))]);

                let mut db_batch = WriteBatchWithIndex::new(1024, true);

                // Note: write to Merkle tree in the Db transaction, so if the write fails
                //       the Db transaction will also fail

                let mut merkle_tree_guard = self.merkle_tree.write();

                let found = merkle_tree_guard
                    .iter_mut()
                    .enumerate()
                    .find(|(_, tree)| tree.leaves_set() < tree.capacity());

                let (last_tree_index, last_index_in_mt) =
                    if let Some((tree_index, tree_to_set)) = found {
                        // Found a tree that can accept our new user

                        let index_in_mt = tree_to_set.leaves_set();
                        tree_to_set
                            .set(index_in_mt, rate_commit)
                            // .map_err(|e| RegisterError::TreeError(e.to_string()))?;
                            .map_err(RegisterError::TreeError)?;

                        (tree_index, index_in_mt)
                    } else {
                        // All trees are full, let's create a new one that can accept our new user

                        // as safe : assume sizeof usize == sizeof 64 (see user_db_types.rs)
                        let tree_count = merkle_tree_guard.len() as u64;

                        if tree_count == self.config.max_tree_count {
                            return Err(RegisterError::TooManyUsers);
                        }

                        let tree_folder = self
                            .config
                            .merkle_tree_folder
                            .join(format!("tree_{}", tree_count));
                        if !tree_folder.exists() {
                            debug!("Creating merkle tree folder: {}", tree_folder.display());
                            // Note: Only we create_dir can happen (as we have a write lock)
                            std::fs::create_dir(&tree_folder)?;
                        }

                        // TODO: config value as args?
                        let tree_config = PmtreeConfig::builder()
                            .path(tree_folder)
                            .temporary(false)
                            .cache_capacity(100_000)
                            .flush_every_ms(12_000)
                            .mode(HighThroughput)
                            .use_compression(false)
                            .build()?;

                        let mut tree = PoseidonTree::new(
                            usize::from(self.config.tree_depth),
                            Default::default(),
                            tree_config,
                        )
                        .unwrap();
                        tree.set(0, rate_commit).map_err(RegisterError::TreeError)?;
                        let tree_index = merkle_tree_guard.len();
                        merkle_tree_guard.push(tree);

                        // Update tree count in rocksdb
                        db_batch.merge_cf(cf_index, INDEX_COUNTERS_KEY_COUNT, 1i64.to_le_bytes());

                        (tree_index, 0)
                    };

                // Drop write guard as soon as possible
                drop(merkle_tree_guard);

                // Value
                // unwrap safe - this is serialized by the Prover + RlnUserIdentitySerializer is unit tested
                self.rln_identity_serializer
                    .serialize(&rln_identity, &mut buffer)
                    .unwrap();
                self.tree_index_serializer
                    .serialize(&TreeIndex::from(last_tree_index as u64), &mut buffer);
                self.index_in_merkle_tree_serializer.serialize(
                    &IndexInMerkleTree::from(last_index_in_mt as u64),
                    &mut buffer,
                );

                // Put user
                db_batch.put_cf(cf_user, key, buffer.as_slice());
                // Put user tx counter
                db_batch.put_cf(
                    cf_counter,
                    key,
                    EpochCounterSerializer::default().as_slice(),
                );

                self.db.write_wbwi(&db_batch).map_err(RegisterError::Db)?;

                (last_tree_index, last_index_in_mt)
            }
        };

        Ok(id_commitment)
    }

    pub(crate) fn has_user(&self, address: &Address) -> Result<bool, rocksdb::Error> {
        let cf_user = self.get_user_cf();
        self.db
            .get_pinned_cf(cf_user, address.as_slice())
            .map(|value| value.is_some())
    }

    pub fn get_user(&self, address: &Address) -> Option<RlnUserIdentity> {
        let cf_user = self.get_user_cf();
        match self.db.get_pinned_cf(cf_user, address.as_slice()) {
            Ok(Some(value)) => {
                // Here we silence the error - this is safe as the prover controls this
                self.rln_identity_deserializer.deserialize(&value).ok()
            }
            Ok(None) => None,
            Err(_e) => None,
        }
    }

    /// Return the tree index + index in the merkle tree for a user address
    pub fn get_user_indexes(
        &self,
        address: &Address,
    ) -> Result<(TreeIndex, IndexInMerkleTree), UserMerkleTreeIndexError> {
        let cf_user = self.get_user_cf();
        match self.db.get_pinned_cf(cf_user, address.as_slice()) {
            Ok(Some(buffer)) => {
                // Here we silence the error - this is safe as the prover controls this
                let start = self.rln_identity_serializer.size_hint();

                let (rem, tree_index) = self
                    .tree_index_deserializer
                    .deserialize(&buffer[start..])
                    .unwrap();

                let (_, index_in_mt) = self
                    .index_in_merkle_tree_deserializer
                    .deserialize(rem)
                    .unwrap();

                Ok((tree_index, index_in_mt))
            }
            Ok(None) => Err(UserMerkleTreeIndexError::NotRegistered(*address)),
            Err(e) => Err(UserMerkleTreeIndexError::Db(e)),
        }
    }

    /// Remove user
    ///
    /// Warning: DO NOT use this function if the user is registered in a smart contract
    ///          This function is intended to be used if registration to the smart contract fails
    pub(crate) fn remove_user(&self, address: &Address, sc_registered: bool) -> bool {
        let cf_user = self.get_user_cf();
        let cf_counter = self.get_counter_cf();

        let mut db_batch = WriteBatch::new();

        if !sc_registered {
            let user_indexes = self.get_user_indexes(address);
            let (tree_index, index_in_mt) = match user_indexes {
                Ok(user_index) => user_index,
                Err(UserMerkleTreeIndexError::NotRegistered(_)) => {
                    return true;
                }
                _ => {
                    error!("Error getting user index: {:?}", user_indexes);
                    return false;
                }
            };

            let tree_idx = usize::from(tree_index);
            let idx_in_mt = usize::from(index_in_mt);
            if usize::from(index_in_mt) == self.merkle_tree.read()[tree_idx].leaves_set() {
                // Only delete it if this is the last index
                if let Err(e) = self.merkle_tree.write()[tree_idx].delete(idx_in_mt) {
                    error!("Error deleting user in merkle tree: {:?}", e);
                    return false;
                }
            }
        }

        // Remove user
        db_batch.delete_cf(cf_user, address.as_slice());
        // Remove user tx counter
        db_batch.delete_cf(cf_counter, address.as_slice());
        self.db.write(db_batch).unwrap();

        true
    }

    fn incr_tx_counter(
        &self,
        address: &Address,
        incr_value: Option<u64>,
    ) -> Result<EpochSliceCounter, TxCounterError> {
        let incr_value = incr_value.unwrap_or(1);
        let cf_counter = self.get_counter_cf();

        let (epoch, epoch_slice) = *self.epoch_store.read();
        let incr = EpochIncr {
            epoch,
            epoch_slice,
            incr_value,
        };
        let mut buffer = Vec::with_capacity(self.epoch_increase_serializer.size_hint());
        self.epoch_increase_serializer.serialize(&incr, &mut buffer);

        // Create a transaction
        // By using a WriteBatchWithIndex, we can "read your own writes" so here we incr then read the new value
        // https://rocksdb.org/blog/2015/02/27/write-batch-with-index.html
        let mut batch = WriteBatchWithIndex::new(buffer.len() + size_of::<Address>(), true);
        batch.merge_cf(cf_counter, address.as_slice(), buffer);
        let res = batch.get_from_batch_and_db_cf(
            &*self.db,
            cf_counter,
            address.as_slice(),
            &ReadOptions::default(),
        )?;
        self.db.write_wbwi(&batch).map_err(TxCounterError::Db)?;
        let (_, epoch_slice_counter) = self.counters_from_key(address, res)?;

        Ok(epoch_slice_counter)
    }

    pub(crate) fn get_tx_counter(
        &self,
        address: &Address,
    ) -> Result<(EpochCounter, EpochSliceCounter), TxCounterError> {
        let cf_counter = self.get_counter_cf();
        match self.db.get_cf(cf_counter, address.as_slice()) {
            Ok(v) => self.counters_from_key(address, v),
            Err(e) => Err(TxCounterError::Db(e)),
        }
    }

    fn counters_from_key(
        &self,
        address: &Address,
        key: Option<Vec<u8>>,
    ) -> Result<(EpochCounter, EpochSliceCounter), TxCounterError> {
        match key {
            Some(value) => {
                let (_, counter) = self.epoch_counter_deserializer.deserialize(&value).unwrap();
                let (epoch, epoch_slice) = *self.epoch_store.read();

                let cmp = (counter.epoch == epoch, counter.epoch_slice == epoch_slice);

                match cmp {
                    (true, true) => {
                        // EpochCounter stored in DB == epoch store
                        // We query for an epoch / epoch slice and this is what is stored in the Db
                        // Return the counters
                        Ok((
                            counter.epoch_counter.into(),
                            counter.epoch_slice_counter.into(),
                        ))
                    }
                    (true, false) => {
                        // EpochCounter.epoch_slice (stored in Db) != epoch_store.epoch_slice
                        // We query for an epoch slice after what is stored in Db
                        // This can happen if no Tx has updated the epoch slice counter (yet)
                        Ok((counter.epoch_counter.into(), EpochSliceCounter::from(0)))
                    }
                    (false, true) => {
                        // EpochCounter.epoch (stored in DB) != epoch_store.epoch
                        // We query for an epoch after what is stored in Db
                        // This can happen if no Tx has updated the epoch counter (yet)
                        Ok((EpochCounter::from(0), EpochSliceCounter::from(0)))
                    }
                    (false, false) => {
                        // EpochCounter (stored in DB) != epoch_store
                        // Outdated value (both for epoch & epoch slice)
                        Ok((EpochCounter::from(0), EpochSliceCounter::from(0)))
                    }
                }
            }
            None => Err(TxCounterError::NotRegistered(*address)),
        }
    }

    // pub

    pub(crate) fn on_new_epoch(&self) {}

    pub(crate) fn on_new_epoch_slice(&self) {}

    pub fn on_new_user(&self, address: &Address) -> Result<Fr, RegisterError> {
        self.register(*address)
    }

    pub fn get_merkle_proof(
        &self,
        address: &Address,
    ) -> Result<MerkleProof, GetMerkleTreeProofError> {
        let (tree_index, index_in_mt) = self
            .get_user_indexes(address)
            .map_err(GetMerkleTreeProofError::MerkleTree)?;

        // let tree_index: usize = tree_index.into();
        self.merkle_tree.read()[usize::from(tree_index)]
            .proof(index_in_mt.into())
            .map_err(|e| GetMerkleTreeProofError::TreeError(e.to_string()))
    }

    pub(crate) fn on_new_tx(
        &self,
        address: &Address,
        incr_value: Option<u64>,
    ) -> Result<EpochSliceCounter, TxCounterError> {
        let has_user = self.has_user(address).map_err(TxCounterError::Db)?;

        if has_user {
            self.incr_tx_counter(address, incr_value)
        } else {
            Err(TxCounterError::NotRegistered(*address))
        }
    }

    fn get_tier_limits(&self) -> Result<TierLimits, rocksdb::Error> {
        let cf = self.get_tier_limits_cf();
        // Unwrap safe - Db is initialized with valid tier limits
        let buffer = self.db.get_cf(cf, TIER_LIMITS_KEY.as_slice())?.unwrap();
        // Unwrap safe - serialized by the prover (should always deserialize)
        let (_, tier_limits) = self.tier_limits_deserializer.deserialize(&buffer).unwrap();
        Ok(tier_limits)
    }

    pub(crate) fn on_tier_limits_updated(
        &self,
        tier_limits: TierLimits,
    ) -> Result<(), SetTierLimitsError> {
        tier_limits.validate()?;

        // Serialize
        let mut buffer =
            Vec::with_capacity(self.tier_limits_serializer.size_hint(tier_limits.len()));
        // Unwrap safe - already validated - should always serialize
        self.tier_limits_serializer
            .serialize(&tier_limits, &mut buffer)
            .unwrap();

        // Write
        let cf = self.get_tier_limits_cf();
        self.db
            .put_cf(cf, TIER_LIMITS_NEXT_KEY.as_slice(), buffer)
            .map_err(SetTierLimitsError::Db)?;

        Ok(())
    }

    /// Get user tier info
    pub(crate) async fn user_tier_info<E: std::error::Error, KSC: KarmaAmountExt<Error = E>>(
        &self,
        address: &Address,
        karma_sc: &KSC,
    ) -> Result<UserTierInfo, UserTierInfoError<E>> {
        let has_user = self.has_user(address).map_err(UserTierInfoError::Db)?;

        if !has_user {
            return Err(UserTierInfoError::NotRegistered(*address));
        }

        let (epoch_tx_count, epoch_slice_tx_count) = self.get_tx_counter(address)?;

        let karma_amount = karma_sc
            .karma_amount(address)
            .await
            .map_err(|e| UserTierInfoError::Contract(e))?;

        let tier_limits = self.get_tier_limits()?;
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

    /// Return the tree count (e.g. number of merkle trees used) stored in the db
    fn get_tree_count_(
        db: Arc<DB>,
        cf: &ColumnFamily,
        tree_count_deserializer: &U64Deserializer,
    ) -> Result<u64, DbError> {
        match db.get_pinned_cf(cf, INDEX_COUNTERS_KEY_COUNT) {
            Ok(Some(v)) => {
                let (_rem, tree_count) = tree_count_deserializer.deserialize(v.as_ref()).unwrap();
                Ok(tree_count)
            }
            Ok(None) => Err(DbError::DbUninitialized),
            Err(e) => Err(DbError::Db(e)),
        }
    }
}

// Test functions
#[cfg(test)]
impl UserDb {
    /// See `get_tree_count_`
    pub(crate) fn get_tree_count(&self) -> Result<u64, DbError> {
        let de = U64Deserializer {};
        Self::get_tree_count_(self.db.clone(), self.get_index_cf(), &de)
    }

    #[allow(dead_code)]
    pub(crate) fn set_merkle_trees(&mut self, trees: Arc<RwLock<Vec<PoseidonTree>>>) {
        self.merkle_tree = trees;
    }

    /// Get the tree count by getting the length of self.merkle_tree
    pub(crate) fn tree_count(&self) -> usize {
        self.merkle_tree.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // std
    // third-party
    use alloy::primitives::address;
    use async_trait::async_trait;
    use claims::assert_matches;
    use derive_more::Display;
    use tracing_test::traced_test;

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

    #[test]
    fn test_user_register() {
        let temp_folder = tempfile::tempdir().unwrap();
        let temp_folder_tree = tempfile::tempdir().unwrap();
        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDbConfig {
            db_path: PathBuf::from(temp_folder.path()),
            merkle_tree_folder: PathBuf::from(temp_folder_tree.path()),
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };
        let user_db = UserDb::new(config, epoch_store, Default::default(), Default::default())
            .expect("Cannot create UserDb");

        let addr = Address::new([0; 20]);
        user_db.register(addr).unwrap();
        assert_matches!(
            user_db.register(addr),
            Err(RegisterError::AlreadyRegistered(_))
        );

        assert!(user_db.get_user(&addr).is_some());
        assert_eq!(user_db.get_tx_counter(&addr).unwrap(), (0.into(), 0.into()));

        assert!(user_db.get_user(&ADDR_1).is_none());
        user_db.register(ADDR_1).unwrap();

        assert!(user_db.get_user(&ADDR_1).is_some());
        assert_eq!(user_db.get_tx_counter(&addr).unwrap(), (0.into(), 0.into()));
        user_db.incr_tx_counter(&addr, Some(42)).unwrap();
        assert_eq!(
            user_db.get_tx_counter(&addr).unwrap(),
            (42.into(), 42.into())
        );
    }

    #[test]
    fn test_get_tx_counter() {
        let temp_folder = tempfile::tempdir().unwrap();
        let temp_folder_tree = tempfile::tempdir().unwrap();
        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDbConfig {
            db_path: PathBuf::from(temp_folder.path()),
            merkle_tree_folder: PathBuf::from(temp_folder_tree.path()),
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };
        let user_db =
            UserDb::new(config, epoch_store, Default::default(), Default::default()).unwrap();

        let addr = Address::new([0; 20]);

        user_db.register(addr).unwrap();

        let (ec, ecs) = user_db.get_tx_counter(&addr).unwrap();
        assert_eq!(ec, 0u64.into());
        assert_eq!(ecs, EpochSliceCounter::from(0u64));

        let ecs_2 = user_db.incr_tx_counter(&addr, Some(42)).unwrap();
        assert_eq!(ecs_2, EpochSliceCounter::from(42));
    }

    #[tokio::test]
    async fn test_incr_tx_counter() {
        let temp_folder = tempfile::tempdir().unwrap();
        let temp_folder_tree = tempfile::tempdir().unwrap();
        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDbConfig {
            db_path: PathBuf::from(temp_folder.path()),
            merkle_tree_folder: PathBuf::from(temp_folder_tree.path()),
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };
        let user_db =
            UserDb::new(config, epoch_store, Default::default(), Default::default()).unwrap();

        let addr = Address::new([0; 20]);

        // Try to update tx counter without registering first
        assert_matches!(
            user_db.on_new_tx(&addr, None),
            Err(TxCounterError::NotRegistered(_))
        );

        let tier_info = user_db.user_tier_info(&addr, &MockKarmaSc {}).await;
        // User is not registered -> no tier info
        assert!(matches!(
            tier_info,
            Err(UserTierInfoError::NotRegistered(_))
        ));
        // Register user
        user_db.register(addr).unwrap();
        // Now update user tx counter
        assert_eq!(
            user_db.on_new_tx(&addr, None),
            Ok(EpochSliceCounter::from(1))
        );
        let tier_info = user_db
            .user_tier_info(&addr, &MockKarmaSc {})
            .await
            .unwrap();
        assert_eq!(tier_info.epoch_tx_count, 1);
        assert_eq!(tier_info.epoch_slice_tx_count, 1);
    }

    #[test]
    fn test_user_remove() {
        let temp_folder = tempfile::tempdir().unwrap();
        let temp_folder_tree = tempfile::tempdir().unwrap();
        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDbConfig {
            db_path: PathBuf::from(temp_folder.path()),
            merkle_tree_folder: PathBuf::from(temp_folder_tree.path()),
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };
        let user_db = UserDb::new(
            config,
            epoch_store.clone(),
            Default::default(),
            Default::default(),
        )
        .unwrap();

        user_db.register(ADDR_1).unwrap();
        let mtree_index_add_addr_1 = user_db.merkle_tree.read()[0].leaves_set();
        user_db.register(ADDR_2).unwrap();
        let mtree_index_add_addr_2 = user_db.merkle_tree.read()[0].leaves_set();
        assert_ne!(mtree_index_add_addr_1, mtree_index_add_addr_2);

        user_db.remove_user(&ADDR_2, false);
        let mtree_index_after_rm_addr_2 = user_db.merkle_tree.read()[0].leaves_set();
        assert_eq!(user_db.has_user(&ADDR_1), Ok(true));
        assert_eq!(user_db.has_user(&ADDR_2), Ok(false));
        // No reuse of index in PmTree (as this is a generic impl and could lead to security issue:
        // like replay attack...)
        assert_eq!(mtree_index_after_rm_addr_2, mtree_index_add_addr_2);
    }

    #[test]
    #[traced_test]
    fn test_user_reg_merkle_tree_fail() {
        // Try to register some users but init UserDb so the merkle tree write will fail (after 1st register)
        // This tests ensures that the DB and the MerkleTree stays in sync

        let temp_folder = tempfile::tempdir().unwrap();
        let temp_folder_tree = tempfile::tempdir().unwrap();
        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDbConfig {
            db_path: PathBuf::from(temp_folder.path()),
            merkle_tree_folder: PathBuf::from(temp_folder_tree.path()),
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };

        let mut user_db = UserDb::new(
            config,
            epoch_store.clone(),
            Default::default(),
            Default::default(),
        )
        .unwrap();

        let temp_folder_tree_2 = tempfile::tempdir().unwrap();
        let config = PmtreeConfig::builder()
            .path(temp_folder_tree_2.path().to_path_buf())
            .temporary(false)
            .cache_capacity(100_000)
            .flush_every_ms(12_000)
            .mode(HighThroughput)
            .use_compression(false)
            .build()
            .unwrap();
        let tree = PoseidonTree::new(1, Default::default(), config).unwrap();
        let tree = Arc::new(RwLock::new(vec![tree]));
        user_db.merkle_tree = tree.clone();

        let addr = Address::new([0; 20]);

        assert_eq!(tree.read()[0].leaves_set(), 0);
        user_db.register(addr).unwrap();
        assert_eq!(tree.read()[0].leaves_set(), 1);
        user_db.register(ADDR_1).unwrap();
        assert_eq!(tree.read()[0].leaves_set(), 2);

        let res = user_db.register(ADDR_2);
        assert_matches!(res, Err(RegisterError::TooManyUsers));
        assert_eq!(user_db.has_user(&ADDR_1), Ok(true));
        assert_eq!(user_db.has_user(&ADDR_2), Ok(false));
        assert_eq!(tree.read()[0].leaves_set(), 2);
    }
}
