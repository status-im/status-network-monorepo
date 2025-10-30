// std
use std::str::FromStr;
// third-party
use alloy::network::EthereumWallet;
use alloy::providers::{ProviderBuilder, WsConnect};
use alloy::signers::local::PrivateKeySigner;
use alloy::{
    hex,
    primitives::{Address, U256},
};
use clap::Parser;
use rustls::crypto::aws_lc_rs;
use url::Url;
// internal
use smart_contract::{KarmaRLNSC::KarmaRLNSCInstance, RlnScError};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// WebSocket RPC URL
    #[arg(long, default_value = "wss://public.sepolia.rpc.status.network/ws")]
    ws_rpc_url: String,

    /// Contract address
    #[arg(long, default_value = "0xc98994691E96D2f4CA2a718Bc8FDF30bd21d1c59")]
    contract_address: String,

    /// Private key for signing transactions
    /// Warning: this is a test key, do not use in production
    #[arg(long, default_value = "")]
    private_key: String,

    /// Test identity commitment
    #[arg(long, default_value = "0")]
    test_identity_commitment: u64,

    /// Test user address
    #[arg(long, default_value = "0x360a45F70De193090A1b13dA8393A02F9119aeCd")]
    test_user_address: String,
}

#[tokio::main]
async fn main() -> Result<(), RlnScError> {
    // install crypto provider for rustls - required for WebSocket TLS connections
    rustls::crypto::CryptoProvider::install_default(aws_lc_rs::default_provider())
        .expect("Failed to install default CryptoProvider");

    let args = Args::parse();

    println!("Testing KarmaRLN Contract Interaction");

    println!("Connecting to RPC: {}", args.ws_rpc_url);

    let contract_addr = Address::from_str(&args.contract_address)
        .map_err(|e| RlnScError::SignerConnectionError(format!("Invalid contract address: {e}")))?;

    let test_identity_commitment = U256::from(args.test_identity_commitment);
    let test_user_address = Address::from_str(&args.test_user_address)
        .map_err(|e| RlnScError::SignerConnectionError(format!("Invalid user address: {e}")))?;

    let url = Url::parse(&args.ws_rpc_url)
        .map_err(|e| RlnScError::SignerConnectionError(format!("Invalid URL: {e}")))?;

    if args.private_key.is_empty() {
        return Err(RlnScError::EmptyPrivateKey);
    }

    // Connect to KarmaRLN contract with signer
    let provider_with_signer = {
        let pk_signer = PrivateKeySigner::from_str(args.private_key.as_str()).unwrap();
        let wallet = EthereumWallet::from(pk_signer);

        let ws = WsConnect::new(url.clone().as_str());
        ProviderBuilder::new()
            .wallet(wallet)
            .connect_ws(ws)
            .await
            .map_err(RlnScError::RpcTransportError)?
    };
    let rln_contract = KarmaRLNSCInstance::new(contract_addr, provider_with_signer);

    println!("Successfully connected to RLN contract with signer at {contract_addr}",);

    println!("\nTesting RLN Contract Functions:");
    println!("=====================================");

    // Test 1: Get registry information
    match (
        rln_contract.SET_SIZE().call().await,
        rln_contract.identityCommitmentIndex().call().await,
        rln_contract.karma().call().await,
    ) {
        (Ok(set_size), Ok(current_index), Ok(karma_address)) => {
            let is_full = current_index >= set_size;
            let available_slots = if is_full {
                U256::ZERO
            } else {
                set_size - current_index
            };

            println!("Registry Info:");
            println!("   Set size: {set_size}");
            println!("   Current index: {current_index}");
            println!("   Karma address: {karma_address}");
            println!("   Is full: {is_full}");
            println!("   Available slots: {available_slots}");
        }
        _ => {
            eprintln!("Failed to get registry info");
        }
    }

    // Test 2: Check if specific member is registered
    match rln_contract.members(test_identity_commitment).call().await {
        Ok(member) => {
            if member.userAddress != Address::ZERO {
                println!("Member {test_identity_commitment} is registered:");
                println!("   User address: {}", member.userAddress);
                println!("   Index: {}", member.index);
            } else {
                println!("Member {test_identity_commitment} is not registered");
            }
        }
        Err(e) => {
            eprintln!("Failed to check member status: {e}");
        }
    }

    // Test 3: Get role information
    match (
        rln_contract.SLASHER_ROLE().call().await,
        rln_contract.REGISTER_ROLE().call().await,
    ) {
        (Ok(slasher_role), Ok(register_role)) => {
            println!("Role Information:");
            println!("   Slasher role: 0x{}", hex::encode(slasher_role));
            println!("   Register role: 0x{}", hex::encode(register_role));
        }
        _ => {
            eprintln!("Failed to get roles");
        }
    }

    // Test 4: Check permissions for test user
    match (
        rln_contract.REGISTER_ROLE().call().await,
        rln_contract.SLASHER_ROLE().call().await,
    ) {
        (Ok(register_role), Ok(slasher_role)) => {
            match rln_contract
                .hasRole(register_role, test_user_address)
                .call()
                .await
            {
                Ok(can_register) => {
                    println!("Account {test_user_address} can register: {can_register}",);
                }
                Err(e) => eprintln!("Failed to check register permission: {e}"),
            }

            match rln_contract
                .hasRole(slasher_role, test_user_address)
                .call()
                .await
            {
                Ok(can_slash) => {
                    println!("Account {test_user_address} can slash: {can_slash}");
                }
                Err(e) => eprintln!("Failed to check slash permission: {e}"),
            }
        }
        _ => {
            eprintln!("Failed to get roles for permission check");
        }
    }

    // Test 5: Check if new member already exists before registering
    match rln_contract.members(test_identity_commitment).call().await {
        Ok(member) => {
            if member.userAddress == Address::ZERO {
                println!("Attempting to register new member...");
                println!("   Identity commitment: {test_identity_commitment}");
                println!("   User address: {test_user_address}");

                // Test 6: Register new member using the register function
                match rln_contract
                    .register(test_identity_commitment, test_user_address)
                    .send()
                    .await
                {
                    Ok(tx_hash) => {
                        match tx_hash.watch().await {
                            Ok(_) => {
                                println!("Registration successful!");

                                // Test 7: Verify registration was successful
                                match rln_contract.members(test_identity_commitment).call().await {
                                    Ok(verified_member) => {
                                        if verified_member.userAddress != Address::ZERO {
                                            println!("Registration verified:");
                                            println!("   User: {}", verified_member.userAddress);
                                            println!("   Index: {}", verified_member.index);
                                        } else {
                                            println!("Registration not found in verification");
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to verify registration: {e}");
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to wait for transaction: {e}");
                                return Err(RlnScError::PendingTransactionError(e));
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to register member: {e}");
                        return Err(RlnScError::Alloy(e));
                    }
                }
            } else {
                println!("Member {test_identity_commitment} already registered:");
                println!("   User: {}", member.userAddress);
                println!("   Index: {}", member.index);
            }
        }
        Err(e) => {
            eprintln!("Failed to check if member exists: {e}");
            return Err(RlnScError::Alloy(e));
        }
    }

    println!("\nTesting completed!");
    Ok(())
}
