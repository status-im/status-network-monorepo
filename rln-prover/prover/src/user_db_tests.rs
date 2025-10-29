#[cfg(test)]
mod tests {
    // std
    use std::path::PathBuf;
    use std::sync::Arc;
    // third-party
    use crate::epoch_service::{Epoch, EpochSlice};
    use alloy::primitives::{Address, address};
    use claims::assert_matches;
    use parking_lot::RwLock;
    // internal
    use crate::user_db::{MERKLE_TREE_HEIGHT, UserDb, UserDbConfig};
    use crate::user_db_error::RegisterError;
    use crate::user_db_types::{EpochCounter, EpochSliceCounter, IndexInMerkleTree, TreeIndex};

    const ADDR_1: Address = address!("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
    const ADDR_2: Address = address!("0xb20a608c624Ca5003905aA834De7156C68b2E1d0");
    const ADDR_3: Address = address!("0x6d2e03b7EfFEae98BD302A9F836D0d6Ab0002766");
    const ADDR_4: Address = address!("0x7A4d20b913B97aD2F30B30610e212D7db11B4BC3");

    #[test]
    fn test_incr_tx_counter_2() {
        // Same as test_incr_tx_counter but multi users AND multi incr

        let temp_folder = tempfile::tempdir().unwrap();
        let temp_folder_tree = tempfile::tempdir().unwrap();

        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let epoch = 1;
        let epoch_slice = 42;
        *epoch_store.write() = (Epoch::from(epoch), EpochSlice::from(epoch_slice));

        let config = UserDbConfig {
            db_path: PathBuf::from(temp_folder.path()),
            merkle_tree_folder: PathBuf::from(temp_folder_tree.path()),
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };

        let user_db =
            UserDb::new(config, epoch_store, Default::default(), Default::default()).unwrap();

        // Register users
        user_db.register(ADDR_1).unwrap();
        user_db.register(ADDR_2).unwrap();

        assert_eq!(
            user_db.get_tx_counter(&ADDR_1),
            Ok((EpochCounter::from(0), EpochSliceCounter::from(0)))
        );
        assert_eq!(
            user_db.get_tx_counter(&ADDR_2),
            Ok((EpochCounter::from(0), EpochSliceCounter::from(0)))
        );

        // Now update user tx counter
        assert_eq!(
            user_db.on_new_tx(&ADDR_1, None),
            Ok(EpochSliceCounter::from(1))
        );
        assert_eq!(
            user_db.on_new_tx(&ADDR_1, None),
            Ok(EpochSliceCounter::from(2))
        );
        assert_eq!(
            user_db.on_new_tx(&ADDR_1, Some(2)),
            Ok(EpochSliceCounter::from(4))
        );

        assert_eq!(
            user_db.on_new_tx(&ADDR_2, None),
            Ok(EpochSliceCounter::from(1))
        );

        assert_eq!(
            user_db.on_new_tx(&ADDR_2, None),
            Ok(EpochSliceCounter::from(2))
        );

        assert_eq!(
            user_db.get_tx_counter(&ADDR_1),
            Ok((EpochCounter::from(4), EpochSliceCounter::from(4)))
        );

        assert_eq!(
            user_db.get_tx_counter(&ADDR_2),
            Ok((EpochCounter::from(2), EpochSliceCounter::from(2)))
        );
    }

    #[test]
    fn test_persistent_storage() {
        let temp_folder = tempfile::tempdir().unwrap();
        let temp_folder_tree = tempfile::tempdir().unwrap();
        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let config = UserDbConfig {
            db_path: PathBuf::from(temp_folder.path()),
            merkle_tree_folder: PathBuf::from(temp_folder_tree.path()),
            tree_count: 1,
            max_tree_count: 1,
            tree_depth: MERKLE_TREE_HEIGHT,
        };

        let addr = Address::new([0; 20]);
        {
            let user_db = UserDb::new(
                config.clone(),
                epoch_store.clone(),
                Default::default(),
                Default::default(),
            )
            .unwrap();

            /*
            assert_eq!(
                user_db.get_next_indexes().unwrap(),
                (TreeIndex::from(0), IndexInMerkleTree::from(0))
            );
            */
            // Register user
            user_db.register(ADDR_1).unwrap();
            /*
            assert_eq!(
                user_db.get_next_indexes().unwrap(),
                (TreeIndex::from(0), IndexInMerkleTree::from(1))
            );
            */
            // + 1 user
            user_db.register(ADDR_2).unwrap();
            /*
            assert_eq!(
                user_db.get_next_indexes().unwrap(),
                (TreeIndex::from(0), IndexInMerkleTree::from(2))
            );
            */
            assert_eq!(
                user_db.get_user_indexes(&ADDR_1).unwrap(),
                (TreeIndex::from(0), IndexInMerkleTree::from(0))
            );
            assert_eq!(
                user_db.get_user_indexes(&ADDR_2).unwrap(),
                (TreeIndex::from(0), IndexInMerkleTree::from(1))
            );

            assert_eq!(
                user_db.on_new_tx(&ADDR_1, Some(2)),
                Ok(EpochSliceCounter::from(2))
            );
            assert_eq!(
                user_db.on_new_tx(&ADDR_2, Some(1000)),
                Ok(EpochSliceCounter::from(1000))
            );

            // user_db is dropped at the end of the scope, but let's make it explicit
            drop(user_db);
        }

        {
            // Reopen Db and check that is inside
            let user_db =
                UserDb::new(config, epoch_store, Default::default(), Default::default()).unwrap();

            assert!(!user_db.has_user(&addr).unwrap());
            assert!(user_db.has_user(&ADDR_1).unwrap());
            assert!(user_db.has_user(&ADDR_2).unwrap());
            assert_eq!(
                user_db.get_tx_counter(&ADDR_1).unwrap(),
                (2.into(), 2.into())
            );
            assert_eq!(
                user_db.get_tx_counter(&ADDR_2).unwrap(),
                (1000.into(), 1000.into())
            );

            /*
            assert_eq!(
                user_db.get_next_indexes().unwrap(),
                (TreeIndex::from(0), IndexInMerkleTree::from(2))
            );
            */
            assert_eq!(
                user_db.get_user_indexes(&ADDR_1).unwrap(),
                (TreeIndex::from(0), IndexInMerkleTree::from(0))
            );
            assert_eq!(
                user_db.get_user_indexes(&ADDR_2).unwrap(),
                (TreeIndex::from(0), IndexInMerkleTree::from(1))
            );
        }
    }

    #[test]
    fn test_multi_tree() {
        let temp_folder = tempfile::tempdir().unwrap();
        let temp_folder_tree = tempfile::tempdir().unwrap();
        // let temp_folder_tree_2 = tempfile::tempdir().unwrap();
        // let temp_folder_tree_3 = tempfile::tempdir().unwrap();
        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let tree_count = 3;
        let config = UserDbConfig {
            db_path: PathBuf::from(temp_folder.path()),
            merkle_tree_folder: PathBuf::from(temp_folder_tree.path()),
            tree_count,
            max_tree_count: 3,
            tree_depth: 1,
        };

        {
            let user_db = UserDb::new(
                config.clone(),
                epoch_store.clone(),
                Default::default(),
                Default::default(),
            )
            .unwrap();

            assert_eq!(user_db.get_tree_count().unwrap(), tree_count);
            assert_eq!(user_db.tree_count() as u64, tree_count);

            user_db.register(ADDR_1).unwrap();
            user_db.register(ADDR_2).unwrap();
            user_db.register(ADDR_3).unwrap();
            user_db.register(ADDR_4).unwrap();

            assert_eq!(
                user_db.get_user_indexes(&ADDR_1).unwrap(),
                (TreeIndex::from(0), IndexInMerkleTree::from(0))
            );
            assert_eq!(
                user_db.get_user_indexes(&ADDR_2).unwrap(),
                (TreeIndex::from(0), IndexInMerkleTree::from(1))
            );
            assert_eq!(
                user_db.get_user_indexes(&ADDR_3).unwrap(),
                (TreeIndex::from(1), IndexInMerkleTree::from(0))
            );
            assert_eq!(
                user_db.get_user_indexes(&ADDR_4).unwrap(),
                (TreeIndex::from(1), IndexInMerkleTree::from(1))
            );

            drop(user_db);
        }

        {
            // reload UserDb from disk and check indexes

            let user_db = UserDb::new(
                config,
                epoch_store.clone(),
                Default::default(),
                Default::default(),
            )
            .unwrap();

            assert_eq!(user_db.get_tree_count().unwrap(), tree_count);
            assert_eq!(user_db.tree_count() as u64, tree_count);

            let addr = Address::random();
            user_db.register(addr).unwrap();

            assert_eq!(
                user_db.get_user_indexes(&ADDR_1).unwrap(),
                (TreeIndex::from(0), IndexInMerkleTree::from(0))
            );
            assert_eq!(
                user_db.get_user_indexes(&ADDR_2).unwrap(),
                (TreeIndex::from(0), IndexInMerkleTree::from(1))
            );
            assert_eq!(
                user_db.get_user_indexes(&ADDR_3).unwrap(),
                (TreeIndex::from(1), IndexInMerkleTree::from(0))
            );
            assert_eq!(
                user_db.get_user_indexes(&ADDR_4).unwrap(),
                (TreeIndex::from(1), IndexInMerkleTree::from(1))
            );
            assert_eq!(
                user_db.get_user_indexes(&addr).unwrap(),
                (TreeIndex::from(2), IndexInMerkleTree::from(0))
            );
        }
    }

    #[test]
    fn test_new_multi_tree() {
        // Check if UserDb add a new tree is a tree is full

        let temp_folder = tempfile::tempdir().unwrap();
        let temp_folder_tree = tempfile::tempdir().unwrap();
        let epoch_store = Arc::new(RwLock::new(Default::default()));
        let tree_depth = 1;
        let tree_count_initial = 1;
        let config = UserDbConfig {
            db_path: PathBuf::from(temp_folder.path()),
            merkle_tree_folder: PathBuf::from(temp_folder_tree.path()),
            tree_count: tree_count_initial,
            max_tree_count: 2,
            tree_depth,
        };

        let user_db = UserDb::new(
            config.clone(),
            epoch_store.clone(),
            Default::default(),
            Default::default(),
        )
        .unwrap();

        /*
        let temp_folder_tree_2 = tempfile::tempdir().unwrap();
        let config = PmtreeConfig::builder()
            .path(temp_folder_tree_2.path().to_path_buf())
            .temporary(false)
            .cache_capacity(100_000)
            .flush_every_ms(12_000)
            .mode(HighThroughput)
            .use_compression(false)
            .build()
            .unwrap();
        let tree = PoseidonTree::new(usize::from(tree_depth), Default::default(), config).unwrap();
        let tree = Arc::new(RwLock::new(vec![tree]));
        user_db.set_merkle_trees(tree.clone());
        */

        assert_eq!(user_db.get_tree_count().unwrap(), tree_count_initial);
        assert_eq!(user_db.tree_count() as u64, tree_count_initial);

        user_db.register(ADDR_1).unwrap();
        assert_eq!(
            user_db.get_user_indexes(&ADDR_1).unwrap(),
            (TreeIndex::from(0), IndexInMerkleTree::from(0))
        );
        user_db.register(ADDR_2).unwrap();
        assert_eq!(
            user_db.get_user_indexes(&ADDR_2).unwrap(),
            (TreeIndex::from(0), IndexInMerkleTree::from(1))
        );
        user_db.register(ADDR_3).unwrap();
        assert_eq!(
            user_db.get_user_indexes(&ADDR_3).unwrap(),
            (TreeIndex::from(1), IndexInMerkleTree::from(0))
        );
        user_db.register(ADDR_4).unwrap();
        assert_eq!(
            user_db.get_user_indexes(&ADDR_4).unwrap(),
            (TreeIndex::from(1), IndexInMerkleTree::from(1))
        );

        let addr = Address::random();
        let res = user_db.register(addr);
        assert_matches!(res, Err(RegisterError::TooManyUsers));
        assert_eq!(user_db.get_tree_count().unwrap(), tree_count_initial + 1);
        assert_eq!(user_db.tree_count() as u64, tree_count_initial + 1);

        drop(user_db);

        {
            let user_db = UserDb::new(
                config,
                epoch_store.clone(),
                Default::default(),
                Default::default(),
            )
            .unwrap();

            assert_eq!(user_db.get_tree_count().unwrap(), tree_count_initial + 1);
            assert_eq!(user_db.tree_count() as u64, tree_count_initial + 1);
        }
    }
}
