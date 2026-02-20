use alloy::primitives::Address;
// internal
use crate::prover_proto::RlnAggProof;

#[cfg_attr(test, derive(PartialEq, Debug))]
pub(crate) struct SlashingData {
    pub(crate) proof_1: RlnAggProof,
    pub(crate) proof_2: RlnAggProof,
    pub(crate) sender: Address,
}
