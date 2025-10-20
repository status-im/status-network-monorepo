# Status L2 Rln Prover

## Docker

* docker build --progress=plain --no-cache -t prover .
* docker run -p 50051:50051 prover --mock-sc true --mock-user mock/mock_user_1.json

## Run prover

PRIVATE_KEY=__MY_PRIVATE_KEY__ RUST_LOG=debug cargo run -p prover_cli -- --no-config

### Run prover + Mock

RUST_LOG=debug cargo run -p prover_cli -- --ip 127.0.0.1 --metrics-ip 127.0.0.1 --mock-sc true --mock-user mock/mock_user_1.json --no-config

### Run prover + opentelemetry

* Run jaeger (locally, port 16686 -> Web ui, port 4317 -> otlp/grpc, port 4318 -> otlp/http)
  * docker run -d --name jaeger -e COLLECTOR_OTLP_ENABLED=true -p 16686:16686 -p 4317:4317 -p 4318:4318 jaegertracing/all-in-one:latest
* Run prover:
  * OTEL_EXPORTER_OTLP_PROTOCOL=grpc RUST_LOG=debug cargo run -p prover_cli -- --ip 127.0.0.1 --metrics-ip 127.0.0.1 --mock-sc true --mock-user mock/mock_user_1.json

### Run prover client (for tests)

* RUST_LOG=debug cargo run -p prover_client -- --help
* RUST_LOG=debug cargo run -p prover_client -- -i 127.0.0.1 -p 50051 register-user
* RUST_LOG=debug cargo run -p prover_client -- -i 127.0.0.1 -p 50051 send-transaction --tx-hash aa
* RUST_LOG=debug cargo run -p prover_client -- -i 127.0.0.1 -p 50051 -a 0xd8da6bf26964af9d7eed9e03e53415d37aa96045 get-user-tier-info

## Debug

* grpcurl -plaintext 127.0.0.1:50051 list
* grpcurl -plaintext -d '{"sender": "Alice", "tx_id": "42"}' '[::1]:50051' prover.RlnProver/SendTransaction
* grpcurl -plaintext '[::1]:50051' prover.RlnProver/GetProofs

## Bench

* SUBSCRIBER_COUNT=2 SUBSCRIBER_IP=10.235.185.198 RAYON_NUM_THREADS=4 PROOF_SERVICE_COUNT=4 PROOF_COUNT=6 cargo bench -p prover --bench prover_many_subscribers

## Unit tests

* cargo test 
* cargo test --features anvil 