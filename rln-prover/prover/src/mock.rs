use std::path::PathBuf;
// third-party
use alloy::primitives::Address;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MockUser {
    pub address: Address,
    pub tx_count: u64,
}

pub fn read_mock_user(path: &PathBuf) -> Result<Vec<MockUser>, MockUserError> {
    let f = std::fs::File::open(path)?;
    let users: Vec<MockUser> = serde_json::from_reader(f)?;
    Ok(users)
}

#[derive(thiserror::Error, Debug)]
pub enum MockUserError {
    #[error("transparent")]
    IOError(#[from] std::io::Error),
    #[error("transparent")]
    JsonError(#[from] serde_json::Error),
}
