use std::{fmt::Formatter, sync::Arc, time::Duration};
// third-party
use alloy::primitives::{Address, U256};
use ark_bn254::Fr;
use ark_ff::AdditiveGroup;
use chrono::Utc;
use parking_lot::RwLock;
// RLN
use rln::{hashers::poseidon_hash, protocol::keygen};
// sqlx
use sqlx::{
    Decode, Encode, Pool, Row, Type,
    error::Error as SqlxError,
    postgres::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueRef, Postgres, types::Oid},
    types::Json,
};
// internal
use crate::epoch_service::{Epoch, EpochSlice};
use crate::tier::{TierLimit, TierLimits, TierMatch, TierName};
// use crate::user_db::UserTierInfo;
use crate::user_db_2_entities::{
    DenyListSqlx, MerkleProof, MerkleTreeConfigSqlx, PGFR_ARRAY_OID, PGFR_OID, PgFrStruct,
    TierLimitsSqlx, TxCounterSqlx, UserIdSqlx, UserSqlx,
};
use crate::user_db_error::{
    GetMerkleTreeProofError2, RegisterError2, SetTierLimitsError2, TxCounterError2,
    UserTierInfoError2,
};
use crate::user_db_types::{EpochCounter, EpochSliceCounter, RateLimit};
use rln_proof::{ProverPoseidonHash, RlnUserIdentity};
use smart_contract::KarmaAmountExt;

pub const MERKLE_TREE_HEIGHT: u8 = 20;
const TIER_LIMITS_KEY: &str = "CURRENT";
const TIER_LIMITS_NEXT_KEY: &str = "NEXT";

#[derive(Debug, PartialEq)]
pub struct UserTierInfo2 {
    pub(crate) current_epoch: Epoch,
    pub(crate) current_epoch_slice: EpochSlice,
    pub(crate) epoch_tx_count: u64,
    pub(crate) karma_amount: U256,
    pub(crate) tier_name: Option<TierName>,
    pub(crate) tier_limit: Option<TierLimit>,
}

#[derive(Clone, Debug)]
pub struct UserDb2Config {
    pub tree_count: u64,
    pub max_tree_count: u64,
    pub tree_depth: u8,
}

#[derive(Clone)]
pub(crate) struct UserDb2 {
    db: Pool<Postgres>,
    config: UserDb2Config,
    rate_limit: RateLimit,
    pub(crate) epoch_store: Arc<RwLock<(Epoch, EpochSlice)>>,
}

impl std::fmt::Debug for UserDb2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserDb2")
            .field("db", &self.db)
            .field("config", &self.config)
            .field("rate_limit", &self.rate_limit)
            .finish()
    }
}

impl UserDb2 {
    /// Returns a new `UserDB` instance
    pub async fn new(
        db: Pool<Postgres>,
        config: UserDb2Config,
        epoch_store: Arc<RwLock<(Epoch, EpochSlice)>>,
        tier_limits: TierLimits,
        rate_limit: RateLimit,
    ) -> Result<Self, SqlxError> {
        debug_assert!(config.tree_count <= config.max_tree_count);

        //
        let row: (Oid, Oid) =
            sqlx::query_as("SELECT oid, typarray FROM pg_type WHERE typname = 'pgfr'")
                .fetch_one(&db)
                .await?;

        let _ = PGFR_OID.set(row.0);
        let _ = PGFR_ARRAY_OID.set(row.1);

        // tier limits
        debug_assert!(tier_limits.validate().is_ok());

        let res_delete = sqlx::query("DELETE FROM tier_limits WHERE name = $1")
            .bind(TIER_LIMITS_KEY)
            .execute(&db)
            .await?;

        let res_insert = sqlx::query("INSERT INTO tier_limits (name, tier_limits) VALUES ($1, $2)")
            .bind(TIER_LIMITS_KEY)
            .bind(Json(tier_limits))
            .execute(&db)
            .await?;

        debug_assert!(res_insert.rows_affected() == 1);

        Ok(Self {
            db,
            config,
            rate_limit,
            epoch_store,
        })
    }

    // (Internal) Simple Db related methods

    pub(crate) async fn has_user(&self, address: &Address) -> Result<bool, SqlxError> {
        let res: Option<UserIdSqlx> =
            sqlx::query_as("SELECT id FROM users WHERE address=$1 LIMIT 1")
                .bind(address.0.0)
                .fetch_optional(&self.db)
                .await?;

        Ok(res.is_some())
    }

    pub(crate) async fn get_user(&self, address: &Address) -> Result<Option<UserSqlx>, SqlxError> {
        let res: Option<UserSqlx> = sqlx::query_as("SELECT * FROM users WHERE address=$1 LIMIT 1")
            .bind(address.0.0)
            .fetch_optional(&self.db)
            .await?;

        Ok(res)
    }

    pub(crate) async fn get_user_identity(&self, address: &Address) -> Option<RlnUserIdentity> {
        let res = self.get_user(address).await.ok()??;
        Some(res.rln_id.0)
    }

    async fn get_tier_limits(&self) -> Result<TierLimits, SqlxError> {
        let res: Option<TierLimitsSqlx> =
            sqlx::query_as("SELECT * FROM tier_limits WHERE name = $1 LIMIT 1")
                .bind(TIER_LIMITS_KEY)
                .fetch_optional(&self.db)
                .await?;

        Ok(res
            .unwrap() // unwrap safe - db is always initialized with this row (name)
            .tier_limits
            .unwrap() // unwrap safe - db is initialized with valid tier limits
            .0)
    }

    async fn set_tier_limits(&self, tier_limits: TierLimits) -> Result<(), SqlxError> {
        sqlx::query("INSERT INTO tier_limits (name, tier_limits) VALUES ($1, $2) ON CONFLICT (name) DO UPDATE SET tier_limits = EXCLUDED.tier_limits")
            .bind(TIER_LIMITS_NEXT_KEY)
            .bind(Json(tier_limits))
            .execute(&self.db)
            .await?;

        Ok(())
    }

    async fn get_merkle_tree_count_from_db(db: &Pool<Postgres>) -> Result<u64, SqlxError> {
        let res: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM m_tree_config")
            .fetch_one(db)
            .await?;
        Ok(res as u64)
    }

    // internal methods for tx_counter

    async fn incr_tx_counter(
        &self,
        address: &Address,
        incr_value: Option<i64>,
    ) -> Result<EpochCounter, SqlxError> {
        let incr_value = incr_value.unwrap_or(1);
        let (epoch, _epoch_slice) = *self.epoch_store.read();
        tracing::info!(
            address = %address,
            incr_value = incr_value,
            current_epoch = i64::from(epoch),
            "incr_tx_counter: incrementing counter"
        );

        let mut txn = self.db.begin().await?;

        let res: Option<TxCounterSqlx> =
            sqlx::query_as("SELECT * FROM tx_counter WHERE address = $1 LIMIT 1")
                .bind(address.0.0)
                .fetch_optional(&mut *txn)
                .await?;

        let new_tx_counter = if let Some(res) = res {
            let model_epoch = res.epoch;
            let model_epoch_counter = res.epoch_counter;

            if model_epoch == 0 || epoch != Epoch::from(model_epoch) {
                let res: TxCounterSqlx = sqlx::query_as("UPDATE tx_counter SET epoch = $1, epoch_counter = $2 WHERE address = $3 RETURNING *")
                    .bind(i64::from(epoch))
                    .bind(incr_value)
                    .bind(address.0.0)
                    .fetch_one(&mut *txn)
                    .await?;
                res
            } else {
                let res: TxCounterSqlx = sqlx::query_as(
                    "UPDATE tx_counter SET epoch_counter = $1 WHERE address = $2 RETURNING *",
                )
                .bind(model_epoch_counter.saturating_add(incr_value))
                .bind(address.0.0)
                .fetch_one(&mut *txn)
                .await?;
                res
            }
        } else {
            let res: TxCounterSqlx = sqlx::query_as("INSERT INTO tx_counter (address, epoch, epoch_counter) VALUES ($1, $2, $3) RETURNING *")
                    .bind(address.0.0)
                    .bind(i64::from(epoch))
                    .bind(incr_value)
                    .fetch_one(&mut *txn)
                    .await?;

            res
        };

        txn.commit().await?;
        // FIXME: no 'as'
        Ok((new_tx_counter.epoch_counter as u64).into())
    }

    pub(crate) async fn get_tx_counter(
        &self,
        address: &Address,
    ) -> Result<EpochCounter, TxCounterError2> {
        let res: Option<TxCounterSqlx> =
            sqlx::query_as("SELECT * FROM tx_counter WHERE address = $1 LIMIT 1")
                .bind(address.0.0)
                .fetch_optional(&self.db)
                .await?;

        match res {
            None => Err(TxCounterError2::NotRegistered(*address)),
            Some(res) => Ok(self.counters_from_key(res)),
        }
    }

    fn counters_from_key(&self, model: TxCounterSqlx) -> EpochCounter {
        let (epoch, _epoch_slice) = *self.epoch_store.read();
        let cmp = model.epoch == i64::from(epoch);

        let address = Address::from_slice(model.address.as_slice());
        match cmp {
            true => {
                // EpochCounter stored in DB == epoch store
                // We query for an epoch and this is what is stored in the Db
                tracing::debug!(
                    address = %address,
                    db_epoch = model.epoch,
                    current_epoch = i64::from(epoch),
                    epoch_counter = model.epoch_counter,
                    "counters_from_key: epochs match, returning actual counter"
                );
                (model.epoch_counter as u64).into()
            }
            false => {
                // EpochCounter.epoch (stored in DB) != epoch_store.epoch
                // We query for an epoch after what is stored in Db
                // This can happen if no Tx has updated the epoch counter (yet)
                tracing::warn!(
                    address = %address,
                    db_epoch = model.epoch,
                    current_epoch = i64::from(epoch),
                    epoch_counter = model.epoch_counter,
                    "counters_from_key: EPOCH MISMATCH - returning 0 instead of {}",
                    model.epoch_counter
                );
                EpochCounter::from(0)
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
            return Err(RegisterError2::AlreadyRegistered(address));
        }

        let rate_commit = poseidon_hash(&[id_commitment, Fr::from(u64::from(self.rate_limit))]);

        let mut txn = self.db.begin().await?;

        // Step 1: find the first tree that is not full
        // Note: FOR UPDATE will wait until it can lock the row
        let max_entry: i64 = 1 << self.config.tree_depth;
        let tree_config: Option<MerkleTreeConfigSqlx> = sqlx::query_as(
            r#"
            SELECT *
            FROM m_tree_config
            WHERE next_index < $1
            ORDER BY tree_index ASC
            LIMIT 1
            FOR UPDATE
        "#,
        )
        .bind(max_entry)
        .fetch_optional(&mut *txn)
        .await?;

        match tree_config {
            Some(tree_config) => {
                // Step 2: update m_tree_config
                let res = sqlx::query(
                    "UPDATE m_tree_config SET next_index = next_index + 1 WHERE id = $1",
                )
                .bind(tree_config.id)
                .execute(&mut *txn)
                .await?;
                debug_assert!(res.rows_affected() == 1);

                // Step 3: update merkle_tree leaf
                let v = PgFrStruct { inner: rate_commit };
                let _res = sqlx::query("SELECT pgfr_mtree_set_leaf($1, $2, $3, $4)")
                    .bind(i16::from(self.config.tree_depth))
                    .bind(tree_config.tree_index) // depth
                    .bind(tree_config.next_index)
                    .bind(&v)
                    .execute(&mut *txn)
                    .await?;

                let _res = sqlx::query("INSERT INTO users (address, rln_id, tree_index, index_in_merkle_tree) VALUES ($1, $2, $3, $4)")
                    .bind(address.0.0)
                    .bind(serde_json::to_value(rln_identity).unwrap())
                    .bind(tree_config.tree_index)
                    .bind(tree_config.next_index)
                    .execute(&mut *txn)
                    .await?;

                let _res = sqlx::query("INSERT INTO tx_counter (address) VALUES ($1)")
                    .bind(address.0.0)
                    .execute(&mut *txn)
                    .await?;
            }
            None => {
                return Err(RegisterError2::TooManyUsers);
            }
        }

        txn.commit().await?;

        Ok(id_commitment)
    }

    pub(crate) async fn remove_user(&self, address: &Address) -> Result<bool, SqlxError> {
        let user = self.get_user(address).await?;

        if user.is_none() {
            // User not found (User not registered)
            return Ok(false);
        }

        let user = user.unwrap(); // Unwrap safe: just checked above
        let tree_index = user.tree_index as usize;
        let index_in_merkle_tree = user.index_in_merkle_tree as usize;

        let mut txn = self.db.begin().await?;
        // Note: never reuse index in Merkle Tree (for security reasons)
        //       instead we reset the user rate commit to 0
        let v = PgFrStruct { inner: Fr::ZERO };
        let _res = sqlx::query("SELECT pgfr_mtree_set_leaf($1, $2, $3, $4)")
            .bind(i16::from(self.config.tree_depth))
            .bind(tree_index as i16) // depth
            .bind(index_in_merkle_tree as i64)
            .bind(&v)
            .execute(&mut *txn)
            .await?;

        sqlx::query("DELETE FROM users WHERE address = $1")
            .bind(address.0.0)
            .execute(&mut *txn)
            .await?;
        // Note: tx_counters has a foreign key on users.address + ON DELETE CASCADE
        // sqlx::query("DELETE FROM tx_counter WHERE address = $1")
        //     .bind(address.to_string())
        //     .execute(&mut *txn)
        //     .await?;

        txn.commit().await?;

        Ok(true)
    }

    // Merkle tree methods
    pub async fn get_merkle_proof(
        &self,
        address: &Address,
    ) -> Result<MerkleProof, GetMerkleTreeProofError2> {
        let (tree_index, index_in_mt) = {
            let user = self.get_user(address).await?;
            if user.is_none() {
                return Err(GetMerkleTreeProofError2::NotRegistered(*address));
            }
            let user = user.unwrap();
            (user.tree_index, user.index_in_merkle_tree)
        };

        let row: (MerkleProof,) = sqlx::query_as("SELECT pgfr_mtree_get_proof($1, $2, $3)")
            .bind(i16::from(self.config.tree_depth))
            .bind(tree_index)
            .bind(0i64)
            .fetch_one(&self.db)
            .await?;

        Ok(row.0)
    }

    // external UserDb methods

    pub async fn on_new_user(&self, address: &Address) -> Result<Fr, RegisterError2> {
        self.register_user(*address).await
    }

    pub async fn on_new_tx(
        &self,
        address: &Address,
        incr_value: Option<i64>,
    ) -> Result<EpochCounter, TxCounterError2> {
        let has_user = self.has_user(address).await?;

        if has_user {
            let epoch_counter = self.incr_tx_counter(address, incr_value).await?;
            Ok(epoch_counter)
        } else {
            Err(TxCounterError2::NotRegistered(*address))
        }
    }

    pub async fn on_tier_limits_updated(
        &self,
        tier_limits: TierLimits,
    ) -> Result<(), SetTierLimitsError2> {
        tier_limits.validate()?;
        self.set_tier_limits(tier_limits)
            .await
            .map_err(SetTierLimitsError2::Db)
    }

    /// Get user tier info
    pub(crate) async fn user_tier_info<E: std::error::Error, KSC: KarmaAmountExt<Error = E>>(
        &self,
        address: &Address,
        karma_sc: &KSC,
    ) -> Result<UserTierInfo2, UserTierInfoError2<E>> {
        let has_user = self
            .has_user(address)
            .await
            .map_err(UserTierInfoError2::Db)?;

        if !has_user {
            return Err(UserTierInfoError2::NotRegistered(*address));
        }

        let karma_amount = karma_sc
            .karma_amount(address)
            .await
            .map_err(|e| UserTierInfoError2::Contract(e))?;

        // TODO
        let epoch_tx_count = self.get_tx_counter(address).await?;
        // TODO: avoid db query the tier limits (keep it in memory)
        let tier_limits = self.get_tier_limits().await?;
        let tier_match = tier_limits.get_tier_by_karma(&karma_amount);

        let user_tier_info = {
            let (current_epoch, current_epoch_slice) = *self.epoch_store.read();
            let mut t = UserTierInfo2 {
                current_epoch,
                current_epoch_slice,
                epoch_tx_count: epoch_tx_count.into(),
                // epoch_slice_tx_count: epoch_slice_tx_count.into(),
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

    // ============ Deny List Methods ============

    /// Check if an address is on the deny list and not expired
    ///
    /// Returns true if the address is denied (and not expired), false otherwise
    pub async fn is_denied<F: Fn() -> Duration>(
        &self,
        address: &Address,
        now: &F,
    ) -> Result<bool, SqlxError> {
        let now_ = now().as_secs() as i64;

        // Single query: check if exists AND (no expiry OR not expired)
        let res: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM deny_list WHERE address = $1 AND (expires_at IS NULL OR expires_at > $2)")
            .bind(address.0.0)
            .bind(now_)
            .fetch_one(&self.db)
            .await?;

        Ok(res > 0)
    }

    /// Add an address to the deny list
    /// Uses UPSERT for atomicity and performance
    ///
    /// - `address`: The address to deny
    /// - `reason`: Optional reason for denial (logged but not stored for performance)
    /// - `ttl_seconds`: Optional time-to-live in seconds (None means no expiry)
    /// - `now`: Function returning the current Duration till UNIX_EPOCH
    ///
    /// Returns true if the address was newly added, false if it was already present
    pub async fn add_to_deny_list<F: Fn() -> Duration>(
        &self,
        address: &Address,
        reason: Option<String>,
        ttl_seconds: Option<i64>,
        now: &F,
    ) -> Result<bool, SqlxError> {
        let now_ = now().as_secs() as i64;
        let expires_at = ttl_seconds.map(|ttl| now_ + ttl);

        if let Some(reason) = &reason {
            tracing::info!(
                address = %address,
                reason = %reason,
                expires_at = ?expires_at,
                "Adding address to deny list"
            );
        } else {
            tracing::info!(
                address = %address,
                expires_at = ?expires_at,
                "Adding address to deny list"
            );
        }

        let result = sqlx::query(r#"
            INSERT INTO deny_list (address, expires_at, denied_at) VALUES ($1, $2, $3)
            ON CONFLICT (address) DO UPDATE SET expires_at = EXCLUDED.expires_at, denied_at = EXCLUDED.denied_at
            RETURNING (xmax = 0) as "is_insert!" "#)
            .bind(address.0.0)
            .bind(expires_at)
            .bind(now_)
            .fetch_one(&self.db)
            .await?;

        // true = row has been inserted, false = row has been updated
        Ok(result.get(0))
    }

    /// Remove an address from the deny list
    ///
    /// Returns true if the address was removed, false if it wasn't on the list
    pub async fn remove_from_deny_list(&self, address: &Address) -> Result<bool, SqlxError> {
        let result = sqlx::query(r#"DELETE FROM deny_list WHERE address = $1"#)
            .bind(address.0.0)
            .execute(&self.db)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get deny list entry for an address (if exists and not expired)
    ///
    /// Returns the deny list entry model if found and not expired
    pub async fn get_deny_list_entry<F: Fn() -> Duration>(
        &self,
        address: &Address,
        now: &F,
    ) -> Result<Option<DenyListSqlx>, SqlxError> {
        let now_ = now().as_secs() as i64;

        // Single query with expiry check
        let result: Option<DenyListSqlx> = sqlx::query_as("SELECT * FROM deny_list WHERE address = $1 AND (expires_at IS NULL OR expires_at > $2)")
            .bind(address.0.0)
            .bind(now_)
            .fetch_optional(&self.db)
            .await?;

        Ok(result)
    }

    /// Clean up expired deny list entries
    ///
    /// Returns the number of entries removed
    pub async fn cleanup_expired_deny_list_entries<F: Fn() -> Duration>(
        &self,
        now: &F,
    ) -> Result<u64, SqlxError> {
        let now_ = now().as_secs() as i64;
        let result =
            sqlx::query("DELETE FROM deny_list WHERE expires_at IS NOT NULL AND expires_at <= $1")
                .bind(now_)
                .execute(&self.db)
                .await?;

        Ok(result.rows_affected())
    }

    // ============ Nullifier Methods (High-Throughput) ============

    /// Check if a nullifier exists for the given epoch
    /// Returns true if the nullifier already exists (duplicate/replay), false otherwise
    pub async fn nullifier_exists(&self, nullifier: &[u8], epoch: i64) -> Result<bool, SqlxError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM nullifiers WHERE nullifier = $1 AND epoch = $2 LIMIT 1",
        )
        .bind(nullifier)
        .bind(epoch)
        .fetch_one(&self.db)
        .await?;

        Ok(count > 0)
    }

    /// Record a nullifier for the given epoch
    /// Returns Ok(true) if inserted, Ok(false) if already exists (duplicate)
    ///
    /// Uses INSERT ... ON CONFLICT DO NOTHING for atomic check-and-insert
    pub async fn record_nullifier(&self, nullifier: &[u8], epoch: i64) -> Result<bool, SqlxError> {
        // Note: ON_CONFLICT cause should be "DO NOTHING" but here with no-op
        //       we force Postgresql to always return a row (is_insert)
        let result = sqlx::query(
            r#"
                INSERT INTO nullifiers (nullifier, epoch) VALUES ($1, $2)
                ON CONFLICT (nullifier, epoch) DO UPDATE SET nullifier = EXCLUDED.nullifier
                RETURNING (xmax = 0) as "is_insert!"
            "#,
        )
        .bind(nullifier)
        .bind(epoch)
        .fetch_one(&self.db)
        .await?;

        Ok(result.get(0))
    }

    /// Check and record a nullifier atomically
    /// Returns Ok(true) if nullifier is new and was recorded
    /// Returns Ok(false) if nullifier already existed (duplicate/replay attack)
    ///
    /// This is the primary method for nullifier tracking at high throughput
    pub async fn check_and_record_nullifier(
        &self,
        nullifier: &[u8],
        epoch: i64,
    ) -> Result<bool, SqlxError> {
        // record_nullifier already does atomic check-and-insert
        self.record_nullifier(nullifier, epoch).await
    }

    /// Clean up nullifiers from old epochs
    /// Call this periodically to prevent unbounded table growth
    ///
    /// - `keep_epochs`: Number of recent epochs to keep
    /// - `current_epoch`: The current epoch
    ///
    /// Returns the number of nullifiers removed
    pub async fn cleanup_old_nullifiers(
        &self,
        current_epoch: i64,
        keep_epochs: i64,
    ) -> Result<u64, SqlxError> {
        let cutoff_epoch = current_epoch - keep_epochs;
        let result = sqlx::query("DELETE FROM nullifiers WHERE epoch < $1")
            .bind(cutoff_epoch)
            .execute(&self.db)
            .await?;

        Ok(result.rows_affected())
    }

    /// Get count of nullifiers for a specific epoch
    /// Useful for monitoring and debugging
    pub async fn get_nullifier_count_for_epoch(&self, epoch: i64) -> Result<u64, SqlxError> {
        let result: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM nullifiers WHERE epoch = $1")
            .bind(epoch)
            .fetch_one(&self.db)
            .await?;

        Ok(result as u64)
    }
}

// Test only functions
#[cfg(test)]
impl UserDb2 {
    pub(crate) async fn get_db_tree_count(&self) -> Result<u64, SqlxError> {
        Self::get_merkle_tree_count_from_db(&self.db).await
    }

    pub(crate) async fn get_user_indexes(&self, address: &Address) -> (i64, i64) {
        let user_model = self.get_user(address).await.unwrap().unwrap();

        (user_model.tree_index, user_model.index_in_merkle_tree)
    }

    pub(crate) async fn get_merkle_tree_leave_set(&self, tree_index: i64) -> i64 {
        let res: i64 = sqlx::query_scalar(
            r#"
            SELECT next_index FROM m_tree_config WHERE tree_index = $1 LIMIT 1
        "#,
        )
        .bind(tree_index)
        .fetch_one(&self.db)
        .await
        .unwrap();

        res
    }

    // deny list
    pub async fn get_raw_deny_list_entry(
        &self,
        address: &Address,
    ) -> Result<Option<DenyListSqlx>, SqlxError> {
        sqlx::query_as("SELECT * FROM deny_list WHERE address = $1")
            .bind(address.0.0)
            .fetch_optional(&self.db)
            .await
    }
}

#[cfg(feature = "postgres")]
#[cfg(test)]
mod tests {
    use super::*;
    // std
    // third-party
    use alloy::primitives::{U256, address};
    use async_trait::async_trait;
    use claims::assert_matches;
    use derive_more::Display;
    use prover_db_migration_sqlx::MigrationConfig;
    use tracing_test::traced_test;
    // internal
    // use prover_db_migration::{Migrator as MigratorCreate, MigratorTrait};
    use crate::tests_common::create_database_connection;

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

    #[tokio::test]
    // #[traced_test]
    async fn test_user_register() {
        // Use this to see sea_orm traces
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

        let (_, db_conn) =
            create_database_connection("user_db_test_user_register", true, config.clone())
                .await
                .expect("create_database_connection_1 failed");

        let user_db = UserDb2::new(
            db_conn,
            config,
            epoch_store,
            Default::default(),
            Default::default(),
        )
        .await
        .expect("Cannot create UserDb");

        let addr = Address::new([0; 20]);
        user_db.register_user(addr).await.unwrap();
        assert_matches!(
            user_db.register_user(addr).await,
            Err(RegisterError2::AlreadyRegistered(_))
        );

        assert!(user_db.get_user_identity(&addr).await.is_some());
        assert_eq!(
            user_db.get_tx_counter(&addr).await.unwrap(),
            EpochCounter::from(0)
        );

        assert!(user_db.get_user_identity(&ADDR_1).await.is_none());
        user_db.register_user(ADDR_1).await.unwrap();
        assert!(user_db.get_user_identity(&ADDR_1).await.is_some());
        assert_eq!(
            user_db.get_tx_counter(&addr).await.unwrap(),
            EpochCounter::from(0)
        );

        user_db.incr_tx_counter(&addr, Some(42)).await.unwrap();
        assert_eq!(
            user_db.get_tx_counter(&addr).await.unwrap(),
            EpochCounter::from(42)
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
        let (_, db_conn) =
            create_database_connection("user_db_test_tx_counter", true, config.clone())
                .await
                .unwrap();

        let user_db = UserDb2::new(
            db_conn,
            config,
            epoch_store,
            Default::default(),
            Default::default(),
        )
        .await
        .expect("Cannot create UserDb");

        let addr = Address::new([0; 20]);

        user_db.register_user(addr).await.unwrap();

        let ec = user_db.get_tx_counter(&addr).await.unwrap();
        assert_eq!(ec, EpochCounter::from(0));
        // assert_eq!(ecs, EpochSliceCounter::from(0u64));

        let ecs_2 = user_db.incr_tx_counter(&addr, Some(42)).await.unwrap();
        // TODO
        assert_eq!(ecs_2, EpochCounter::from(42));
    }

    #[tokio::test]
    async fn test_incr_tx_counter() {
        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDb2Config {
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };
        let (_, db_conn) =
            create_database_connection("user_db_test_incr_tx_counter", true, config.clone())
                .await
                .unwrap();

        let user_db = UserDb2::new(
            db_conn,
            config,
            epoch_store,
            Default::default(),
            Default::default(),
        )
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
            user_db.on_new_tx(&addr, None).await.unwrap(),
            EpochCounter::from(1)
        );
        let tier_info = user_db
            .user_tier_info(&addr, &MockKarmaSc {})
            .await
            .unwrap();
        assert_eq!(tier_info.epoch_tx_count, 1);
        // assert_eq!(tier_info.epoch_slice_tx_count, 1);
    }

    #[tokio::test]
    async fn test_user_remove() {
        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDb2Config {
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };
        let db_conn = create_database_connection("user_db_test_user_remove", true, config.clone())
            .await
            .unwrap()
            .1;

        let user_db = UserDb2::new(
            db_conn,
            config,
            epoch_store,
            Default::default(),
            Default::default(),
        )
        .await
        .expect("Cannot create UserDb");

        user_db.register_user(ADDR_1).await.unwrap();
        let leaves_set_1 = user_db.get_merkle_tree_leave_set(0).await;

        user_db.register_user(ADDR_2).await.unwrap();
        let leaves_set_2 = user_db.get_merkle_tree_leave_set(0).await;
        assert_ne!(leaves_set_1, leaves_set_2);
        // println!("index addr 1: {}", leaves_set_1);
        // println!("index addr 2: {}", leaves_set_2);

        user_db.remove_user(&ADDR_2).await.unwrap();
        {
            /*
            let guard = user_db.merkle_trees.read().await;
            let mtree_index_after_rm_addr_2 = guard[0].leaves_set();
            drop(guard);
            */
            let leaves_set_after_rm = user_db.get_merkle_tree_leave_set(0).await;

            assert_eq!(user_db.has_user(&ADDR_1).await.unwrap(), true);
            assert_eq!(user_db.has_user(&ADDR_2).await.unwrap(), false);
            // Check tx_counter for user is deleted as well (foreign key)
            assert_matches!(
                user_db.get_tx_counter(&ADDR_2).await,
                Err(TxCounterError2::NotRegistered(_))
            );
            // No reuse of index in PmTree (as this is a generic impl and could lead to security issue:
            // like replay attack...)
            assert_eq!(leaves_set_after_rm, leaves_set_2);
        }
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
        let db_conn = create_database_connection(
            "user_db_test_user_reg_merkle_tree_fail",
            true,
            config.clone(),
        )
        .await
        .unwrap()
        .1;

        let user_db = UserDb2::new(
            db_conn,
            config,
            epoch_store,
            Default::default(),
            Default::default(),
        )
        .await
        .expect("Cannot create UserDb");

        let addr = Address::new([0; 20]);
        {
            /*
            let guard = user_db.merkle_trees.read().await;
            let mt = guard.first().unwrap();
            assert_eq!(mt.leaves_set(), 0);
            */

            assert_eq!(user_db.get_merkle_tree_leave_set(0).await, 0);
        }
        user_db.register_user(addr).await.unwrap();
        assert_eq!(user_db.has_user(&addr).await.unwrap(), true);
        {
            /*
            let guard = user_db.merkle_trees.read().await;
            let mt = guard.first().unwrap();
            assert_eq!(mt.leaves_set(), 1);
            */
            assert_eq!(user_db.get_merkle_tree_leave_set(0).await, 1);
        }
        user_db.register_user(ADDR_1).await.unwrap();
        {
            // let guard = user_db.merkle_trees.read().await;
            // let mt = guard.first().unwrap();
            // assert_eq!(mt.leaves_set(), 2);
            assert_eq!(user_db.get_merkle_tree_leave_set(0).await, 2);
        }

        let res = user_db.register_user(ADDR_2).await;
        assert_matches!(res, Err(RegisterError2::TooManyUsers));
        assert_eq!(user_db.has_user(&ADDR_1).await.unwrap(), true);
        assert_eq!(user_db.has_user(&ADDR_2).await.unwrap(), false);
        {
            // let guard = user_db.merkle_trees.read().await;
            // let mt = guard.first().unwrap();
            // assert_eq!(mt.leaves_set(), 2);
            assert_eq!(user_db.get_merkle_tree_leave_set(0).await, 2);
        }
    }
}
