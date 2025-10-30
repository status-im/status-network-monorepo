mod proof;

pub use proof::{RlnData, RlnIdentifier, RlnUserIdentity, compute_rln_proof_and_values};

// re export trait from zerokit utils crate (for prover)
pub use zerokit_utils::merkle_tree::merkle_tree::ZerokitMerkleTree;
