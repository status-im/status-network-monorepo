use std::net::IpAddr;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::str::FromStr;
// third-party
use alloy::primitives::ruint::ParseError;
use alloy::primitives::{Address, U256};
use chrono::{DateTime, Utc};
use clap::Parser;
use clap_config::ClapConfig;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use url::Url;

/// Broadcast channel size
///
/// A Bounded tokio broadcast channel is used to send RLN proof to the Verifier
/// Warning: There should be only one client receiving the proof, but if there are many, a too
///          low value could stall all the proof services
const ARGS_DEFAULT_BROADCAST_CHANNEL_SIZE: &str = "100";
/// Number of proof services (tasks)
///
/// This service is waiting for new tx to generate the RLN proof. Increase this value
/// if you need to process more Transactions in //.
const ARGS_DEFAULT_PROOF_SERVICE_COUNT: &str = "8";
/// Transaction channel size
///
/// Used by grpc service to send the transaction to one of the proof services. A too low value could stall
/// the grpc service when it receives a transaction.
const ARGS_DEFAULT_TRANSACTION_CHANNEL_SIZE: &str = "256";
/// Proof sender channel size
///
/// Used by grpc service to send the generated proof to the Verifier. A too low value could stall
/// the broadcast channel.
const ARGS_DEFAULT_PROOF_SENDER_CHANNEL_SIZE: &str = "100";
/// Disable the grpc reflection service
///
/// By default, the prover offers GRPC reflection (to ease with the development). This could be turned off
/// in production.
const ARGS_DEFAULT_NO_GRPC_REFLECTION: &str = "false";

const ARGS_DEFAULT_RLN_IDENTIFIER_NAME: &str = "test-rln-identifier";
const ARGS_DEFAULT_PROVER_SPAM_LIMIT: u64 = 10_000_u64;
pub const ARGS_DEFAULT_GENESIS: DateTime<Utc> = DateTime::from_timestamp(1431648000, 0).unwrap();
const ARGS_DEFAULT_PROVER_MINIMAL_AMOUNT_FOR_REGISTRATION: WrappedU256 =
    WrappedU256(U256::from_le_slice(10u64.to_le_bytes().as_slice()));

/// Tx gas quota
///
/// Prover will receive a Tx with the field 'estimated_gas_used'.
/// If 'estimated_gas_used' <= 'tx gas quota', tx counter is increased by 1
/// If 'estimated_gas_used' <= 'tx gas quota', tx counter is increased by (estimated_gas_used / tx gas quota)
const ARGS_DEFAULT_TX_GAS_QUOTA: NonZeroU64 = NonZeroU64::new(100_000).unwrap();

#[derive(Debug, Clone, Parser, ClapConfig)]
#[command(about = "RLN prover service", long_about = None)]
pub struct AppArgs {
    #[arg(short = 'i', long = "ip", default_value = "::1", help = "Service ip")]
    pub ip: IpAddr,
    #[arg(
        short = 'p',
        long = "port",
        default_value = "50051",
        help = "Service port"
    )]
    pub port: u16,
    #[arg(
        short = 'u',
        long = "ws-rpc-url",
        help = "Websocket rpc url (e.g. wss://eth-mainnet.g.alchemy.com/v2/your-api-key)"
    )]
    pub ws_rpc_url: Option<Url>,
    #[arg(long = "db", help = "Db path", default_value = "./storage/db")]
    pub db_path: PathBuf,
    #[arg(
        long = "tree",
        help = "Merkle tree folder",
        default_value = "./storage/trees"
    )]
    pub merkle_tree_folder: PathBuf,
    #[arg(long = "tree-count", help = "Merkle tree count", default_value = "1")]
    pub merkle_tree_count: u64,
    #[arg(
        long = "max-tree-count",
        help = "Merkle tree max count",
        default_value = "100"
    )]
    pub merkle_tree_max_count: u64,
    #[arg(
        short = 'k',
        long = "ksc",
        default_value = "0x011b9de308BE357BbF24EfB387a270a14A04E5d2",
        help = "Karma smart contract address",
        help_heading = "smart contract"
    )]
    pub ksc_address: Option<Address>,
    #[arg(
        short = 'r',
        long = "rlnsc",
        default_value = "0xc98994691E96D2f4CA2a718Bc8FDF30bd21d1c59",
        help = "RLN smart contract address",
        help_heading = "smart contract"
    )]
    pub rlnsc_address: Option<Address>,
    #[arg(
        short = 't',
        long = "tsc",
        default_value = "0x011b9de308BE357BbF24EfB387a270a14A04E5d2",
        help = "KarmaTiers smart contract address",
        help_heading = "smart contract"
    )]
    pub tsc_address: Option<Address>,
    #[arg(
        help_heading = "mock",
        long = "mock-sc",
        help = "Test only - mock smart contracts",
        action
    )]
    pub mock_sc: Option<bool>,
    #[arg(
        help_heading = "mock",
        long = "mock-user",
        help = "Test only - register user (requite --mock-sc to be enabled)",
        action
    )]
    pub mock_user: Option<PathBuf>,
    #[arg(
        short = 'c',
        long = "config",
        help = "Config file path",
        default_value = "./config.toml",
        help_heading = "config file"
    )]
    pub config_path: PathBuf,
    #[arg(
        long = "no-config",
        help = "Dont read a config file",
        required = false,
        action,
        help_heading = "config file"
    )]
    pub no_config: bool,
    #[arg(
        long = "metrics-ip",
        default_value = "::1",
        help = "Prometheus Metrics ip",
        help_heading = "monitoring"
    )]
    pub metrics_ip: IpAddr,
    #[arg(
        long = "metrics-port",
        default_value = "30031",
        help = "Metrics port",
        help_heading = "monitoring"
    )]
    pub metrics_port: u16,

    #[arg(
        help_heading = "RLN",
        long = "rln-identifier",
        default_value_t = AppArgs::default_rln_identifier_name(),
        help = "RLN identifier name"
    )]
    pub rln_identifier: String,
    #[arg(
        help_heading = "RLN",
        long = "spam-limit",
        help = "RLN spam limit",
        default_value_t = AppArgs::default_spam_limit(),
    )]
    pub spam_limit: u64,

    #[arg(
        help_heading = "prover config",
        long = "registration-min",
        help = "Minimal amount of Karma to register a user in the prover",
        default_value_t = AppArgs::default_minimal_amount_for_registration(),
    )]
    pub registration_min_amount: WrappedU256,

    #[arg(
        help_heading = "prover config",
        long = "tx-gas-quota",
        help = "Gas quota for a Tx",
        default_value_t = AppArgs::default_tx_gas_quota(),
    )]
    pub tx_gas_quota: NonZeroU64,

    // Hidden option - expect user set it via a config file
    #[arg(
        long = "broadcast-channel-size",
        help = "Broadcast bounded channel size",
        default_value = ARGS_DEFAULT_BROADCAST_CHANNEL_SIZE,
        hide = true,
    )] // see const doc for more info
    pub broadcast_channel_size: usize,
    #[arg(
        long = "proof-service",
        help = "Number of proof service (tasks) to generate proof",
        default_value = ARGS_DEFAULT_PROOF_SERVICE_COUNT,
        hide = true,
    )] // see const doc for more info
    pub proof_service_count: u16,
    #[arg(
        long = "transaction-channel-size",
        help = "Proof bounded channel size",
        default_value = ARGS_DEFAULT_TRANSACTION_CHANNEL_SIZE,
        hide = true,
    )] // see const doc for more info
    pub transaction_channel_size: usize,
    #[arg(
        long = "proof-sender-channel-size",
        help = "Proof bounded sender channel size",
        default_value = ARGS_DEFAULT_PROOF_SENDER_CHANNEL_SIZE,
        hide = true,
    )] // see const doc for more info
    pub proof_sender_channel_size: usize,
    #[arg(
        help_heading = "grpc",
        long = "no-grpc-reflection",
        help = "Disable grpc reflection",
        default_value = ARGS_DEFAULT_NO_GRPC_REFLECTION,
        hide = true,
    )] // see const doc for more info
    pub no_grpc_reflection: bool,
}

impl AppArgs {
    pub fn default_spam_limit() -> u64 {
        ARGS_DEFAULT_PROVER_SPAM_LIMIT
    }

    pub fn default_genesis() -> DateTime<Utc> {
        ARGS_DEFAULT_GENESIS
    }

    pub fn default_minimal_amount_for_registration() -> WrappedU256 {
        ARGS_DEFAULT_PROVER_MINIMAL_AMOUNT_FOR_REGISTRATION
    }

    pub fn default_rln_identifier_name() -> String {
        ARGS_DEFAULT_RLN_IDENTIFIER_NAME.to_string()
    }

    pub fn default_tx_gas_quota() -> NonZeroU64 {
        ARGS_DEFAULT_TX_GAS_QUOTA
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Display)]
pub struct WrappedU256(U256);

impl WrappedU256 {
    pub fn to_u256(&self) -> U256 {
        self.0
    }
}

impl FromStr for WrappedU256 {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(WrappedU256(U256::from_str(s)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_args_config_merge() {
        let config_port = 42942;
        let config = AppArgsConfig {
            ip: None,
            port: Some(config_port),
            mock_sc: Some(true),
            ..Default::default()
        };

        println!("config: {:?}", config);

        {
            let args_1 = vec!["program", "--ip", "127.0.0.1", "--port", "50051"];
            let cmd = <AppArgs as CommandFactory>::command();
            let app_args = cmd.try_get_matches_from(args_1).unwrap(); //  .get_matches();
            let app_args_2 = AppArgs::from_merged(app_args, Some(config.clone()));
            assert_eq!(app_args_2.port, 50051);
        }
        {
            let args_2 = vec!["program", "--ip", "127.0.0.1"];
            let cmd = <AppArgs as CommandFactory>::command();
            let app_args = cmd.try_get_matches_from(args_2).unwrap(); //  .get_matches();
            let app_args_2 = AppArgs::from_merged(app_args, Some(config));
            assert_eq!(app_args_2.port, config_port);
        }
    }
}
