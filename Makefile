include makefile-contracts.mk

docker-pull-images-external-to-monorepo:
		docker compose -f docker/compose-tracing-v2-ci-extension.yml --profile external-to-monorepo pull

clean-local-folders:
		make clean-smc-folders
		rm -rf tmp/local/* || true # ignore failure if folders do not exist already

clean-testnet-folders:
		make clean-smc-folders
		rm -rf tmp/testnet/* || true # ignore failure if folders do not exist already

clean-environment:
		docker compose -f docker/compose-tracing-v2-ci-extension.yml -f docker/compose-tracing-v2-staterecovery-extension.yml --profile l1 --profile l2 --profile debug --profile staterecovery kill -s 9 || true;
		docker compose -f docker/compose-tracing-v2-ci-extension.yml -f docker/compose-tracing-v2-staterecovery-extension.yml --profile l1 --profile l2 --profile debug --profile staterecovery down || true;
		# Ensure RLN stack containers are stopped as well
		docker rm -f rln-prover karma-service sequencer || true;
		make clean-local-folders;
		# Remove both legacy and RLN stack volumes (ignore failures if they don't exist)
		docker volume rm linea-local-dev linea-logs docker_local-dev docker_logs docker_rln-data || true; # ignore failure if volumes do not exist already
		docker system prune -f || true;

start-env: COMPOSE_PROFILES:=l1,l2
start-env: CLEAN_PREVIOUS_ENV:=true
start-env: COMPOSE_FILE:=docker/compose-tracing-v2.yml
start-env: L1_CONTRACT_VERSION:=6
start-env: SKIP_CONTRACTS_DEPLOYMENT:=false
start-env: SKIP_L1_L2_NODE_HEALTH_CHECK:=false
start-env: LINEA_PROTOCOL_CONTRACTS_ONLY:=false
start-env:
	@if [ "$(CLEAN_PREVIOUS_ENV)" = "true" ]; then \
		$(MAKE) clean-environment; \
	else \
		echo "Starting stack reusing previous state"; \
	fi; \
	mkdir -p tmp/local; \
	COMPOSE_PROFILES=$(COMPOSE_PROFILES) docker compose -f $(COMPOSE_FILE) up -d; \
	while [ "$(SKIP_L1_L2_NODE_HEALTH_CHECK)" = "false" ] && \
			{ [ "$$(docker compose -f $(COMPOSE_FILE) ps -q l1-el-node | xargs docker inspect -f '{{.State.Health.Status}}')" != "healthy" ] || \
  			[ "$$(docker compose -f $(COMPOSE_FILE) ps -q sequencer | xargs docker inspect -f '{{.State.Health.Status}}')" != "healthy" ]; }; do \
  			sleep 2; \
  			echo "Checking health status of l1-el-node and sequencer..."; \
  	done; \
  	if [ "$(SKIP_L1_L2_NODE_HEALTH_CHECK)" = "false" ]; then \
  		echo "Container health checks passed"; \
  		echo "Performing network readiness verification..."; \
  		./scripts/verify-network-ready.sh || { echo "❌ Network readiness verification failed"; exit 1; }; \
  	fi
	if [ "$(SKIP_CONTRACTS_DEPLOYMENT)" = "true" ]; then \
		echo "Skipping contracts deployment"; \
	else \
		$(MAKE) deploy-contracts L1_CONTRACT_VERSION=$(L1_CONTRACT_VERSION) LINEA_PROTOCOL_CONTRACTS_ONLY=$(LINEA_PROTOCOL_CONTRACTS_ONLY) STATUS_NETWORK_CONTRACTS_ENABLED=$${STATUS_NETWORK_CONTRACTS_ENABLED:-false}; \
	fi

start-l1:
	make start-env COMPOSE_PROFILES:=l1 COMPOSE_FILE:=docker/compose-tracing-v2.yml SKIP_CONTRACTS_DEPLOYMENT:=true SKIP_L1_L2_NODE_HEALTH_CHECK:=true

start-l1-l2:
	make start-env COMPOSE_PROFILES:=l1,l2 COMPOSE_FILE:=docker/compose-tracing-v2.yml SKIP_CONTRACTS_DEPLOYMENT:=true SKIP_L1_L2_NODE_HEALTH_CHECK:=true

start-l2-blockchain-only:
	make start-env COMPOSE_PROFILES:=l2-bc COMPOSE_FILE:=docker/compose-tracing-v2.yml SKIP_CONTRACTS_DEPLOYMENT:=true SKIP_L1_L2_NODE_HEALTH_CHECK:=true

fresh-start-l2-blockchain-only:
	make clean-environment
	make start-l2-blockchain-only

##
## Creating new targets to avoid conflicts with existing targets
## Redundant targets above will cleanup once this get's merged
##
start-env-with-tracing-v2:
	make start-env COMPOSE_FILE=docker/compose-tracing-v2.yml LINEA_PROTOCOL_CONTRACTS_ONLY=true

## Enable L2 geth node
start-env-with-tracing-v2-extra:
	make start-env COMPOSE_PROFILES:=l1,l2 COMPOSE_FILE:=docker/compose-tracing-v2-extra-extension.yml LINEA_PROTOCOL_CONTRACTS_ONLY=true LINEA_COORDINATOR_DISABLE_TYPE2_STATE_PROOF_PROVIDER=false LINEA_COORDINATOR_SIGNER_TYPE=web3signer

start-env-with-tracing-v2-ci:
	make start-env COMPOSE_FILE=docker/compose-tracing-v2-ci-extension.yml LINEA_COORDINATOR_DISABLE_TYPE2_STATE_PROOF_PROVIDER=false LINEA_COORDINATOR_SIGNER_TYPE=web3signer

start-env-with-staterecovery: COMPOSE_PROFILES:=l1,l2,staterecovery
start-env-with-staterecovery: L1_CONTRACT_VERSION:=6
start-env-with-staterecovery:
	make start-env COMPOSE_FILE=docker/compose-tracing-v2-staterecovery-extension.yml LINEA_PROTOCOL_CONTRACTS_ONLY=true L1_CONTRACT_VERSION=$(L1_CONTRACT_VERSION) COMPOSE_PROFILES=$(COMPOSE_PROFILES)

start-env-with-rln:
	make start-env COMPOSE_FILE=docker/compose-tracing-v2-rln.yml LINEA_PROTOCOL_CONTRACTS_ONLY=true STATUS_NETWORK_CONTRACTS_ENABLED=true

start-env-with-rln-and-contracts:
	@echo "Starting complete RLN environment with automated contract deployment..."
	make start-env COMPOSE_FILE=docker/compose-tracing-v2-rln.yml LINEA_PROTOCOL_CONTRACTS_ONLY=true STATUS_NETWORK_CONTRACTS_ENABLED=true
	@echo "Complete RLN environment with contracts is ready!"

# Production mode: RLN prover connects to real smart contracts
# This starts the network in mock mode first, deploys contracts, initializes tiers, then restarts RLN services in production mode
start-env-with-rln-production:
	@echo "🚀 Starting RLN environment in PRODUCTION mode..."
	@echo "Step 1: Starting network with mock RLN (to allow contract deployment)..."
	make start-env COMPOSE_FILE=docker/compose-tracing-v2-rln.yml LINEA_PROTOCOL_CONTRACTS_ONLY=true STATUS_NETWORK_CONTRACTS_ENABLED=true
	@echo "Step 2: Extracting deployed contract addresses..."
	@KARMA_ADDR=$$(cat status-network-contracts/deployments/karma_address.txt 2>/dev/null) && \
	RLN_ADDR=$$(cat status-network-contracts/deployments/rln_address.txt 2>/dev/null) && \
	TIERS_ADDR=$$(cat status-network-contracts/deployments/karma_tiers_address.txt 2>/dev/null) && \
	echo "  Karma: $$KARMA_ADDR" && \
	echo "  RLN: $$RLN_ADDR" && \
	echo "  KarmaTiers: $$TIERS_ADDR" && \
	echo "Step 3: Initializing karma tiers..." && \
	(cd e2e && KARMA_TIERS_ADDRESS=$$TIERS_ADDR npx ts-node ../scripts/initialize-karma-tiers.ts || true) && \
	echo "Step 4: Setting up prover account permissions..." && \
	(cd e2e && KARMA_CONTRACT_ADDRESS=$$KARMA_ADDR RLN_CONTRACT_ADDRESS=$$RLN_ADDR node ../scripts/setup-prover-account.js || true) && \
	(cd e2e && KARMA_CONTRACT_ADDRESS=$$KARMA_ADDR node ../scripts/grant-operator-role.js || true) && \
	echo "Step 5: Restarting RLN prover in production mode..." && \
	docker stop rln-prover karma-service 2>/dev/null || true && \
	docker rm rln-prover karma-service 2>/dev/null || true && \
	RLN_PROVER_IMAGE=$$(docker images --format '{{.Repository}}:{{.Tag}}' | grep status-rln-prover | head -1) && \
	echo "  Using prover image: $$RLN_PROVER_IMAGE" && \
	docker run -d --name rln-prover --hostname rln-prover \
		--network docker_linea --ip 11.11.11.120 \
		-p 50051:50051 -p 50052:50052 \
		-v linea-local-dev:/app/data \
		-e RUST_LOG=debug \
		-e DATABASE_URL=postgres://postgres:postgres@postgres:5432/prover_db \
		-e PRIVATE_KEY=0x8f5a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a \
		$$RLN_PROVER_IMAGE \
		--no-config --ip 0.0.0.0 --port 50051 \
		--ws-rpc-url ws://sequencer:8546 \
		--ksc $$KARMA_ADDR --rlnsc $$RLN_ADDR --tsc $$TIERS_ADDR \
		--registration-min 1 \
		--db postgres://postgres:postgres@postgres:5432/prover_db && \
	echo "✅ RLN environment running in PRODUCTION mode!" && \
	echo "   - RLN Prover connected to real smart contracts" && \
	echo "   - Karma tiers initialized" && \
	echo "   - Users must have Karma to use gasless transactions"

staterecovery-replay-from-block: L1_ROLLUP_CONTRACT_ADDRESS:=0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9
staterecovery-replay-from-block: STATERECOVERY_OVERRIDE_START_BLOCK_NUMBER:=1
staterecovery-replay-from-block:
	docker compose -f docker/compose-tracing-v2-staterecovery-extension.yml down zkbesu-shomei-sr shomei-sr
	L1_ROLLUP_CONTRACT_ADDRESS=$(L1_ROLLUP_CONTRACT_ADDRESS) STATERECOVERY_OVERRIDE_START_BLOCK_NUMBER=$(STATERECOVERY_OVERRIDE_START_BLOCK_NUMBER) docker compose -f docker/compose-tracing-v2-staterecovery-extension.yml up zkbesu-shomei-sr shomei-sr -d

stop_pid:
		if [ -f $(PID_FILE) ]; then \
			kill `cat $(PID_FILE)`; \
			echo "Stopped process with PID `cat $(PID_FILE)`"; \
			rm $(PID_FILE); \
		else \
			echo "$(PID_FILE) does not exist. No process to stop."; \
		fi


