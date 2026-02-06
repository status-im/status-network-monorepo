# Rln aggregator

An aggregator service collection Rln proof and sending them to slasher nodes

## Rln aggregator development

* Start rln-prover (Postgres + Mock)
* `cargo run -- -u http://localhost:50051`

### Send tx to rln-prover

* in rln prover folder
  * `RUST_LOG=debug cargo run -p prover_client -- -i 127.0.0.1 -p 50051 send-transaction --tx-hash aa` 

