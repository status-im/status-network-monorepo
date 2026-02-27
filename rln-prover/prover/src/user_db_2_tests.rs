#[cfg(feature = "postgres")]
#[cfg(test)]
mod tests {
    // std
    use std::sync::Arc;
    use std::time::Duration;
    // third-party
    use crate::epoch_service::{Epoch, EpochSlice};
    use crate::user_db_2::{MERKLE_TREE_HEIGHT, UserDb2, UserDb2Config};
    use alloy::primitives::{Address, address};
    use claims::assert_matches;
    use parking_lot::RwLock;
    // sqlx
    use crate::tests_common::create_database_connection;
    use sqlx::error::Error as SqlxError;
    // internal
    use crate::user_db_error::RegisterError2;
    use crate::user_db_types::EpochCounter;

    const ADDR_1: Address = address!("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    const ADDR_2: Address = address!("0xb20a608c624Ca5003905aA834De7156C68b2E1d0");
    const ADDR_3: Address = address!("0x6d2e03b7EfFEae98BD302A9F836D0d6Ab0002766");
    const ADDR_4: Address = address!("0x7A4d20b913B97aD2F30B30610e212D7db11B4BC3");

    #[tokio::test]
    async fn test_incr_tx_counter_2() {
        // Same as test_incr_tx_counter but multi users AND multi incr

        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let epoch = 1;
        let epoch_slice = 42;
        *epoch_store.write() = (Epoch::from(epoch), EpochSlice::from(epoch_slice));

        let config = UserDb2Config {
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };

        let db_conn = create_database_connection(
            "user_db_tests_test_incr_tx_counter_2",
            true,
            config.clone(),
        )
        .await
        .unwrap()
        .1;

        let user_db = UserDb2::new(
            db_conn,
            config,
            epoch_store,
            Default::default(),
            Default::default(),
        )
        .await
        .expect("Cannot create UserDb");

        // Register users
        user_db.register_user(ADDR_1).await.unwrap();
        user_db.register_user(ADDR_2).await.unwrap();

        assert_eq!(
            user_db.get_tx_counter(&ADDR_1).await.unwrap(),
            (EpochCounter::from(0), 0)
        );
        assert_eq!(
            user_db.get_tx_counter(&ADDR_2).await.unwrap(),
            (EpochCounter::from(0), 0)
        );

        // Now update user tx counter
        assert_eq!(
            user_db.on_new_tx(&ADDR_1, None).await.unwrap(),
            (EpochCounter::from(1), 0)
        );
        assert_eq!(
            user_db.on_new_tx(&ADDR_1, None).await.unwrap(),
            (EpochCounter::from(2), 0)
        );
        assert_eq!(
            user_db.on_new_tx(&ADDR_1, Some(2)).await.unwrap(),
            (EpochCounter::from(4), 0)
        );

        assert_eq!(
            user_db.on_new_tx(&ADDR_2, None).await.unwrap(),
            (EpochCounter::from(1), 0)
        );

        assert_eq!(
            user_db.on_new_tx(&ADDR_2, None).await.unwrap(),
            (EpochCounter::from(2), 0)
        );

        assert_eq!(
            user_db.get_tx_counter(&ADDR_1).await.unwrap(),
            (EpochCounter::from(4), 0)
        );

        assert_eq!(
            user_db.get_tx_counter(&ADDR_2).await.unwrap(),
            (EpochCounter::from(2), 0)
        );
    }

    #[tokio::test]
    async fn test_persistent_storage() {
        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDb2Config {
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };

        let addr = Address::new([0; 20]);
        {
            let (_, db_conn) = create_database_connection(
                "user_db_tests_test_persistent_storage",
                true,
                config.clone(),
            )
            .await
            .unwrap();

            let user_db = UserDb2::new(
                db_conn.clone(),
                config.clone(),
                epoch_store.clone(),
                Default::default(),
                Default::default(),
            )
            .await
            .expect("Cannot create UserDb");

            // Register user
            user_db.register_user(ADDR_1).await.unwrap();

            // + 1 user
            user_db.register_user(ADDR_2).await.unwrap();

            let user_model = user_db.get_user(&ADDR_1).await.unwrap().unwrap();
            assert_eq!(
                (user_model.tree_index, user_model.index_in_merkle_tree),
                (0, 0)
            );
            let user_model = user_db.get_user(&ADDR_2).await.unwrap().unwrap();
            assert_eq!(
                (user_model.tree_index, user_model.index_in_merkle_tree),
                (0, 1)
            );

            assert_eq!(
                user_db.on_new_tx(&ADDR_1, Some(2)).await.unwrap(),
                (EpochCounter::from(2), 0)
            );
            assert_eq!(
                user_db.on_new_tx(&ADDR_2, Some(1000)).await.unwrap(),
                (EpochCounter::from(1000), 0)
            );

            db_conn.close().await;
            // user_db is dropped at the end of the scope, but let's make it explicit
            drop(user_db);
        }

        {
            // Reopen Db and check that is inside
            let (_, db_conn) = create_database_connection(
                "user_db_tests_test_persistent_storage",
                false,
                config.clone(),
            )
            .await
            .unwrap();

            let user_db = UserDb2::new(
                db_conn,
                config,
                epoch_store,
                Default::default(),
                Default::default(),
            )
            .await
            .expect("Cannot create UserDb");

            assert!(!user_db.has_user(&addr).await.unwrap());
            assert!(user_db.has_user(&ADDR_1).await.unwrap());
            assert!(user_db.has_user(&ADDR_2).await.unwrap());
            assert_eq!(
                user_db.get_tx_counter(&ADDR_1).await.unwrap(),
                (EpochCounter::from(2), 0)
            );
            assert_eq!(
                user_db.get_tx_counter(&ADDR_2).await.unwrap(),
                (EpochCounter::from(1000), 0)
            );

            let user_model = user_db.get_user(&ADDR_1).await.unwrap().unwrap();
            assert_eq!(
                (user_model.tree_index, user_model.index_in_merkle_tree),
                (0, 0)
            );
            let user_model = user_db.get_user(&ADDR_2).await.unwrap().unwrap();
            assert_eq!(
                (user_model.tree_index, user_model.index_in_merkle_tree),
                (0, 1)
            );
        }
    }

    #[tokio::test]
    async fn test_multi_tree() {
        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let tree_count = 3;
        let config = UserDb2Config {
            tree_count,
            max_tree_count: 3,
            tree_depth: 1,
        };

        {
            let (_, db_conn) =
                create_database_connection("user_db_tests_test_multi_tree", true, config.clone())
                    .await
                    .unwrap();

            let user_db = UserDb2::new(
                db_conn.clone(),
                config.clone(),
                epoch_store.clone(),
                Default::default(),
                Default::default(),
            )
            .await
            .expect("Cannot create UserDb");

            assert_eq!(user_db.get_db_tree_count().await.unwrap(), tree_count);
            // assert_eq!(user_db.get_vec_tree_count().await as u64, tree_count);

            user_db.register_user(ADDR_1).await.unwrap();
            user_db.register_user(ADDR_2).await.unwrap();
            user_db.register_user(ADDR_3).await.unwrap();
            user_db.register_user(ADDR_4).await.unwrap();

            assert_eq!(user_db.get_user_indexes(&ADDR_1).await, (0, 0));
            assert_eq!(user_db.get_user_indexes(&ADDR_2).await, (0, 1));
            assert_eq!(user_db.get_user_indexes(&ADDR_3).await, (1, 0));
            assert_eq!(user_db.get_user_indexes(&ADDR_4).await, (1, 1));

            drop(user_db);
        }

        {
            // reload UserDb from disk and check indexes

            let (_, db_conn) =
                create_database_connection("user_db_tests_test_multi_tree", false, config.clone())
                    .await
                    .unwrap();

            let user_db = UserDb2::new(
                db_conn,
                config,
                epoch_store,
                Default::default(),
                Default::default(),
            )
            .await
            .expect("Cannot create UserDb");

            assert_eq!(user_db.get_db_tree_count().await.unwrap(), tree_count);
            // assert_eq!(user_db.get_vec_tree_count().await as u64, tree_count);

            let addr = Address::random();
            user_db.register_user(addr).await.unwrap();

            assert_eq!(user_db.get_user_indexes(&ADDR_1).await, (0, 0));
            assert_eq!(user_db.get_user_indexes(&ADDR_2).await, (0, 1));
            assert_eq!(user_db.get_user_indexes(&ADDR_3).await, (1, 0));
            assert_eq!(user_db.get_user_indexes(&ADDR_4).await, (1, 1));
            assert_eq!(user_db.get_user_indexes(&addr).await, (2, 0));
        }
    }

    #[tokio::test]
    async fn test_new_multi_tree() {
        // Check if UserDb add a new tree is a tree is full

        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let tree_depth = 1;
        let tree_count_initial = 1;
        let max_tree_count = 2;
        let config = UserDb2Config {
            tree_count: tree_count_initial,
            max_tree_count,
            tree_depth,
        };

        let (_, db_conn) =
            create_database_connection("user_db_tests_test_new_multi_tree", true, config.clone())
                .await
                .unwrap();

        let user_db = UserDb2::new(
            db_conn.clone(),
            config.clone(),
            epoch_store.clone(),
            Default::default(),
            Default::default(),
        )
        .await
        .expect("Cannot create UserDb");

        assert_eq!(user_db.get_db_tree_count().await.unwrap(), max_tree_count);
        // assert_eq!(
        //     user_db.get_vec_tree_count().await as u64,
        //     tree_count_initial
        // );

        user_db.register_user(ADDR_1).await.unwrap();
        assert_eq!(user_db.get_user_indexes(&ADDR_1).await, (0, 0));
        user_db.register_user(ADDR_2).await.unwrap();
        assert_eq!(user_db.get_user_indexes(&ADDR_2).await, (0, 1));
        user_db.register_user(ADDR_3).await.unwrap();
        assert_eq!(user_db.get_user_indexes(&ADDR_3).await, (1, 0));
        user_db.register_user(ADDR_4).await.unwrap();
        assert_eq!(user_db.get_user_indexes(&ADDR_4).await, (1, 1));

        let addr = Address::random();
        let res = user_db.register_user(addr).await;
        assert_matches!(res, Err(RegisterError2::TooManyUsers));
        assert_eq!(
            user_db.get_db_tree_count().await.unwrap(),
            tree_count_initial + 1
        );
        // assert_eq!(
        //     user_db.get_vec_tree_count().await as u64,
        //     tree_count_initial + 1
        // );

        drop(user_db);

        {
            let (_, db_conn) = create_database_connection(
                "user_db_tests_test_new_multi_tree",
                false,
                config.clone(),
            )
            .await
            .unwrap();

            let user_db = UserDb2::new(
                db_conn.clone(),
                config.clone(),
                epoch_store.clone(),
                Default::default(),
                Default::default(),
            )
            .await
            .expect("Cannot create UserDb");

            assert_eq!(
                user_db.get_db_tree_count().await.unwrap(),
                tree_count_initial + 1
            );
            // assert_eq!(
            //     user_db.get_vec_tree_count().await as u64,
            //     tree_count_initial + 1
            // );
        }
    }

    #[tokio::test]
    async fn test_deny_list_1() -> Result<(), SqlxError> {
        // Check UserDb + deny list basic functionalities

        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let tree_depth = 1;
        let tree_count_initial = 1;
        let config = UserDb2Config {
            tree_count: tree_count_initial,
            max_tree_count: 1,
            tree_depth,
        };

        let (_, db_conn) =
            create_database_connection("user_db_tests_test_deny_list_1", true, config.clone())
                .await?;

        let user_db = UserDb2::new(
            db_conn.clone(),
            config.clone(),
            epoch_store.clone(),
            Default::default(),
            Default::default(),
        )
        .await?;

        // Define some custom 'now' functions so we are in control
        let now_v = 10;
        let now = || Duration::from_secs(now_v);
        let now2 = || Duration::from_secs(12);

        assert_eq!(user_db.is_denied(&ADDR_1, &now).await?, false);
        assert_eq!(user_db.is_denied(&ADDR_2, &now).await?, false);

        let ttl = 5;
        user_db.register_user(ADDR_3).await.unwrap();
        user_db
            .add_to_deny_list(&ADDR_3, None, Some(ttl), &now)
            .await?;

        assert_eq!(user_db.is_denied(&ADDR_3, &now).await?, true);
        // Check that in the future, ADDR_3 is not denied anymore
        assert_eq!(
            user_db
                .is_denied(&ADDR_3, &|| Duration::from_secs(now_v)
                    + Duration::from_secs(ttl as u64))
                .await?,
            false
        );
        assert_eq!(user_db.is_denied(&ADDR_1, &now).await?, false);
        assert_eq!(user_db.is_denied(&ADDR_2, &now).await?, false);

        user_db.remove_from_deny_list(&ADDR_3).await?;
        assert_eq!(user_db.is_denied(&ADDR_3, &now).await?, false);
        assert_eq!(user_db.is_denied(&ADDR_1, &now).await?, false);
        assert_eq!(user_db.is_denied(&ADDR_2, &now).await?, false);

        user_db
            .add_to_deny_list(&ADDR_3, None, Some(5), &now2)
            .await?;
        assert_eq!(user_db.is_denied(&ADDR_3, &now2).await?, true);
        let deny_list_entry = user_db.get_deny_list_entry(&ADDR_3, &now2).await?.unwrap();

        assert_eq!(
            Address::from_slice(deny_list_entry.address.as_slice()),
            ADDR_3
        );
        assert_eq!(
            deny_list_entry.expires_at.unwrap(),
            (now2() + Duration::from_secs(5)).as_secs() as i64
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_deny_list_upsert_res() -> Result<(), SqlxError> {
        // Check UserDb deny_list add_to_deny_list result

        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let tree_depth = 1;
        let tree_count_initial = 1;
        let config = UserDb2Config {
            tree_count: tree_count_initial,
            max_tree_count: 1,
            tree_depth,
        };

        let (_, db_conn) =
            create_database_connection("user_db_tests_test_deny_list_2", true, config.clone())
                .await?;

        let user_db = UserDb2::new(
            db_conn.clone(),
            config.clone(),
            epoch_store.clone(),
            Default::default(),
            Default::default(),
        )
        .await?;

        // Define some custom 'now' functions so we are in control
        let now = || Duration::from_secs(10);
        user_db.register_user(ADDR_3).await.unwrap();
        // Check this is an INSERT
        assert_eq!(
            user_db
                .add_to_deny_list(&ADDR_3, None, Some(5), &now)
                .await?,
            true
        );
        // Check this is an UPDATE
        assert_eq!(
            user_db
                .add_to_deny_list(&ADDR_3, None, Some(7), &now)
                .await?,
            false
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_deny_list_cleanup() -> Result<(), SqlxError> {
        // Check UserDb deny_list cleanup

        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let tree_depth = 1;
        let tree_count_initial = 1;
        let config = UserDb2Config {
            tree_count: tree_count_initial,
            max_tree_count: 1,
            tree_depth,
        };

        let (_, db_conn) =
            create_database_connection("user_db_tests_test_deny_list_cleanup", true, config.clone())
                .await?;

        let user_db = UserDb2::new(
            db_conn.clone(),
            config.clone(),
            epoch_store.clone(),
            Default::default(),
            Default::default(),
        )
        .await?;

        // Define some custom 'now' functions so we are in control
        let now_v = 10;
        let now2_v = 12;
        let now = || Duration::from_secs(now_v);
        let now2 = || Duration::from_secs(now2_v);
        let ttl = 5;
        let now3 = || Duration::from_secs(now_v + ttl);
        let now4 = || Duration::from_secs(now2_v + ttl);

        user_db.register_user(ADDR_3).await.unwrap();
        user_db.register_user(ADDR_1).await.unwrap();

        user_db
            .add_to_deny_list(&ADDR_3, None, Some(ttl as i64), &now)
            .await?;
        user_db
            .add_to_deny_list(&ADDR_1, None, Some(ttl as i64), &now2)
            .await?;

        // Will clean nothing (no expiration yet)
        user_db.cleanup_expired_deny_list_entries(&now).await?;
        assert!(user_db.get_deny_list_entry(&ADDR_3, &now).await?.is_some());
        assert!(user_db.get_deny_list_entry(&ADDR_1, &now).await?.is_some());

        // Cleanup only ADDR3
        user_db.cleanup_expired_deny_list_entries(&now3).await?;
        assert!(user_db.get_deny_list_entry(&ADDR_3, &now3).await?.is_none());
        assert!(user_db.get_raw_deny_list_entry(&ADDR_3).await?.is_none());
        assert!(user_db.get_deny_list_entry(&ADDR_1, &now3).await?.is_some());
        assert!(user_db.get_raw_deny_list_entry(&ADDR_1).await?.is_some());

        // Cleanup ADDR_1 too
        user_db.cleanup_expired_deny_list_entries(&now4).await?;
        assert!(user_db.get_deny_list_entry(&ADDR_1, &now4).await?.is_none());
        assert!(user_db.get_raw_deny_list_entry(&ADDR_1).await?.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_nullifier_1() -> Result<(), SqlxError> {
        // Check UserDb nullifier basic functionalities

        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let tree_depth = 1;
        let tree_count_initial = 1;
        let config = UserDb2Config {
            tree_count: tree_count_initial,
            max_tree_count: 1,
            tree_depth,
        };

        let (_, db_conn) =
            create_database_connection("user_db_tests_test_nullifier_1", true, config.clone())
                .await?;

        let user_db = UserDb2::new(
            db_conn.clone(),
            config.clone(),
            epoch_store.clone(),
            Default::default(),
            Default::default(),
        )
        .await?;

        let nullifier_1 = vec![1; 32];
        let epoch_1 = 1;
        let nullifier_2 = vec![1; 32];
        let epoch_2 = 42;

        assert_eq!(
            user_db.nullifier_exists(&nullifier_1, epoch_1).await?,
            false
        );

        // INSERT
        assert_eq!(user_db.record_nullifier(&nullifier_1, epoch_1).await?, true);
        // UPDATE
        assert_eq!(
            user_db.record_nullifier(&nullifier_1, epoch_1).await?,
            false
        );

        assert_eq!(user_db.get_nullifier_count_for_epoch(epoch_1).await?, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_nullifier_cleanup() -> Result<(), SqlxError> {
        // Check UserDb nullifier cleanup

        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let tree_depth = 1;
        let tree_count_initial = 1;
        let config = UserDb2Config {
            tree_count: tree_count_initial,
            max_tree_count: 1,
            tree_depth,
        };

        let (_, db_conn) =
            create_database_connection("user_db_tests_test_nullifier_cleanup", true, config.clone())
                .await?;

        let user_db = UserDb2::new(
            db_conn.clone(),
            config.clone(),
            epoch_store.clone(),
            Default::default(),
            Default::default(),
        )
        .await?;

        let nullifier_1 = vec![1; 32];
        let epoch_1 = 1;
        let nullifier_2 = vec![2; 32];
        let epoch_2 = 42;

        assert_eq!(user_db.record_nullifier(&nullifier_1, epoch_1).await?, true);
        assert_eq!(user_db.record_nullifier(&nullifier_2, epoch_2).await?, true);

        assert_eq!(user_db.get_nullifier_count_for_epoch(epoch_1).await?, 1);
        assert_eq!(user_db.get_nullifier_count_for_epoch(epoch_2).await?, 1);

        assert_eq!(user_db.cleanup_old_nullifiers(epoch_2, 0).await?, 1);
        assert_eq!(user_db.get_nullifier_count_for_epoch(epoch_1).await?, 0);
        assert_eq!(user_db.get_nullifier_count_for_epoch(epoch_2).await?, 1);

        assert_eq!(user_db.cleanup_old_nullifiers(epoch_2 + 1, 0).await?, 1);
        assert_eq!(user_db.get_nullifier_count_for_epoch(epoch_1).await?, 0);
        assert_eq!(user_db.get_nullifier_count_for_epoch(epoch_2).await?, 0);

        Ok(())
    }
}
