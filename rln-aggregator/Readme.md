# Rln aggregator

An aggregator service collection Rln proof and sending them to slasher nodes

## Rln aggregator development

* Start aggregator
  * `cargo run -- -u http://localhost:50051`
* Start aggregator (using mocked proofs)
  * `cargo run -p aggregator -- -i 127.0.0.1 --mock-prover-proof true`

* Start slasher node
  * `cargo run -p slasher` 

### Send tx to rln-prover

* in rln prover folder
  * `RUST_LOG=debug cargo run -p prover_client -- -i 127.0.0.1 -p 50051 send-transaction --tx-hash aa` 

## Development

* Compiling SC (to bytecode)
  * Need openzeppelin-contracts && openzeppelin-contracts-upgradeable
    * Install them with the instruction in: `makefile-contracts.mk` (see section `status-network-contracts-setup`) 

* Docker
  * `docker build --build-context prover_proto=../rln-prover/proto -t rln_aggregator .`
  * `docker run --rm rln_aggregator --help`