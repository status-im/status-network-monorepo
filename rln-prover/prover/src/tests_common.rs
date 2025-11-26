use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr, Statement};
use prover_db_migration::{Migrator as MigratorCreate, MigratorTrait};

pub(crate) async fn create_database_connection_1(f_name: &str, test_name: &str) -> Result<DatabaseConnection, DbErr> {

    // Drop / Create db_name then return a connection to it

    let db_name = format!("{}_{}",
        std::path::Path::new(f_name).file_stem().unwrap().to_str().unwrap(),
        test_name);

    println!("db_name: {}", db_name);

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
