mod proof_delivery_service;
#[cfg(test)]
mod proof_delivery_service_tests;

use std::net::{IpAddr, SocketAddr};
// third-party
use anyhow::Context;
use clap::Parser;
use tokio::task::JoinSet;
use tonic::{IntoRequest, codegen::tokio_stream::StreamExt, transport::Channel};

pub mod prover_proto {
    // Include generated code (see build.rs)
    tonic::include_proto!("prover");
}
use crate::proof_delivery_service::{ProofDeliveryServer, ProofDeliveryServerConfig};
use crate::prover_proto::RlnProofReply;
use prover_proto::{RlnProofFilter, rln_prover_client::RlnProverClient};

#[derive(Debug, Clone, Parser)]
#[command(about = "RLN prover client", long_about = None)]
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

    #[arg(short = 'u', long = "url", num_args=1.., help = "Proof listening url")]
    pub urls: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app_args = AppArgs::parse();
    println!("{:#?}", app_args);

    /*
    let url_0 = app_args.urls[0].clone();
    println!("Trying to connect to: {}", url_0);
    let mut client = RlnProverClient::connect(url_0).await.unwrap();
    println!("client: {:?}", client);

    let proof_filter = RlnProofFilter::default();
    let req = proof_filter.into_request();
    let mut s = client.get_proofs(req).await.unwrap().into_inner();
    while let Some(proof) = s.next().await {
        println!("proof: {:?}", proof);
    }
    */

    run_aggregator(app_args).await
}

async fn run_aggregator(app_args: AppArgs) -> anyhow::Result<()> {
    // queue
    // Used to send proof from "proof listening clients" to "proof delivery service"
    let (tx, rx) = tokio::sync::broadcast::channel(10);

    let mut set = JoinSet::new();

    // proof delivery server

    let addr = SocketAddr::new(app_args.ip, app_args.port);
    let config = ProofDeliveryServerConfig::default();
    let delivery_server = ProofDeliveryServer::new(config, addr, (tx.clone(), rx));

    set.spawn(async move { delivery_server.serve().await });

    // proof listening clients

    for (id, url) in app_args.urls.into_iter().enumerate() {
        // TODO
        /*
        let mut client = RlnProverClient::connect(url).await.unwrap();
        let proof_filter = RlnProofFilter::default();
        let req = proof_filter.into_request();
        let mut s = client.get_proofs(req).await.unwrap().into_inner();
        */

        let tx = tx.clone();

        // rln-prover clients
        set.spawn(async move {
            /*
            let mut client = RlnProverClient::connect(url).await?;
            let proof_filter = RlnProofFilter::default();
            let req = proof_filter.into_request();
            let mut s = client.get_proofs(req).await?.into_inner();
            */

            let mut client = ProverClient::new(id as u64, url, tx).await?;
            client.serve().await

            // Ok::<(), anyhow::Error>(())
        });
    }

    let res = set.join_all().await;
    // Print all errors from services (if any)
    // We expect that the Aggregator should never stop unexpectedly, but printing error can help to debug
    res.iter().for_each(|r| {
        if r.is_err() {
            println!("Error: {:?}", r);
        }
    });
    Ok(())
}

struct ProverClient {
    id: u64,
    url: String,
    client: RlnProverClient<Channel>,
    sender: tokio::sync::broadcast::Sender<RlnProofReply>,
}

impl ProverClient {
    async fn new(
        id: u64,
        url: String,
        sender: tokio::sync::broadcast::Sender<RlnProofReply>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            id,
            url: url.clone(),
            client: RlnProverClient::connect(url).await?,
            sender,
        })
    }

    async fn serve(&mut self) -> anyhow::Result<()> {
        let proof_filter = RlnProofFilter::default();
        let req = proof_filter.into_request();
        let mut s = self.client.get_proofs(req).await?.into_inner();
        loop {
            let n = s.next().await;
            if let Some(r) = n {
                match r {
                    Ok(proof_reply) => {
                        self.sender.send(proof_reply).context(format!(
                            "[client {} {}] failed to send proof to channel",
                            self.id, self.url
                        ))?;
                    }
                    Err(e) => {
                        // FIXME: what to do here?
                        println!("[client {} {}] received an error: {}", self.id, self.url, e);
                    }
                }
            } else {
                // Stream has ended
                // TODO: log this
                break;
            }
        }

        Ok(())
    }
}
