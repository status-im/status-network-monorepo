use sqlx::{
    Pool,
    Postgres,
    error::Error as SqlxError,
};

#[derive(Clone)]
pub struct MigrationConfig {
    pub tree_count: i64,
    pub max_tree_count: i64,
    pub tree_depth: i16,
}

pub struct Migrator();

impl Migrator {

    pub async fn up(&self, db: Pool<Postgres>, config: MigrationConfig) -> Result<(), SqlxError> {
        self.up_0(db.clone(), config.clone()).await?;
        // self.up_1(db.clone(), config.clone()).await?;
        Ok(())
    }

    async fn up_0(&self, db: Pool<Postgres>, config: MigrationConfig) -> Result<(), SqlxError> {

        sqlx::query(r#"
            CREATE TABLE users (
                id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                address CHAR(42) NOT NULL,
                rln_id JSON NOT NULL,
                tree_index BIGINT,
                index_in_merkle_tree BIGINT,
                CONSTRAINT user_prod UNIQUE(address)
            )
        "#)
            .execute(&db)
            .await?;

        sqlx::query(r#"
            CREATE TABLE tx_counter (
                id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                address CHAR(42) NOT NULL,
                epoch BIGINT NOT NULL DEFAULT 0,
                epoch_counter BIGINT NOT NULL DEFAULT 0,
                CONSTRAINT tx_counter_prod UNIQUE(address)
            )
        "#)
            .execute(&db)
            .await?;

        sqlx::query(r#"
            CREATE TABLE tier_limits (
                id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                name TEXT NOT NULL,
                tier_limits JSONB,
                CONSTRAINT tier_limits_prod UNIQUE(name)
            )
        "#)
            .execute(&db)
            .await?;

        // Merkle tree config

        sqlx::query(r#"
            CREATE TABLE m_tree_config (
                id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                tree_index SMALLINT,
                depth BIGINT NOT NULL,
                next_index BIGINT NOT NULL,
                CONSTRAINT m_tree_config_prod UNIQUE(tree_index)
            )
        "#)
            .execute(&db)
            .await?;

        // Merkle tree

        sqlx::query(r#"
            CREATE EXTENSION pg_merkle_tree
        "#)
            .execute(&db)
            .await?;

        for i in 0..config.max_tree_count {

            sqlx::query(r#"
                INSERT INTO m_tree_config (tree_index, depth, next_index) VALUES ($1, $2, $3)
            "#)
                .bind(i)
                .bind(config.tree_depth as i64)
                .bind(0i64)
                .execute(&db)
                .await?;

            let query = format!("CREATE TABLE pgfr_mtree_{} (index_in_mtree bigint PRIMARY KEY, value pgfr)", i);
            sqlx::query(query.as_str())
                .execute(&db)
                .await?;

            sqlx::query(r#"SELECT pgfr_mtree_init($1, $2)"#)
                .bind(config.tree_depth)
                .bind(i)
                .execute(&db)
                .await?;
        }

        Ok(())
    }

    async fn up_1(&self, db: Pool<Postgres>) -> Result<(), SqlxError> {
        unimplemented!()
    }

    pub async fn down(&self, db: Pool<Postgres>) -> Result<(), SqlxError> {
        self.down_0(db.clone()).await?;
        Ok(())
    }

    async fn down_0(&self, db: Pool<Postgres>) -> Result<(), SqlxError> {

        sqlx::query(r#"
            DROP TABLE IF EXISTS users
        "#)
            .execute(&db)
            .await?;
        sqlx::query(r#"
            DROP TABLE IF EXISTS tx_counter
        "#)
            .execute(&db)
            .await?;
        sqlx::query(r#"
            DROP TABLE IF EXISTS tier_limits
        "#)
            .execute(&db)
            .await?;

        Ok(())
    }

}