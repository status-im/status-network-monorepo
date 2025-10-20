// std
// use std::io::Cursor;
// third-party
use ark_bn254::{Bn254, Fr};
use ark_groth16::{Proof, ProvingKey};
use ark_relations::r1cs::ConstraintMatrices;
use rln::utils::IdSecret;
use rln::{
    circuit::{ARKZKEY_BYTES, read_arkzkey_from_bytes_uncompressed as read_zkey},
    error::ProofError,
    hashers::{hash_to_field_le, poseidon_hash},
    poseidon_tree::MerkleProof,
    protocol::{
        RLNProofValues, generate_proof, proof_values_from_witness, rln_witness_from_values,
    },
};
use zerokit_utils::ZerokitMerkleProof;

/// A RLN user identity & limit
#[derive(Debug, Clone, PartialEq)]
pub struct RlnUserIdentity {
    pub commitment: Fr,
    pub secret_hash: IdSecret,
    pub user_limit: Fr,
}

impl From<(Fr, IdSecret, Fr)> for RlnUserIdentity {
    fn from((commitment, secret_hash, user_limit): (Fr, IdSecret, Fr)) -> Self {
        Self {
            commitment,
            secret_hash,
            user_limit,
        }
    }
}

/// RLN info for a channel / group
#[derive(Debug, Clone)]
pub struct RlnIdentifier {
    pub identifier: Fr,
    pub pkey_and_constraints: (ProvingKey<Bn254>, ConstraintMatrices<Fr>),
    pub graph: Vec<u8>,
}

impl RlnIdentifier {
    pub fn new(identifier: &[u8]) -> Self {
        let pk_and_matrices = {
            // let mut reader = Cursor::new(ARKZKEY_BYTES);
            read_zkey(ARKZKEY_BYTES).unwrap()
        };
        let graph_bytes = include_bytes!("../resources/graph.bin");

        Self {
            identifier: hash_to_field_le(identifier),
            pkey_and_constraints: pk_and_matrices,
            graph: graph_bytes.to_vec(),
        }
    }
}

/// Data to be proven by RLN
#[derive(Debug, Clone)]
pub struct RlnData {
    /// message index (in a given epoch)
    pub message_id: Fr,
    /// message / signal hashed (bytes hashed to a field element type)
    pub data: Fr,
}

pub fn compute_rln_proof_and_values(
    user_identity: &RlnUserIdentity,
    rln_identifier: &RlnIdentifier,
    rln_data: RlnData,
    epoch: Fr,
    merkle_proof: &MerkleProof,
) -> Result<(Proof<Bn254>, RLNProofValues), ProofError> {
    let external_nullifier = poseidon_hash(&[rln_identifier.identifier, epoch]);

    let path_elements = merkle_proof.get_path_elements();
    let identity_path_index = merkle_proof.get_path_index();

    // let mut id_s = user_identity.secret_hash;

    let witness = rln_witness_from_values(
        user_identity.secret_hash.clone(),
        path_elements,
        identity_path_index,
        rln_data.data,
        external_nullifier,
        user_identity.user_limit,
        rln_data.message_id,
    )?;

    let proof_values = proof_values_from_witness(&witness)?;
    let proof = generate_proof(
        &rln_identifier.pkey_and_constraints,
        &witness,
        rln_identifier.graph.as_slice(),
    )?;
    Ok((proof, proof_values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rln::poseidon_tree::PoseidonTree;
    use rln::protocol::{compute_id_secret, keygen};
    use zerokit_utils::ZerokitMerkleTree;

    #[test]
    fn test_recover_secret_hash() {
        let (user_co, mut user_sh_) = keygen();
        let user_sh = IdSecret::from(&mut user_sh_);
        let epoch = hash_to_field_le(b"foo");
        let spam_limit = Fr::from(10);

        // let mut tree = OptimalMerkleTree::new(20, Default::default(), Default::default()).unwrap();
        let mut tree = PoseidonTree::new(20, Default::default(), Default::default()).unwrap();
        tree.set(0, spam_limit).unwrap();
        let m_proof = tree.proof(0).unwrap();

        let rln_identifier = RlnIdentifier::new(b"rln id test");

        let message_id = Fr::from(1);

        let (_proof_0, proof_values_0) = compute_rln_proof_and_values(
            &RlnUserIdentity {
                commitment: *user_co,
                secret_hash: user_sh.clone(),
                user_limit: spam_limit,
            },
            &rln_identifier,
            RlnData {
                message_id,
                data: hash_to_field_le(b"sig"),
            },
            epoch,
            &m_proof,
        )
        .unwrap();

        let (_proof_1, proof_values_1) = compute_rln_proof_and_values(
            &RlnUserIdentity {
                commitment: *user_co,
                secret_hash: user_sh.clone(),
                user_limit: spam_limit,
            },
            &rln_identifier,
            RlnData {
                message_id,
                data: hash_to_field_le(b"sig 2"),
            },
            epoch,
            &m_proof,
        )
        .unwrap();

        let share1 = (proof_values_0.x, proof_values_0.y);
        let share2 = (proof_values_1.x, proof_values_1.y);
        let recovered_identity_secret_hash = compute_id_secret(share1, share2).unwrap();
        assert_eq!(user_sh, recovered_identity_secret_hash);
    }
}
