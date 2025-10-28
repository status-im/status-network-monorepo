// std
use std::path::PathBuf;
// third-party
use clap::CommandFactory;
use rustls::crypto::aws_lc_rs;
use tracing::level_filters::LevelFilter;
use tracing::{
    debug,
    // error,
    // info
};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use anyhow::{Context, Result, anyhow};
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithTonicConfig;
use opentelemetry_sdk::Resource;
// internal
use prover::{AppArgs, AppArgsConfig, metrics::init_metrics, run_prover};

const APP_NAME: &str = "prover-cli";

#[tokio::main]
async fn main() -> Result<()> {
    // install crypto provider for rustls - required for WebSocket TLS connections
    rustls::crypto::CryptoProvider::install_default(aws_lc_rs::default_provider())
        .expect("Failed to install default CryptoProvider");

    // tracing
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy()
        // TODO: Add a way to disable this for maximum log?
        .add_directive("h2=error".parse()?)
        .add_directive("sled::pagecache=error".parse()?)
        .add_directive("opentelemetry_sdk=error".parse()?);

    let fmt_layer = tracing_subscriber::fmt::layer();

    let telemetry_layer = match create_otlp_tracer_provider() {
        Some(tracer_provider) => {
            let tracer = tracer_provider.tracer(APP_NAME);
            Some(tracing_opentelemetry::OpenTelemetryLayer::new(tracer))
        }
        None => None,
    };

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(filter)
        .with(telemetry_layer)
        .init();

    // let app_args = AppArgs::parse();
    let app_args = <AppArgs as CommandFactory>::command().get_matches();
    debug!("Arguments: {:?}", app_args);

    let app_ars_config = if !app_args.get_flag("no_config") {
        // Unwrap safe - default value provided
        let config_path = app_args.get_one::<PathBuf>("config_path").unwrap();
        debug!("Reading config path: {:?}...", config_path);
        let config_str = std::fs::read_to_string(config_path)
            .context(format!("Failed to read config file: {:?}", config_path))?;
        let config: AppArgsConfig = toml::from_str(config_str.as_str())?;
        debug!("Config: {:?}", config);
        config
    } else {
        AppArgsConfig::default()
    };

    // Merge command line args & config
    let app_args = AppArgs::from_merged(app_args, Some(app_ars_config));
    debug!("Arguments (merged with config): {:?}", app_args);

    // Application cli arguments checks
    if app_args.ws_rpc_url.is_some() {
        if app_args.ksc_address.is_none()
            || app_args.ksc_address.is_none()
            || app_args.tsc_address.is_none()
        {
            return Err(anyhow!("Please provide smart contract addresses"));
        }
    } else if app_args.mock_sc.is_none() {
        return Err(anyhow!(
            "Please provide rpc url (--ws-rpc-url) or mock (--mock-sc)"
        ));
    }

    init_metrics(app_args.metrics_ip, &app_args.metrics_port);

    run_prover(app_args).await.map_err(anyhow::Error::new)
}

fn create_otlp_tracer_provider() -> Option<opentelemetry_sdk::trace::SdkTracerProvider> {
    if !std::env::vars().any(|(name, _)| name.starts_with("OTEL_")) {
        return None;
    }
    let protocol = std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL").unwrap_or("grpc".to_string());

    let exporter = match protocol.as_str() {
        "grpc" => {
            // Note - Performance:
            // https://docs.rs/opentelemetry-otlp/latest/opentelemetry_otlp/index.html#performance
            let mut exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                //.with_endpoint(...)
                ;

            // Check if we need TLS
            if let Ok(endpoint) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                && endpoint.starts_with("https")
            {
                exporter = exporter.with_tls_config(
                    opentelemetry_otlp::tonic_types::transport::ClientTlsConfig::default()
                        .with_enabled_roots(),
                );
            }
            exporter.build().expect("Failed to create tonic exporter")
        }
        "http/protobuf" => opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .build()
            .expect("Failed to create http/protobuf exporter"),
        p => panic!("Unsupported protocol {p}"),
    };

    let resource = Resource::builder().with_service_name(APP_NAME).build();

    Some(
        opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_resource(resource)
            .with_batch_exporter(exporter)
            .build(),
    )
}
