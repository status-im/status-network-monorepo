use crate::*;

use std::cmp::{max, min};
use std::collections::HashMap;
use std::error::Error;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};

use crate::persistent_db::PersistentDatabase;

#[cfg(feature = "parallel")]
use rayon;

// db[DEPTH_KEY] = depth
const DEPTH_KEY: DBKey = (u64::MAX - 1).to_be_bytes();

// db[NEXT_INDEX_KEY] = next_index;
const NEXT_INDEX_KEY: DBKey = u64::MAX.to_be_bytes();

// Denotes keys (depth, index) in Merkle Tree. Can be converted to DBKey
// TODO! Think about using hashing for that
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key(pub usize, pub usize);
impl From<Key> for DBKey {
    fn from(key: Key) -> Self {
        let cantor_pairing = ((key.0 + key.1) * (key.0 + key.1 + 1) / 2 + key.1) as u64;
        cantor_pairing.to_be_bytes()
    }
}

impl Key {
    pub fn new(depth: usize, index: usize) -> Self {
        Key(depth, index)
    }
}

/// The Merkle Tree structure
pub struct MerkleTree<D, H, PDB, E>
where
    // D: Database,
    H: Hasher,
{
    pub db: D,
    depth: usize,
    next_index: usize,
    cache: Vec<H::Fr>,
    root: H::Fr,

    persistent_db: PDB,
    phantom: PhantomData<E>,
}

/// The Merkle proof structure
#[derive(Clone, PartialEq, Eq)]
pub struct MerkleProof<H: Hasher>(pub Vec<(H::Fr, u8)>);

impl<D, H, PDB, E> MerkleTree<D, H, PDB, E>
where
    D: Database,
    H: Hasher,
    PDB: PersistentDatabase,
    E: Error + From<PmtreeErrorKind> + From<PDB::Error>,
{

    /// Creates new `MerkleTree` and store it to the specified path/db
    pub async fn new(depth: usize, db_config: D::Config, persistent_db_config: PDB::Config) -> Result<Self, E> {

        // Create new db instance
        let mut db = D::new(db_config)?;
        let mut persistent_db = PDB::new(persistent_db_config);

        // Insert depth val into db
        let depth_val = depth.to_be_bytes().to_vec();
        db.put(DEPTH_KEY, depth_val)?;
        persistent_db.put_cfg("depth", depth);

        // Insert next_index val into db
        let next_index = 0usize;
        let next_index_val = next_index.to_be_bytes().to_vec();
        db.put(NEXT_INDEX_KEY, next_index_val)?;
        persistent_db.put_cfg("next_index", next_index);

        // Cache nodes
        let mut cache = vec![H::default_leaf(); depth + 1];

        // Initialize one branch of the `Merkle Tree` from bottom to top
        cache[depth] = H::default_leaf();

        let k = (depth, 0);
        let v = H::serialize(cache[depth]);
        db.put(Key(k.0, k.1).into(), v.clone())?;
        persistent_db.put((k.0, k.1), v.clone());
        for i in (0..depth).rev() {
            cache[i] = H::hash(&[cache[i + 1], cache[i + 1]]);

            let k = (i, 0);
            let v = H::serialize(cache[i]);
            db.put(Key(k.0, k.1).into(), v.clone())?;
            persistent_db.put((k.0, k.1), v.clone());
        }

        let root = cache[0];

        persistent_db.fsync().await?;

        // end

        Ok(Self {
            db,
            depth,
            next_index,
            cache,
            root,
            persistent_db,
            phantom: Default::default(),
        })
    }

    /// Loads existing Merkle Tree from the specified path/db
    pub async fn load(db_config: D::Config, persistent_db_config: PDB::Config) -> Result<Self, E> {

        let persistent_db = PDB::new(persistent_db_config);

        let root_ = persistent_db.get((0, 0))
            .await?
            .ok_or(PmtreeErrorKind::CustomError("Root not found".to_string()))?;
        let root = H::deserialize(root_);

        let cfg = persistent_db.get_cfg()
            .await?
            .ok_or(PmtreeErrorKind::CustomError("Pdb cfg not found".to_string()))?;

        // FIXME: return iterator here?
        let all_nodes = persistent_db.get_all().await?;

        let mut db = D::new(db_config)?;

        db.put_batch(all_nodes.into_iter().map(|(depth, index, v)| {
            (Key(depth, index).into(), v)
        }))?;

        // Load cache vec
        let depth = cfg.0;
        let mut cache = vec![H::default_leaf(); depth + 1];
        cache[depth] = H::default_leaf();
        for i in (0..depth).rev() {
            cache[i] = H::hash(&[cache[i + 1], cache[i + 1]]);
        }

        let res = Self {
            db,
            depth: cfg.0,
            next_index: cfg.1,
            cache,
            root,
            persistent_db,
            phantom: Default::default(),
        };

        Ok(res)
    }

    /// Closes the db connection
    pub fn close(&mut self) -> PmtreeResult<()> {
        self.db.close()
    }

    /// Sets a leaf at the specified tree index
    pub async fn set(&mut self, key: usize, leaf: H::Fr) -> Result<(), E> {

        if key >= self.capacity() {
            return Err(PmtreeErrorKind::TreeError(TreeErrorKind::IndexOutOfBounds).into());
        }

        let value = H::serialize(leaf);
        self.db
            .put(Key(self.depth, key).into(), value.clone())?;
        self.persistent_db.put((self.depth, key), value);

        self.recalculate_from(key)?;

        // Update next_index in memory
        self.next_index = max(self.next_index, key + 1);

        // Update next_index in db
        let next_index_val = self.next_index.to_be_bytes().to_vec();
        self.db.put(NEXT_INDEX_KEY, next_index_val)?;

        self.persistent_db.put_cfg("next_index", self.next_index);
        self.persistent_db.fsync().await?;

        Ok(())
    }

    // Recalculates `Merkle Tree` from the specified key
    fn recalculate_from(&mut self, key: usize) -> PmtreeResult<()> {
        let mut depth = self.depth;
        let mut i = key;

        loop {
            let value = self.hash_couple(depth, i)?;
            i >>= 1;
            depth -= 1;

            let v = H::serialize(value);
            self.db.put(Key(depth, i).into(), v.clone())?;
            self.persistent_db.put((depth, i), v);

            if depth == 0 {
                self.root = value;
                break;
            }
        }

        Ok(())
    }

    // Hashes the correct couple for the key
    fn hash_couple(&self, depth: usize, key: usize) -> PmtreeResult<H::Fr> {
        let b = key & !1;

        let elem_a = self.get_elem(Key(depth, b));
        let elem_b = self.get_elem(Key(depth, b + 1));
        Ok(H::hash(&[
            elem_a?,
            elem_b?,
        ]))
    }

    // Returns elem by the key
    pub fn get_elem(&self, key: Key) -> PmtreeResult<H::Fr> {
        let res = self
            .db
            .get(key.into())?
            .map_or(self.cache[key.0], |value| H::deserialize(value));

        Ok(res)
    }

    /// Deletes a leaf at the `key` by setting it to its default value
    pub async fn delete(&mut self, key: usize) -> Result<(), E> {
        if key >= self.next_index {
            return Err(PmtreeErrorKind::TreeError(TreeErrorKind::InvalidKey).into());
        }

        self.set(key, H::default_leaf()).await?;

        Ok(())
    }

    /// Inserts a leaf to the next available index
    pub async fn update_next(&mut self, leaf: H::Fr) -> Result<(), E> {
        self.set(self.next_index, leaf).await?;

        Ok(())
    }

    /// Batch insertion from starting index
    pub async fn set_range<I: IntoIterator<Item = H::Fr>>(
        &mut self,
        start: usize,
        leaves: I,
    ) -> Result<(), E> {
        self.batch_insert(
            Some(start),
            leaves.into_iter().collect::<Vec<_>>().as_slice(),
        ).await
    }

    /// Batch insertion, updates the tree in parallel.
    pub async fn batch_insert(&mut self, start: Option<usize>, leaves: &[H::Fr]) -> Result<(), E> {
        let start = start.unwrap_or(self.next_index);
        let end = start + leaves.len();

        if end > self.capacity() {
            return Err(PmtreeErrorKind::TreeError(TreeErrorKind::MerkleTreeIsFull).into());
        }

        let mut subtree = HashMap::<Key, H::Fr>::new();

        let root_key = Key(0, 0);

        subtree.insert(root_key, self.root);
        self.fill_nodes(root_key, start, end, &mut subtree, leaves, start)?;

        let subtree = Arc::new(RwLock::new(subtree));

        let root_val = Self::batch_recalculate(root_key, Arc::clone(&subtree), self.depth);

        let subtree = RwLock::into_inner(Arc::try_unwrap(subtree).unwrap()).unwrap();

        let subtree_iter = subtree
            .iter()
            .map(|(key, value)| (key, H::serialize(*value)))
            ;

        self.db.put_batch(
            subtree_iter
                .clone()
                .map(|(k, v)| ((*k).into(), v))
        )?;

        // FIXME
        self.persistent_db.put_batch(
            subtree_iter
        );

        // Update next_index value in db
        if end > self.next_index {
            self.next_index = end;
            self.db
                .put(NEXT_INDEX_KEY, self.next_index.to_be_bytes().to_vec())?;
            self.persistent_db.put_cfg("next_index", self.next_index);
        }

        // Update root value in memory
        self.root = root_val;

        self.persistent_db.fsync().await?;

        Ok(())
    }

    // Fills hashmap subtree
    fn fill_nodes(
        &self,
        key: Key,
        start: usize,
        end: usize,
        subtree: &mut HashMap<Key, H::Fr>,
        leaves: &[H::Fr],
        from: usize,
    ) -> PmtreeResult<()> {
        if key.0 == self.depth {
            if key.1 >= from {
                subtree.insert(key, leaves[key.1 - from]);
            }
            return Ok(());
        }

        let left = Key(key.0 + 1, key.1 * 2);
        let right = Key(key.0 + 1, key.1 * 2 + 1);

        println!("get elem (left): {:?}", left);
        let left_val = self.get_elem(left)?;
        println!("get elem (right): {:?}", right);
        let right_val = self.get_elem(right)?;

        subtree.insert(left, left_val);
        subtree.insert(right, right_val);

        let half = 1 << (self.depth - key.0 - 1);

        if start < half {
            self.fill_nodes(left, start, min(end, half), subtree, leaves, from)?;
        }

        if end > half {
            self.fill_nodes(right, 0, end - half, subtree, leaves, from)?;
        }

        Ok(())
    }

    // Recalculates tree in parallel (in-memory)
    fn batch_recalculate(
        key: Key,
        subtree: Arc<RwLock<HashMap<Key, H::Fr>>>,
        depth: usize,
    ) -> H::Fr {
        let left_child = Key(key.0 + 1, key.1 * 2);
        let right_child = Key(key.0 + 1, key.1 * 2 + 1);

        if key.0 == depth || !subtree.read().unwrap().contains_key(&left_child) {
            return *subtree.read().unwrap().get(&key).unwrap();
        }

        #[cfg(feature = "parallel")]
        let (left, right) = rayon::join(
            || Self::batch_recalculate(left_child, Arc::clone(&subtree), depth),
            || Self::batch_recalculate(right_child, Arc::clone(&subtree), depth),
        );

        #[cfg(not(feature = "parallel"))]
        let (left, right) = (
            Self::batch_recalculate(left_child, Arc::clone(&subtree), depth),
            Self::batch_recalculate(right_child, Arc::clone(&subtree), depth),
        );

        let result = H::hash(&[left, right]);

        subtree.write().unwrap().insert(key, result);

        result
    }

    /// Computes a Merkle proof for the leaf at the specified index
    pub fn proof(&self, index: usize) -> PmtreeResult<MerkleProof<H>> {
        if index >= self.capacity() {
            return Err(PmtreeErrorKind::TreeError(TreeErrorKind::IndexOutOfBounds));
        }

        let mut witness = Vec::with_capacity(self.depth);

        let mut i = index;
        let mut depth = self.depth;
        while depth != 0 {
            i ^= 1;
            witness.push((
                self.get_elem(Key(depth, i))?,
                (1 - (i & 1)).try_into().unwrap(),
            ));
            i >>= 1;
            depth -= 1;
        }

        Ok(MerkleProof(witness))
    }

    /// Verifies a Merkle proof with respect to the input leaf and the tree root
    pub fn verify(&self, leaf: &H::Fr, witness: &MerkleProof<H>) -> bool {
        let expected_root = witness.compute_root_from(leaf);
        self.root() == expected_root
    }

    /// Returns the leaf by the key
    pub fn get(&self, key: usize) -> PmtreeResult<H::Fr> {
        if key >= self.capacity() {
            return Err(PmtreeErrorKind::TreeError(TreeErrorKind::IndexOutOfBounds));
        }

        self.get_elem(Key(self.depth, key))
    }

    /// Returns the root of the tree
    pub fn root(&self) -> H::Fr {
        self.root
    }

    /// Returns the total number of leaves set
    pub fn leaves_set(&self) -> usize {
        self.next_index
    }

    /// Returns the capacity of the tree, i.e. the maximum number of leaves
    pub fn capacity(&self) -> usize {
        1 << self.depth
    }

    /// Returns the depth of the tree
    pub fn depth(&self) -> usize {
        self.depth
    }
}

impl<H: Hasher> MerkleProof<H> {
    /// Computes the Merkle root by iteratively hashing specified Merkle proof with specified leaf
    pub fn compute_root_from(&self, leaf: &H::Fr) -> H::Fr {
        let mut acc = *leaf;
        for w in self.0.iter() {
            if w.1 == 0 {
                acc = H::hash(&[acc, w.0]);
            } else {
                acc = H::hash(&[w.0, acc]);
            }
        }

        acc
    }

    /// Computes the leaf index corresponding to a Merkle proof
    pub fn leaf_index(&self) -> usize {
        self.get_path_index()
            .into_iter()
            .rev()
            .fold(0, |acc, digit| (acc << 1) + usize::from(digit))
    }

    /// Returns the path indexes forming a Merkle Proof
    pub fn get_path_index(&self) -> Vec<u8> {
        self.0.iter().map(|x| x.1).collect()
    }

    /// Returns the path elements forming a Merkle proof
    pub fn get_path_elements(&self) -> Vec<H::Fr> {
        self.0.iter().map(|x| x.0).collect()
    }

    /// Returns the length of a Merkle proof
    pub fn length(&self) -> usize {
        self.0.len()
    }
}
