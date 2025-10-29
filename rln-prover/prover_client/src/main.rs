use alloy::primitives::{Address, U256};
use clap::{Args, Parser, Subcommand};
use sha2::Digest;
use std::net::IpAddr;
use tonic::Response;

pub mod prover_proto {
    // Include generated code (see build.rs)
    tonic::include_proto!("prover");
}

use crate::prover_proto::{GetUserTierInfoReply, GetUserTierInfoRequest};
use prover_proto::{
    Address as GrpcAddress, SendTransactionReply, SendTransactionRequest, U256 as GrpcU256,
    Wei as GrpcWei, rln_prover_client::RlnProverClient,
};

#[derive(Debug, Clone, Parser)]
#[command(about = "RLN prover client", long_about = None)]
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
        short = 'a',
        long = "address",
        default_value = "0xb20a608c624Ca5003905aA834De7156C68b2E1d0",
        help = "User address"
    )]
    pub address: Address,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub(crate) enum Commands {
    #[command(about = "Send a transaction")]
    SendTransaction(SendTransactionArgs),
    #[command(about = "Get user tier info")]
    GetUserTierInfo,
}

#[derive(Debug, Clone, PartialEq, Args)]
pub struct SendTransactionArgs {
    #[arg(short = 'c', long = "chain-id", help = "Chain id", default_value = "1")]
    chain_id: U256,
    #[arg(short = 'w', long = "wei", help = "Tx fee", default_value = "1000")]
    wei: U256,
    #[arg(short = 't', long = "tx-hash", help = "Tx hash", default_value = "foo")]
    tx_hash: String,
    #[arg(
        long = "invalid-tx-hash",
        default_missing_value = "false",
        help = "Send an invalid tx hash"
    )]
    invalid_hash: bool,
    #[arg(
        short = 'g',
        long = "tx-gas",
        help = "Tx estimated gas used",
        default_value = "21000"
    )]
    estimated_gas_used: u64,
}

#[tokio::main]
async fn main() {
    let app_args = AppArgs::parse();

    let url = format!("http://{}:{}", app_args.ip, app_args.port);
    println!("url: {url}");
    let mut client = RlnProverClient::connect(url).await.unwrap();
    let grpc_addr = GrpcAddress {
        value: app_args.address.to_vec(),
    };

    match app_args.command {
        Commands::SendTransaction(send_transaction_args) => {
            let chain_id = GrpcU256 {
                // FIXME: LE or BE?
                value: send_transaction_args.chain_id.to_le_bytes::<32>().to_vec(),
            };
            let wei = GrpcWei {
                // FIXME: LE or BE?
                value: send_transaction_args.wei.to_le_bytes::<32>().to_vec(),
            };

            let tx_hash = if send_transaction_args.invalid_hash {
                vec![]
            } else {
                // U256::from(42).to_le_bytes::<32>().to_vec()
                sha2::Sha256::digest(send_transaction_args.tx_hash).to_vec()
            };

            let request_0 = SendTransactionRequest {
                gas_price: Some(wei),
                sender: Some(grpc_addr),
                chain_id: Some(chain_id),
                transaction_hash: tx_hash,
                estimated_gas_used: send_transaction_args.estimated_gas_used,
            };
            let request = tonic::Request::new(request_0);
            let response: Response<SendTransactionReply> =
                client.send_transaction(request).await.unwrap();

            println!("SendTransactionReply status: {:?}", response.into_inner());
        }
        Commands::GetUserTierInfo => {
            let request_0 = GetUserTierInfoRequest {
                user: Some(grpc_addr),
            };
            let request = tonic::Request::new(request_0);
            let response: Response<GetUserTierInfoReply> =
                client.get_user_tier_info(request).await.unwrap();

            println!("GetUserTierInfoReply: {:?}", response.into_inner());
        }
    }
}
