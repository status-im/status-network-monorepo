mod args;
mod epoch_service;
mod epoch_service_2;
mod error;
mod grpc_service;
mod karma_sc_listener;
mod kill_switch;
pub mod metrics;
mod mock;
mod nonce_manager;
mod proof_generation;
mod proof_service;
// mod rocksdb_operands;
mod tier;
mod tiers_listener;
// mod user_db;
mod user_db_error;
// mod user_db_serialization;
mod user_db_service;
mod user_db_types;

// tests
mod epoch_service_tests;
mod grpc_e2e;
mod nonce_manager_tests;
mod proof_service_tests;
#[cfg(test)]
pub mod tests_common;
mod user_db_2;
mod user_db_2_entities;
mod user_db_2_tests;
// mod user_db_tests;

// std
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
// third-party
use alloy::{
    network::EthereumWallet,
    providers::{ProviderBuilder, WsConnect},
    signers::local::PrivateKeySigner,
};
// use prover_db_migration::{Migrator, MigratorTrait};
// use sea_orm::Database;
use prover_db_migration_sqlx::{MigrationConfig, Migrator};
use sqlx::{
    Decode, Encode, Pool, Type,
    postgres::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueRef, Postgres, types::Oid},
};
use tokio::task::JoinSet;
use tracing::{debug, info, warn};
use zeroize::Zeroizing;
// internal
pub use crate::args::{ARGS_DEFAULT_GENESIS, AppArgs, AppArgsConfig};
use crate::epoch_service_2::{EpochService2, EpochService2Config};
use crate::error::AppError2;
use crate::grpc_service::GrpcProverService;
use crate::karma_sc_listener::KarmaScEventListener;
use crate::kill_switch::GasKillSwitch;
pub use crate::mock::MockUser;
use crate::mock::read_mock_user;
use crate::nonce_manager::{
    ManagedRLNRegister, NonceManager, NonceManagerConfig, monitor_stuck_transactions,
    process_registrations,
};
use crate::proof_service::ProofService;
use crate::tier::TierLimits;
use crate::tiers_listener::TiersListener;
pub use crate::user_db_2::{MERKLE_TREE_HEIGHT, UserDb2Config};
use crate::user_db_error::{RegisterError2, UserDb2OpenError};
use crate::user_db_service::UserDbService;
use crate::user_db_types::RateLimit;
use rln_proof::RlnIdentifier;
use smart_contract::{KarmaTiers::KarmaTiersInstance, KarmaTiersError, RLN, TIER_LIMITS};

pub async fn run_prover(app_args: AppArgs) -> Result<(), AppError2> {
    let epoch_duration = Duration::from_secs(app_args.epoch_duration_secs);
    info!(
        "Starting epoch service: epoch_duration={}s",
        app_args.epoch_duration_secs
    );
    let epoch_config = EpochService2Config::new(epoch_duration, ARGS_DEFAULT_GENESIS);

    let epoch_service =
        EpochService2::try_from(epoch_config).expect("Failed to create epoch service");

    // Alloy provider (Smart contract provider)
    let provider = if app_args.ws_rpc_url.is_some() {
        let ws = WsConnect::new(app_args.ws_rpc_url.clone().unwrap().as_str());
        let provider = ProviderBuilder::new().connect_ws(ws).await?;
        Some(provider)
    } else {
        None
    };

    // Alloy provider + signer
    let (provider_with_signer, wallet_address) = if app_args.ws_rpc_url.is_some() {
        let pk: Zeroizing<String> =
            Zeroizing::new(std::env::var("PRIVATE_KEY").expect("Please provide a private key"));
        let pk_signer = PrivateKeySigner::from_str(pk.as_str())?;
        let wallet_addr = pk_signer.address();
        let wallet = EthereumWallet::from(pk_signer);

        let ws = WsConnect::new(app_args.ws_rpc_url.clone().unwrap().as_str());
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect_ws(ws)
            .await
            .map_err(KarmaTiersError::RpcTransportError)?;
        (Some(provider), Some(wallet_addr))
    } else {
        (None, None)
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
    let user_db_config = UserDb2Config {
        tree_count: app_args.merkle_tree_count,
        max_tree_count: app_args.merkle_tree_max_count,
        tree_depth: MERKLE_TREE_HEIGHT,
    };
    let db_url = app_args.db_url.ok_or_else(|| {
        AppError2::InvalidArgs(
            "--db <database_url> is required. Example: --db postgres://user:pass@host:5432/db"
                .to_string(),
        )
    })?;
    /*
    let db_conn = Database::connect(&db_url)
        .await
        .map_err(UserDb2OpenError::from)?;
    */
    let db_conn = sqlx::PgPool::connect(db_url.as_str())
        .await
        .map_err(UserDb2OpenError::from)?;

    // Run database migrations
    info!("Running database migrations...");
    let migrator = Migrator();
    let migration_config = MigrationConfig {
        tree_count: user_db_config.tree_count as i64,
        max_tree_count: user_db_config.max_tree_count as i64,
        tree_depth: user_db_config.tree_depth as i16,
    };
    migrator
        .up(db_conn.clone(), migration_config)
        .await
        .map_err(UserDb2OpenError::from)?;

    // Migrator::up(&db_conn, None)
    //     .await
    //     .map_err(|e| AppError2::MigrationError(e.to_string()))?;
    info!("Database migrations complete");

    let user_db_service = UserDbService::new(
        db_conn.clone(),
        user_db_config,
        epoch_service.epoch_changes.clone(),
        epoch_service.current_epoch.clone(),
        RateLimit::new(app_args.spam_limit),
        tier_limits,
    )
    .await?;

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
            match user_db.on_new_user(&mock_user.address).await {
                Ok(id_co) => debug!(
                    "id_commitment: {} for address: {}",
                    id_co, mock_user.address
                ),
                Err(e) => match e {
                    RegisterError2::AlreadyRegistered(_) => {
                        debug!("User {} already registered", mock_user.address);
                    }
                    _ => {
                        return Err(AppError2::from(e));
                    }
                },
            }
            user_db
                .on_new_tx(&mock_user.address, Some(mock_user.tx_count))
                .await?;
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

    // Gas kill switch
    let gas_kill_switch = GasKillSwitch::new();
    if !app_args.kill_switch_file.is_empty() {
        gas_kill_switch.start_watcher(
            std::path::PathBuf::from(&app_args.kill_switch_file),
            Duration::from_secs(app_args.kill_switch_poll_secs),
        );
    }

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
            gas_kill_switch: gas_kill_switch.clone(),
        };

        if app_args.ws_rpc_url.is_some() {
            let ws_rpc_url = app_args.ws_rpc_url.clone().unwrap();
            service.karma_sc_info = Some((ws_rpc_url.clone(), app_args.ksc_address.unwrap()));
        }
        service
    };

    let mut set = JoinSet::new();
    info!(
        "Spawning {} ProofService tasks...",
        app_args.proof_service_count
    );
    for i in 0..app_args.proof_service_count {
        let proof_recv = proof_receiver.clone();
        let broadcast_sender = tx.clone();
        let current_epoch = epoch_service.current_epoch.clone();
        let user_db = user_db_service.get_user_db();

        info!("Spawning ProofService task {}", i);
        set.spawn(async move {
            info!(
                "[ProofService {}] Task started, creating ProofService...",
                i
            );
            let proof_service = ProofService::new(
                proof_recv,
                broadcast_sender,
                current_epoch,
                user_db,
                RateLimit::new(app_args.spam_limit),
                u64::from(i),
            );
            info!("[ProofService {}] Calling serve()...", i);
            proof_service.serve().await
        });
    }
    info!(
        "All {} ProofService tasks spawned",
        app_args.proof_service_count
    );

    if let Some(registry_listener) = registry_listener {
        let p = provider.clone().unwrap();
        let pws = provider_with_signer.unwrap();

        // Set up nonce manager and managed RLN register
        let rln_sc_instance = RLN::new(app_args.rlnsc_address.unwrap(), pws.clone());

        let nonce_config = {
            let mut cfg = NonceManagerConfig::default();
            if app_args.registration_gas_price_gwei > 0 {
                cfg.min_gas_price = app_args.registration_gas_price_gwei as u128 * 1_000_000_000;
                info!(
                    "Registration gas price set to {} gwei ({} wei)",
                    app_args.registration_gas_price_gwei, cfg.min_gas_price
                );
            }
            cfg
        };

        let nonce_manager = Arc::new(
            NonceManager::new(db_conn.clone(), nonce_config, wallet_address.unwrap(), &pws)
                .await
                .map_err(|e| AppError2::MigrationError(format!("Nonce manager init: {}", e)))?,
        );

        // Recover any pending registrations from a previous run
        nonce_manager
            .recover_pending(&pws)
            .await
            .map_err(|e| AppError2::MigrationError(format!("Nonce manager recovery: {}", e)))?;

        let (reg_tx, reg_rx) = tokio::sync::mpsc::channel(256);
        let managed_rln = ManagedRLNRegister::new(reg_tx);

        // Spawn registration processor (sequential nonce-aware tx submission)
        let nm1 = nonce_manager.clone();
        let contract1 = rln_sc_instance.clone();
        set.spawn(async move {
            process_registrations(reg_rx, nm1, contract1).await;
            Ok(())
        });

        // Spawn stuck transaction monitor
        let nm2 = nonce_manager.clone();
        let contract2 = rln_sc_instance.clone();
        let pws2 = pws.clone();
        set.spawn(async move {
            monitor_stuck_transactions(nm2, contract2, pws2).await;
            Ok(())
        });

        // Pass managed_rln (implements RLNRegister) to event listener
        set.spawn(async move { registry_listener.listen(p, managed_rln).await });
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

    // TODO: Add periodic cleanup task for nullifiers.
    // Deny list entries are epoch-aligned and cleared via clear_deny_list() on epoch change.

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
