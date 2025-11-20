use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {

        manager
            .create_table(
                Table::create()
                    .table(User::Table)
                    .col(big_pk_auto(User::Id))
                    // TODO: address as binary + length limit (20 bytes)
                    .col(text(User::Address).unique_key())
                    // TODO: save this as binary directly? or json only?
                    .col(json(User::RlnId))
                    .col(big_unsigned(User::TreeIndex))
                    .col(big_unsigned(User::IndexInMerkleTree))
                    .to_owned()
            ).await?;

        manager
            .create_table(
                Table::create()
                    .table(TxCounter::Table)
                    .col(big_pk_auto(TxCounter::Id))
                    // TODO: should be a foreign key to user table so we could drop user and tx_counter as well (cascade)
                    // TODO: address as binary + length limit (20 bytes)
                    .col(text(TxCounter::Address).unique_key())
                    .col(big_integer(TxCounter::Epoch).default(0))
                    .col(big_integer(TxCounter::EpochSlice).default(0))
                    .col(big_integer(TxCounter::EpochCounter).default(0))
                    .col(big_integer(TxCounter::EpochSliceCounter).default(0))
                    .to_owned()
            ).await?;

        manager
            .create_table(
                Table::create()
                    .table(TierLimits::Table)
                    .col(big_pk_auto(TierLimits::Id))
                    // TODO: Name limit
                    .col(text(TierLimits::Name).unique_key())
                    .col(json_null(TierLimits::TierLimits))
                    .to_owned()
            ).await?;

        // The merkle tree configurations
        manager
            .create_table(
                Table::create()
                    .table(MTreeConfig::Table)
                    .col(pk_auto(MTreeConfig::Id))
                    .col(small_unsigned(MTreeConfig::TreeIndex).unique_key())
                    .col(big_integer(MTreeConfig::Depth))
                    .col(big_integer(MTreeConfig::NextIndex))
                    .to_owned()
            ).await?;

        // Table to store the merkle tree
        // Each row represents a node in the tree
        // TreeIndex is the index of the tree (we could have multiple merkle trees)
        // IndexInTree is the index of the node in the current tree: depth & index
        manager
            .create_table(
                Table::create()
                    .table(MTree::Table)
                    .col(big_pk_auto(MTree::Id))
                    .col(small_unsigned(MTree::TreeIndex))
                    .col(big_integer(MTree::IndexInTree))
                    // TODO: var_binary + size limit
                    .col(blob(MTree::Value))
                    .to_owned()
            ).await?;

        // Need tree_index & index_in_tree to be unique (avoid multiple rows with the same index)
        manager.create_index(
            Index::create()
                .table(MTree::Table)
                .col(MTree::TreeIndex)
                .col(MTree::IndexInTree)
                .unique()
                .to_owned()
        ).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {

        manager.drop_table(
            Table::drop().table(User::Table).if_exists().to_owned()
        ).await?;

        manager.drop_table(
            Table::drop().table(TxCounter::Table).if_exists().to_owned()
        ).await?;

        manager.drop_table(
            Table::drop().table(TierLimits::Table).if_exists().to_owned()
        ).await?;

        manager.drop_table(
            Table::drop().table(MTree::Table).if_exists().to_owned()
        ).await?;

        manager.drop_table(
            Table::drop().table(MTreeConfig::Table).if_exists().to_owned()
        ).await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
    Address,
    RlnId,
    TreeIndex,
    IndexInMerkleTree,
}

#[derive(DeriveIden)]
enum TxCounter {
    Table,
    Id,
    Address,
    Epoch,
    EpochSlice,
    EpochCounter,
    EpochSliceCounter,
}

#[derive(DeriveIden)]
enum TierLimits {
    Table,
    Id,
    Name,
    TierLimits
}

#[derive(DeriveIden)]
enum MTree {
    Table,
    Id,
    TreeIndex,
    IndexInTree,
    Value,
}

#[derive(DeriveIden)]
enum MTreeConfig {
    Table,
    Id,
    TreeIndex,
    Depth,
    NextIndex,
}