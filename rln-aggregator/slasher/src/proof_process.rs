use std::collections::HashMap;
use std::sync::Arc;
// third-party
use alloy::primitives::Address;
use tokio::{
    sync::RwLock,
    sync::mpsc::{Receiver, Sender},
};
use tracing::{debug, error, info, warn};
// internal - Grpc
use crate::common::SlashingData;
use crate::prover_proto::RlnAggProof;

pub(crate) struct ProofProcessService {
    config: ProofProcessConfig,
    db: Arc<RwLock<Db>>,
    proof_rx: Receiver<RlnAggProof>,
    current_epoch: Option<u64>,
    slashing_tx: Sender<SlashingData>,
}

impl ProofProcessService {
    pub(crate) fn new(
        config: ProofProcessConfig,
        proof_rx: Receiver<RlnAggProof>,
        slashing_tx: Sender<SlashingData>,
    ) -> Self {
        Self {
            config,
            db: Default::default(),
            proof_rx,
            current_epoch: None,
            slashing_tx,
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) async fn serve(&mut self) -> anyhow::Result<()> {
        loop {
            let res = self.proof_rx.recv().await;
            if let Some(proof) = res {
                if let Err(e) = self.proof_process(proof).await {
                    match e {
                        ProofProcessError::InvalidSender => {
                            continue;
                        }
                        ProofProcessError::DecreasingEpoch => {
                            break;
                        }
                        ProofProcessError::SendForSlasing(_) => {
                            error!("Cannot send to slashing_tx, aborting...");
                            break;
                        }
                    }
                }
            } else {
                warn!("Channel has been closed");
                break;
            }
        }

        Ok(())
    }

    async fn proof_process(&mut self, proof: RlnAggProof) -> Result<(), ProofProcessError> {
        if proof.sender.len() != Address::len_bytes() {
            warn!(
                "Received an invalid sender address: invalid length: {}",
                proof.sender.len()
            );
            return Err(ProofProcessError::InvalidSender);
        }

        // Unwrap safe: length has already been tested
        let sender_addr: &[u8; Address::len_bytes()] = proof.sender.as_slice().try_into().unwrap();
        let sender_addr = Address::from(sender_addr);

        let mut guard = self.db.write().await;

        self.current_epoch = match self.current_epoch {
            Some(current_epoch) => {
                if current_epoch < proof.epoch {
                    guard.0.clear();
                    debug!("New epoch: {}, resetting db...", proof.epoch);
                    Some(proof.epoch)
                } else if current_epoch == proof.epoch {
                    Some(current_epoch)
                } else {
                    // Decreasing epoch WTF? - aborting...
                    error!(
                        "Slasher current epoch is {} but received new epoch: {}, aborting...",
                        current_epoch, proof.epoch
                    );
                    return Err(ProofProcessError::DecreasingEpoch);
                }
            }
            None => Some(proof.epoch),
        };

        let db_entry = guard.insert_proof(&sender_addr, &proof);

        if db_entry.seen_proof_count >= self.config.rln_limit {
            info!("Detected too many messages for address: {:?}", sender_addr);

            let slashing_data = SlashingData {
                proof_1: db_entry.proof_1.unwrap(),
                proof_2: db_entry.proof_2.unwrap(),
                sender: sender_addr,
            };

            self.slashing_tx.send(slashing_data)
                .await
                // .context(
                //     format!("Failed to send proof to slashing task, db_entry: {:?}", db_entry)
                // )
                ?;
        }

        drop(guard);

        Ok(())
    }
}

#[derive(Default)]
struct Db(HashMap<Address, DbEntry>);

impl Db {
    fn insert_proof(&mut self, addr: &Address, proof: &RlnAggProof) -> DbEntry {
        let e = self
            .0
            .entry(*addr)
            .and_modify(|db_e| {
                // Note: rln-prover manually tweaks the RLN message id if there is a spam
                //       this allows the slasher to keep only the two last proofs received
                db_e.set_proof(proof);
            })
            .or_insert_with(|| {
                let mut db_e = DbEntry::default();
                db_e.set_proof(proof);
                db_e
            });

        e.clone()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DbEntry {
    proof_1: Option<RlnAggProof>,
    proof_2: Option<RlnAggProof>,
    seen_proof_count: u64,
}

impl DbEntry {
    fn set_proof(&mut self, proof: &RlnAggProof) {
        let proof = proof.clone();
        if self.proof_1.is_none() {
            self.proof_1 = Some(proof);
        } else if self.proof_2.is_none() {
            self.proof_2 = Some(proof);
        } else {
            // Both proof_1 & proof_2 has been set - keep oldest (proof_2) and store new proof
            self.proof_1 = self.proof_2.clone();
            self.proof_2 = Some(proof);
        }

        self.seen_proof_count += 1;
    }
}

#[derive(Debug)]
pub(crate) struct ProofProcessConfig {
    pub(crate) rln_limit: u64,
}

#[derive(thiserror::Error, Debug)]
enum ProofProcessError {
    #[error("Invalid sender address received")]
    InvalidSender,
    #[error("Received an invalid epoch")]
    DecreasingEpoch,
    #[error(transparent)]
    SendForSlasing(#[from] tokio::sync::mpsc::error::SendError<SlashingData>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_db() {
        let config = ProofProcessConfig { rln_limit: 10 };
        let (_proof_tx, proof_rx) = tokio::sync::mpsc::channel(2);
        let (slashing_tx, _slashing_rx) = tokio::sync::mpsc::channel(2);
        let mut service = ProofProcessService::new(config, proof_rx, slashing_tx);

        let addr_1 = Address::random();
        let proof_1 = RlnAggProof {
            sender: addr_1.to_vec(),
            ..Default::default()
        };
        service.proof_process(proof_1.clone()).await.unwrap();
        {
            let guard = service.db.read().await;
            assert!(guard.0.contains_key(&addr_1));
            assert_eq!(guard.0.get(&addr_1).unwrap().proof_1, Some(proof_1.clone()));
            assert_eq!(guard.0.get(&addr_1).unwrap().proof_2, None);
            assert_eq!(guard.0.get(&addr_1).unwrap().seen_proof_count, 1);
            // drop(guard);
        }

        let proof_2 = RlnAggProof {
            sender: addr_1.to_vec(),
            ..Default::default()
        };

        service.proof_process(proof_2.clone()).await.unwrap();
        {
            let guard = service.db.read().await;
            assert!(guard.0.contains_key(&addr_1));
            assert_eq!(guard.0.get(&addr_1).unwrap().proof_1, Some(proof_1));
            assert_eq!(guard.0.get(&addr_1).unwrap().proof_2, Some(proof_2));
            assert_eq!(guard.0.get(&addr_1).unwrap().seen_proof_count, 2);
        }
    }

    #[tokio::test]
    async fn test_db_epoch_changes() {
        let config = ProofProcessConfig { rln_limit: 10 };
        let (_proof_tx, proof_rx) = tokio::sync::mpsc::channel(2);
        let (slashing_tx, _slashing_rx) = tokio::sync::mpsc::channel(2);
        let mut service = ProofProcessService::new(config, proof_rx, slashing_tx);

        let addr_1 = Address::random();
        let proof_1 = RlnAggProof {
            sender: addr_1.to_vec(),
            epoch: 0,
            ..Default::default()
        };
        service.proof_process(proof_1.clone()).await.unwrap();

        let proof_2 = RlnAggProof {
            sender: addr_1.to_vec(),
            epoch: 1, // New epoch
            ..Default::default()
        };

        service.proof_process(proof_2.clone()).await.unwrap();

        {
            assert_eq!(service.current_epoch, Some(1));
            let guard = service.db.read().await;
            assert_eq!(guard.0.get(&addr_1).unwrap().proof_1, Some(proof_2));
            assert_eq!(guard.0.get(&addr_1).unwrap().proof_2, None);
            assert_eq!(guard.0.get(&addr_1).unwrap().seen_proof_count, 1);
        }
    }

    #[tokio::test]
    async fn test_db_slashing_detect() {
        let config = ProofProcessConfig { rln_limit: 2 };
        let (_proof_tx, proof_rx) = tokio::sync::mpsc::channel(3);
        let (slashing_tx, mut slashing_rx) = tokio::sync::mpsc::channel(3);

        // Tokio task simulating the slashing service
        let handle = tokio::spawn(async move {
            loop {
                let proofs = slashing_rx.recv().await;
                if proofs.is_none() {
                    break;
                }
                let proofs = proofs.unwrap();
                return Some(proofs);
            }

            None
        });

        let mut service = ProofProcessService::new(config, proof_rx, slashing_tx);

        let addr_1 = Address::random();
        let proof_1 = RlnAggProof {
            sender: addr_1.to_vec(),
            epoch: 0,
            ..Default::default()
        };
        service.proof_process(proof_1.clone()).await.unwrap();

        let proof_2 = RlnAggProof {
            sender: addr_1.to_vec(),
            epoch: 0,
            ..Default::default()
        };
        let proof_3 = proof_2.clone();

        service.proof_process(proof_2.clone()).await.unwrap();
        service.proof_process(proof_3.clone()).await.unwrap();

        let res = tokio::time::timeout(Duration::from_secs(10), handle)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            res,
            Some(SlashingData {
                proof_1: proof_2,
                proof_2: proof_3,
                sender: addr_1,
            })
        );
    }
}
