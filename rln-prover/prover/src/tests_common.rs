// use prover_db_migration::{Migrator as MigratorCreate, MigratorTrait};
// use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr, Statement};

use sqlx::error::Error as SqlxError;
use sqlx::{Pool, Postgres};
use prover_db_migration_sqlx::{MigrationConfig, Migrator};
use crate::user_db_2::UserDb2Config;

pub async fn create_database_connection(
    // f_name: &str,
    // test_name: &str,
    db_name: &str,
    db_refresh: bool,
    config: UserDb2Config,
) -> Result<(String, Pool<Postgres>), SqlxError> {

    // Drop / Create db_name then return a connection to it

    let db_url_base = "postgres://postgres:postgres@localhost";
    // Connect first to a default db (cause we need to create a new database for our test)
    let db_url_0 = format!("{db_url_base}/postgres");
    let db_url = format!("{}/{}", db_url_base, db_name);

    if db_refresh {
        let db = sqlx::PgPool::connect(db_url_0.as_str()).await?;

        let query_drop = format!("DROP DATABASE IF EXISTS {}", db_name);
        let _res = sqlx::query(query_drop.as_str())
            .execute(&db)
            .await?;

        let query_crate = format!("CREATE DATABASE {}", db_name);
        let _res = sqlx::query(query_crate.as_str())
            .execute(&db)
            .await?;

        db.close().await;
    }

    let db = sqlx::PgPool::connect(db_url.as_str()).await?;

    if db_refresh {
        // Migration
        let migrator = Migrator();
        migrator.down(db.clone()).await?;

        let cfg = MigrationConfig {
            tree_count: config.tree_count as i64,
            max_tree_count: config.max_tree_count as i64,
            tree_depth: config.tree_depth as i16,
        };

        migrator.up(db.clone(), cfg).await?;
    }

    Ok((db_url, db))
}
