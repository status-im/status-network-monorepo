use tokio::sync::mpsc::Receiver;
use tracing::{debug, warn};
// RLN
use rln::protocol::deserialize_proof_values;
use rln::utils::fr_to_bytes_le;
// internal
use crate::prover_proto::{RlnAggLightProofReply, RlnProofReply};
use crate::prover_proto::rln_proof_reply::Resp;

pub struct ProofReduceService {
    pub(crate) receiver: Receiver<RlnProofReply>,
    pub(crate) bcast_sender: tokio::sync::broadcast::Sender<RlnAggLightProofReply>,
}

impl ProofReduceService {

    pub(crate) fn new(receiver: Receiver<RlnProofReply>, bcast_sender: tokio::sync::broadcast::Sender<RlnAggLightProofReply>) -> Self {
        Self {
            receiver,
            bcast_sender
        }
    }

    pub(crate) async fn serve(&mut self) -> anyhow::Result<()> {

        loop {

            let res = self.receiver.recv().await;
            if res.is_none() {
                warn!("ProofReduceService::serve: receiver closed");
            }
            let res = res.unwrap();

            let light_proof = RlnAggLightProofReply::try_from(res).unwrap();

            debug!("light_proof: {:?}", light_proof);

            let start = std::time::Instant::now();
            if let Err(e) = self.bcast_sender.send(light_proof) {
                warn!("[Proof reduce service] Client disconnected during send: {}", e);
                break;
            };
            let _elapsed = start.elapsed();

            debug!("Sent RlnAggLightProofReply in {} secs", _elapsed.as_secs_f64());
        }

        Ok(())
    }
}

impl TryFrom<RlnProofReply> for RlnAggLightProofReply {

    type Error = ();

    fn try_from(pr: RlnProofReply) -> Result<Self, Self::Error> {
        match pr.resp {
            Some(r) => match r {

                Resp::Proof(p) => {

                    // Note: First 128 bytes are the Proof<Bn254>, no need to deserialize it
                    // let _proof_1_de: Proof<Bn254> =
                    //     CanonicalDeserialize::deserialize_compressed(&proof_1.proof[..128])
                    //     .context("Failed to deserialize proof 1")?;
                    let proof_values_de = deserialize_proof_values(&p.proof.as_slice()[128..]).0;

                    Ok(Self {
                        sender: p.sender,
                        proof_x: fr_to_bytes_le(&proof_values_de.x),
                        proof_y: fr_to_bytes_le(&proof_values_de.y),
                        external_nullifier: fr_to_bytes_le(&proof_values_de.external_nullifier),
                        epoch: p.epoch,
                    })
                },
                Resp::Error(_e) => Err(())
            },
            None => Err(()),
        }
    }
}
