use rln::circuit::Fr;
use rln::poseidon_tree::PoseidonTree;
use zerokit_utils::merkle_tree::merkle_tree::ZerokitMerkleTree;

fn main() {

    // Root for Merkle tree of depth 3
    {
        let mut mtree = PoseidonTree::default(3).unwrap();
        let root_0 = mtree.root();
        println!("[depth 3] root_0: {:?}", root_0);
        mtree.set(0, Fr::from(2));
        println!("[depth 3] get leaf 0: {}", mtree.get(0).unwrap());
        let root_1 = mtree.root();
        println!("[depth 3] root_1: {:?}", root_1);
        mtree.set(7, Fr::from(42));
        println!("[depth 3] get leaf 7: {}", mtree.get(7).unwrap());
        let root_2 = mtree.root();
        println!("[depth 3] root_2: {:?}", root_2);
    }

    /*
    println!("{}", "#".repeat(40));

    let mut mtree = PoseidonTree::default(20).unwrap();
    let root_0 = mtree.root();
    let start = std::time::Instant::now();
    mtree.set(0, Fr::from(2)).unwrap();
    let elapsed = start.elapsed();
    let root_1 = mtree.root();
    println!("root_0: {:?}", root_0);
    println!("[set(0, 2)] elapsed: {:?} secs", elapsed.as_secs_f64());
    println!("root_1: {:?}", root_1);
    */

    // get proof result for merkle tree of depth 3
    {
        let mut mtree = PoseidonTree::default(3).unwrap();
        // let root_0 = mtree.root();
        // println!("[depth 3] root_0: {:?}", root_0);

        let proof = mtree.proof(0).unwrap();
        println!("[depth 3] proof at leaf 0: {:?}", proof);

        let proof = mtree.proof(1).unwrap();
        println!("[depth 3] proof at leaf 1: {:?}", proof);

        let proof = mtree.proof(7).unwrap();
        println!("[depth 3] proof at leaf 7: {:?}", proof);
    }

}