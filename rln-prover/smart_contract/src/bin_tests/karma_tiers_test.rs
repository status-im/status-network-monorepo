use alloy::network::EthereumWallet;
use alloy::providers::{ProviderBuilder, WsConnect};
use alloy::signers::local::PrivateKeySigner;
use alloy::{
    hex,
    primitives::{Address, U256},
};
use clap::Parser;
use rustls::crypto::aws_lc_rs;
use smart_contract::KarmaTiers::KarmaTiersInstance;
use smart_contract::{KarmaTiers, KarmaTiersError};
use std::str::FromStr;
use url::Url;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// WebSocket RPC URL
    #[arg(long, default_value = "wss://public.sepolia.rpc.status.network/ws")]
    ws_rpc_url: String,

    /// Contract address
    #[arg(long, default_value = "0x011b9de308BE357BbF24EfB387a270a14A04E5d2")]
    contract_address: String,

    /// Private key for signing transactions
    #[arg(long, default_value = "")]
    private_key: String,
}

#[tokio::main]
async fn main() -> Result<(), KarmaTiersError> {
    // install crypto provider for rustls - required for WebSocket TLS connections
    rustls::crypto::CryptoProvider::install_default(aws_lc_rs::default_provider())
        .expect("Failed to install default CryptoProvider");

    let args = Args::parse();

    println!("Testing KarmaTiers Contract Interaction");

    println!("Connecting to RPC: {}", args.ws_rpc_url);

    let contract_addr = Address::from_str(&args.contract_address).map_err(|e| {
        KarmaTiersError::SignerConnectionError(format!("Invalid contract address: {e}"))
    })?;

    let url = Url::parse(&args.ws_rpc_url)
        .map_err(|e| KarmaTiersError::SignerConnectionError(format!("Invalid URL: {e}")))?;

    if args.private_key.is_empty() {
        return Err(KarmaTiersError::EmptyPrivateKey);
    }

    // Alloy provider + signer

    let provider_with_signer = {
        let pk_signer = PrivateKeySigner::from_str(args.private_key.as_str()).unwrap();
        let wallet = EthereumWallet::from(pk_signer);

        let ws = WsConnect::new(url.clone().as_str());
        ProviderBuilder::new()
            .wallet(wallet)
            .connect_ws(ws)
            .await
            .map_err(KarmaTiersError::RpcTransportError)?
    };

    // Connect to KarmaTiers contract
    let karma_tiers_contract = KarmaTiersInstance::new(contract_addr, provider_with_signer.clone());

    println!("Successfully connected to KarmaTiers contract for reading at {contract_addr}",);

    println!("\nChecking Current Contract State:");
    println!("=====================================");

    // Test 1: Get current tier count and contract info
    let contract_info = match (
        karma_tiers_contract.getTierCount().call().await,
        karma_tiers_contract.MAX_TIER_NAME_LENGTH().call().await,
    ) {
        (Ok(tier_count), Ok(max_length)) => {
            let is_empty = tier_count == U256::ZERO;
            println!("Current tier count: {tier_count}");
            println!("Maximum tier name length: {max_length}");
            println!("Contract is empty: {is_empty}");

            (tier_count, is_empty)
        }
        _ => {
            eprintln!("Failed to get contract info");
            return Err(KarmaTiersError::SignerConnectionError(
                "Failed to get contract info".to_string(),
            ));
        }
    };

    let mut current_tier_count_u8 = 0;
    if !contract_info.1 {
        println!("\nCurrent tiers in contract:");
        println!("============================");

        current_tier_count_u8 = contract_info.0.as_limbs()[0] as u8;

        // Use the get_tiers function from karma_tiers.rs instead of duplicating code
        let current_tiers =
            KarmaTiersInstance::get_tiers_from_provider(&provider_with_signer, &contract_addr)
                .await?;

        for (i, tier) in current_tiers.iter().enumerate() {
            println!(
                "Tier {}: {} ({} - {}) - {} tx/epoch",
                i, tier.name, tier.min_karma, tier.max_karma, tier.tx_per_epoch
            );
        }
    }

    // Test 3: Create tiers
    let test_tiers = vec![
        KarmaTiers::Tier {
            minKarma: U256::from(0),
            maxKarma: U256::from(10),
            name: "NoTier".to_string(),
            txPerEpoch: 0,
        },
        KarmaTiers::Tier {
            minKarma: U256::from(10),
            maxKarma: U256::from(49),
            name: "Basic".to_string(),
            txPerEpoch: 6,
        },
        KarmaTiers::Tier {
            minKarma: U256::from(50),
            maxKarma: U256::from(99),
            name: "Active".to_string(),
            txPerEpoch: 120,
        },
        KarmaTiers::Tier {
            minKarma: U256::from(100),
            maxKarma: U256::from(499),
            name: "Regular".to_string(),
            txPerEpoch: 720,
        },
        KarmaTiers::Tier {
            minKarma: U256::from(500),
            maxKarma: U256::from(999),
            name: "Power User".to_string(),
            txPerEpoch: 86400,
        },
        KarmaTiers::Tier {
            minKarma: U256::from(1000),
            maxKarma: U256::from(4999),
            name: "S-Tier".to_string(),
            txPerEpoch: 432000,
        },
    ];

    println!(
        "Attempting to update tiers with {} new tiers:",
        test_tiers.len()
    );
    for (i, tier) in test_tiers.iter().enumerate() {
        println!(
            "  Tier {}: {} ({} - {}) - {} tx/epoch",
            i, tier.name, tier.minKarma, tier.maxKarma, tier.txPerEpoch
        );
    }

    if current_tier_count_u8 == 0 {
        match karma_tiers_contract.updateTiers(test_tiers).send().await {
            Ok(pending_tx) => match pending_tx.get_receipt().await {
                Ok(receipt) => {
                    if receipt.status() {
                        println!("Tier update successful!");
                        println!(
                            "   Transaction hash: 0x{}",
                            hex::encode(receipt.transaction_hash)
                        );
                        println!("   Block number: {:?}", receipt.block_number);
                        println!("   Gas used: {}", receipt.gas_used);
                    } else {
                        println!("Tier update transaction failed");
                    }
                }
                Err(e) => eprintln!("Failed to get receipt: {e}"),
            },
            Err(e) => eprintln!("Failed to send tier update transaction: {e}"),
        }
    }

    // Test 4: Verify the update by reading the contract again
    println!("\nVerifying tier update:");
    println!("=======================");

    match karma_tiers_contract.getTierCount().call().await {
        Ok(new_tier_count) => {
            println!("New tier count: {new_tier_count}");

            if new_tier_count > U256::ZERO {
                println!("\nUpdated tiers in contract:");
                println!("============================");

                // Use the get_tiers function from karma_tiers.rs instead of duplicating code
                let updated_tiers = KarmaTiersInstance::get_tiers_from_provider(
                    &provider_with_signer,
                    &contract_addr,
                )
                .await?;

                for (i, tier) in updated_tiers.iter().enumerate() {
                    println!(
                        "Tier {}: {} ({} - {}) - {} tx/epoch",
                        i, tier.name, tier.min_karma, tier.max_karma, tier.tx_per_epoch
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to get new tier count: {e}");
        }
    }

    // Test 5: Test with invalid tier ID
    println!("\nTesting with invalid tier ID:");
    println!("=====================");

    let invalid_tier_id = 255u8;
    match karma_tiers_contract
        .getTierById(invalid_tier_id)
        .call()
        .await
    {
        Ok(_tier) => {
            println!("Unexpectedly found a tier for invalid tier ID: {invalid_tier_id}",);
        }
        Err(e) => {
            println!("Expected error for invalid tier ID {invalid_tier_id}: {e}",);
        }
    }

    println!("\nTesting completed!");
    Ok(())
}
