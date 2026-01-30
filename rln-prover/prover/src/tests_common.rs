// use prover_db_migration::{Migrator as MigratorCreate, MigratorTrait};
// use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr, Statement};

use sqlx::error::Error as SqlxError;
use sqlx::{Pool, Postgres};

pub async fn create_database_connection_1(
    // f_name: &str,
    // test_name: &str,
    db_name: &str,
    db_refresh: bool,
) -> Result<(String, Pool<Postgres>), SqlxError> {

    // Drop / Create db_name then return a connection to it

    /*
    let db_name = format!(
        "{}_{}",
        std::path::Path::new(f_name)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap(),
        test_name
    );

    println!("db_name: {db_name}");
    */

    let db_url_base = "postgres://myuser:mysecretpassword@localhost";
    // let db_url = format!("{db_url_base}/mydatabase");
    let db_url = format!("{}/{}", db_url_base, db_name);

    if db_refresh {
        let db = sqlx::PgPool::connect(db_url.as_str()).await?;

        sqlx::query("DROP DATABASE IF EXISTS $1")
            .bind(db_name)
            .execute(&db)
            .await?;

        sqlx::query("CREATE DATABASE $1")
            .bind(db_name)
            .execute(&db)
            .await?;

        db.close().await;
    }

    let db = sqlx::PgPool::connect(db_url.as_str()).await?;
    // MigratorCreate::up(&db, None).await?;
    unimplemented!("migration");

    Ok((db_url, db))
}
