use sea_orm_migration::prelude::*;

#[tokio::main]
async fn main() {
    cli::run_cli(prover_db_migration::Migrator).await;
}