mod proof_delivery_service;
#[cfg(test)]
mod proof_delivery_service_tests;
mod proof_reduce_service;

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
// third-party
use anyhow::Context;
use clap::Parser;
use rand::{RngExt, rngs::StdRng};
use tokio::{net::TcpListener, task::JoinSet};
use tonic::{IntoRequest, codegen::tokio_stream::StreamExt, transport::Channel};
use tracing::{debug, error, info, level_filters::LevelFilter};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
// internal
use crate::proof_delivery_service::{ProofDeliveryServer, ProofDeliveryServerConfig};
use crate::proof_reduce_service::ProofReduceService;

pub mod prover_proto {
    // Include generated code (see build.rs)
    tonic::include_proto!("prover");
    // for reflection service
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("prover_descriptor");
}

use prover_proto::{
    RlnProofError, RlnProofFilter, RlnProofReply, rln_proof_reply::Resp,
    rln_prover_client::RlnProverClient,
};

#[derive(Debug, Clone, Parser)]
#[command(about = "RLN aggregator node", long_about = None)]
pub struct AppArgs {
    #[arg(short = 'i', long = "ip", default_value = "::1", help = "Service ip")]
    pub ip: IpAddr,
    #[arg(
        short = 'p',
        long = "port",
        default_value = "50061",
        help = "Service port"
    )]
    pub port: u16,

    #[arg(short = 'u', long = "url", num_args=1.., help = "Proof listening url")]
    pub urls: Vec<String>,

    #[arg(
        help_heading = "mock",
        long = "mock-prover-proof",
        help = "Test only - mock proof received from rln-prover",
        action
    )]
    pub mock_prover_proof: Option<bool>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_tracing().context("Failed to setup tracing in main")?;

    info!("Starting rln-aggregator...");

    let app_args = AppArgs::parse();
    debug!("{:#?}", app_args);

    run_aggregator(app_args).await
}

async fn run_aggregator(app_args: AppArgs) -> anyhow::Result<()> {
    // queue
    // Used to send proof from "proof listening clients" to "proof delivery service"
    let (tx, rx) = tokio::sync::mpsc::channel(128);
    // Used to send reduced proof values
    let (bcast_tx, bcast_rx) = tokio::sync::broadcast::channel(2);

    let mut set = JoinSet::new();

    // proof delivery server

    let addr_ = SocketAddr::new(app_args.ip, app_args.port);
    let listener = TcpListener::bind(addr_).await?;
    info!("Listening on {}", addr_);
    let config = ProofDeliveryServerConfig::default();
    let delivery_server = ProofDeliveryServer::new(config, (bcast_tx.clone(), bcast_rx));

    set.spawn(async move { delivery_server.serve_with(listener).await });

    // proof reduce service (process incoming proofs and lighten them)
    let mut pr_service = ProofReduceService::new(rx, bcast_tx.clone());
    set.spawn(async move { pr_service.serve().await });

    // proof listening clients

    let mock_prover_proof = app_args.mock_prover_proof.unwrap_or(false);

    if mock_prover_proof {
        info!("Using mock prover proof...");
        let mut mock = MockProverProof::new(0, "Mock prover proof".to_string(), tx);
        set.spawn(async move { mock.serve().await });
    } else {
        for (id, url) in app_args.urls.into_iter().enumerate() {
            let tx = tx.clone();

            // rln-prover clients
            set.spawn(async move {
                let mut client = ProverClient::new(id as u64, url, tx).await?;
                client.serve().await
            });
        }
    }

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

struct ProverClient {
    id: u64,
    url: String,
    client: RlnProverClient<Channel>,
    sender: tokio::sync::mpsc::Sender<RlnProofReply>,
}

impl ProverClient {
    async fn new(
        id: u64,
        url: String,
        sender: tokio::sync::mpsc::Sender<RlnProofReply>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            id,
            url: url.clone(),
            client: RlnProverClient::connect(url.clone())
                .await
                .context(format!("Cannot connect to {}", url))?,
            sender,
        })
    }

    #[tracing::instrument(skip(self), err, ret)]
    async fn serve(&mut self) -> anyhow::Result<()> {
        let proof_filter = RlnProofFilter::default();
        let req = proof_filter.into_request();
        let mut s = self.client.get_proofs(req).await?.into_inner();
        loop {
            let n = s.next().await;
            if let Some(r) = n {
                match r {
                    Ok(proof_reply) => {
                        debug!("Received: {:?}", proof_reply);
                        self.sender.send(proof_reply).await.context(format!(
                            "[client {} {}] failed to send proof to channel",
                            self.id, self.url
                        ))?;
                    }
                    Err(e) => {
                        // FIXME: what to do here?
                        error!("[client {} {}] received an error: {}", self.id, self.url, e);
                    }
                }
            } else {
                // Stream has ended
                // TODO: log this
                error!("Stream has ended");
                break;
            }
        }

        Ok(())
    }
}

struct MockProverProof {
    id: u64,
    url: String,
    sender: tokio::sync::mpsc::Sender<RlnProofReply>,
}

impl MockProverProof {
    fn new(id: u64, url: String, sender: tokio::sync::mpsc::Sender<RlnProofReply>) -> Self {
        Self {
            id,
            url: url.clone(),
            sender,
        }
    }
}

impl MockProverProof {
    #[tracing::instrument(skip(self), err, ret)]
    async fn serve(&mut self) -> anyhow::Result<()> {
        let mut _rng: StdRng = rand::make_rng();

        // Simulate time to connect to client
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let mut i = 0u64;
        loop {
            // let proof_reply = RlnProofReply::default();
            let proof_reply = RlnProofReply {
                resp: Some(Resp::Error(RlnProofError {
                    error: format!("index: {}", i),
                })),
            };

            self.sender.send(proof_reply).await.context(format!(
                "[client {} {}] failed to send proof to channel",
                self.id, self.url
            ))?;

            // Simulate network time
            let sleep_time_ = _rng.random_range(25..=75);
            let sleep_time = Duration::from_millis(sleep_time_);
            tokio::time::sleep(sleep_time).await;

            i += 1;
        }
        // Ok(())
    }
}

fn setup_tracing() -> anyhow::Result<()> {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy()
        // TODO: Add a way to disable this for maximum log?
        .add_directive("h2=error".parse()?)
        .add_directive("opentelemetry_sdk=error".parse()?);

    let fmt_layer = tracing_subscriber::fmt::layer();

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(filter)
        // .with(telemetry_layer)
        .init();

    Ok(())
}
