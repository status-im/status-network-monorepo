use std::sync::atomic::{AtomicBool, Ordering};

use crate::user_db_2::UserDb2Config;
use prover_db_migration_sqlx::{MigrationConfig, Migrator};
use sqlx::error::Error as SqlxError;
use sqlx::{Pool, Postgres};

/// Whether the test template database has been set up in this process.
static TEMPLATE_READY: AtomicBool = AtomicBool::new(false);

const TEMPLATE_DB_NAME: &str = "rln_test_template";

/// Ensure a template database with pg_merkle_tree extension exists.
///
/// All test databases are created from this template so they share the same
/// pgfr type OID, which is required because PGFR_OID is a process-global OnceLock.
async fn ensure_test_template(
    base_pool: &Pool<Postgres>,
    db_url_base: &str,
) -> Result<(), SqlxError> {
    if TEMPLATE_READY.load(Ordering::Acquire) {
        return Ok(());
    }

    // Check if template already exists (from a previous test run or concurrent test)
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = 'rln_test_template')",
    )
    .fetch_one(base_pool)
    .await?;

    if !exists {
        match sqlx::query(&format!("CREATE DATABASE {}", TEMPLATE_DB_NAME))
            .execute(base_pool)
            .await
        {
            Ok(_) => {
                // We created it — install the extension
                let tpl_url = format!("{}/{}", db_url_base, TEMPLATE_DB_NAME);
                let tpl_pool = sqlx::PgPool::connect(&tpl_url).await?;
                sqlx::query("CREATE EXTENSION IF NOT EXISTS pg_merkle_tree")
                    .execute(&tpl_pool)
                    .await?;
                tpl_pool.close().await;
            }
            Err(_) => {
                // Another test likely beat us — wait for it to finish installing the extension
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
    }

    TEMPLATE_READY.store(true, Ordering::Release);
    Ok(())
}

pub async fn create_database_connection(
    db_name: &str,
    db_refresh: bool,
    config: UserDb2Config,
) -> Result<(String, Pool<Postgres>), SqlxError> {
    let db_url_base = "postgres://postgres:postgres@localhost";
    let db_url_0 = format!("{db_url_base}/postgres");
    let db_url = format!("{}/{}", db_url_base, db_name);

    if db_refresh {
        let db = sqlx::PgPool::connect(db_url_0.as_str()).await?;

        // Ensure template database with pg_merkle_tree extension exists
        ensure_test_template(&db, db_url_base).await?;

        // Terminate existing connections to allow DROP
        let _ = sqlx::query(&format!(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}' AND pid <> pg_backend_pid()",
            db_name
        ))
        .execute(&db)
        .await;

        let query_drop = format!("DROP DATABASE IF EXISTS {}", db_name);
        sqlx::query(query_drop.as_str()).execute(&db).await?;

        // Create from template so all test databases share the same pgfr type OID
        let query_create =
            format!("CREATE DATABASE {} TEMPLATE {}", db_name, TEMPLATE_DB_NAME);
        // Retry in case the template still has an active connection from setup
        let mut last_err = None;
        for i in 0..5 {
            match sqlx::query(query_create.as_str()).execute(&db).await {
                Ok(_) => {
                    last_err = None;
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    tokio::time::sleep(std::time::Duration::from_millis(500 * (i + 1))).await;
                }
            }
        }
        if let Some(e) = last_err {
            return Err(e);
        }

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
