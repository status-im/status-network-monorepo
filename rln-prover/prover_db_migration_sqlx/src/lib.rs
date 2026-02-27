#![allow(clippy::explicit_auto_deref)] // allow: `&mut *txn` which is more explicit (Deref to Connection then take a exclusive ref)

use sqlx::{PgConnection, Pool, Postgres, error::Error as SqlxError};

#[derive(Clone)]
pub struct MigrationConfig {
    pub tree_count: i64,
    pub max_tree_count: i64,
    pub tree_depth: i16,
}

pub struct Migrator();

impl Migrator {
    const UP_0_VERSION: &str = "m20250203_init";
    const UP_1_VERSION: &str = "m20260226_nonce_manager";
    const UP_2_VERSION: &str = "m20260227_quota_bonus";

    pub async fn up(&self, db: Pool<Postgres>, config: MigrationConfig) -> Result<(), SqlxError> {
        let mut txn = db.begin().await?;

        self.migrations_init(&mut *txn).await?;

        if !self.migration_exists(&mut *txn, Self::UP_0_VERSION).await? {
            self.up_0(&mut *txn, config.clone()).await?;
            self.migration_add(&mut *txn, Self::UP_0_VERSION).await?;
        }
        if !self.migration_exists(&mut *txn, Self::UP_1_VERSION).await? {
            self.up_1(&mut *txn).await?;
            self.migration_add(&mut *txn, Self::UP_1_VERSION).await?;
        }
        if !self.migration_exists(&mut *txn, Self::UP_2_VERSION).await? {
            self.up_2(&mut *txn).await?;
            self.migration_add(&mut *txn, Self::UP_2_VERSION).await?;
        }
        txn.commit().await?;
        Ok(())
    }

    async fn migrations_init(&self, db: &mut PgConnection) -> Result<(), SqlxError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS rln_prover_migrations (
                id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                version TEXT NOT NULL UNIQUE,
                applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
        "#,
        )
        .execute(&mut *db)
        .await?;
        Ok(())
    }

    async fn migration_exists(
        &self,
        db: &mut PgConnection,
        version: &str,
    ) -> Result<bool, SqlxError> {
        let res: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT id FROM rln_prover_migrations WHERE version = $1 LIMIT 1
        "#,
        )
        .bind(version)
        .fetch_optional(&mut *db)
        .await?;

        match res {
            Some(_id) => Ok(true),
            None => Ok(false),
        }
    }

    async fn migration_add(&self, db: &mut PgConnection, version: &str) -> Result<(), SqlxError> {
        sqlx::query(
            r#"
            INSERT INTO rln_prover_migrations (version) VALUES ($1)
        "#,
        )
        .bind(version)
        .execute(&mut *db)
        .await?;
        Ok(())
    }

    async fn migration_remove(
        &self,
        db: &mut PgConnection,
        version: &str,
    ) -> Result<(), SqlxError> {
        sqlx::query(
            r#"
            DELETE FROM rln_prover_migrations WHERE version = $1
        "#,
        )
        .bind(version)
        .execute(&mut *db)
        .await?;

        Ok(())
    }

    async fn up_0(&self, db: &mut PgConnection, config: MigrationConfig) -> Result<(), SqlxError> {
        sqlx::query(
            r#"
            CREATE TABLE users (
                id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                address BYTEA NOT NULL CHECK (OCTET_LENGTH(address) = 20),
                rln_id JSON NOT NULL,
                tree_index BIGINT,
                index_in_merkle_tree BIGINT,
                CONSTRAINT users_prod UNIQUE(address)
            )
        "#,
        )
        .execute(&mut *db)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE tx_counter (
                id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                address BYTEA NOT NULL REFERENCES users (address) ON DELETE CASCADE,
                epoch BIGINT NOT NULL DEFAULT 0,
                epoch_counter BIGINT NOT NULL DEFAULT 0,
                CONSTRAINT tx_counter_prod UNIQUE(address)
            )
        "#,
        )
        .execute(&mut *db)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE tier_limits (
                id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                name TEXT NOT NULL,
                tier_limits JSONB,
                CONSTRAINT tier_limits_prod UNIQUE(name)
            )
        "#,
        )
        .execute(&mut *db)
        .await?;

        // Deny list

        sqlx::query(
            r#"
            CREATE TABLE deny_list (
                id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                address BYTEA REFERENCES users (address) ON DELETE CASCADE,
                expires_at BIGINT,
                denied_at BIGINT,
                CONSTRAINT deny_list_prod UNIQUE(address)
            )
        "#,
        )
        .execute(&mut *db)
        .await?;

        // Nullifiers

        sqlx::query(
            r#"
            CREATE TABLE nullifiers (
                id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                nullifier BYTEA NOT NULL,
                epoch BIGINT NOT NULL,
                CONSTRAINT nullifiers_prod CHECK (OCTET_LENGTH(nullifier) = 32)
            )
        "#,
        )
        .execute(&mut *db)
        .await?;

        sqlx::query(
            r#"
            CREATE UNIQUE INDEX index_nullifiers_nullifier_epoch ON nullifiers (
                nullifier,
                epoch
            )
        "#,
        )
        .execute(&mut *db)
        .await?;

        // Merkle tree config

        sqlx::query(
            r#"
            CREATE TABLE m_tree_config (
                id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                tree_index SMALLINT,
                depth BIGINT NOT NULL,
                next_index BIGINT NOT NULL,
                CONSTRAINT m_tree_config_prod UNIQUE(tree_index)
            )
        "#,
        )
        .execute(&mut *db)
        .await?;

        // Merkle tree

        sqlx::query(
            r#"
            CREATE EXTENSION IF NOT EXISTS pg_merkle_tree
        "#,
        )
        .execute(&mut *db)
        .await?;

        let tree_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM m_tree_config")
            .fetch_one(&mut *db)
            .await?;

        if tree_count == 0 {
            for i in 0..config.max_tree_count {
                // println!("Creating merkle tree {}", i);

                sqlx::query(
                    r#"
                INSERT INTO m_tree_config (tree_index, depth, next_index) VALUES ($1, $2, $3)
            "#,
                )
                .bind(i)
                .bind(config.tree_depth as i64)
                .bind(0i64)
                .execute(&mut *db)
                .await?;

                let query = format!(
                    "CREATE TABLE pgfr_mtree_{} (index_in_mtree bigint PRIMARY KEY, value pgfr)",
                    i
                );
                sqlx::query(query.as_str()).execute(&mut *db).await?;

                sqlx::query(r#"SELECT pgfr_mtree_init($1, $2)"#)
                    .bind(config.tree_depth)
                    .bind(i)
                    .execute(&mut *db)
                    .await?;
            }
        }

        Ok(())
    }

    async fn up_2(&self, db: &mut PgConnection) -> Result<(), SqlxError> {
        sqlx::query("ALTER TABLE tx_counter ADD COLUMN quota_bonus BIGINT NOT NULL DEFAULT 0")
            .execute(&mut *db)
            .await?;
        Ok(())
    }

    async fn up_1(&self, db: &mut PgConnection) -> Result<(), SqlxError> {
        sqlx::query(
            r#"
            CREATE TABLE nonce_state (
                id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                wallet_address BYTEA NOT NULL CHECK (OCTET_LENGTH(wallet_address) = 20),
                current_nonce BIGINT NOT NULL DEFAULT 0,
                last_synced_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                CONSTRAINT nonce_state_wallet UNIQUE(wallet_address)
            )
        "#,
        )
        .execute(&mut *db)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE pending_registrations (
                id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                user_address BYTEA NOT NULL CHECK (OCTET_LENGTH(user_address) = 20),
                identity_commitment BYTEA NOT NULL CHECK (OCTET_LENGTH(identity_commitment) = 32),
                nonce BIGINT NOT NULL,
                tx_hash BYTEA,
                status TEXT NOT NULL DEFAULT 'queued',
                gas_price BIGINT,
                attempt_count INT NOT NULL DEFAULT 0,
                max_attempts INT NOT NULL DEFAULT 5,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                submitted_at TIMESTAMP,
                confirmed_at TIMESTAMP,
                last_error TEXT,
                CONSTRAINT pending_registrations_nonce UNIQUE(nonce)
            )
        "#,
        )
        .execute(&mut *db)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX idx_pending_registrations_status
            ON pending_registrations (status) WHERE status IN ('queued', 'submitted')
        "#,
        )
        .execute(&mut *db)
        .await?;

        Ok(())
    }

    pub async fn down(&self, db: Pool<Postgres>) -> Result<(), SqlxError> {
        let mut txn = db.begin().await?;

        // Note: this forces the creation of the migration table so we can query it
        self.migrations_init(&mut *txn).await?;

        // Down in reverse order
        if self.migration_exists(&mut *txn, Self::UP_2_VERSION).await? {
            self.down_2(&mut *txn).await?;
            self.migration_remove(&mut *txn, Self::UP_2_VERSION).await?;
        }
        if self.migration_exists(&mut *txn, Self::UP_1_VERSION).await? {
            self.down_1(&mut *txn).await?;
            self.migration_remove(&mut *txn, Self::UP_1_VERSION).await?;
        }
        if self.migration_exists(&mut *txn, Self::UP_0_VERSION).await? {
            self.down_0(&mut *txn).await?;
            self.migration_remove(&mut *txn, Self::UP_0_VERSION).await?;
        }
        txn.commit().await?;
        Ok(())
    }

    async fn down_2(&self, db: &mut PgConnection) -> Result<(), SqlxError> {
        sqlx::query("ALTER TABLE tx_counter DROP COLUMN IF EXISTS quota_bonus")
            .execute(&mut *db)
            .await?;
        Ok(())
    }

    async fn down_1(&self, db: &mut PgConnection) -> Result<(), SqlxError> {
        sqlx::query("DROP TABLE IF EXISTS pending_registrations")
            .execute(&mut *db)
            .await?;
        sqlx::query("DROP TABLE IF EXISTS nonce_state")
            .execute(&mut *db)
            .await?;
        Ok(())
    }

    async fn down_0(&self, db: &mut PgConnection) -> Result<(), SqlxError> {
        sqlx::query(
            r#"
            DROP TABLE users
        "#,
        )
        .execute(&mut *db)
        .await?;
        sqlx::query(
            r#"
            DROP TABLE tx_counter
        "#,
        )
        .execute(&mut *db)
        .await?;
        sqlx::query(
            r#"
            DROP TABLE tier_limits
        "#,
        )
        .execute(&mut *db)
        .await?;

        sqlx::query(
            r#"
            DROP TABLE deny_list
        "#,
        )
        .execute(&mut *db)
        .await?;

        sqlx::query(
            r#"
            DROP TABLE nullifiers
        "#,
        )
        .execute(&mut *db)
        .await?;

        Ok(())
    }
}
