use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create nullifiers table - optimized for high-throughput duplicate detection
        // Primary use case: check if (nullifier, epoch) exists (hot path - 500+ TPS)
        // Secondary: cleanup old epochs
        //
        // Design decisions for performance:
        // 1. Composite primary key (nullifier, epoch) - single index lookup
        // 2. No auto-increment ID - saves write overhead
        // 3. Nullifier as BYTEA(32) - compact storage, fast comparison
        // 4. Epoch as BIGINT - supports timestamps or block numbers
        manager
            .create_table(
                Table::create()
                    .table(Nullifiers::Table)
                    // Nullifier: 32 bytes from RLN proof (internal nullifier)
                    .col(
                        ColumnDef::new(Nullifiers::Nullifier)
                            .binary_len(32)
                            .not_null(),
                    )
                    // Epoch: time period identifier (block number or timestamp bucket)
                    .col(
                        ColumnDef::new(Nullifiers::Epoch)
                            .big_integer()
                            .not_null(),
                    )
                    // Composite primary key for O(log n) duplicate detection
                    .primary_key(
                        Index::create()
                            .col(Nullifiers::Nullifier)
                            .col(Nullifiers::Epoch),
                    )
                    .to_owned(),
            )
            .await?;

        // Index on epoch for efficient cleanup of old nullifiers
        // When epoch N+K starts, we can delete all entries where epoch < N
        manager
            .create_index(
                Index::create()
                    .table(Nullifiers::Table)
                    .name("idx_nullifiers_epoch")
                    .col(Nullifiers::Epoch)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Nullifiers::Table).if_exists().to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum Nullifiers {
    Table,
    Nullifier,
    Epoch,
}

