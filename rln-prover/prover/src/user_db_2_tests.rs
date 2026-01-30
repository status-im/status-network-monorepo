#[cfg(feature = "postgres")]
#[cfg(test)]
mod tests {
    // std
    use std::sync::Arc;
    // third-party
    use crate::epoch_service::{Epoch, EpochSlice};
    use crate::user_db::MERKLE_TREE_HEIGHT;
    use crate::user_db_2::{UserDb2, UserDb2Config};
    use alloy::primitives::{Address, address};
    use claims::assert_matches;
    use parking_lot::RwLock;
    // use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr, Statement};
    // sqlx
    use sqlx::{
        error::Error as SqlxError,
        Pool,
        Postgres,
    };
    use crate::tests_common::create_database_connection_1;
    // internal
    use crate::user_db_error::RegisterError2;
    use crate::user_db_types::{EpochCounter, EpochSliceCounter};
    // use prover_db_migration::{Migrator as MigratorCreate, MigratorTrait};

    const ADDR_1: Address = address!("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    const ADDR_2: Address = address!("0xb20a608c624Ca5003905aA834De7156C68b2E1d0");
    const ADDR_3: Address = address!("0x6d2e03b7EfFEae98BD302A9F836D0d6Ab0002766");
    const ADDR_4: Address = address!("0x7A4d20b913B97aD2F30B30610e212D7db11B4BC3");

    /*
    pub async fn create_database_connection(
        db_name: &str,
        db_refresh: bool,
    ) -> Result<Pool<Postgres>, SqlxError> {
        // Drop / Create db_name then return a connection to it

        let db_url_base = "postgres://myuser:mysecretpassword@localhost";
        let db_url = format!("{}/{}", db_url_base, "mydatabase");

        if db_refresh {
            let db = sqlx::PgPool::connect(db_url.as_str()).await?;

            sqlx::query("DROP DATABASE IF EXISTS $1")
                .bind(db_name)
                .execute(&db)
                .await?;

            sqlx::query("CREATE DATABASE $1")
                .bind(db_name)
                .execute(&db)
                .await?;

            db.close().await;
        }

        let db_url_final = format!("{}/{}", db_url_base, db_name);
        let db = sqlx::PgPool::connect(db_url.as_str()).await?;
        // MigratorCreate::up(&db, None).await?;
        todo!();

        Ok(db)
    }
    */

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

        let (_, db_conn) = create_database_connection_1("user_db_tests_test_incr_tx_counter_2", true)
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

        // Register users
        user_db.register_user(ADDR_1).await.unwrap();
        user_db.register_user(ADDR_2).await.unwrap();

        assert_eq!(
            user_db.get_tx_counter(&ADDR_1).await.unwrap(),
            EpochCounter::from(0)
        );
        assert_eq!(
            user_db.get_tx_counter(&ADDR_2).await.unwrap(),
            EpochCounter::from(0)
        );

        // Now update user tx counter
        assert_eq!(
            user_db.on_new_tx(&ADDR_1, None).await.unwrap(),
            EpochCounter::from(1)
        );
        assert_eq!(
            user_db.on_new_tx(&ADDR_1, None).await.unwrap(),
            EpochCounter::from(2)
        );
        assert_eq!(
            user_db.on_new_tx(&ADDR_1, Some(2)).await.unwrap(),
            EpochCounter::from(4)
        );

        assert_eq!(
            user_db.on_new_tx(&ADDR_2, None).await.unwrap(),
            EpochCounter::from(1)
        );

        assert_eq!(
            user_db.on_new_tx(&ADDR_2, None).await.unwrap(),
            EpochCounter::from(2)
        );

        assert_eq!(
            user_db.get_tx_counter(&ADDR_1).await.unwrap(),
            EpochCounter::from(4)
        );

        assert_eq!(
            user_db.get_tx_counter(&ADDR_2).await.unwrap(),
            EpochCounter::from(2)
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
            let (_, db_conn) = create_database_connection_1("user_db_tests_test_persistent_storage", true)
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
                EpochCounter::from(2)
            );
            assert_eq!(
                user_db.on_new_tx(&ADDR_2, Some(1000)).await.unwrap(),
                EpochCounter::from(1000)
            );

            db_conn.close().await;
            // user_db is dropped at the end of the scope, but let's make it explicit
            drop(user_db);
        }

        {
            // Reopen Db and check that is inside
            let (_, db_conn) =
                create_database_connection_1("user_db_tests_test_persistent_storage", false)
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
                EpochCounter::from(2)
            );
            assert_eq!(
                user_db.get_tx_counter(&ADDR_2).await.unwrap(),
                EpochCounter::from(1000)
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
            let (_, db_conn) = create_database_connection_1("user_db_tests_test_multi_tree", true)
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
            assert_eq!(user_db.get_vec_tree_count().await as u64, tree_count);

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

            let (_, db_conn) = create_database_connection_1("user_db_tests_test_multi_tree", false)
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
            assert_eq!(user_db.get_vec_tree_count().await as u64, tree_count);

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
        let config = UserDb2Config {
            tree_count: tree_count_initial,
            max_tree_count: 2,
            tree_depth,
        };

        let (_, db_conn) = create_database_connection_1("user_db_tests_test_new_multi_tree", true)
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
            tree_count_initial
        );
        assert_eq!(
            user_db.get_vec_tree_count().await as u64,
            tree_count_initial
        );

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
        assert_eq!(
            user_db.get_vec_tree_count().await as u64,
            tree_count_initial + 1
        );

        drop(user_db);

        {
            let (_, db_conn) = create_database_connection_1("user_db_tests_test_new_multi_tree", false)
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
            assert_eq!(
                user_db.get_vec_tree_count().await as u64,
                tree_count_initial + 1
            );
        }
    }
}
