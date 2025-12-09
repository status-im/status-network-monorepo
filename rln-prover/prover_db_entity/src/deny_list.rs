//! `SeaORM` Entity for deny_list table
//! Optimized for fast lookups - address is primary key

use sea_orm::entity::prelude::*;

/// Deny list entry - minimal schema for performance
/// Hot path: check if address exists and not expired
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "deny_list")]
pub struct Model {
    /// Ethereum address (0x prefixed hex string, 42 chars)
    /// Primary key - no separate ID needed
    #[sea_orm(primary_key, auto_increment = false, column_type = "Char(Some(42))")]
    pub address: String,
    /// Optional expiry timestamp (Unix seconds, NULL = never expires)
    pub expires_at: Option<i64>,
    /// Optional timestamp when denied (metadata, not used in hot path)
    pub denied_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
