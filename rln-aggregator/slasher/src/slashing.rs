use std::sync::Arc;
use alloy::{
    primitives::Address,
    providers::Provider
};
use anyhow::Context;
use ark_bn254::Bn254;
use ark_groth16::Proof;
use ark_serialize::CanonicalDeserialize;
use tokio::sync::mpsc::Receiver;
use tracing::{debug, error, warn};
// RLN
use rln::protocol::{compute_id_secret, deserialize_proof_values};
use rln::utils::IdSecret;
use tokio::sync::TryAcquireError;
// internal
use crate::common::SlashingData;
use crate::smart_contract::RLN;
use crate::smart_contract::RLN::RLNInstance;

pub(crate) struct SlashingServiceConfig {
    pub(crate) rln_sc_address: Address,
    pub(crate) account_to_reward: Address,
    pub(crate) slashing_limit: u64,
}

pub(crate) struct SlashingService {
    slashing_rx: Receiver<SlashingData>,
    slashing_limit: Arc<tokio::sync::Semaphore>,
    config: SlashingServiceConfig,
}

impl SlashingService {
    
    pub(crate) fn new(slashing_rx: Receiver<SlashingData>, config: SlashingServiceConfig) -> Self {
        Self {
            slashing_rx,
            slashing_limit: Arc::new(tokio::sync::Semaphore::new(config.slashing_limit as usize)),
            config
        }
    }
    
    // #[tracing::instrument(skip(self))]
    pub(crate) async fn serve<P: Provider + Clone + 'static>(&mut self, provider: P) -> anyhow::Result<()> {

        let rln_sc_ = RLN::new(self.config.rln_sc_address, provider);

        loop {
            let res = self.slashing_rx.recv().await;
            
            if let Some(slashing_data) = res {

                let sem_permit = self.slashing_limit.clone().try_acquire_owned();
                let sem_permit = match sem_permit {
                    Ok(sem_permit) => sem_permit,
                    Err(TryAcquireError::Closed) => {
                        // Semaphore is closed - slasher is likely closing
                        warn!("Semaphore closed");
                        break;
                    }
                    Err(TryAcquireError::NoPermits) => {
                        // Semaphore is full - let the client knows about it
                        warn!("Semaphore full, skip this slashing data...");
                        continue;
                    }
                };

                let account_to_reward = self.config.account_to_reward;
                let rln_sc = rln_sc_.clone();

                tokio::spawn(async move {

                    // Move semaphore permit in async closure so it will only be dropped once
                    // slashing is done
                    let _sem_permit = sem_permit;

                    if let Err(e) = slash(rln_sc, slashing_data, account_to_reward).await {
                        error!("e: {}", e);
                        debug!("account_to_reward: {}", account_to_reward);
                    }

                    Ok::<(), anyhow::Error>(())
                });

            } else {
                warn!("Slashing channel has been closed");
                break;
            }
        }
        
        Ok(())
    }
}

fn recover(slashing_data: SlashingData) -> anyhow::Result<IdSecret> {

    let proof_1 = slashing_data.proof_1;
    let proof_2 = slashing_data.proof_2;

    let _proof_1_de: Proof<Bn254> = CanonicalDeserialize::deserialize_compressed(&proof_1.proof[..128])
        .context("Failed to deserialize proof 1")?;
    let proof_1_values_de = deserialize_proof_values(&proof_1.proof.as_slice()[128..]).0;

    let _proof_2_de: Proof<Bn254> = CanonicalDeserialize::deserialize_compressed(&proof_2.proof[..128])
        .context("Failed to deserialize proof 2")?;
    let proof_2_values_de = deserialize_proof_values(&proof_2.proof.as_slice()[128..]).0;

    let recovered_identity_secret_hash = compute_id_secret(
        (proof_1_values_de.x, proof_1_values_de.y),
        (proof_2_values_de.x, proof_2_values_de.y),
    ).context("Fail to recover identity secret hash")?;

    Ok(recovered_identity_secret_hash)
}

async fn slash<P: Provider>(rln_sc: RLNInstance<P>, slashing_data: SlashingData, account_to_reward: Address) -> anyhow::Result<()> {

    let sender = slashing_data.sender;
    let recovered_identity_secret_hash= recover(slashing_data)?;
    rln_sc.slash(sender, recovered_identity_secret_hash, account_to_reward)
        .await
        .context("Failed to call slash on RLN SC")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};
    use alloy::primitives::address;
    use super::*;
    use ark_bn254::Fr;
    use ark_serialize::CanonicalSerialize;
    use rln::circuit::{zkey_from_folder, Curve};
    use rln::hashers::{hash_to_field_le, poseidon_hash};
    use rln::poseidon_tree::PoseidonTree;
    use rln::protocol::{generate_proof, keygen, proof_values_from_witness, rln_witness_from_values, serialize_proof_values, RLNProofValues};
    use rln::utils::IdSecret;
    use zerokit_utils::{ZerokitMerkleProof, ZerokitMerkleTree};
    use crate::prover_proto::{RlnAggProof, RlnProof};

    const addr_alice: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
    const addr_bob: Address = address!("0x70997970C51812dc3A010C7d01b50e0d17dc79C8");

    fn serialize_proof(proof: Proof<Curve>, proof_values: RLNProofValues) -> Vec<u8> {
        let mut output_buffer = Cursor::new(Vec::new());
        if let Err(e) = proof.serialize_compressed(&mut output_buffer) {
            panic!();
        }
        if let Err(e) = output_buffer.write_all(&serialize_proof_values(&proof_values)) {
            panic!();
        }
        output_buffer.into_inner()
    }

    #[test]
    fn test_recover() {

        let (user_secret, user_co) = keygen();
        let epoch = hash_to_field_le(b"foo");
        let spam_limit = Fr::from(10);

        // let mut tree = OptimalMerkleTree::new(20, Default::default(), Default::default()).unwrap();
        let mut tree = PoseidonTree::new(20, Default::default(), Default::default()).unwrap();
        tree.set(0, spam_limit).unwrap();
        let m_proof = tree.proof(0).unwrap();

        let (rln_identifier, pk, matrices, graph_bytes) = {
            // RlnIdentifier::new(b"rln id test");
            let (pk, matrices) = zkey_from_folder();
            // Load the graph.bin file that's compatible with rln 0.9.0
            // This was copied from rln-0.9.0/resources/tree_depth_20/graph.bin
            let graph_bytes = include_bytes!("../resources/graph.bin");

            (hash_to_field_le(b"rln id test"), pk.clone(), matrices.clone(), graph_bytes)
        };

        let message_id = Fr::from(1);

        let (proof_0, proof_values_0) = {
            let external_nullifier = poseidon_hash(&[rln_identifier, epoch]);
            let witness = rln_witness_from_values(
                user_secret.clone(),
                m_proof.get_path_elements(),
                m_proof.get_path_index(),
                hash_to_field_le(b"sig"),
                external_nullifier,
                spam_limit,
                message_id
            ).unwrap();

            let proof_values = proof_values_from_witness(&witness).unwrap();
            let proof = generate_proof(
                &(pk.clone(), matrices.clone()),
                &witness,
                graph_bytes,
            ).unwrap();

            (proof, proof_values)
        };

        let (proof_1, proof_values_1) = {

            let external_nullifier = poseidon_hash(&[rln_identifier, epoch]);
            let witness = rln_witness_from_values(
                user_secret.clone(),
                m_proof.get_path_elements(),
                m_proof.get_path_index(),
                hash_to_field_le(b"sig 2"),
                external_nullifier,
                spam_limit,
                message_id,
            ).unwrap();

            let proof_values = proof_values_from_witness(&witness).unwrap();
            let proof = generate_proof(
                &(pk, matrices),
                &witness,
                graph_bytes,
            ).unwrap();

            (proof, proof_values)


        };

        let share1 = (proof_values_0.x, proof_values_0.y);
        let share2 = (proof_values_1.x, proof_values_1.y);
        let recovered_identity_secret_hash = compute_id_secret(share1, share2).unwrap();
        assert_eq!(user_secret, recovered_identity_secret_hash);

        // Now, serialize this to RlnAggProof and try to recover secret

        let proof_0_se = serialize_proof(proof_0, proof_values_0);
        let proof_1_se = serialize_proof(proof_1, proof_values_1);
        let slashing_data = SlashingData {
            proof_1: RlnAggProof {
                sender: addr_bob.to_vec(),
                tx_hash: vec![0; 32],
                proof: proof_0_se,
                epoch: 0,
            },
            proof_2: RlnAggProof {
                sender: addr_bob.to_vec(),
                tx_hash: vec![1; 32],
                proof: proof_1_se,
                epoch: 0,
            },
            sender: addr_alice,
        };
        let rec = recover(slashing_data).unwrap();
        assert_eq!(user_secret, rec);
    }


}