# Invoked from repo root via: include contracts/makefile-contracts.mk
# Directory containing this file (contracts package root); works with relative or absolute include paths.
contracts_package_dir := $(dir $(lastword $(MAKEFILE_LIST)))

pnpm-install:
		pnpm install

clean-smc-folders:
		rm -f $(contracts_package_dir).openzeppelin/unknown-31648428.json
		rm -f $(contracts_package_dir).openzeppelin/unknown-1337.json

compile-contracts:
		cd $(contracts_package_dir); \
		make compile

compile-contracts-no-cache:
		cd $(contracts_package_dir); \
		make force-compile

deploy-eip-system-contracts:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		cd $(contracts_package_dir); \
		DEPLOYER_PRIVATE_KEY=$${DEPLOYMENT_PRIVATE_KEY:-0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae} \
		RPC_URL=http:\\localhost:8545/ \
		npx ts-node local-deployments-artifacts/deployEIPSystemContracts.ts

deploy-upgradeable-predeploys:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		cd $(contracts_package_dir); \
		DEPLOYER_PRIVATE_KEY=$${DEPLOYMENT_PRIVATE_KEY:-0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae} \
		RPC_URL=http:\\localhost:8545/ \
		npx ts-node local-deployments-artifacts/deployPredeployContractsV1.ts

deploy-linea-rollup: L1_CONTRACT_VERSION:=7_1
deploy-linea-rollup:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		cd $(contracts_package_dir); \
		DEPLOYER_PRIVATE_KEY=$${DEPLOYMENT_PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80} \
		RPC_URL=http:\\localhost:8445/ \
		VERIFIER_CONTRACT_NAME=IntegrationTestTrueVerifier \
		INITIAL_L2_STATE_ROOT_HASH=0x01d9afcd495c870f3ae9d8362cd0257a7de2057055058183596719285cae6101 \
		INITIAL_L2_BLOCK_NUMBER=0 \
		L2_GENESIS_TIMESTAMP=1683325137 \
		L1_SECURITY_COUNCIL=0x90F79bf6EB2c4f870365E785982E1f101E93b906 \
		LINEA_ROLLUP_OPERATORS=$${LINEA_ROLLUP_OPERATORS:-0x70997970C51812dc3A010C7d01b50e0d17dc79C8,0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC} \
		LINEA_ROLLUP_RATE_LIMIT_PERIOD=86400 \
		LINEA_ROLLUP_RATE_LIMIT_AMOUNT=1000000000000000000000 \
		YIELD_MANAGER_ADDRESS=0x000000000000000000000000000000000000dEaD \
		npx ts-node local-deployments-artifacts/deployPlonkVerifierAndLineaRollupV$(L1_CONTRACT_VERSION).ts

deploy-linea-rollup-v6:
		$(MAKE) deploy-linea-rollup L1_CONTRACT_VERSION=6

deploy-linea-rollup-v7_1:
		$(MAKE) deploy-linea-rollup L1_CONTRACT_VERSION=7_1

deploy-linea-rollup-v7: deploy-linea-rollup-v7_1

deploy-validium: L1_CONTRACT_VERSION:=1
deploy-validium:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		cd $(contracts_package_dir); \
		DEPLOYER_PRIVATE_KEY=$${DEPLOYMENT_PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80} \
		RPC_URL=http:\\localhost:8445/ \
		VERIFIER_CONTRACT_NAME=IntegrationTestTrueVerifier \
		INITIAL_L2_STATE_ROOT_HASH=0x01d9afcd495c870f3ae9d8362cd0257a7de2057055058183596719285cae6101 \
		INITIAL_L2_BLOCK_NUMBER=0 \
		L2_GENESIS_TIMESTAMP=1683325137 \
		L1_SECURITY_COUNCIL=0x90F79bf6EB2c4f870365E785982E1f101E93b906 \
		VALIDIUM_OPERATORS=$${VALIDIUM_OPERATORS:-0x70997970C51812dc3A010C7d01b50e0d17dc79C8,0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC} \
		VALIDIUM_RATE_LIMIT_PERIOD=86400 \
		VALIDIUM_RATE_LIMIT_AMOUNT=1000000000000000000000 \
		npx ts-node local-deployments-artifacts/deployPlonkVerifierAndValidiumV$(L1_CONTRACT_VERSION).ts

deploy-l2messageservice:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		cd $(contracts_package_dir); \
		L2_MESSAGE_SERVICE_CONTRACT_NAME=L2MessageService \
		DEPLOYER_PRIVATE_KEY=$${DEPLOYMENT_PRIVATE_KEY:-0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae} \
		RPC_URL=http:\\localhost:8545/ \
		L2_SECURITY_COUNCIL=0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266 \
		L2_MESSAGE_SERVICE_L1L2_MESSAGE_SETTER=$${L2_MESSAGE_SERVICE_L1L2_MESSAGE_SETTER:-0xd42e308fc964b71e18126df469c21b0d7bcb86cc} \
		L2_MESSAGE_SERVICE_RATE_LIMIT_PERIOD=86400 \
		L2_MESSAGE_SERVICE_RATE_LIMIT_AMOUNT=1000000000000000000000 \
		npx ts-node local-deployments-artifacts/deployL2MessageServiceV1.ts

deploy-token-bridge-l1:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		cd $(contracts_package_dir); \
		DEPLOYER_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
		REMOTE_DEPLOYER_ADDRESS=0x1B9AbEeC3215D8AdE8a33607f2cF0f4F60e5F0D0 \
		RPC_URL=http:\\localhost:8445/ \
		REMOTE_CHAIN_ID=1337 \
		TOKEN_BRIDGE_L1=true \
		L1_SECURITY_COUNCIL=0x90F79bf6EB2c4f870365E785982E1f101E93b906 \
		L2_MESSAGE_SERVICE_ADDRESS=0xe537D669CA013d86EBeF1D64e40fC74CADC91987 \
		LINEA_ROLLUP_ADDRESS=0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9 \
		npx ts-node local-deployments-artifacts/deployBridgedTokenAndTokenBridgeV1_1.ts

deploy-token-bridge-l2:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		cd $(contracts_package_dir); \
		DEPLOYER_PRIVATE_KEY=0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae \
		REMOTE_DEPLOYER_ADDRESS=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 \
		RPC_URL=http:\\localhost:8545/ \
		REMOTE_CHAIN_ID=31648428 \
		TOKEN_BRIDGE_L1=false \
		L2_SECURITY_COUNCIL=0xf17f52151EbEF6C7334FAD080c5704D77216b732 \
		L2_MESSAGE_SERVICE_ADDRESS=0xe537D669CA013d86EBeF1D64e40fC74CADC91987 \
		LINEA_ROLLUP_ADDRESS=0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9 \
		npx ts-node local-deployments-artifacts/deployBridgedTokenAndTokenBridgeV1_1.ts

deploy-l1-test-erc20:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		cd $(contracts_package_dir); \
		DEPLOYER_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
		RPC_URL=http:\\localhost:8445/ \
		TEST_ERC20_L1=true \
		TEST_ERC20_NAME=TestERC20 \
		TEST_ERC20_SYMBOL=TERC20 \
		TEST_ERC20_INITIAL_SUPPLY=100000 \
		npx ts-node local-deployments-artifacts/deployTestERC20.ts

deploy-l2-test-erc20:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		cd $(contracts_package_dir); \
		DEPLOYER_PRIVATE_KEY=0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae \
		RPC_URL=http:\\localhost:8545/ \
		TEST_ERC20_L1=false \
		TEST_ERC20_NAME=TestERC20 \
		TEST_ERC20_SYMBOL=TERC20 \
		TEST_ERC20_INITIAL_SUPPLY=100000 \
		npx ts-node local-deployments-artifacts/deployTestERC20.ts

# Status Network: ensure Foundry libraries for the status-network-contracts submodule are present.
# Ported back from develop after the Linea upstream merge moved makefile-contracts.mk and dropped it.
status-network-contracts-setup:
		cd status-network-contracts; \
		rm -f foundry.lock; \
		if ! command -v forge >/dev/null 2>&1; then \
			echo "Foundry (forge) not found. Installing via foundryup..."; \
			curl -L https://foundry.paradigm.xyz | bash; \
			if [ -f "$$HOME/.foundry/bin/foundryup" ]; then \
				"$$HOME/.foundry/bin/foundryup" -y; \
				export PATH="$$HOME/.foundry/bin:$$PATH"; \
			fi; \
		fi; \
		if [ ! -d lib/openzeppelin-contracts/contracts ]; then \
			mkdir -p lib; \
			TMP_DIR=$$(mktemp -d); \
			curl -L --fail https://github.com/OpenZeppelin/openzeppelin-contracts/archive/refs/tags/v4.9.6.tar.gz -o "$$TMP_DIR/oz496.tar.gz"; \
			tar -xzf "$$TMP_DIR/oz496.tar.gz" -C "$$TMP_DIR"; \
			rm -rf lib/openzeppelin-contracts; \
			mv "$$TMP_DIR/openzeppelin-contracts-4.9.6" lib/openzeppelin-contracts; \
			rm -rf "$$TMP_DIR"; \
		fi; \
		if [ ! -d lib/openzeppelin-contracts-upgradeable/contracts ]; then \
			TMP_DIR=$$(mktemp -d); \
			curl -L --fail https://github.com/OpenZeppelin/openzeppelin-contracts-upgradeable/archive/refs/tags/v4.9.6.tar.gz -o "$$TMP_DIR/ozu496.tar.gz"; \
			tar -xzf "$$TMP_DIR/ozu496.tar.gz" -C "$$TMP_DIR"; \
			rm -rf lib/openzeppelin-contracts-upgradeable; \
			mv "$$TMP_DIR/openzeppelin-contracts-upgradeable-4.9.6" lib/openzeppelin-contracts-upgradeable; \
			rm -rf "$$TMP_DIR"; \
		fi; \
		if [ ! -f lib/openzeppelin-contracts/contracts/security/Pausable.sol ] || [ ! -f lib/openzeppelin-contracts/contracts/token/ERC20/extensions/draft-ERC20Permit.sol ]; then \
			echo "Expected OZ files missing, reinstalling OpenZeppelin v4.8.3 for compatibility"; \
			rm -rf lib/openzeppelin-contracts; \
			mkdir -p lib; \
			TMP_DIR=$$(mktemp -d); \
			curl -L --fail https://github.com/OpenZeppelin/openzeppelin-contracts/archive/refs/tags/v4.8.3.tar.gz -o "$$TMP_DIR/oz.tar.gz"; \
			tar -xzf "$$TMP_DIR/oz.tar.gz" -C "$$TMP_DIR"; \
			mv "$$TMP_DIR/openzeppelin-contracts-4.8.3" lib/openzeppelin-contracts; \
			rm -rf lib/openzeppelin-contracts-upgradeable; \
			curl -L --fail https://github.com/OpenZeppelin/openzeppelin-contracts-upgradeable/archive/refs/tags/v4.8.3.tar.gz -o "$$TMP_DIR/ozu.tar.gz"; \
			tar -xzf "$$TMP_DIR/ozu.tar.gz" -C "$$TMP_DIR"; \
			mv "$$TMP_DIR/openzeppelin-contracts-upgradeable-4.8.3" lib/openzeppelin-contracts-upgradeable; \
			rm -rf "$$TMP_DIR"; \
		fi
		if [ ! -d status-network-contracts/lib/forge-std/src ]; then \
			cd status-network-contracts; \
			mkdir -p lib; \
			TMP_DIR=$$(mktemp -d); \
			curl -L --fail https://github.com/foundry-rs/forge-std/archive/refs/tags/v1.8.2.tar.gz -o "$$TMP_DIR/fs.tar.gz"; \
			tar -xzf "$$TMP_DIR/fs.tar.gz" -C "$$TMP_DIR"; \
			rm -rf lib/forge-std; \
			mv "$$TMP_DIR/forge-std-1.8.2" lib/forge-std; \
			rm -rf "$$TMP_DIR"; \
		fi

# Status Network: deploy Karma, KarmaTiers, StakeManager, RLN, KarmaNFT via Forge scripts.
# Order matters — Karma must exist before StakeManager, RLN, and KarmaNFT (they reference Karma).
deploy-status-network-contracts:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		$(MAKE) status-network-contracts-setup
		@echo "Deploying Status Network Contracts..."
		@echo "Deploying KarmaTiers contract..."
		@cd status-network-contracts && \
		FOUNDRY_DISABLE_NIGHTLY_WARNING=true ETH_FROM=0x1B9AbEeC3215D8AdE8A33607f2cF0f4F60e5F0D0 forge script script/DeployKarmaTiers.s.sol:DeployKarmaTiersScript \
			--rpc-url http://localhost:8545 \
			--private-key $${DEPLOYMENT_PRIVATE_KEY:-0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae} \
			--broadcast --slow --root . || { echo "KarmaTiers deployment failed"; exit 1; }
		@echo "KarmaTiers deployed successfully!"
		@echo "Deploying Karma contract..."
		@cd status-network-contracts && \
		FOUNDRY_DISABLE_NIGHTLY_WARNING=true ETH_FROM=0x1B9AbEeC3215D8AdE8A33607f2cF0f4F60e5F0D0 forge script script/DeployKarma.s.sol:DeployKarmaScript \
			--rpc-url http://localhost:8545 \
			--private-key $${DEPLOYMENT_PRIVATE_KEY:-0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae} \
			--broadcast --slow --root . || { echo "Karma deployment failed"; exit 1; }
		@echo "Karma deployed successfully!"
		@echo "Deploying StakeManager contract..."
		@cd status-network-contracts && \
		KARMA_ADDRESS=$$(./scripts/get-deployed-address.sh DeployKarma.s.sol Karma 2>/dev/null) && \
		if [ -z "$$KARMA_ADDRESS" ]; then \
			echo "Failed to extract Karma contract address"; \
			exit 1; \
		fi && \
		echo "Using Karma address: $$KARMA_ADDRESS" && \
		FOUNDRY_DISABLE_NIGHTLY_WARNING=true ETH_FROM=0x1B9AbEeC3215D8AdE8A33607f2cF0f4F60e5F0D0 KARMA_ADDRESS=$$KARMA_ADDRESS MAX_VAULTS_PER_USER=5 forge script script/DeployStakeManager.s.sol:DeployStakeManagerScript \
			--rpc-url http://localhost:8545 \
			--private-key $${DEPLOYMENT_PRIVATE_KEY:-0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae} \
			--broadcast --slow --root . || { echo "StakeManager deployment failed"; exit 1; }
		@echo "StakeManager deployed successfully!"
		@echo "Deploying RLN contract..."
		@cd status-network-contracts && \
		KARMA_ADDRESS=$$(./scripts/get-deployed-address.sh DeployKarma.s.sol Karma 2>/dev/null) && \
		if [ -z "$$KARMA_ADDRESS" ]; then \
			echo "Failed to extract Karma contract address"; \
			exit 1; \
		fi && \
		echo "Using Karma address: $$KARMA_ADDRESS" && \
		FOUNDRY_DISABLE_NIGHTLY_WARNING=true ETH_FROM=0x1B9AbEeC3215D8AdE8A33607f2cF0f4F60e5F0D0 DEPTH=20 KARMA_ADDRESS=$$KARMA_ADDRESS forge script script/RLN.s.sol:DeployRLNScript \
			--rpc-url http://localhost:8545 \
			--private-key $${DEPLOYMENT_PRIVATE_KEY:-0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae} \
			--broadcast --slow --root . || { echo "RLN deployment failed"; exit 1; }
		@echo "RLN deployed successfully!"
		@echo "Deploying KarmaNFT contract..."
		@cd status-network-contracts && \
		KARMA_ADDRESS=$$(./scripts/get-deployed-address.sh DeployKarma.s.sol Karma 2>/dev/null) && \
		if [ -z "$$KARMA_ADDRESS" ]; then \
			echo "Failed to extract Karma contract address"; \
			exit 1; \
		fi && \
		echo "Using Karma address: $$KARMA_ADDRESS" && \
		FOUNDRY_DISABLE_NIGHTLY_WARNING=true ETH_FROM=0x1B9AbEeC3215D8AdE8A33607f2cF0f4F60e5F0D0 KARMA_ADDRESS=$$KARMA_ADDRESS NFT_METADATA_GENERATOR_ADDRESS=0x1B9AbEeC3215D8AdE8A33607f2cF0f4F60e5F0D0 forge script script/DeployKarmaNFT.s.sol:DeployKarmaNFTScript \
			--rpc-url http://localhost:8545 \
			--private-key $${DEPLOYMENT_PRIVATE_KEY:-0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae} \
			--broadcast --slow --root . || { echo "KarmaNFT deployment failed"; exit 1; }
		@echo "KarmaNFT deployed successfully!"
		@echo "All Status Network contracts deployed successfully!"
		@mkdir -p status-network-contracts/deployments
		@cd status-network-contracts && \
		KARMA_TIERS=$$(./scripts/get-deployed-address.sh DeployKarmaTiers.s.sol KarmaTiers 2>/dev/null) && \
		STAKE_MANAGER=$$(./scripts/get-deployed-address.sh DeployStakeManager.s.sol StakeManager 2>/dev/null) && \
		KARMA=$$(./scripts/get-deployed-address.sh DeployKarma.s.sol Karma 2>/dev/null) && \
		RLN=$$(./scripts/get-deployed-address.sh RLN.s.sol RLN 2>/dev/null) && \
		KARMA_NFT=$$(./scripts/get-deployed-address.sh DeployKarmaNFT.s.sol KarmaNFT 2>/dev/null) && \
		echo "   KarmaTiers: $$KARMA_TIERS" && \
		echo "   StakeManager: $$STAKE_MANAGER" && \
		echo "   Karma: $$KARMA" && \
		echo "   RLN: $$RLN" && \
		echo "   KarmaNFT: $$KARMA_NFT" && \
		echo "$$KARMA_TIERS" > deployments/karma_tiers_address.txt && \
		echo "$$KARMA" > deployments/karma_address.txt && \
		echo "$$RLN" > deployments/rln_address.txt && \
		echo "$$STAKE_MANAGER" > deployments/stake_manager_address.txt && \
		echo "$$KARMA_NFT" > deployments/karma_nft_address.txt && \
		echo "Contract addresses saved to status-network-contracts/deployments/"

deploy-contracts: L1_CONTRACT_VERSION:=6
deploy-contracts: LINEA_PROTOCOL_CONTRACTS_ONLY:=false
deploy-contracts: STATUS_NETWORK_CONTRACTS_ENABLED:=false
deploy-contracts: LINEA_L1_CONTRACT_DEPLOYMENT_TARGET:=deploy-linea-rollup-v$(L1_CONTRACT_VERSION)
deploy-contracts:
	cd $(contracts_package_dir); \
	export L1_NONCE=$$(npx ts-node local-deployments-artifacts/get-wallet-nonce.ts --wallet-priv-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 --rpc-url http://localhost:8445) && \
	export L2_NONCE=$$(npx ts-node local-deployments-artifacts/get-wallet-nonce.ts --wallet-priv-key 0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae --rpc-url http://localhost:8545) && \
	cd .. && \
	if [ "$(LINEA_PROTOCOL_CONTRACTS_ONLY)" = "false" ]; then \
		$(MAKE) -j7 $(LINEA_L1_CONTRACT_DEPLOYMENT_TARGET) deploy-token-bridge-l1 deploy-l1-test-erc20 deploy-l2messageservice deploy-token-bridge-l2 deploy-l2-test-erc20 deploy-l2-evm-opcode-tester; \
	else \
		$(MAKE) -j6 $(LINEA_L1_CONTRACT_DEPLOYMENT_TARGET) deploy-l2messageservice; \
	fi && \
	if [ "$(STATUS_NETWORK_CONTRACTS_ENABLED)" = "true" ]; then \
		echo "Deploying Status Network contracts (Karma, KarmaTiers, StakeManager, RLN, KarmaNFT)..." && \
		$(MAKE) deploy-status-network-contracts; \
	else \
		echo "Status Network contracts deployment skipped (set STATUS_NETWORK_CONTRACTS_ENABLED=true to enable)."; \
	fi


deploy-l2-evm-opcode-tester:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		cd $(contracts_package_dir); \
		DEPLOYER_PRIVATE_KEY=0x8f2a55949038a9610f50fb23b5883af3b4ecb3c3bb792cbcefbd1542c692be63 \
		RPC_URL=http:\\localhost:8545/ \
		npx ts-node local-deployments-artifacts/deployOpcodeTestingFramework.ts

evm-opcode-tester-execute-all-opcodes: OPCODE_TEST_CONTRACT_ADDRESS:=0xa50a51c09a5c451C52BB714527E1974b686D8e77
evm-opcode-tester-execute-all-opcodes: NUMBER_OF_RUNS:=3
evm-opcode-tester-execute-all-opcodes:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		cd $(contracts_package_dir); \
		OPCODE_TEST_CONTRACT_ADDRESS=$(OPCODE_TEST_CONTRACT_ADDRESS) \
		NUMBER_OF_RUNS=$(NUMBER_OF_RUNS) \
		DEPLOYER_PRIVATE_KEY=0x8f2a55949038a9610f50fb23b5883af3b4ecb3c3bb792cbcefbd1542c692be63 \
		RPC_URL=http:\\localhost:8545/ \
		npx ts-node local-deployments-artifacts/executeAllOpcodes.ts

deploy-l2-scenario-testing-proxy:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		cd $(contracts_package_dir); \
		DEPLOYER_PRIVATE_KEY=0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae \
		RPC_URL=http:\\localhost:8545/ \
		npx ts-node local-deployments-artifacts/deployLineaScenarioDelegatingProxy.ts

execute-scenario-testing-proxy-scenario: LINEA_SCENARIO_DELEGATING_PROXY_ADDRESS:=0x2f6dAaF8A81AB675fbD37Ca6Ed5b72cf86237453
execute-scenario-testing-proxy-scenario:
		# WARNING: FOR LOCAL DEV ONLY - DO NOT REUSE THESE KEYS ELSEWHERE
		# GAS_LIMIT=452500 will cause it to fail
		cd $(contracts_package_dir); \
		LINEA_SCENARIO_DELEGATING_PROXY_ADDRESS=$(LINEA_SCENARIO_DELEGATING_PROXY_ADDRESS) \
		NUMBER_OF_LOOPS=10000000 \
		LINEA_SCENARIO=1 \
		GAS_LIMIT=452500 \
		DEPLOYER_PRIVATE_KEY=0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae \
		RPC_URL=http:\\localhost:8545/ \
		npx ts-node local-deployments-artifacts/executeLineaScenarioDelegatingProxyScenario.ts

