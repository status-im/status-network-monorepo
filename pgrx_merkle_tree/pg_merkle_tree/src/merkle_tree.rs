// std
use std::collections::BTreeMap;
// third-party
use ark_bn254::Fr;
use ark_serialize::CanonicalSerialize;
// pgrx
use pgrx::{
    spi::{SpiClient, SpiResult},
    prelude::*,
    datum::DatumWithOid
};
use crate::PgFr;
use crate::poseidon::poseidon_hash_;
use crate::merkle_tree_utils::{node_parent, first_child};

#[pg_extern(parallel_unsafe)]
fn pgfr_mtree_init(depth: i64, tree_index: i64) {

    let depth = depth as usize;

    // Note: init the merkle tree as 1 hash / level of the tree
    //       so we can insert into the tree with only a few queries
    let mut level_hashes = Vec::with_capacity(depth + 1);
    level_hashes.push(Fr::default()); // set the initial leaf value
    // Compute hash from the initial leaf value up to the root node
    (0..depth).for_each(|level_index| {
        level_hashes.push(poseidon_hash_(&[level_hashes[level_index]; 2]))
    });

    let query = format!(r#"
        INSERT INTO pgfr_mtree_{} (index_in_mtree, value)
        SELECT i, $1
        FROM generate_series($2, $3) as i
    "#, tree_index);

    Spi::connect_mut(|client| {

        for (level, hash) in level_hashes.iter().rev().enumerate() {
            let level_start_index = (1i64 << level) - 1;
            let level_end_index = (1i64 << (level + 1)) - 2;

            client.update(
                query.as_str(),
                None,
                &[
                    // $1: The hash value for this entire level
                    PgFr(*hash).into(),
                    // $2: Start Index
                    level_start_index.into(),
                    // $3: End Index
                    level_end_index.into(),
                ]
            ).expect(format!("Failed to insert hash {hash} at level {level}").as_str());
        }
    });
}

#[pg_extern(stable, strict, parallel_safe)]
fn pgfr_mtree_get_root(tree_index: i64) -> Result<Option<PgFr>, pgrx::spi::Error> {

    let res: SpiResult<Option<PgFr>> = Spi::get_one_with_args(
        format!("SELECT value::pgfr FROM pgfr_mtree_{} WHERE index_in_mtree = 0 LIMIT 1;", tree_index).as_str(),
        &[]
    );

    res
}

#[pg_extern(parallel_unsafe)]
fn pgfr_mtree_set_leaf(depth: i16, tree_index: i64, index_in_mtree: i64, leaf_value: PgFr) -> Result<(), pgrx::spi::Error> {

    // TODO: rename index_in_mtree to leaf_index ?_index ?

    let index = index_in_mtree as usize;
    let leaf_index_ = (1 << depth) + index - 1;
    let leaf_index = leaf_index_ as i64;

    let query = format!("UPDATE pgfr_mtree_{} SET value = $1 WHERE index_in_mtree = $2", tree_index);

    Spi::run_with_args(
        query.as_str(),
        &[
            unsafe { DatumWithOid::new(leaf_value, PgFr::type_oid()) },
            leaf_index.into()
        ]
    )?;

    // Get index and new hashes to insert in tree after leaf update
    let mut to_update = BTreeMap::new();
    Spi::connect(|client| {
        mtree_get_hashes(client, tree_index, leaf_index_, leaf_index_, &mut to_update);
    });

    let (to_update_indexes, to_update_values): (Vec<i64>, Vec<PgFr>) = to_update.into_iter().unzip();

    let query_2 = format!(r#"
        UPDATE pgfr_mtree_{}
        SET value = data.new_value
        FROM (
            SELECT * FROM UNNEST($1::bigint[], $2::pgfr[])
            AS t(i_index, new_value)
        ) AS data
        WHERE pgfr_mtree_{}.index_in_mtree = data.i_index;
        "#, tree_index, tree_index);

    Spi::run_with_args(query_2.as_str(),
                       &[
                           to_update_indexes.into(),
                           to_update_values.into()
                       ]
    )?;

    Ok(())
}

fn mtree_get_hashes(client: &SpiClient, tree_index: i64, start_index: usize, end_index: usize, to_update: &mut BTreeMap<i64, PgFr>) {

    let query_1 = format!("SELECT value::pgfr FROM pgfr_mtree_{} WHERE index_in_mtree = $1 LIMIT 1", tree_index);

    let mut start_index = start_index;
    let mut end_index = end_index;

    while let (Some(start_parent), Some(end_parent)) = (node_parent(start_index), node_parent(end_index)) {

        for parent in start_parent..=end_parent {

            // iter over parent nodes - for each parent, get left child and right child 'value' column
            let left_child_ = first_child(parent);
            let right_child_ = left_child_ + 1;

            let left_child = left_child_ as i64;
            let right_child = right_child_ as i64;

            // Get value for left child
            let left_child_value = if to_update.contains_key(&left_child) {
                to_update[&left_child]
            } else {
                let left_child_value_ = client.select(query_1.as_str(), None, &[left_child.into()]);
                let left_child_value = left_child_value_
                    .unwrap() // unwrap safe: assume merkle tree table has been correctly initialized
                    .first() // SELECT query only returns 1 element
                    .get_one::<PgFr>() // SELECT query returns only column 'value'
                    .unwrap()// unwrap safe: SELECT query returns only column 'value'
                    .unwrap(); // unwrap safe: assume 'value' column is always initialized
                left_child_value
            };

            // Get value for right child
            let right_child_value = if to_update.contains_key(&right_child) {
                to_update[&right_child]
            } else {
                let right_child_value_ = client.select(query_1.as_str(), None, &[right_child.into()]);
                let right_child_value = right_child_value_
                    .unwrap()
                    .first()
                    .get_one::<PgFr>()
                    .unwrap()
                    .unwrap();
                right_child_value
            };

            // Compute hash
            let value = poseidon_hash_(&[left_child_value.0, right_child_value.0]);
            let parent_ = parent as i64;

            // Store it in our hashmap (db will be updated later in bulk)
            to_update.insert(parent_, PgFr(value));

            // Loop until we reach the merkle tree root node (which has no parent)
            start_index = start_parent;
            end_index = end_parent;
        }
    }
}

#[pg_extern(stable, strict, parallel_safe)]
fn pgfr_mtree_get_proof(depth: i16, tree_index: i64, leaf_index: i64) -> Vec<u8> {

    let leaf_index_ = leaf_index as usize;
    // TODO: rename to leaf_index or node_index ?
    let mut index = (1 << depth) + leaf_index_ - 1;
    // let mut proof_inner_ = Vec::with_capacity(depth as usize + 1);
    // TODO: with_cap
    let mut left_or_right = Vec::new();
    let mut mtree_indexes = Vec::new();

    // Traverse the tree from bottom to top (node_parent will return None at the root)
    while let Some(parent) = node_parent(index) {

        // TODO: explain the 'index & 1'
        match index & 1 {
            0 => {
                let index_ = (index - 1) as i64;
                // (1, index_)
                left_or_right.push(1);
                mtree_indexes.push(index_);
            },
            1 => {
                let index_ = (index + 1) as i64;
                // (0, index_)
                left_or_right.push(0);
                mtree_indexes.push(index_);
            },
            _ => unreachable!(),
        };
        index = parent
    }

    let mtree_indexes_len = mtree_indexes.len();

    // Note: Using JOIN implicitly assumes that all nodes & leaves are initialized in the DB
    //       This is doubled-checked after the query
    // Note 2: UNNEST Expands an array into a set of rows. The array's elements are read out in storage order.
    //         So using WITH ORDINALITY we can return the SELECT in the array order
    let query = format!(r#"
        SELECT m.value
        FROM UNNEST($1::bigint[]) WITH ORDINALITY AS t(req_idx, ord)
        JOIN pgfr_mtree_{} m
            ON m.index_in_mtree = t.req_idx
        ORDER BY t.ord ASC
    "#, tree_index);

    let oid = PgBuiltInOids::INT8ARRAYOID.oid();

    let values = Spi::connect(|client| {

        let result = client.select(
            query.as_str(),
            None,
            &[
                unsafe { DatumWithOid::new(mtree_indexes, oid.value()) },
            ]
        ).expect("Error executing SPI query");

        result
            .into_iter()
            .map(|row| {
                row.get::<PgFr>(1)
                    .expect("no value")
                    .expect("null value")
                    .0
            })
            .collect::<Vec<Fr>>()

    });

    // Note: cf query notes about UNNEST + JOIN
    if values.len() != mtree_indexes_len {
        panic!("Merkle tree not fully init: Requested {} nodes but found only {}",
               mtree_indexes_len, values.len());
    }

    let proof_data: Vec<(i64, Fr)> = left_or_right
        .iter()
        .zip(values)
        .map(|(i, values)| {
            (*i, values)
        })
        .collect();

    // info!("proof_data: {:?}", proof_data);

    let mut buffer = Vec::new();
    proof_data.serialize_compressed(&mut buffer).expect("Serialization failed");
    buffer
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {

    use std::str::FromStr;
    use ark_serialize::CanonicalDeserialize;
    use super::*;

    #[pg_test]
    fn test_merkle_tree_init() {

        let _res = Spi::run("
            CREATE TABLE pgfr_mtree_1 (index_in_mtree bigint PRIMARY KEY, value pgfr);
            "
        );

        let tree_index = 1;
        let m_tree_depth = 3;
        pgfr_mtree_init(m_tree_depth, tree_index);

        // Get root manually
        let root_node = Spi::get_one::<PgFr>("SELECT value FROM pgfr_mtree_1 WHERE index_in_mtree = 0;").unwrap().unwrap();
        assert_eq!(root_node.0, Fr::from_str("11286972368698509976183087595462810875513684078608517520839298933882497716792").unwrap());
    }

    #[pg_test]
    fn test_merkle_tree_get_root() {

        let _res = Spi::run("
            CREATE TABLE pgfr_mtree_1 (index_in_mtree bigint PRIMARY KEY, value pgfr);
            "
        );

        let tree_index = 1;
        let m_tree_depth = 3;
        pgfr_mtree_init(m_tree_depth, tree_index);

        // Get root manually
        let root_node = Spi::get_one::<PgFr>("SELECT value FROM pgfr_mtree_1 WHERE index_in_mtree = 0;").unwrap().unwrap();

        let root_node_2 = pgfr_mtree_get_root(tree_index).unwrap().unwrap();

        assert_eq!(root_node.0, Fr::from_str("11286972368698509976183087595462810875513684078608517520839298933882497716792").unwrap());
        assert_eq!(root_node.0, root_node_2.0);
    }

    #[pg_test]
    fn test_pgfr_set_leaf() {
        let _res = Spi::run("
            CREATE TABLE pgfr_mtree_1 (index_in_mtree bigint PRIMARY KEY, value pgfr);
            "
        );
        let tree_index = 1;
        let m_tree_depth = 3;
        pgfr_mtree_init(m_tree_depth, tree_index);

        pgfr_mtree_set_leaf(m_tree_depth as i16, tree_index, 0, PgFr(Fr::from(2))).unwrap();
        let root = pgfr_mtree_get_root(tree_index).unwrap().unwrap();
        assert_eq!(root.0, Fr::from_str("3799385896495180565562780950112041501871782716691607926126180421168246094289").unwrap());

        pgfr_mtree_set_leaf(m_tree_depth as i16, tree_index, 7, PgFr(Fr::from(42))).unwrap();
        let root = pgfr_mtree_get_root(tree_index).unwrap().unwrap();
        assert_eq!(root.0, Fr::from_str("9164054056146260648413073295070635933539618302378139976693739565479035405901").unwrap());
    }

    #[pg_test]
    fn test_pgfr_get_proof() {
        let _res = Spi::run("
            CREATE TABLE pgfr_mtree_1 (index_in_mtree bigint PRIMARY KEY, value pgfr);
            "
        );

        let tree_index = 1;
        let m_tree_depth = 3;
        pgfr_mtree_init(m_tree_depth, tree_index);

        {
            let proof_bytes = pgfr_mtree_get_proof(m_tree_depth as i16, tree_index, 0);
            let proof = Vec::<(i64, Fr)>::deserialize_compressed(proof_bytes.as_slice()).unwrap();
            assert_eq!(
                proof,
                vec![
                    (0, Fr::from(0)),
                    (0, Fr::from_str("14744269619966411208579211824598458697587494354926760081771325075741142829156").unwrap()),
                    (0, Fr::from_str("7423237065226347324353380772367382631490014989348495481811164164159255474657").unwrap())
                ]);
        }
        {
            let proof_bytes = pgfr_mtree_get_proof(m_tree_depth as i16, tree_index, 1);
            let proof = Vec::<(i64, Fr)>::deserialize_compressed(proof_bytes.as_slice()).unwrap();
            assert_eq!(
                proof,
                vec![
                    (1, Fr::from(0)),
                    (0, Fr::from_str("14744269619966411208579211824598458697587494354926760081771325075741142829156").unwrap()),
                    (0, Fr::from_str("7423237065226347324353380772367382631490014989348495481811164164159255474657").unwrap())
                ]);
        }
        {
            let proof_bytes = pgfr_mtree_get_proof(m_tree_depth as i16, tree_index, 7);
            let proof = Vec::<(i64, Fr)>::deserialize_compressed(proof_bytes.as_slice()).unwrap();

            assert_eq!(
                proof,
                vec![
                    (1, Fr::from(0)),
                    (1, Fr::from_str("14744269619966411208579211824598458697587494354926760081771325075741142829156").unwrap()),
                    (1, Fr::from_str("7423237065226347324353380772367382631490014989348495481811164164159255474657").unwrap())
                ]);
        }
    }
}

