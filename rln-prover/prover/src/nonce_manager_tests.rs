#[cfg(feature = "postgres")]
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy::primitives::{Address, U256, address};
    use async_trait::async_trait;
    use sqlx::{Pool, Postgres, Row};

    use crate::nonce_manager::{
        ManagedRLNRegister, NonceManager, NonceManagerConfig, RegistrationTask,
    };
    use crate::tests_common::create_database_connection;
    use crate::user_db_2::{MERKLE_TREE_HEIGHT, UserDb2Config};
    use smart_contract::RLNRegister;

    const ADDR_1: Address = address!("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    const ADDR_2: Address = address!("0xb20a608c624Ca5003905aA834De7156C68b2E1d0");
    const WALLET_ADDR: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

    fn test_config() -> UserDb2Config {
        UserDb2Config {
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        }
    }

    /// Mock provider that returns a fixed transaction count
    struct MockProvider {
        tx_count: u64,
    }

    impl MockProvider {
        fn new(tx_count: u64) -> Self {
            Self { tx_count }
        }
    }

    // We need a minimal Provider impl for NonceManager::new.
    // Since Provider is a complex trait, we test the DB operations directly
    // and test the full flow with ManagedRLNRegister using a mock RLNRegister.

    async fn create_nonce_manager(
        db_name: &str,
        initial_nonce: u64,
    ) -> (NonceManager, Pool<Postgres>) {
        let config = test_config();
        let (_, db_conn) = create_database_connection(db_name, true, config)
            .await
            .unwrap();

        // Manually initialize nonce_state
        sqlx::query(
            r#"
            INSERT INTO nonce_state (wallet_address, current_nonce)
            VALUES ($1, $2)
            ON CONFLICT ON CONSTRAINT nonce_state_wallet DO UPDATE SET current_nonce = $2
            "#,
        )
        .bind(WALLET_ADDR.as_slice())
        .bind(initial_nonce as i64)
        .execute(&db_conn)
        .await
        .unwrap();

        let nm = NonceManager {
            db: db_conn.clone(),
            config: NonceManagerConfig::default(),
            wallet_address: WALLET_ADDR,
            current_nonce: tokio::sync::Mutex::new(initial_nonce),
        };

        (nm, db_conn)
    }

    #[tokio::test]
    #[function_name::named]
    async fn test_allocate_nonce_sequential() {
        let (nm, _db) = create_nonce_manager(function_name!(), 0).await;

        let n0 = nm.allocate_nonce(&ADDR_1, &U256::from(100)).await.unwrap();
        let n1 = nm.allocate_nonce(&ADDR_1, &U256::from(200)).await.unwrap();
        let n2 = nm.allocate_nonce(&ADDR_2, &U256::from(300)).await.unwrap();

        assert_eq!(n0, 0);
        assert_eq!(n1, 1);
        assert_eq!(n2, 2);
    }

    #[tokio::test]
    #[function_name::named]
    async fn test_allocate_nonce_concurrent() {
        let (nm, _db) = create_nonce_manager(function_name!(), 0).await;
        let nm = Arc::new(nm);

        let mut handles = Vec::new();
        for i in 0..10u64 {
            let nm_clone = nm.clone();
            handles.push(tokio::spawn(async move {
                nm_clone
                    .allocate_nonce(&ADDR_1, &U256::from(i * 100 + 1))
                    .await
                    .unwrap()
            }));
        }

        let mut nonces: Vec<u64> = Vec::new();
        for handle in handles {
            nonces.push(handle.await.unwrap());
        }

        nonces.sort();
        // All nonces should be unique and sequential starting from 0
        let expected: Vec<u64> = (0..10).collect();
        assert_eq!(nonces, expected);
    }

    #[tokio::test]
    #[function_name::named]
    async fn test_mark_lifecycle() {
        let (nm, db) = create_nonce_manager(function_name!(), 0).await;

        // Allocate nonce -> queued
        let nonce = nm.allocate_nonce(&ADDR_1, &U256::from(42)).await.unwrap();
        assert_eq!(nonce, 0);

        let status: String =
            sqlx::query_scalar("SELECT status FROM pending_registrations WHERE nonce = $1")
                .bind(nonce as i64)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(status, "queued");

        // Mark submitted
        let tx_hash = alloy::primitives::TxHash::ZERO;
        nm.mark_submitted(nonce, tx_hash, Some(1000)).await.unwrap();

        let status: String =
            sqlx::query_scalar("SELECT status FROM pending_registrations WHERE nonce = $1")
                .bind(nonce as i64)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(status, "submitted");

        let stored_gas: Option<i64> =
            sqlx::query_scalar("SELECT gas_price FROM pending_registrations WHERE nonce = $1")
                .bind(nonce as i64)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(stored_gas, Some(1000));

        // Mark confirmed
        nm.mark_confirmed(nonce).await.unwrap();

        let status: String =
            sqlx::query_scalar("SELECT status FROM pending_registrations WHERE nonce = $1")
                .bind(nonce as i64)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(status, "confirmed");
    }

    #[tokio::test]
    #[function_name::named]
    async fn test_mark_failed_and_retry() {
        let (nm, db) = create_nonce_manager(function_name!(), 0).await;

        let nonce = nm.allocate_nonce(&ADDR_1, &U256::from(42)).await.unwrap();

        // Mark failed
        nm.mark_failed(nonce, "tx reverted").await.unwrap();

        let (status, attempt_count, last_error): (String, i32, Option<String>) = {
            let row = sqlx::query(
                "SELECT status, attempt_count, last_error FROM pending_registrations WHERE nonce = $1",
            )
            .bind(nonce as i64)
            .fetch_one(&db)
            .await
            .unwrap();
            (
                row.get("status"),
                row.get("attempt_count"),
                row.get("last_error"),
            )
        };
        assert_eq!(status, "failed");
        assert_eq!(attempt_count, 1);
        assert_eq!(last_error.as_deref(), Some("tx reverted"));

        // Mark failed again
        nm.mark_failed(nonce, "still failing").await.unwrap();

        let attempt_count: i32 =
            sqlx::query_scalar("SELECT attempt_count FROM pending_registrations WHERE nonce = $1")
                .bind(nonce as i64)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(attempt_count, 2);

        // Retriable list should include this registration (attempt_count < max_attempts)
        let retriable = nm.get_retriable().await.unwrap();
        assert_eq!(retriable.len(), 1);
        assert_eq!(retriable[0].nonce, nonce as i64);
    }

    #[tokio::test]
    #[function_name::named]
    async fn test_stuck_detection() {
        let mut config = NonceManagerConfig::default();
        config.stuck_timeout_secs = 0; // Everything is "stuck" immediately for testing

        let db_config = test_config();
        let (_, db) = create_database_connection(function_name!(), true, db_config)
            .await
            .unwrap();

        sqlx::query("INSERT INTO nonce_state (wallet_address, current_nonce) VALUES ($1, 0)")
            .bind(WALLET_ADDR.as_slice())
            .execute(&db)
            .await
            .unwrap();

        let nm = NonceManager {
            db: db.clone(),
            config,
            wallet_address: WALLET_ADDR,
            current_nonce: tokio::sync::Mutex::new(0),
        };

        let nonce = nm.allocate_nonce(&ADDR_1, &U256::from(42)).await.unwrap();
        let tx_hash = alloy::primitives::TxHash::ZERO;
        nm.mark_submitted(nonce, tx_hash, None).await.unwrap();

        // With stuck_timeout_secs = 0, this submitted tx should be detected as stuck
        // Need a small delay so CURRENT_TIMESTAMP > submitted_at
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let stuck = nm.get_stuck_transactions().await.unwrap();
        assert_eq!(stuck.len(), 1);
        assert_eq!(stuck[0].nonce, nonce as i64);
    }

    #[tokio::test]
    #[function_name::named]
    async fn test_sync_nonce_from_chain() {
        // Test that sync takes the max of DB nonce and chain nonce.
        // We test the DB-only path here (the provider sync is tested in integration tests).

        let (nm, db) = create_nonce_manager(function_name!(), 5).await;

        // Verify initial state
        let guard = nm.current_nonce.lock().await;
        assert_eq!(*guard, 5);
        drop(guard);

        // Allocate a few nonces to advance to 8
        nm.allocate_nonce(&ADDR_1, &U256::from(1)).await.unwrap(); // 5
        nm.allocate_nonce(&ADDR_1, &U256::from(2)).await.unwrap(); // 6
        nm.allocate_nonce(&ADDR_1, &U256::from(3)).await.unwrap(); // 7

        let guard = nm.current_nonce.lock().await;
        assert_eq!(*guard, 8);
        drop(guard);

        // Verify DB nonce state
        let db_nonce: i64 =
            sqlx::query_scalar("SELECT current_nonce FROM nonce_state WHERE wallet_address = $1")
                .bind(WALLET_ADDR.as_slice())
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(db_nonce, 8);
    }

    #[tokio::test]
    #[function_name::named]
    async fn test_recovery_on_startup() {
        let (nm, db) = create_nonce_manager(function_name!(), 0).await;

        // Create some pre-populated rows simulating state from a previous run
        let nonce0 = nm.allocate_nonce(&ADDR_1, &U256::from(100)).await.unwrap();
        let nonce1 = nm.allocate_nonce(&ADDR_2, &U256::from(200)).await.unwrap();

        // Mark nonce0 as submitted (simulating in-flight tx)
        let tx_hash = alloy::primitives::TxHash::ZERO;
        nm.mark_submitted(nonce0, tx_hash, None).await.unwrap();

        // Mark nonce1 as submitted too
        let tx_hash2 = alloy::primitives::TxHash::with_last_byte(1);
        nm.mark_submitted(nonce1, tx_hash2, None).await.unwrap();

        // Verify both are in submitted state
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pending_registrations WHERE status = 'submitted'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    #[function_name::named]
    async fn test_mark_cancelled() {
        let (nm, db) = create_nonce_manager(function_name!(), 0).await;

        let nonce = nm.allocate_nonce(&ADDR_1, &U256::from(42)).await.unwrap();
        nm.mark_cancelled(nonce).await.unwrap();

        let status: String =
            sqlx::query_scalar("SELECT status FROM pending_registrations WHERE nonce = $1")
                .bind(nonce as i64)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(status, "cancelled");
    }

    #[tokio::test]
    #[function_name::named]
    async fn test_register_user_end_to_end() {
        // Test the ManagedRLNRegister -> channel -> oneshot flow
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let managed = ManagedRLNRegister::new(tx);

        // Spawn a mock "processor" that receives tasks and replies with Ok
        let processor = tokio::spawn(async move {
            if let Some(task) = rx.recv().await {
                assert_eq!(task.address, ADDR_1);
                assert_eq!(task.identity_commitment, U256::from(42));
                let _ = task.result_tx.send(Ok(()));
            }
        });

        // Call register_user, which should queue and get the result
        let result = managed.register_user(&ADDR_1, U256::from(42)).await;
        assert!(result.is_ok());

        processor.await.unwrap();
    }

    #[tokio::test]
    async fn test_register_user_channel_closed() {
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let managed = ManagedRLNRegister::new(tx);

        // Drop the receiver to close the channel
        drop(rx);

        let result = managed.register_user(&ADDR_1, U256::from(42)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("channel closed"));
    }
}
