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

async fn slash<P: Provider>(rln_sc: RLNInstance<P>, slashing_data: SlashingData, account_to_reward: Address) -> anyhow::Result<()> {

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

    rln_sc.slash(slashing_data.sender, recovered_identity_secret_hash, account_to_reward).await
        .context("Failed to call slash on RLN SC")?;
    Ok(())
}