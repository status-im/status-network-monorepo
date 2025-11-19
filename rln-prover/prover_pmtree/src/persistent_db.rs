use crate::{PmtreeResult, Value};
use crate::tree::Key;

pub trait PersistentDatabase {

    type Config;
    // type Entity;
    // type EntityConfig;
    type Error;

    /// Creates new instance of db
    fn new(config: Self::Config) -> Self;

    /// Puts the value to the db by the key
    fn put_cfg(&mut self, key: &str, value: usize);

    /// Puts the value to the db by the key
    fn put(&mut self, key: (usize, usize), value: Value);

    /// Puts the leaves batch to the db
    fn put_batch<'a>(&mut self, subtree: impl IntoIterator<Item=(&'a Key, Value)>);

    // async fn sync(&mut self) -> Result<(), Self::Error>;
    fn fsync(&mut self) -> impl Future<Output = Result<(), Self::Error>>;

    fn get(&self, key: (usize, usize)) -> impl Future<Output = Result<Option<Value>, Self::Error>>;

    fn get_all(&self) -> impl Future<Output = Result<Vec<(usize, usize, Value)>, Self::Error>>;

    fn get_cfg(&self) -> impl Future<Output = Result<Option<(usize, usize)>, Self::Error>>;
}