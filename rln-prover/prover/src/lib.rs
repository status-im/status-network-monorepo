mod args;
mod epoch_service;
mod error;
mod grpc_service;
mod karma_sc_listener;
pub mod metrics;
mod mock;
mod proof_generation;
mod proof_service;
mod rocksdb_operands;
mod tier;
mod tiers_listener;
mod user_db;
mod user_db_error;
mod user_db_serialization;
mod user_db_service;
mod user_db_types;

// tests
mod epoch_service_tests;
mod proof_service_tests;
mod user_db_tests;

// std
use alloy::network::EthereumWallet;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;
// third-party
use alloy::providers::{ProviderBuilder, WsConnect};
use alloy::signers::local::PrivateKeySigner;
use tokio::task::JoinSet;
use tracing::{debug, info};
use zeroize::Zeroizing;
// internal
pub use crate::args::{ARGS_DEFAULT_GENESIS, AppArgs, AppArgsConfig};
use crate::epoch_service::EpochService;
use crate::error::AppError;
use crate::grpc_service::GrpcProverService;
use crate::karma_sc_listener::KarmaScEventListener;
pub use crate::mock::MockUser;
use crate::mock::read_mock_user;
use crate::proof_service::ProofService;
use crate::tier::TierLimits;
use crate::tiers_listener::TiersListener;
use crate::user_db::{MERKLE_TREE_HEIGHT, UserDbConfig};
use crate::user_db_error::RegisterError;
use crate::user_db_service::UserDbService;
use crate::user_db_types::RateLimit;
use rln_proof::RlnIdentifier;
use smart_contract::KarmaTiers::KarmaTiersInstance;
use smart_contract::{KarmaTiersError, TIER_LIMITS};

pub async fn run_prover(app_args: AppArgs) -> Result<(), AppError> {
    // Epoch
    let epoch_service = EpochService::try_from((Duration::from_secs(60 * 2), ARGS_DEFAULT_GENESIS))
        .expect("Failed to create epoch service");

    // Alloy provider (Smart contract provider)
    let provider = if app_args.ws_rpc_url.is_some() {
        let ws = WsConnect::new(app_args.ws_rpc_url.clone().unwrap().as_str());
        let provider = ProviderBuilder::new().connect_ws(ws).await?;
        Some(provider)
    } else {
        None
    };

    // Alloy provider + signer
    let provider_with_signer = if app_args.ws_rpc_url.is_some() {
        let pk: Zeroizing<String> =
            Zeroizing::new(std::env::var("PRIVATE_KEY").expect("Please provide a private key"));
        let pk_signer = PrivateKeySigner::from_str(pk.as_str())?;
        let wallet = EthereumWallet::from(pk_signer);

        let ws = WsConnect::new(app_args.ws_rpc_url.clone().unwrap().as_str());
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect_ws(ws)
            .await
            .map_err(KarmaTiersError::RpcTransportError)?;
        Some(provider)
    } else {
        None
    };

    //

    let tier_limits = if app_args.ws_rpc_url.is_some() {
        TierLimits::from(
            KarmaTiersInstance::get_tiers_from_provider(
                &provider.clone().unwrap(),
                &app_args.tsc_address.unwrap(),
            )
            .await?,
        )
    } else {
        // mock
        let tl = TierLimits::from(TIER_LIMITS.clone());
        debug!("Mock - will use tier limits: {:#?}", tl);
        tl
    };

    tier_limits.validate()?;

    // User db service
    let user_db_config = UserDbConfig {
        db_path: app_args.db_path.clone(),
        merkle_tree_folder: app_args.merkle_tree_folder.clone(),
        tree_count: app_args.merkle_tree_count,
        max_tree_count: app_args.merkle_tree_max_count,
        tree_depth: MERKLE_TREE_HEIGHT,
    };
    let user_db_service = UserDbService::new(
        user_db_config,
        epoch_service.epoch_changes.clone(),
        epoch_service.current_epoch.clone(),
        RateLimit::new(app_args.spam_limit),
        tier_limits,
    )?;

    if app_args.mock_sc.is_some()
        && let Some(user_filepath) = app_args.mock_user.as_ref()
    {
        let mock_users = read_mock_user(user_filepath);
        let mock_users = mock_users.unwrap();
        debug!("Mock - will register {} users", mock_users.len());
        for mock_user in mock_users {
            debug!(
                "Registering user address: {} - tx count: {}",
                mock_user.address, mock_user.tx_count
            );

            let user_db = user_db_service.get_user_db();
            if let Err(e) = user_db.on_new_user(&mock_user.address) {
                match e {
                    RegisterError::AlreadyRegistered(_) => {
                        debug!("User {} already registered", mock_user.address);
                    }
                    _ => {
                        return Err(AppError::from(e));
                    }
                }
            }
            user_db.on_new_tx(&mock_user.address, Some(mock_user.tx_count))?;
        }
    }

    // Smart contract
    let registry_listener = if app_args.mock_sc.is_some() {
        // No registry listener when mock is enabled
        None
    } else {
        Some(KarmaScEventListener::new(
            app_args.ksc_address.unwrap(),
            app_args.rlnsc_address.unwrap(),
            user_db_service.get_user_db(),
            app_args.registration_min_amount.to_u256(),
        ))
    };

    let tiers_listener = if app_args.mock_sc.is_some() {
        None
    } else {
        Some(TiersListener::new(
            app_args.tsc_address.unwrap(),
            user_db_service.get_user_db(),
        ))
    };

    // proof service
    let (tx, rx) = tokio::sync::broadcast::channel(app_args.broadcast_channel_size);
    let (proof_sender, proof_receiver) = async_channel::bounded(app_args.transaction_channel_size);

    // grpc

    let rln_identifier = RlnIdentifier::new(app_args.rln_identifier.as_bytes());
    let addr = SocketAddr::new(app_args.ip, app_args.port);
    info!("Listening on: {}", addr);
    let prover_grpc_service = {
        let mut service = GrpcProverService {
            proof_sender,
            broadcast_channel: (tx.clone(), rx),
            addr,
            rln_identifier,
            user_db: user_db_service.get_user_db(),
            karma_sc_info: None,
            provider: provider.clone(),
            proof_sender_channel_size: app_args.proof_sender_channel_size,
            grpc_reflection: !app_args.no_grpc_reflection,
            tx_gas_quota: app_args.tx_gas_quota,
            rate_limit: RateLimit::from(app_args.spam_limit),
        };

        if app_args.ws_rpc_url.is_some() {
            let ws_rpc_url = app_args.ws_rpc_url.clone().unwrap();
            service.karma_sc_info = Some((ws_rpc_url.clone(), app_args.ksc_address.unwrap()));
        }
        service
    };

    let mut set = JoinSet::new();
    for i in 0..app_args.proof_service_count {
        let proof_recv = proof_receiver.clone();
        let broadcast_sender = tx.clone();
        let current_epoch = epoch_service.current_epoch.clone();
        let user_db = user_db_service.get_user_db();

        set.spawn(async move {
            let proof_service = ProofService::new(
                proof_recv,
                broadcast_sender,
                current_epoch,
                user_db,
                RateLimit::new(app_args.spam_limit),
                u64::from(i),
            );
            proof_service.serve().await
        });
    }

    if let Some(registry_listener) = registry_listener {
        let p = provider.clone().unwrap();
        set.spawn(async move {
            registry_listener
                .listen(p, provider_with_signer.unwrap())
                .await
        });
    }
    if let Some(tiers_listener) = tiers_listener {
        let p = provider.clone().unwrap();
        set.spawn(async move { tiers_listener.listen(p).await });
    }
    set.spawn(async move { epoch_service.listen_for_new_epoch().await });
    set.spawn(async move { user_db_service.listen_for_epoch_changes().await });
    if app_args.ws_rpc_url.is_some() {
        set.spawn(async move { prover_grpc_service.serve().await });
    } else {
        info!("Grpc service started with mocked smart contracts");
        set.spawn(async move { prover_grpc_service.serve_with_mock().await });
    }

    let res = set.join_all().await;
    // Print all errors from services (if any)
    // We expect that the Prover should never stop unexpectedly, but printing error can help to debug
    res.iter().for_each(|r| {
        if r.is_err() {
            info!("Error: {:?}", r);
        }
    });
    Ok(())
}
