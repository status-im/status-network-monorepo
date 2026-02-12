use std::net::IpAddr;
// third-party
use anyhow::Context;
use clap::Parser;
use tonic::{
    codegen::tokio_stream::StreamExt,
    IntoRequest
};
use tracing::{debug, error, info, level_filters::LevelFilter};
use tracing_subscriber::{
    EnvFilter,
    layer::SubscriberExt,
    util::SubscriberInitExt
};

// Internal - proto file
pub mod prover_proto {
    // Include generated code (see build.rs)
    tonic::include_proto!("prover");
    // for reflection service
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("prover_descriptor");
}
use crate::prover_proto::{
    rln_aggregator_client::RlnAggregatorClient,
    RlnAggFilter
};

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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {

    setup_tracing().context("Failed to setup tracing in main")?;

    let app_args = AppArgs::parse();
    debug!("{:#?}", app_args);

    let url = format!("http://{}:{}", app_args.ip, app_args.port);

    let mut client_1 = RlnAggregatorClient::connect(url.clone()).await
        .context(format!("Failed to connect to RLN aggregator (url: {})", url))?;
    let filter_1 = RlnAggFilter::default().into_request();
    let mut gp_1 = client_1.get_proofs(filter_1).await?.into_inner();

    loop {
        let p = gp_1.next().await;

        match p {
            Some(Ok(p)) => { info!("Received proof: {:?}", p) },
            Some(Err(_e)) => { todo!() },
            None => { error!("Received nothing, aborting slasher node"); break }
        }
    }

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