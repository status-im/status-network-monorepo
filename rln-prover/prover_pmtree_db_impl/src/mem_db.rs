use prover_pmtree::Database as PmtreeDatabase;
use prover_pmtree::{DBKey, DatabaseErrorKind, PmtreeErrorKind, PmtreeResult, Value};
use std::collections::HashMap;

pub struct MemoryDb(HashMap<DBKey, Value>);

#[derive(Default)]
pub struct MemoryDbConfig;

impl PmtreeDatabase for MemoryDb {
    type Config = MemoryDbConfig;

    fn new(_db_config: MemoryDbConfig) -> PmtreeResult<Self> {
        Ok(MemoryDb(HashMap::new()))
    }

    fn load(_db_config: MemoryDbConfig) -> PmtreeResult<Self> {
        Err(PmtreeErrorKind::DatabaseError(
            DatabaseErrorKind::CannotLoadDatabase,
        ))
    }

    fn get(&self, key: DBKey) -> PmtreeResult<Option<Value>> {
        Ok(self.0.get(&key).cloned())
    }

    fn put(&mut self, key: DBKey, value: Value) -> PmtreeResult<()> {
        self.0.insert(key, value);
        Ok(())
    }

    fn put_batch(&mut self, subtree: impl IntoIterator<Item = (DBKey, Value)>) -> PmtreeResult<()> {
        self.0.extend(subtree);
        Ok(())
    }

    fn close(&mut self) -> PmtreeResult<()> {
        Ok(())
    }
}
