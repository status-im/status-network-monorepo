# Rln aggregator

An aggregator service collection Rln proof and sending them to slasher nodes

## Run the rln-aggregator

* With docker:
    * `docker build --build-context prover_proto=../rln-prover/proto -t rln_aggregator .`
    * `docker run --rm rln_aggregator -i 0.0.0.0 -p 50061 -u http://127.0.0.1:50051` 
* Without:
  * `cargo run -p aggregator -- -i 0.0.0.0 -p 50061 -u http://127.0.0.1:50051`
    * Serve the rln-aggregator on port 50061 and connect to a rln-prover (listening on ip 127.0.0.1 port 50051)

## Run a slasher node

* With Docker:
  * `docker build --build-context prover_proto=../rln-prover/proto -t rln_slasher -f ./Dockerfile.slasher .`
  * `docker run --rm rln_slasher -i {RLN_AGGREGATOR_IP} -p {RLN_AGGREGATOR_PORT} --ws-rpc-url {STATUS_RPC_WS_URL} --spam_limit {SPAM_LIMIT} --account_to_reward {ETH_ADDRESS} --rln_sc {RLN_SMART_CONTRACT_ADDRESS}`
* Without:
  * `cargo run -p slasher -- -i {RLN_AGGREGATOR_IP} -p {RLN_AGGREGATOR_PORT} --ws-rpc-url {STATUS_RPC_WS_URL} --spam_limit {SPAM_LIMIT} --account_to_reward {ETH_ADDRESS} --rln_sc {RLN_SMART_CONTRACT_ADDRESS}`

Note:
* spam_limit must be set to the spam_limit defined in the rln-prover
* account_to_reward is the account receiving a reward (in Karma token) after a successful slash

## Dev (aggregator)

* Start aggregator
  * `cargo run -- -u http://localhost:50051`
* Start aggregator (using mocked proofs)
  * `cargo run -p aggregator -- -i 127.0.0.1 --mock-prover-proof true`

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

## Dev (slasher)

* Start slasher node
  * `cargo run -p slasher -- --help` 
