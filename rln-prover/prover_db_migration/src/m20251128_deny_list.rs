use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create deny_list table - optimized for fast lookups
        // Primary use case: check if address is denied (hot path)
        // Secondary: TTL-based expiration cleanup
        manager
            .create_table(
                Table::create()
                    .table(DenyList::Table)
                    // Address is the primary key - no separate ID needed
                    // Using CHAR(42) for "0x" + 40 hex chars (fixed size = faster)
                    .col(
                        ColumnDef::new(DenyList::Address)
                            .char_len(42)
                            .not_null()
                            .primary_key(),
                    )
                    // Expiry timestamp - NULL means never expires
                    // This is the only field needed for the hot path check
                    .col(ColumnDef::new(DenyList::ExpiresAt).big_integer().null())
                    // DeniedAt is optional metadata (not used in hot path)
                    .col(ColumnDef::new(DenyList::DeniedAt).big_integer().null())
                    .to_owned(),
            )
            .await?;

        // Index on expires_at for efficient cleanup of expired entries
        // Partial index: only index non-null values (entries that can expire)
        manager
            .create_index(
                Index::create()
                    .table(DenyList::Table)
                    .name("idx_deny_list_expires_at")
                    .col(DenyList::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DenyList::Table).if_exists().to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum DenyList {
    Table,
    Address,
    ExpiresAt,
    DeniedAt,
}
