use std::hint::black_box;
// std
use std::io::{Cursor, Write};
// criterion
use criterion::{Criterion, criterion_group, criterion_main};
// third-party
use ark_bn254::Fr;
use ark_serialize::CanonicalSerialize;
use rln::hashers::{hash_to_field_le, poseidon_hash};
use rln::poseidon_tree::PoseidonTree;
use rln::protocol::{keygen, serialize_proof_values};
// internal
use rln_proof::{
    RlnData, RlnIdentifier, RlnUserIdentity, ZerokitMerkleTree, compute_rln_proof_and_values,
};

pub fn criterion_benchmark(c: &mut Criterion) {
    let (identity_secret_hash, id_commitment) = keygen();
    let user_limit = 100;
    let rln_identity = RlnUserIdentity {
        commitment: id_commitment,
        secret_hash: identity_secret_hash,
        user_limit: Fr::from(user_limit),
    };
    let rln_identifier = RlnIdentifier::new(b"test-test");
    let rln_data = RlnData {
        message_id: Fr::from(user_limit - 2),
        data: hash_to_field_le(b"data-from-message"),
    };

    // Merkle tree
    let tree_height = 20;
    let mut tree = PoseidonTree::new(tree_height, Fr::from(0), Default::default()).unwrap();
    let rate_commit = poseidon_hash(&[rln_identity.commitment, rln_identity.user_limit]);
    tree.set(0, rate_commit).unwrap();
    let merkle_proof = tree.proof(0).unwrap();

    // Epoch
    let epoch = hash_to_field_le(b"Today at noon, this year");

    {
        // Not a benchmark but print the proof size (serialized)
        let (proof, proof_values) = compute_rln_proof_and_values(
            &rln_identity,
            &rln_identifier,
            rln_data.clone(),
            epoch,
            &merkle_proof,
        )
        .unwrap();

        let mut output_buffer = Cursor::new(Vec::new());
        proof.serialize_compressed(&mut output_buffer).unwrap();
        output_buffer
            .write_all(&serialize_proof_values(&proof_values))
            .unwrap();

        println!(
            "Proof size (serialized): {:?}",
            output_buffer.into_inner().len()
        )
    }

    c.bench_function("compute proof and values", |b| {
        b.iter_batched(
            || {
                // generate setup data
                rln_data.clone()
            },
            |data| {
                // function to benchmark
                compute_rln_proof_and_values(
                    black_box(&rln_identity),
                    black_box(&rln_identifier),
                    black_box(data),
                    black_box(epoch),
                    black_box(&merkle_proof),
                )
            },
            criterion::BatchSize::SmallInput,
        );
    });

    c.bench_function("serialize proof and values", |b| {
        b.iter_batched(
            || {
                // generate setup data
                compute_rln_proof_and_values(
                    black_box(&rln_identity),
                    black_box(&rln_identifier),
                    black_box(rln_data.clone()),
                    black_box(epoch),
                    black_box(&merkle_proof),
                )
                .unwrap()
            },
            |(proof, proof_values)| {
                let mut output_buffer = Cursor::new(Vec::with_capacity(320));
                proof
                    .serialize_compressed(black_box(&mut output_buffer))
                    .unwrap();
                output_buffer
                    .write_all(black_box(&serialize_proof_values(black_box(&proof_values))))
                    .unwrap();
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(50);
    targets = criterion_benchmark
}
criterion_main!(benches);
