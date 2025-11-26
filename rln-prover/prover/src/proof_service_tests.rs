
#[cfg(feature = "postgres")]
#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::sync::Arc;
    // third-party
    use alloy::primitives::{Address, address};
    use ark_groth16::{Proof as ArkProof, Proof, VerifyingKey};
    use ark_serialize::CanonicalDeserialize;
    use claims::assert_matches;
    use futures::TryFutureExt;
    use parking_lot::RwLock;
    use rln::circuit::{Curve, zkey_from_folder};
    use rln::error::ComputeIdSecretError;
    use rln::protocol::{compute_id_secret, deserialize_proof_values, verify_proof};
    use rln::utils::IdSecret;
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr, Statement};
    use tokio::sync::broadcast;
    use tracing::{debug, info};
    // internal
    use crate::epoch_service::{Epoch, EpochSlice};
    use crate::error::{AppError, AppError2, ProofGenerationStringError};
    use crate::proof_generation::{ProofGenerationData, ProofSendingData};
    use crate::proof_service::ProofService;
    // use crate::user_db::{MERKLE_TREE_HEIGHT, UserDb, UserDbConfig};
    use crate::user_db_service::UserDbService;
    use crate::user_db_types::RateLimit;
    use rln_proof::RlnIdentifier;
    use crate::user_db::MERKLE_TREE_HEIGHT;
    use crate::user_db_2::{UserDb2, UserDb2Config};
    use prover_db_migration::{Migrator as MigratorCreate, MigratorTrait};

    const ADDR_1: Address = address!("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    const ADDR_2: Address = address!("0xb20a608c624Ca5003905aA834De7156C68b2E1d0");

    const TX_HASH_1: [u8; 32] = [0x011; 32];
    const TX_HASH_1_2: [u8; 32] = [0x12; 32];

    // Increase timeout to reduce CI flakiness on slower runners.
    const PROOF_VERIFY_TEST_TIMEOUT: u64 = 30;

    #[derive(thiserror::Error, Debug)]
    enum AppErrorExt {
        #[error("AppError: {0}")]
        AppError(#[from] AppError2),
        #[error("Future timeout")]
        Elapsed,
        #[error("Proof generation failed: {0}")]
        ProofGeneration(#[from] ProofGenerationStringError),
        #[error("Proof verification failed")]
        ProofVerification,
        #[error("Exiting...")]
        Exit,
        #[error(transparent)]
        RecoverSecretFailed(ComputeIdSecretError),
        #[error("Recovered secret")]
        RecoveredSecret(IdSecret),
    }

    async fn create_database_connection(db_name: &str) -> Result<DatabaseConnection, DbErr> {

        // Drop / Create db_name then return a connection to it

        let db_url_base = "postgres://myuser:mysecretpassword@localhost";
        let db_url = format!("{}/{}", db_url_base, "mydatabase");
        let db = Database::connect(db_url)
            .await
            .expect("Database connection 0 failed");

        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            format!("DROP DATABASE IF EXISTS \"{}\";", db_name),
        ))
            .await?;
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            format!("CREATE DATABASE \"{}\";", db_name),
        ))
            .await?;

        db.close().await?;

        let db_url_final = format!("{}/{}", db_url_base, db_name);
        let db = Database::connect(db_url_final)
            .await
            .expect("Database connection failed");
        MigratorCreate::up(&db, None).await?;

        Ok(db)
    }

    async fn proof_sender(
        sender: Address,
        proof_tx: &mut async_channel::Sender<ProofGenerationData>,
        rln_identifier: Arc<RlnIdentifier>,
        user_db: &UserDb2,
    ) -> Result<(), AppErrorExt> {
        // used by test_proof_generation unit test

        debug!("Starting proof sender...");
        debug!("Waiting a bit before sending proof...");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        debug!("Sending proof...");

        let user_identity = user_db.get_user_identity(&ADDR_1).await.unwrap();
        proof_tx
            .send(ProofGenerationData {
                user_identity,
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
        let res = tokio::time::timeout(
            std::time::Duration::from_secs(PROOF_VERIFY_TEST_TIMEOUT),
            broadcast_receiver.recv(),
        )
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
    #[tracing_test::traced_test]
    async fn test_user_not_registered() {
        // Ask for a proof for an unregistered user

        // Queues
        let (broadcast_sender, _broadcast_receiver) = broadcast::channel(2);
        let mut broadcast_receiver = broadcast_sender.subscribe();
        let (mut proof_tx, proof_rx) = async_channel::unbounded();

        // Epoch
        let epoch = Epoch::from(11);
        let epoch_slice = EpochSlice::from(42);
        let epoch_store = Arc::new(RwLock::new((epoch, epoch_slice)));

        // User db
        let config = UserDb2Config {
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };

        let db_conn = create_database_connection("proof_service_tests_test_user_not_registered")
            .await.unwrap();

        let user_db_service = UserDbService::new(
            db_conn,
            config,
            Default::default(),
            epoch_store.clone(),
            10.into(),
            Default::default(),
        )
        .await.unwrap();
        let user_db = user_db_service.get_user_db();
        user_db.on_new_user(&ADDR_1).await.unwrap();
        // user_db.on_new_user(ADDR_2).unwrap();

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
            proof_sender(ADDR_2, &mut proof_tx, rln_identifier.clone(), &user_db),
        );

        // Expect this error (any other error is a real error)
        assert_matches!(
            res,
            Err(AppErrorExt::ProofGeneration(
                ProofGenerationStringError::MerkleProofError(_)
            ))
        );
    }

    async fn proof_reveal_secret(
        broadcast_receiver: &mut broadcast::Receiver<
            Result<ProofSendingData, ProofGenerationStringError>,
        >,
        // verifying_key: &VerifyingKey<Curve>,
    ) -> Result<(), AppErrorExt> {
        // used by test_user_spamming unit test

        debug!("Starting broadcast receiver...");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let mut proof_values_store = vec![];

        loop {
            let res = tokio::time::timeout(
                std::time::Duration::from_secs(PROOF_VERIFY_TEST_TIMEOUT),
                broadcast_receiver.recv(),
            )
            .await
            .map_err(|_e| AppErrorExt::Elapsed)?;

            let res = res.unwrap();
            let res = res?;
            let mut proof_cursor = Cursor::new(&res.proof);
            let _proof: Proof<Curve> = ArkProof::deserialize_compressed(&mut proof_cursor).unwrap();
            let position = proof_cursor.position() as usize;
            let proof_cursor_2 = &proof_cursor.get_ref().as_slice()[position..];
            let (proof_values, _) = deserialize_proof_values(proof_cursor_2);
            proof_values_store.push(proof_values);
            if proof_values_store.len() >= 2 {
                break;
            }
        }

        debug!("Now recovering secret hash...");
        let proof_values_0 = proof_values_store.first().unwrap();
        let proof_values_1 = proof_values_store.get(1).unwrap();
        println!("proof_values_0: {proof_values_0:?}");
        println!("proof_values_1: {proof_values_1:?}");
        let share1 = (proof_values_0.x, proof_values_0.y);
        let share2 = (proof_values_1.x, proof_values_1.y);

        // Note: if not in test, should check for external nullifier
        let recovered_identity_secret_hash =
            compute_id_secret(share1, share2).map_err(AppErrorExt::RecoverSecretFailed)?;

        debug!(
            "recovered_identity_secret_hash: {:?}",
            recovered_identity_secret_hash
        );

        // Exit after receiving one proof
        Err::<(), AppErrorExt>(AppErrorExt::RecoveredSecret(recovered_identity_secret_hash))
    }

    async fn proof_sender_2(
        proof_tx: &mut async_channel::Sender<ProofGenerationData>,
        rln_identifier: Arc<RlnIdentifier>,
        user_db: &UserDb2,
        sender: Address,
        tx_hashes: ([u8; 32], [u8; 32]),
    ) -> Result<(), AppErrorExt> {
        // used by test_proof_generation unit test

        debug!("Starting proof sender 2...");
        debug!("Waiting a bit before sending proof...");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        debug!("Sending proof...");
        let user_identity = user_db.get_user_identity(&sender).await.unwrap();
        proof_tx
            .send(ProofGenerationData {
                user_identity,
                rln_identifier: rln_identifier.clone(),
                tx_counter: 0,
                tx_sender: sender,
                tx_hash: tx_hashes.0.to_vec(),
            })
            .await
            .unwrap();
        debug!("Sending proof done");

        debug!("Waiting a bit before sending 2nd proof...");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        debug!("Sending 2nd proof...");
        let user_identity = user_db.get_user_identity(&sender).await.unwrap();
        proof_tx
            .send(ProofGenerationData {
                user_identity,
                rln_identifier,
                tx_counter: 1,
                tx_sender: sender,
                tx_hash: tx_hashes.1.to_vec(),
            })
            .await
            .unwrap();
        debug!("Sending 2nd proof done");

        Ok::<(), AppErrorExt>(())
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_user_spamming() {
        // Recover secret from a user spamming the system

        // Queues
        let (broadcast_sender, _broadcast_receiver) = broadcast::channel(2);
        let mut broadcast_receiver = broadcast_sender.subscribe();
        let (mut proof_tx, proof_rx) = async_channel::unbounded();

        // Epoch
        let epoch = Epoch::from(11);
        let epoch_slice = EpochSlice::from(42);
        let epoch_store = Arc::new(RwLock::new((epoch, epoch_slice)));

        // Limits
        let rate_limit = RateLimit::from(1);

        // User db
        let config = UserDb2Config {
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };
        let db_conn = create_database_connection("proof_service_tests_test_user_spamming").await.unwrap();
        let user_db_service = UserDbService::new(
            db_conn,
            config,
            Default::default(),
            epoch_store.clone(),
            rate_limit,
            Default::default(),
        )
        .await.unwrap();
        let user_db = user_db_service.get_user_db();
        user_db.on_new_user(&ADDR_1).await.unwrap();
        // let user_addr_1 = user_db.get_user(&ADDR_1).await.unwrap().unwrap();

        let user_addr1_identity = user_db.get_user_identity(&ADDR_1).await.unwrap();

        user_db.on_new_user(&ADDR_2).await.unwrap();

        let rln_identifier = Arc::new(RlnIdentifier::new(b"foo bar baz"));

        // Proof service
        let proof_service = ProofService::new(
            proof_rx,
            broadcast_sender,
            epoch_store,
            user_db.clone(),
            rate_limit,
            0,
        );

        info!("Starting...");
        let res = tokio::try_join!(
            proof_service.serve().map_err(AppErrorExt::AppError),
            proof_reveal_secret(&mut broadcast_receiver),
            proof_sender_2(
                &mut proof_tx,
                rln_identifier.clone(),
                &user_db,
                ADDR_1,
                (TX_HASH_1, TX_HASH_1_2)
            ),
        );

        match res {
            Err(AppErrorExt::RecoveredSecret(secret_hash)) => {
                assert_eq!(secret_hash, user_addr1_identity.secret_hash);
            }
            _ => {
                panic!("Expected to RecoveredSecret, got: {res:?}");
            }
        }
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_user_spamming_same_signal() {
        // Recover secret from a user spamming the system

        // Queues
        let (broadcast_sender, _broadcast_receiver) = broadcast::channel(2);
        let mut broadcast_receiver = broadcast_sender.subscribe();
        let (mut proof_tx, proof_rx) = async_channel::unbounded();

        // Epoch
        let epoch = Epoch::from(11);
        let epoch_slice = EpochSlice::from(42);
        let epoch_store = Arc::new(RwLock::new((epoch, epoch_slice)));

        // Limits
        let rate_limit = RateLimit::from(1);

        // User db - limit is 1 message per epoch
        let config = UserDb2Config {
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };
        let db_conn = create_database_connection("proof_service_tests_test_user_spamming_same_signal").await.unwrap();
        let user_db_service = UserDbService::new(
            db_conn,
            config,
            Default::default(),
            epoch_store.clone(),
            rate_limit,
            Default::default(),
        )
        .await.unwrap();
        let user_db = user_db_service.get_user_db();
        user_db.on_new_user(&ADDR_1).await.unwrap();
        let user_addr_1 = user_db.get_user(&ADDR_1).await.unwrap();
        debug!("user_addr_1: {:?}", user_addr_1);
        user_db.on_new_user(&ADDR_2).await.unwrap();

        let rln_identifier = Arc::new(RlnIdentifier::new(b"foo bar baz"));

        // Proof service
        let proof_service = ProofService::new(
            proof_rx,
            broadcast_sender,
            epoch_store,
            user_db.clone(),
            rate_limit,
            0,
        );

        info!("Starting...");
        let res = tokio::try_join!(
            proof_service.serve().map_err(AppErrorExt::AppError),
            proof_reveal_secret(&mut broadcast_receiver),
            proof_sender_2(
                &mut proof_tx,
                rln_identifier.clone(),
                &user_db,
                ADDR_1,
                (TX_HASH_1, TX_HASH_1)
            ),
        );

        assert_matches!(res, Err(AppErrorExt::RecoverSecretFailed(_)));
    }
}
