use alloy::primitives::Address;
use alloy::providers::Provider;
use clap::Parser;
use rustls::crypto::aws_lc_rs;
use smart_contract::{KarmaSC, KarmaScError, ws_provider};
use std::str::FromStr;
use url::Url;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// WebSocket RPC URL
    #[arg(long, default_value = "wss://public.sepolia.rpc.status.network/ws")]
    rpc_url: String,

    /// Karma contract address
    #[arg(long, default_value = "0x9ADD5A2F68d0d95F74C751a2081EFF57Ff1C836F")]
    karma_contract_address: String,

    /// Test account address
    #[arg(long, default_value = "0x360a45F70De193090A1b13dA8393A02F9119aeCd")]
    test_account: String,
}

#[tokio::main]
async fn main() -> Result<(), KarmaScError> {
    // install crypto provider for rustls - required for WebSocket TLS connections
    rustls::crypto::CryptoProvider::install_default(aws_lc_rs::default_provider())
        .expect("Failed to install default CryptoProvider");

    let args = Args::parse();

    println!("Testing Karma Contract Interaction");

    println!("Connecting to RPC: {}", args.rpc_url);

    let karma_contract_addr = Address::from_str(&args.karma_contract_address).map_err(|e| {
        KarmaScError::SignerConnectionError(format!("Invalid karma contract address: {e}"))
    })?;

    let account_addr = Address::from_str(&args.test_account).map_err(|e| {
        KarmaScError::SignerConnectionError(format!("Invalid account address: {e}"))
    })?;

    let url = Url::parse(&args.rpc_url)
        .map_err(|e| KarmaScError::SignerConnectionError(format!("Invalid URL: {e}")))?;

    // Connect to Karma contract
    let provider = ws_provider(url.to_string()).await?.erased();
    let karma_contract = KarmaSC::KarmaSCInstance::new(karma_contract_addr, provider);

    println!("Successfully connected to Karma contract at {karma_contract_addr}",);

    println!("\nTesting Contract Functions:");
    println!("=====================================");

    // Test Karma Contract Functions
    println!("\n--- Karma Contract Tests ---");

    // Test 1: Get account balance (karma_amount test)
    match karma_contract.balanceOf(account_addr).call().await {
        Ok(balance) => {
            println!("Account {account_addr} balance: {balance} KARMA");

            // Additional test to verify karma_amount matches balanceOf
            match smart_contract::KarmaAmountExt::karma_amount(&karma_contract, &account_addr).await
            {
                Ok(karma_amount) => {
                    if balance == karma_amount {
                        println!("✓ karma_amount matches balanceOf: {karma_amount}");
                    } else {
                        println!(
                            "✗ karma_amount mismatch! balanceOf: {balance}, karma_amount: {karma_amount}"
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Failed to get karma_amount: {e}");
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to get balance: {e}");
        }
    }

    println!("\nTesting completed!");
    Ok(())
}
