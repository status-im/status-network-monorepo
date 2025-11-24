use std::num::TryFromIntError;
// third-party
use alloy::primitives::Address;
use sea_orm::DbErr;
use zerokit_utils::error::{FromConfigError, ZerokitMerkleTreeError};
// internal
use crate::tier::ValidateTierLimitsError;
// TODO: define MerkleTreeError here?
use crate::user_db_2::MerkleTreeError;

#[derive(Debug, thiserror::Error)]
pub enum UserDbOpenError {
    #[error(transparent)]
    RocksDb(#[from] rocksdb::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] TryFromIntError),
    #[error(transparent)]
    JsonSerialization(#[from] serde_json::Error),
    #[error(transparent)]
    TreeConfig(#[from] FromConfigError),
    #[error(transparent)]
    MerkleTree(#[from] ZerokitMerkleTreeError),
    // #[error(transparent)]
    // MerkleTreeIndex(#[from] MerkleTreeIndexError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum RegisterError {
    #[error("User (address: {0:?}) has already been registered")]
    AlreadyRegistered(Address),
    #[error(transparent)]
    Db(#[from] rocksdb::Error),
    #[error("Too many users, exceeding merkle tree capacity...")]
    TooManyUsers,
    #[error("Merkle tree error: {0}")]
    TreeError(ZerokitMerkleTreeError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    FromConfig(#[from] FromConfigError),
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum TxCounterError {
    #[error("User (address: {0:?}) is not registered")]
    NotRegistered(Address),
    #[error(transparent)]
    Db(#[from] rocksdb::Error),
}

/*
#[derive(thiserror::Error, Debug, PartialEq, Clone)]
pub enum MerkleTreeIndexError {
    #[error("Uninitialized counter")]
    DbUninitialized,
    #[error(transparent)]
    Db(#[from] rocksdb::Error),
}
*/

#[derive(thiserror::Error, Debug, PartialEq, Clone)]
pub enum DbError {
    #[error("Uninitialized counter")]
    DbUninitialized,
    #[error(transparent)]
    Db(#[from] rocksdb::Error),
}

#[derive(thiserror::Error, Debug, PartialEq, Clone)]
pub enum UserMerkleTreeIndexError {
    #[error("User (address: {0:?}) is not registered")]
    NotRegistered(Address),
    #[error(transparent)]
    Db(#[from] rocksdb::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SetTierLimitsError {
    #[error(transparent)]
    Validate(#[from] ValidateTierLimitsError),
    #[error(transparent)]
    Db(#[from] rocksdb::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum UserTierInfoError<E: std::error::Error> {
    #[error("User {0} not registered")]
    NotRegistered(Address),
    #[error(transparent)]
    Contract(E),
    #[error(transparent)]
    TxCounter(#[from] TxCounterError),
    #[error(transparent)]
    Db(#[from] rocksdb::Error),
}

// UserDb2

#[derive(thiserror::Error, Debug)]
pub enum RegisterError2 {
    #[error("User (address: {0:?}) has already been registered")]
    AlreadyRegistered(Address),
    #[error(transparent)]
    Db(#[from] DbErr),
    #[error("Too many users, exceeding merkle tree capacity...")]
    TooManyUsers,
    #[error("Merkle tree error: {0}")]
    TreeError(MerkleTreeError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    FromConfig(#[from] FromConfigError),
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum TxCounterError2 {
    #[error("User (address: {0:?}) is not registered")]
    NotRegistered(Address),
    #[error(transparent)]
    Db(#[from] DbErr),
}

#[derive(Debug, thiserror::Error)]
pub enum SetTierLimitsError2 {
    #[error(transparent)]
    Validate(#[from] ValidateTierLimitsError),
    #[error(transparent)]
    Db(#[from] DbErr),
}

#[derive(Debug, thiserror::Error)]
pub enum UserTierInfoError2<E: std::error::Error> {
    #[error("User {0} not registered")]
    NotRegistered(Address),
    #[error(transparent)]
    Contract(E),
    #[error(transparent)]
    TxCounter(#[from] TxCounterError2),
    #[error(transparent)]
    Db(#[from] DbErr),
}