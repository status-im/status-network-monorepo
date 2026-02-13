mod proof_process;
mod slashing;
mod smart_contract;

use std::net::IpAddr;
// third-party
use anyhow::Context;
use clap::Parser;
use tokio::task::JoinSet;
use tonic::{IntoRequest, codegen::tokio_stream::StreamExt};
use tracing::{
    debug,
    error,
    // info,
    level_filters::LevelFilter,
    warn,
};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
// internal
use proof_process::{ProofProcessConfig, ProofProcessService};

// Internal - proto file
pub mod prover_proto {
    // Include generated code (see build.rs)
    tonic::include_proto!("prover");
    // for reflection service
    // pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
    //     tonic::include_file_descriptor_set!("prover_descriptor");
}
use crate::prover_proto::rln_agg_proof_reply::Resp;
use crate::prover_proto::{
    // Address,
    RlnAggFilter,
    // RlnAggProof
    rln_aggregator_client::RlnAggregatorClient,
};
use crate::slashing::SlashingService;

#[derive(Debug, Clone, Parser)]
#[command(about = "RLN slasher node", long_about = None)]
pub struct AppArgs {
    #[arg(short = 'i', long = "ip", default_value = "::1", help = "Service ip")]
    pub ip: IpAddr,
    #[arg(
        short = 'p',
        long = "port",
        default_value = "50067",
        help = "Service port"
    )]
    pub port: u16,
    #[arg(
        long = "spam_limit",
        default_value = "",
        help = "RLN spam limit (or message limit / rate limit in RLN specs)"
    )]
    pub rln_spam_limit: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_tracing().context("Failed to setup tracing in main")?;

    let app_args = AppArgs::parse();
    debug!("{:#?}", app_args);

    // TODO: config
    let (proof_process_tx, proof_process_rx) = tokio::sync::mpsc::channel(128);
    let (slashing_tx, _slashing_rx) = tokio::sync::mpsc::channel(128);
    // let db = Arc::new(RwLock::new(HashMap::new()));

    let mut set = JoinSet::new();

    // Slashing service
    let mut slashing_service = SlashingService::new(_slashing_rx);
    set.spawn(async move { slashing_service.serve().await });

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
                    // info!("Received proof reply: {:?}", pr)

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

    let res = set.join_all().await;
    // Print all errors from services (if any)
    res.iter().for_each(|r| {
        if r.is_err() {
            error!("Error: {:?}", r);
        }
    });

    Ok(())
}

fn setup_tracing() -> anyhow::Result<()> {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    let fmt_layer = tracing_subscriber::fmt::layer();

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(filter)
        .init();

    Ok(())
}
