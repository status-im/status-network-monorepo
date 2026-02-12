use std::collections::HashMap;
use std::sync::Arc;
// use parking_lot::RwLock;
use crate::prover_proto::RlnAggProof;
use alloy::primitives::Address;
use tokio::sync::RwLock;
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{error, info, warn};

pub(crate) struct ProofProcessService {
    config: ProofProcessConfig,
    db: Arc<RwLock<Db>>,
    proof_rx: Receiver<RlnAggProof>,
    current_epoch: Option<u64>,
    slashing_tx: Sender<(RlnAggProof, RlnAggProof)>,
}

impl ProofProcessService {
    pub(crate) fn new(
        config: ProofProcessConfig,
        proof_rx: Receiver<RlnAggProof>,
        slashing_tx: Sender<(RlnAggProof, RlnAggProof)>,
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
        let sender_addr = Address::try_from(sender_addr);

        let sender_addr = match sender_addr {
            Ok(sender_addr) => sender_addr,
            Err(e) => {
                warn!("Received an invalid sender address: {}", e);
                return Err(ProofProcessError::InvalidSender);
            }
        };

        let mut guard = self.db.write().await;

        // TODO
        /*
        let current_epoch = match current_epoch {
            Some(v) => {
                // Epoch has changed
                if *v < proof.epoch {
                    self.0.clear();
                    debug!("New epoch: {}, resetting db...", proof.epoch);
                    Some(proof.epoch)
                } else if *v == proof.epoch {
                    Some(*v) // Same epoch - only store proof here
                } else {
                    // Decreasing epoch ?? - aborting...
                    error!("Slasher current epoch is {} but received new epoch: {}, aborting...", v, res.epoch);
                    break;
                }
            },
            None => Some(res.epoch) // Initial epoch right after slasher app starts
        };
        */

        let db_entry = guard.insert_proof(&sender_addr, &proof);

        if db_entry.seen_proof_count > self.config.rln_limit {
            info!("Detected too many messages for address: {:?}", sender_addr);
            self.slashing_tx.send((
                db_entry.proof_1.unwrap(),
                db_entry.proof_2.unwrap()
            ))
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
            .entry(addr.clone())
            .and_modify(|db_e| {
                // Note: rln-prover manually tweaks the RLN message id if there is a spam
                //       this allows the slasher to keep only the two last proofs received
                db_e.set_proof(&proof);
            })
            .or_insert_with(|| {
                let mut db_e = DbEntry::default();
                db_e.set_proof(&proof);
                db_e
            });

        e.clone()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DbEntry {
    proof_1: Option<RlnAggProof>,
    proof_2: Option<RlnAggProof>,
    seen_proof_count: u64,
}

impl Default for DbEntry {
    fn default() -> Self {
        Self {
            proof_1: None,
            proof_2: None,
            seen_proof_count: 0,
        }
    }
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
    #[error(transparent)]
    SendForSlasing(#[from] tokio::sync::mpsc::error::SendError<(RlnAggProof, RlnAggProof)>),
}
