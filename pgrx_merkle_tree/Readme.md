# Pg merkle tree

Postgresql extension to store a merkle tree in a Postgresql DB

Code adapted from: https://github.com/sydhds/pgrx_merkle_tree

# Compile

* `cd pg_merkle_tree`
* `cargo build --lib --features pg18 --no-default-features`

# Run unit tests

* `cd pg_merkle_tree`
* `CARGO_TARGET_DIR=/tmp/pgrx_target cargo pgrx test pg18 merkle_tree::tests::pg_test_pgfr_set_leaf`