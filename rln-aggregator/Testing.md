# Testing scenario for rln-prover / rln-aggregator / slasher node

In this scenario, we are going to run:
* 1 instance of rln-prover (the service that generate RLN proof from Tx)
* 1 instance of rln-aggregator (the service that relay RLN proof from prover to slashers)
* 1 instance of slasher (the end user that collect proof and can slash spammer)

## Run the prover 

* Start a Postgresql DB for the prover:
  * `docker compose -f docker/compose-spec-l2-services-rln.yml up -d postgres --build` 
    * Note: this must be executed at the root of the repository 
* `cd rln-prover`
* `RUST_LOG=debug cargo run -p prover_cli -- --ip 127.0.0.1 --metrics-ip 127.0.0.1 --mock-sc true --mock-user mock/mock_user_2.json --db postgres://postgres:postgres@localhost:5432/prover_db --spam-limit 2 --no-config`
  * Register 3 users in prover (alice, bob, mickey) - see mock_user_2.json (Note: User are registered in Prover db only)
  * RLN spam limit is set to 2

## Run the aggregator

* `cd rln-aggregator`
* `RUST_LOG=debug cargo run -p aggregator -- -i 127.0.0.1 -u http://127.0.0.1:50051`

## Run the slasher

* `cd rln-aggregator`
* `cargo run -p slasher -- -i 127.0.0.1 --spam_limit 2 --mock-sc true 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266,ID_1 --mock-register 0x70997970C51812dc3A010C7d01b50e0d17dc79C8,ID_2`
  * Note: Replace ID_1 & ID_2 with id_commitment log found in rln-prover 
    * Locate log line like: 
      * `id_commitment: 641268640633432408379932586332057791689593312865548737964104215166574128323 for address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` 
  * Note: Smart contract are mocked by running them using [Anvil](https://www.getfoundry.sh/anvil) 

## Testing the services

## Test 1

We will generate 1 transaction and check that the slasher receive it

* `cd rln-prover`
* `RUST_LOG=debug cargo run -p prover_client -- -i 127.0.0.1 -p 50051 -a 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 send-transaction --tx-hash aa`
  * Sent a transaction from user bob (see -a argument) to the rln-prover (127.0.0.1:50051)
  * If everything goes well, you should see something like (in slasher logs)
  
```
2026-02-27T16:13:00.192227Z DEBUG slasher: Received proof reply: RlnAggProofReply { resp: Some(Proof(RlnAggProof { sender: [112, 153, ...], epoch: 3941 })) }
```

## Test 2

We will generate 3 transactions from bob. As the RLN spam limit is set to only 2, bob is going to be slashed

* `cd rln-prover`
* `RUST_LOG=debug cargo run -p prover_client -- -i 127.0.0.1 -p 50051 -a 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 send-transaction --tx-hash aa &&
   RUST_LOG=debug cargo run -p prover_client -- -i 127.0.0.1 -p 50051 -a 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 send-transaction --tx-hash ab`

* If everything goes well, you should see something like (in slasher logs)

```
2026-02-27T16:30:06.800981Z  INFO serve: slasher::proof_process: Detected too many messages for address: 0x70997970c51812dc3a010c7d01b50e0d17dc79c8
2026-02-27T16:30:06.808901Z DEBUG slasher::slashing: recovered secret hash: IdSecret(13581109468334085770107599219425644333287588127722006372575642226366077997776)
```

Note:
* If you get this error message: `Too many transactions sent by this user`, the user has already hit the RLN spam limit and prover prevent further spamming
  * Wait until the next epoch (or restart the rln-prover if you don't want to wait :-D)





