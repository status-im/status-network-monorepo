mod common;
mod proof_process;
mod slashing;
mod smart_contract;

use std::net::IpAddr;
use std::str::FromStr;
// third-party
use alloy::{
    network::EthereumWallet,
    primitives::{Address, U256, address},
    providers::{ProviderBuilder, WsConnect},
    signers::local::PrivateKeySigner,
};
use anyhow::{Context, anyhow};
use clap::Parser;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinSet;
use tonic::{IntoRequest, codegen::tokio_stream::StreamExt};
use tracing::{debug, error, info, level_filters::LevelFilter, warn};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;
use zeroize::Zeroizing;
// internal
use crate::common::SlashingData;
use crate::slashing::{SlashingService, SlashingServiceConfig};
use proof_process::{ProofProcessConfig, ProofProcessService};
use smart_contract::deploy_sc_for_slashing;

// Internal - proto file
pub mod prover_proto {
    // Include generated code (see build.rs)
    tonic::include_proto!("prover");
}
use crate::prover_proto::rln_agg_proof_reply::Resp;
use crate::prover_proto::{RlnAggFilter, rln_aggregator_client::RlnAggregatorClient};
use crate::smart_contract::RLN;

#[derive(Debug, Clone, Parser)]
#[command(about = "RLN slasher node", long_about = None)]
pub struct AppArgs {
    #[arg(
        short = 'i',
        long = "ip",
        default_value = "::1",
        help = "RLN aggregator ip address"
    )]
    pub ip: IpAddr,
    #[arg(
        short = 'p',
        long = "port",
        default_value = "50061",
        help = "RLN aggregator port"
    )]
    pub port: u16,
    #[arg(
        short = 'u',
        long = "ws-rpc-url",
        help = "Websocket rpc url (e.g. wss://eth-mainnet.g.alchemy.com/v2/your-api-key)"
    )]
    pub rpc_url_ws: Option<Url>,
    #[arg(
        long = "spam_limit",
        help = "RLN spam limit (or message limit / rate limit in RLN specs)",
        help_heading = "rln"
    )]
    pub rln_spam_limit: u64,
    #[arg(
        long = "account_to_reward",
        help = "Account that will receive the reward after a successful slashing",
        help_heading = "slash"
    )]
    pub account_to_reward: Option<Address>,
    #[arg(
        long = "slash_limit",
        default_value = "5",
        help = "Maximum number of concurrent slashing task",
        help_heading = "slash"
    )]
    pub slashing_limit: u64,
    #[arg(
        short = 'r',
        long = "rln_sc",
        help = "RLN smart contract address",
        help_heading = "smart contract"
    )]
    pub rln_sc_address: Option<Address>,

    #[arg(
        help_heading = "mock",
        long = "mock-sc",
        help = "Test only - mock rln sc (using Anvil)",
        action
    )]
    pub mock_smart_contract: Option<bool>,
    #[arg(
        help_heading = "mock",
        long = "mock-register",
        help = "Test only - register user in RLN smart contract (using Anvil)"
    )]
    pub mock_register: Option<Vec<MockRegisterArg>>,
}

#[derive(Debug, Clone)]
pub struct MockRegisterArg {
    pub address: String,
    pub value: String,
}

impl FromStr for MockRegisterArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 2 {
            return Err("Must be in format ADDRESS,VALUE".to_string());
        }
        Ok(MockRegisterArg {
            address: parts[0].to_string(),
            value: parts[1].to_string(),
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_tracing().context("Failed to setup tracing in main")?;

    let app_args = AppArgs::parse();
    debug!("{:#?}", app_args);

    // TODO: config
    let (proof_process_tx, proof_process_rx) = tokio::sync::mpsc::channel(128);
    let (slashing_tx, slashing_rx) = tokio::sync::mpsc::channel(128);

    let mut set = JoinSet::new();

    start_slashing_service(app_args.clone(), slashing_rx, &mut set).await?;

    // Proof process service
    let cfg = ProofProcessConfig {
        rln_limit: app_args.rln_spam_limit,
    };
    let mut proof_process_service = ProofProcessService::new(cfg, proof_process_rx, slashing_tx);

    set.spawn(async move { proof_process_service.serve().await });

    // Aggregator client
    let url = format!("http://{}:{}", app_args.ip, app_args.port);
    let mut client_1 = RlnAggregatorClient::connect(url.clone())
        .await
        .context(format!(
            "Failed to connect to RLN aggregator (url: {})",
            url
        ))?;
    let filter_1 = RlnAggFilter::default().into_request();
    let mut gp_1 = client_1.get_proofs(filter_1).await?.into_inner();

    set.spawn(async move {
        loop {
            let pr_ = gp_1.next().await;

            match pr_ {
                Some(Ok(pr)) => {
                    debug!("Received proof reply: {:?}", pr);

                    match pr.resp {
                        Some(Resp::Proof(p)) => {
                            proof_process_tx
                                .send(p)
                                .await
                                .context("Failed to send proof in proof_process channel")?;
                        }
                        Some(Resp::Error(pe)) => {
                            warn!("Received an proof reply with an error: {:?}", pe);
                        }
                        None => {
                            warn!(
                                "Received an unexpected empty RlnAggProofReply response: {:?}",
                                pr
                            );
                        }
                    }
                }
                Some(Err(e)) => {
                    error!("Error has been received: {}, aborting slasher node...", e)
                }
                None => {
                    // Stream is finished - aggregator is down?
                    error!("Received nothing, aborting slasher node...");
                    break;
                }
            }
        }

        Ok(())
    });

    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                error!("Task error: {:#}", e);
                break;
            }
            Err(e) => {
                error!("Join error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn start_slashing_service(
    app_args: AppArgs,
    slashing_rx: Receiver<SlashingData>,
    set: &mut JoinSet<anyhow::Result<()>>,
) -> anyhow::Result<()> {
    match app_args.mock_smart_contract {
        Some(true) => {
            let provider = ProviderBuilder::new().connect_anvil_with_wallet();
            // Need to deploy the SC in Anvil
            let (_, _, rln_sc) = deploy_sc_for_slashing(&provider, None).await;

            // FIXME: should be returned by deploy_sc_for_slashing
            let account_to_reward = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

            if let Some(mock_register) = app_args.mock_register {
                for mock_register_arg in mock_register.iter() {
                    let address = Address::from_str(mock_register_arg.address.as_str())
                        .expect("Invalid address");
                    // let id_commitment = Fr::from_str(mock_register_arg.value.as_str()).expect("Invalid address");
                    let id_commitment =
                        U256::from_str(mock_register_arg.value.as_str()).expect("Invalid address");
                    info!(
                        "Registering user: {}, with id_commitment: {}",
                        address, id_commitment
                    );
                    let call_1 = rln_sc.register(id_commitment, address);
                    let _ = call_1.send().await.unwrap().watch().await.unwrap();
                }
            }

            let slashing_service_config = SlashingServiceConfig {
                account_to_reward,
                slashing_limit: app_args.slashing_limit,
            };
            let mut slashing_service = SlashingService::new(slashing_rx, slashing_service_config);
            set.spawn(async move { slashing_service.serve(rln_sc).await });
        }
        _ => {
            let pk: Zeroizing<String> =
                Zeroizing::new(std::env::var("PRIVATE_KEY").expect("Please provide a private key"));
            let pk_signer = PrivateKeySigner::from_str(pk.as_str())?;
            let wallet = EthereumWallet::from(pk_signer);

            let ws = WsConnect::new(
                app_args
                    .rpc_url_ws
                    .expect("Please provide a rpc ws endpoint")
                    .as_str(),
            );
            let p = ProviderBuilder::new()
                .wallet(wallet)
                .connect_ws(ws)
                .await
                .map_err(|e| anyhow!(e))?;

            let rln_sc = RLN::new(
                app_args
                    .rln_sc_address
                    .expect("Please provide RLN smart contract address"),
                p,
            );

            let account_to_reward = app_args
                .account_to_reward
                .expect("Please provide an account to reward");

            let slashing_service_config = SlashingServiceConfig {
                account_to_reward,
                slashing_limit: app_args.slashing_limit,
            };
            let mut slashing_service = SlashingService::new(slashing_rx, slashing_service_config);
            set.spawn(async move { slashing_service.serve(rln_sc).await });
        }
    }

    Ok(())
}

fn setup_tracing() -> anyhow::Result<()> {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy()
        .add_directive("h2=error".parse()?)
        .add_directive("opentelemetry_sdk=error".parse()?);

    let fmt_layer = tracing_subscriber::fmt::layer();

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(filter)
        .init();

    Ok(())
}
