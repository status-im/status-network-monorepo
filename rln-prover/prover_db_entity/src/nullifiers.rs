//! `SeaORM` Entity for nullifiers table
//! Optimized for high-throughput duplicate detection (500+ TPS)

use sea_orm::entity::prelude::*;

/// Nullifier entry - tracks used nullifiers per epoch
/// Hot path: check if (nullifier, epoch) exists
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "nullifiers")]
pub struct Model {
    /// RLN internal nullifier (32 bytes)
    /// Part of composite primary key
    #[sea_orm(primary_key, auto_increment = false, column_type = "VarBinary(StringLen::None)")]
    pub nullifier: Vec<u8>,
    /// Epoch identifier (block number or timestamp bucket)
    /// Part of composite primary key
    #[sea_orm(primary_key, auto_increment = false)]
    pub epoch: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
