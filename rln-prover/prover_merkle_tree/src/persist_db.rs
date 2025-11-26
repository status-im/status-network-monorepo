use std::collections::HashMap;
// third-party
use num_packer::U32Packer;
// use sea-orm
use sea_orm::{DatabaseConnection, DbErr, Set, sea_query::OnConflict};
// sea-orm traits
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ExprTrait, IntoActiveModel, QueryFilter,
    TransactionTrait,
};
// internal - db
use prover_db_entity::{m_tree, m_tree_config};
// internal
use prover_pmtree::{Value, persistent_db::PersistentDatabase, tree::Key};

#[derive(thiserror::Error, Debug)]
pub enum PersistentDbError {
    #[error(transparent)]
    Db(#[from] DbErr),
    #[error("Invalid config")]
    Config,
}

#[derive(Clone, Debug)]
pub struct PersistentDbConfig {
    pub db_conn: DatabaseConnection,
    pub tree_index: i16,
    pub insert_batch_size: usize,
}

pub struct PersistentDb {
    config: PersistentDbConfig,
    put_cfg_store: HashMap<String, usize>,
    put_store: Vec<m_tree::ActiveModel>,
}

impl PersistentDatabase for PersistentDb {
    // Note - Limits :
    // tree_index (i16) -> max 32k tree supported (if required to support more, use u16 serialized as i16)
    // depth (u32) -> depth in prover == 20, so this can be reduced down to u8
    // index (u32) -> so max u32::MAX entries - large enough for tree of depth 20
    // if depth is reduced to u8 then index can be set to u56

    type Config = PersistentDbConfig;
    type Error = PersistentDbError;

    fn new(config: Self::Config) -> Self {
        PersistentDb {
            config,
            put_cfg_store: Default::default(),
            put_store: vec![],
        }
    }

    fn put_cfg(&mut self, key: &str, value: usize) {
        // FIXME: add debug_assert! if key is not supported
        self.put_cfg_store.insert(key.to_string(), value);
    }

    fn put(&mut self, key: (usize, usize), value: Value) {
        let index_in_tree = i64::pack_u32(key.0 as u32, key.1 as u32);
        self.put_store.push(m_tree::ActiveModel {
            tree_index: Set(self.config.tree_index),
            index_in_tree: Set(index_in_tree),
            value: Set(value),
            ..Default::default()
        });
    }

    fn put_batch<'a>(&mut self, subtree: impl IntoIterator<Item = (&'a Key, Value)>) {
        self.put_store.extend(subtree.into_iter().map(|(k, v)| {
            // FIXME: factorize
            let index_in_tree = i64::pack_u32(k.0 as u32, k.1 as u32);
            m_tree::ActiveModel {
                tree_index: Set(self.config.tree_index),
                index_in_tree: Set(index_in_tree),
                value: Set(v),
                ..Default::default()
            }
        }));
    }

    async fn fsync(&mut self) -> Result<(), Self::Error> {
        let cfg_map = std::mem::take(&mut self.put_cfg_store);
        let put_list = std::mem::take(&mut self.put_store);

        let txn = self.config.db_conn.begin().await?;
        if !cfg_map.is_empty() {
            let cfg_ = m_tree_config::Entity::find()
                .filter(
                    <m_tree_config::Entity as EntityTrait>::Column::TreeIndex
                        .eq(self.config.tree_index),
                )
                .one(&txn)
                .await?;

            if let Some(cfg_) = cfg_ {
                let mut cfg = cfg_.into_active_model();
                if let Some(cfg_value) = cfg_map.get("depth") {
                    // FIXME
                    cfg.depth = Set(*cfg_value as i64);
                }
                if let Some(cfg_value) = cfg_map.get("next_index") {
                    // FIXME
                    cfg.next_index = Set(*cfg_value as i64);
                }

                cfg.update(&txn).await?;
            } else {
                // TODO: unwrap safe notes?
                let cfg_depth = cfg_map.get("depth").unwrap();
                let cfg_next_index = cfg_map.get("next_index").unwrap();

                let cfg = m_tree_config::ActiveModel {
                    tree_index: Set(self.config.tree_index),
                    depth: Set(*cfg_depth as i64),
                    next_index: Set(*cfg_next_index as i64),
                    ..Default::default()
                };

                cfg.insert(&txn).await?;
            }
        }

        // prepare on_conflict statement for insert_many
        let on_conflict = OnConflict::columns([
            <m_tree::Entity as EntityTrait>::Column::TreeIndex,
            <m_tree::Entity as EntityTrait>::Column::IndexInTree,
        ])
        .update_column(<m_tree::Entity as EntityTrait>::Column::Value)
        .to_owned();

        /*
        // Chunk put_list into batches (postgres limit is around ~ 15_000 params)
        let put_list_ = &put_list
            .into_iter()
            .chunks(self.config.insert_batch_size);

        for chunk in put_list_ {
            m_tree::Entity::insert_many::<m_tree::ActiveModel, _>(chunk)
                .on_conflict(on_conflict.clone())
                .exec(&txn)
                .await
                ?;
        }
        */

        // FIXME: chunk
        m_tree::Entity::insert_many::<m_tree::ActiveModel, _>(put_list)
            .on_conflict(on_conflict.clone())
            .exec(&txn)
            .await?;

        txn.commit().await?;

        Ok(())
    }

    async fn get(&self, key: (usize, usize)) -> Result<Option<Value>, Self::Error> {
        let index_in_tree = i64::pack_u32(key.0 as u32, key.1 as u32);
        let res = m_tree::Entity::find()
            .filter(
                <m_tree::Entity as EntityTrait>::Column::TreeIndex
                    .eq(self.config.tree_index)
                    .and(<m_tree::Entity as EntityTrait>::Column::IndexInTree.eq(index_in_tree)),
            )
            .one(&self.config.db_conn)
            .await?;

        Ok(res.map(|m| m.value))
    }

    async fn get_all(&self) -> Result<Vec<(usize, usize, Value)>, Self::Error> {
        Ok(m_tree::Entity::find()
            .filter(<m_tree::Entity as EntityTrait>::Column::TreeIndex.eq(self.config.tree_index))
            .all(&self.config.db_conn)
            .await?
            .into_iter()
            .map(|m| {
                let (depth, index) = i64::unpack_u32(&m.index_in_tree);
                (depth as usize, index as usize, m.value)
            })
            .collect())
    }

    async fn get_cfg(&self) -> Result<Option<(usize, usize)>, Self::Error> {
        let res = m_tree_config::Entity::find()
            .filter(
                <m_tree_config::Entity as EntityTrait>::Column::TreeIndex
                    .eq(self.config.tree_index),
            )
            .one(&self.config.db_conn)
            .await?;

        Ok(res.map(|m| (m.depth as usize, m.next_index as usize)))
    }
}
