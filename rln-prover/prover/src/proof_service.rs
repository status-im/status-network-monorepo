use std::io::{Cursor, Write};
use std::sync::Arc;
// third-party
use ark_bn254::Fr;
use ark_serialize::CanonicalSerialize;
use async_channel::Receiver;
use metrics::{counter, histogram};
use parking_lot::RwLock;
use rln::hashers::hash_to_field_le;
use rln::protocol::serialize_proof_values;
use tracing::{Instrument, debug_span, error, info};
// internal
use crate::epoch_service::{Epoch, EpochSlice};
use crate::error::{AppError, ProofGenerationError, ProofGenerationStringError};
use crate::metrics::{
    BROADCAST_CHANNEL_QUEUE_LEN, PROOF_SERVICE_GEN_PROOF_TIME, PROOF_SERVICE_PROOF_COMPUTED,
};
use crate::proof_generation::{ProofGenerationData, ProofSendingData};
use crate::user_db::UserDb;
use crate::user_db_types::RateLimit;
use rln_proof::{RlnData, compute_rln_proof_and_values};

const PROOF_SIZE: usize = 512;

/// A service to generate a RLN proof (and then to broadcast it)
pub struct ProofService {
    receiver: Receiver<ProofGenerationData>,
    broadcast_sender:
        tokio::sync::broadcast::Sender<Result<ProofSendingData, ProofGenerationStringError>>,
    current_epoch: Arc<RwLock<(Epoch, EpochSlice)>>,
    user_db: UserDb,
    rate_limit: RateLimit,
    id: u64,
}

impl ProofService {
    pub(crate) fn new(
        receiver: Receiver<ProofGenerationData>,
        broadcast_sender: tokio::sync::broadcast::Sender<
            Result<ProofSendingData, ProofGenerationStringError>,
        >,
        current_epoch: Arc<RwLock<(Epoch, EpochSlice)>>,
        user_db: UserDb,
        rate_limit: RateLimit,
        id: u64,
    ) -> Self {
        debug_assert!(rate_limit > RateLimit::ZERO);
        Self {
            receiver,
            broadcast_sender,
            current_epoch,
            user_db,
            rate_limit,
            id,
        }
    }

    pub(crate) async fn serve(&self) -> Result<(), AppError> {
        loop {
            let received = self.receiver.recv().await;

            if let Err(e) = received {
                info!("Stopping proof generation service: {}", e);
                break;
            }

            let proof_generation_data = received.unwrap();

            let (current_epoch, current_epoch_slice) = *self.current_epoch.read();
            let user_db = self.user_db.clone();
            let proof_generation_data_ = proof_generation_data.clone();
            let rate_limit = self.rate_limit;

            let counter_id = self.id;
            // println!("[proof service {counter_id}] starting to generate proof...");

            // Communicate between rayon & current task
            let (send, recv) = tokio::sync::oneshot::channel();

            // Move to a task (as generating the proof can take quite some time) - avoid blocking the tokio runtime
            // Note: avoid tokio spawn_blocking as it does not perform great for CPU bounds tasks
            //       see https://ryhl.io/blog/async-what-is-blocking/

            rayon::spawn(move || {
                let proof_generation_start = std::time::Instant::now();

                let message_id = {
                    let mut m_id = proof_generation_data.tx_counter;
                    // Note: Zerokit can only recover user secret hash with 2 messages with the
                    //       same message_id so here we force to use the previous message_id
                    //       so the Verifier could recover the secret hash
                    if RateLimit::from(m_id) == rate_limit {
                        m_id -= 1;
                    }
                    m_id
                };

                let rln_data = RlnData {
                    message_id: Fr::from(message_id),
                    data: hash_to_field_le(proof_generation_data.tx_hash.as_slice()),
                };

                let epoch_bytes = {
                    let mut v = current_epoch.to_le_bytes().to_vec();
                    v.extend(current_epoch_slice.to_le_bytes());
                    v
                };
                let epoch = hash_to_field_le(epoch_bytes.as_slice());

                let merkle_proof = match user_db.get_merkle_proof(&proof_generation_data.tx_sender)
                {
                    Ok(merkle_proof) => merkle_proof,
                    Err(e) => {
                        let _ = send.send(Err(ProofGenerationError::MerkleProofError(e)));
                        return;
                    }
                };

                // let compute_proof_start = std::time::Instant::now();
                let (proof, proof_values) = match compute_rln_proof_and_values(
                    &proof_generation_data.user_identity,
                    &proof_generation_data.rln_identifier,
                    rln_data,
                    epoch,
                    &merkle_proof,
                ) {
                    Ok((proof, proof_values)) => (proof, proof_values),
                    Err(e) => {
                        let _ = send.send(Err(ProofGenerationError::Proof(e)));
                        return;
                    }
                };

                // debug!("proof: {:?}", proof);
                // debug!("proof_values: {:?}", proof_values);

                // Serialize proof
                let mut output_buffer = Cursor::new(Vec::with_capacity(PROOF_SIZE));
                if let Err(e) = proof.serialize_compressed(&mut output_buffer) {
                    let _ = send.send(Err(ProofGenerationError::Serialization(e)));
                    return;
                }
                if let Err(e) = output_buffer.write_all(&serialize_proof_values(&proof_values)) {
                    let _ = send.send(Err(ProofGenerationError::SerializationWrite(e)));
                    return;
                }

                histogram!(PROOF_SERVICE_GEN_PROOF_TIME.name, "prover" => "proof service")
                    .record(proof_generation_start.elapsed().as_secs_f64());
                // println!("[proof service {counter_id}] proof generation time: {:?} secs", proof_generation_start.elapsed().as_secs_f64());
                let labels = [("prover", format!("proof service id: {counter_id}"))];
                counter!(PROOF_SERVICE_PROOF_COMPUTED.name, &labels).increment(1);

                // Send the result back to Tokio.
                let _ = send.send(Ok::<Vec<u8>, ProofGenerationError>(
                    output_buffer.into_inner(),
                ));

                /*
                std::thread::sleep(std::time::Duration::from_millis(100));
                let mut output_buffer = Cursor::new(Vec::with_capacity(PROOF_SIZE));
                // Send the result back to Tokio.
                let _ = send.send(Ok::<Vec<u8>, ProofGenerationError>(
                    output_buffer.into_inner(),
                ));
                */
            });

            // Wait for the rayon task.
            // Result 1st is from send channel (no errors expected)
            // Result 2nd can be a ProofGenerationError
            let result = recv
                .instrument(debug_span!("compute proof"))
                .await
                .expect("Panic in rayon::spawn"); // Should never happen (but panic if it does)

            let proof_sending_data = result
                .map(|r| ProofSendingData {
                    tx_hash: proof_generation_data_.tx_hash,
                    tx_sender: proof_generation_data_.tx_sender,
                    proof: r,
                })
                .map_err(ProofGenerationStringError::from);

            if proof_sending_data.is_err() {
                error!(
                    "[proof service {counter_id}] error: {:?}",
                    proof_sending_data
                );
            }

            if let Err(e) = self.broadcast_sender.send(proof_sending_data) {
                info!("Stopping proof generation service: {}", e);
                break;
            };

            // Note: based on this link https://doc.rust-lang.org/reference/expressions/operator-expr.html#type-cast-expressions
            //       "Casting from an integer to float will produce the closest possible float *"
            histogram!(BROADCAST_CHANNEL_QUEUE_LEN.name, "prover" => "proof service")
                .record(self.broadcast_sender.len() as f64);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // std
    use std::path::PathBuf;
    // third-party
    use alloy::primitives::{Address, address};
    use ark_groth16::{Proof as ArkProof, VerifyingKey};
    use ark_serialize::CanonicalDeserialize;
    use claims::assert_matches;
    use futures::TryFutureExt;
    use tokio::sync::broadcast;
    use tracing::{debug, info};
    // third-party: zerokit
    use rln::{
        circuit::{Curve, zkey_from_folder},
        protocol::{deserialize_proof_values, verify_proof},
    };
    // internal
    use crate::user_db::{MERKLE_TREE_HEIGHT, UserDbConfig};
    use crate::user_db_service::UserDbService;
    use rln_proof::RlnIdentifier;

    const ADDR_1: Address = address!("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    const ADDR_2: Address = address!("0xb20a608c624Ca5003905aA834De7156C68b2E1d0");

    const TX_HASH_1: [u8; 32] = [0x011; 32];

    #[derive(thiserror::Error, Debug)]
    enum AppErrorExt {
        #[error("AppError: {0}")]
        AppError(#[from] AppError),
        #[error("Future timeout")]
        Elapsed,
        #[error("Proof generation failed: {0}")]
        ProofGeneration(#[from] ProofGenerationStringError),
        #[error("Proof verification failed")]
        ProofVerification,
        #[error("Exiting...")]
        Exit,
    }

    async fn proof_sender(
        sender: Address,
        proof_tx: &mut async_channel::Sender<ProofGenerationData>,
        rln_identifier: Arc<RlnIdentifier>,
        user_db: &UserDb,
    ) -> Result<(), AppErrorExt> {
        // used by test_proof_generation unit test

        debug!("Starting proof sender...");
        debug!("Waiting a bit before sending proof...");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        debug!("Sending proof...");
        proof_tx
            .send(ProofGenerationData {
                user_identity: user_db.get_user(&ADDR_1).unwrap(),
                rln_identifier,
                tx_counter: 0,
                tx_sender: sender,
                tx_hash: TX_HASH_1.to_vec(),
            })
            .await
            .unwrap();
        debug!("Sending proof done");
        // tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        Ok::<(), AppErrorExt>(())
    }

    async fn proof_verifier(
        broadcast_receiver: &mut broadcast::Receiver<
            Result<ProofSendingData, ProofGenerationStringError>,
        >,
        verifying_key: &VerifyingKey<Curve>,
    ) -> Result<(), AppErrorExt> {
        // used by test_proof_generation unit test

        debug!("Starting broadcast receiver...");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let res =
            tokio::time::timeout(std::time::Duration::from_secs(10), broadcast_receiver.recv())
                .await
                .map_err(|_e| AppErrorExt::Elapsed)?;
        debug!("res: {:?}", res);

        let res = res.unwrap();
        let res = res?;
        let mut proof_cursor = Cursor::new(&res.proof);
        debug!("proof cursor: {:?}", proof_cursor);
        let proof = ArkProof::deserialize_compressed(&mut proof_cursor).unwrap();
        let position = proof_cursor.position() as usize;
        let proof_cursor_2 = &proof_cursor.get_ref().as_slice()[position..];
        let (proof_values, _) = deserialize_proof_values(proof_cursor_2);
        debug!("[proof verifier] proof: {:?}", proof);
        debug!("[proof verifier] proof_values: {:?}", proof_values);

        let verified = verify_proof(verifying_key, &proof, &proof_values)
            .map_err(|_e| AppErrorExt::ProofVerification)?;

        debug!("verified: {:?}", verified);

        // Exit after receiving one proof
        Err::<(), AppErrorExt>(AppErrorExt::Exit)
    }

    #[tokio::test]
    // #[tracing_test::traced_test]
    async fn test_proof_generation() {
        // Queues
        let (broadcast_sender, _broadcast_receiver) = broadcast::channel(2);
        let mut broadcast_receiver = broadcast_sender.subscribe();
        let (mut proof_tx, proof_rx) = async_channel::unbounded();

        // Epoch
        let epoch = Epoch::from(11);
        let epoch_slice = EpochSlice::from(42);
        let epoch_store = Arc::new(RwLock::new((epoch, epoch_slice)));

        // User db
        let temp_folder = tempfile::tempdir().unwrap();
        let temp_folder_tree = tempfile::tempdir().unwrap();
        let config = UserDbConfig {
            db_path: PathBuf::from(temp_folder.path()),
            merkle_tree_folder: PathBuf::from(temp_folder_tree.path()),
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };

        let user_db_service = UserDbService::new(
            config,
            Default::default(),
            epoch_store.clone(),
            10.into(),
            Default::default(),
        )
        .unwrap();
        let user_db = user_db_service.get_user_db();
        user_db.on_new_user(&ADDR_1).unwrap();
        user_db.on_new_user(&ADDR_2).unwrap();

        let rln_identifier = Arc::new(RlnIdentifier::new(b"foo bar baz"));

        // Proof service
        let proof_service = ProofService::new(
            proof_rx,
            broadcast_sender,
            epoch_store,
            user_db.clone(),
            RateLimit::from(10),
            0,
        );

        // Verification
        let proving_key = zkey_from_folder();
        let verification_key = &proving_key.0.vk;

        info!("Starting...");
        let res = tokio::try_join!(
            proof_service.serve().map_err(AppErrorExt::AppError),
            proof_verifier(&mut broadcast_receiver, verification_key),
            proof_sender(ADDR_1, &mut proof_tx, rln_identifier.clone(), &user_db),
        );

        // Everything ok if proof_verifier return AppErrorExt::Exit else there is a real error
        assert_matches!(res, Err(AppErrorExt::Exit));
    }
}
