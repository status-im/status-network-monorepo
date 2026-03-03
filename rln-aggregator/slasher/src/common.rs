use alloy::primitives::Address;
// internal
use crate::prover_proto::{RlnAggLightProofReply, RlnAggProof};

#[cfg_attr(test, derive(PartialEq, Debug))]
pub(crate) struct SlashingData {
    pub(crate) proof_1: RlnAggLightProofReply,
    pub(crate) proof_2: RlnAggLightProofReply,
    pub(crate) sender: Address,
}
