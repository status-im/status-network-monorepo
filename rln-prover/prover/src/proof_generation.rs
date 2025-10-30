use std::sync::Arc;
// third-party
use alloy::primitives::Address;
// internal
use rln_proof::{RlnIdentifier, RlnUserIdentity};

#[derive(Debug, Clone)]
pub(crate) struct ProofGenerationData {
    pub(crate) user_identity: RlnUserIdentity,
    pub(crate) rln_identifier: Arc<RlnIdentifier>,
    pub(crate) tx_counter: u64,
    pub(crate) tx_sender: Address,
    pub(crate) tx_hash: Vec<u8>,
}

impl From<(RlnUserIdentity, Arc<RlnIdentifier>, u64, Address, Vec<u8>)> for ProofGenerationData {
    /// Create a new ProofGenerationData - assume tx_hash is 32 bytes long
    fn from(
        (user_identity, rln_identifier, tx_counter, tx_sender, tx_hash): (
            RlnUserIdentity,
            Arc<RlnIdentifier>,
            u64,
            Address,
            Vec<u8>,
        ),
    ) -> Self {
        debug_assert!(tx_hash.len() == 32);
        Self {
            user_identity,
            rln_identifier,
            tx_counter,
            tx_sender,
            tx_hash,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ProofSendingData {
    pub(crate) tx_hash: Vec<u8>,
    pub(crate) tx_sender: Address,
    pub(crate) proof: Vec<u8>,
}
