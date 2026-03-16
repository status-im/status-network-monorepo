use alloy::signers::local::LocalSignerError;
use alloy::transports::{RpcError, TransportErrorKind};
use ark_serialize::SerializationError;
use rln::error::ProofError;
use smart_contract::{KarmaScError, KarmaTiersError, RlnScError};
// internal
use crate::epoch_service::WaitUntilError;
use crate::tier::ValidateTierLimitsError;
use crate::user_db_error::{
    GetMerkleTreeProofError2,
    // RegisterError,
    RegisterError2,
    // TxCounterError,
    TxCounterError2,
    UserDb2OpenError,
    // UserDbOpenError, UserMerkleTreeIndexError,
};

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Tonic (grpc) error: {0}")]
    Tonic(#[from] tonic::transport::Error),
    #[error("Tonic reflection (grpc) error: {0}")]
    TonicReflection(#[from] tonic_reflection::server::Error),
    #[error("Rpc error 1: {0}")]
    RpcError(#[from] RpcError<RpcError<TransportErrorKind>>),
    #[error("Rpc transport error 2: {0}")]
    RpcTransportError(#[from] RpcError<TransportErrorKind>),
    #[error("Epoch service error: {0}")]
    EpochError(#[from] WaitUntilError),
    #[error(transparent)]
    RegistryError(#[from] HandleTransferError),
    #[error(transparent)]
    KarmaScError(#[from] KarmaScError),
    #[error(transparent)]
    KarmaTiersError(#[from] KarmaTiersError),
    #[error(transparent)]
    RlnScError(#[from] RlnScError),
    #[error(transparent)]
    SignerInitError(#[from] LocalSignerError),
    #[error(transparent)]
    ValidateTierError(#[from] ValidateTierLimitsError),
    /*
    #[error(transparent)]
    UserDbOpenError(#[from] UserDbOpenError),
    #[error(transparent)]
    MockUserRegisterError(#[from] RegisterError),
    #[error(transparent)]
    MockUserTxCounterError(#[from] TxCounterError),
    */
}

#[derive(thiserror::Error, Debug)]
pub enum AppError2 {
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),
    #[error("Migration failed: {0}")]
    MigrationError(String),
    #[error("Tonic (grpc) error: {0}")]
    Tonic(#[from] tonic::transport::Error),
    #[error("Tonic reflection (grpc) error: {0}")]
    TonicReflection(#[from] tonic_reflection::server::Error),
    #[error("Rpc error 1: {0}")]
    RpcError(#[from] RpcError<RpcError<TransportErrorKind>>),
    #[error("Rpc transport error 2: {0}")]
    RpcTransportError(#[from] RpcError<TransportErrorKind>),
    #[error("Epoch service error: {0}")]
    EpochError(#[from] WaitUntilError),
    #[error(transparent)]
    RegistryError(#[from] HandleTransferError2),
    #[error(transparent)]
    KarmaScError(#[from] KarmaScError),
    #[error(transparent)]
    KarmaTiersError(#[from] KarmaTiersError),
    #[error(transparent)]
    RlnScError(#[from] RlnScError),
    #[error(transparent)]
    SignerInitError(#[from] LocalSignerError),
    #[error(transparent)]
    ValidateTierError(#[from] ValidateTierLimitsError),
    #[error(transparent)]
    UserDbOpenError(#[from] UserDb2OpenError),
    #[error(transparent)]
    MockUserRegisterError(#[from] RegisterError2),
    #[error(transparent)]
    MockUserTxCounterError(#[from] TxCounterError2),
}

#[derive(thiserror::Error, Debug)]
pub enum ProofGenerationError {
    #[error("Proof generation failed: {0}")]
    Proof(#[from] ProofError),
    #[error("Proof serialization failed: {0}")]
    Serialization(#[from] SerializationError),
    #[error("Proof serialization failed: {0}")]
    SerializationWrite(#[from] std::io::Error),
    #[error(transparent)]
    MerkleProofError(#[from] GetMerkleTreeProofError2),
}

/// Same as ProofGenerationError but can be Cloned (can be used in Tokio broadcast channels)
#[derive(thiserror::Error, Debug, Clone)]
pub enum ProofGenerationStringError {
    #[error("Proof generation failed: {0}")]
    Proof(String),
    #[error("Proof serialization failed: {0}")]
    Serialization(String),
    #[error("Proof serialization failed: {0}")]
    SerializationWrite(String),
    #[error("Merkle proof generation failed: {0}")]
    MerkleProofError(String),
}

impl From<ProofGenerationError> for ProofGenerationStringError {
    fn from(value: ProofGenerationError) -> Self {
        match value {
            ProofGenerationError::Proof(e) => ProofGenerationStringError::Proof(e.to_string()),
            ProofGenerationError::Serialization(e) => Self::Serialization(e.to_string()),
            ProofGenerationError::SerializationWrite(e) => Self::SerializationWrite(e.to_string()),
            ProofGenerationError::MerkleProofError(e) => Self::MerkleProofError(e.to_string()),
        }
    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum GetMerkleTreeProofError {
    #[error("Merkle tree error: {0}")]
    TreeError(String),
    // #[error(transparent)]
    // MerkleTree(#[from] UserMerkleTreeIndexError),
}

#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct RegisterSCError(#[from] alloy::contract::Error);

#[derive(thiserror::Error, Debug)]
pub enum HandleTransferError {
    // #[error(transparent)]
    // Register(#[from] RegisterError),
    #[error("Fail to register user in RLN SC: {0}")]
    ScRegister(#[from] RegisterSCError),
    #[error("Unable to query balance: {0}")]
    FetchBalanceOf(#[from] alloy::contract::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum HandleTransferError2 {
    #[error(transparent)]
    Register(#[from] RegisterError2),
    #[error("Fail to register user in RLN SC: {0}")]
    ScRegister(#[from] RegisterSCError),
    #[error("Unable to query balance: {0}")]
    FetchBalanceOf(alloy::contract::Error),
    #[error("Nonce manager error: {0}")]
    NonceManager(#[from] crate::nonce_manager::NonceManagerError),
}

// Route alloy::contract::Error -> ScRegister (not FetchBalanceOf)
// This allows `RE: Into<HandleTransferError2>` to work for both
// alloy::contract::Error and NonceManagerError in handle_transfer_event.
impl From<alloy::contract::Error> for HandleTransferError2 {
    fn from(e: alloy::contract::Error) -> Self {
        HandleTransferError2::ScRegister(RegisterSCError::from(e))
    }
}
