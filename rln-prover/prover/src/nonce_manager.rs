use std::sync::Arc;

use alloy::primitives::{Address, TxHash, U256};
use alloy::providers::Provider;
use async_trait::async_trait;
use smart_contract::{KarmaRLNSC, RLNRegister};
use sqlx::{Pool, Postgres};
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum NonceManagerError {
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Registration channel closed")]
    ChannelClosed,
    #[error("Contract error: {0}")]
    Contract(String),
    #[error("Provider error: {0}")]
    Provider(String),
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NonceManagerConfig {
    pub stuck_timeout_secs: u64,
    pub max_attempts: i32,
    pub min_gas_price: u128,
    pub monitor_interval_secs: u64,
}

impl Default for NonceManagerConfig {
    fn default() -> Self {
        Self {
            stuck_timeout_secs: 300,
            max_attempts: 5,
            min_gas_price: 0,
            monitor_interval_secs: 30,
        }
    }
}

// ---------------------------------------------------------------------------
// DB entity
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct PendingRegistration {
    pub id: i64,
    pub user_address: Vec<u8>,
    pub identity_commitment: Vec<u8>,
    pub nonce: i64,
    pub tx_hash: Option<Vec<u8>>,
    pub status: String,
    pub gas_price: Option<i64>,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
}

// ---------------------------------------------------------------------------
// Registration task (sent through the mpsc channel)
// ---------------------------------------------------------------------------

pub struct RegistrationTask {
    pub address: Address,
    pub identity_commitment: U256,
    pub result_tx: oneshot::Sender<Result<(), NonceManagerError>>,
}

// ---------------------------------------------------------------------------
// NonceManager
// ---------------------------------------------------------------------------

pub struct NonceManager {
    pub(crate) db: Pool<Postgres>,
    pub(crate) config: NonceManagerConfig,
    pub(crate) wallet_address: Address,
    pub(crate) current_nonce: Mutex<u64>,
}

impl NonceManager {
    /// Create a new NonceManager, syncing the nonce from both chain and DB.
    pub async fn new<P: Provider>(
        db: Pool<Postgres>,
        config: NonceManagerConfig,
        wallet_address: Address,
        provider: &P,
    ) -> Result<Self, NonceManagerError> {
        let chain_nonce = provider
            .get_transaction_count(wallet_address)
            .await
            .map_err(|e| NonceManagerError::Provider(e.to_string()))?;

        let db_nonce: Option<i64> = sqlx::query_scalar(
            "SELECT current_nonce FROM nonce_state WHERE wallet_address = $1",
        )
        .bind(wallet_address.as_slice())
        .fetch_optional(&db)
        .await?;

        if db_nonce.is_none() {
            sqlx::query(
                "INSERT INTO nonce_state (wallet_address, current_nonce) VALUES ($1, $2) \
                 ON CONFLICT ON CONSTRAINT nonce_state_wallet DO NOTHING",
            )
            .bind(wallet_address.as_slice())
            .bind(chain_nonce as i64)
            .execute(&db)
            .await?;
        }

        let current_nonce = std::cmp::max(chain_nonce, db_nonce.unwrap_or(0) as u64);

        sqlx::query(
            "UPDATE nonce_state SET current_nonce = $1, last_synced_at = CURRENT_TIMESTAMP \
             WHERE wallet_address = $2",
        )
        .bind(current_nonce as i64)
        .bind(wallet_address.as_slice())
        .execute(&db)
        .await?;

        info!(
            "NonceManager initialized: wallet={}, chain_nonce={}, db_nonce={:?}, using={}",
            wallet_address, chain_nonce, db_nonce, current_nonce
        );

        Ok(Self {
            db,
            config,
            wallet_address,
            current_nonce: Mutex::new(current_nonce),
        })
    }

    /// Allocate the next nonce and create a `queued` pending registration row.
    pub async fn allocate_nonce(
        &self,
        address: &Address,
        identity_commitment: &U256,
    ) -> Result<u64, NonceManagerError> {
        let mut guard = self.current_nonce.lock().await;
        let nonce = *guard;

        sqlx::query(
            "INSERT INTO pending_registrations (user_address, identity_commitment, nonce, status) \
             VALUES ($1, $2, $3, 'queued')",
        )
        .bind(address.as_slice())
        .bind(&identity_commitment.to_be_bytes::<32>()[..])
        .bind(nonce as i64)
        .execute(&self.db)
        .await?;

        *guard = nonce + 1;

        sqlx::query("UPDATE nonce_state SET current_nonce = $1 WHERE wallet_address = $2")
            .bind(*guard as i64)
            .bind(self.wallet_address.as_slice())
            .execute(&self.db)
            .await?;

        Ok(nonce)
    }

    /// Mark a registration as submitted with its tx hash and optional gas price.
    pub async fn mark_submitted(
        &self,
        nonce: u64,
        tx_hash: TxHash,
        gas_price: Option<i64>,
    ) -> Result<(), NonceManagerError> {
        sqlx::query(
            "UPDATE pending_registrations \
             SET status = 'submitted', tx_hash = $1, gas_price = $2, submitted_at = CURRENT_TIMESTAMP \
             WHERE nonce = $3",
        )
        .bind(tx_hash.as_slice())
        .bind(gas_price)
        .bind(nonce as i64)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Mark a registration as confirmed on-chain.
    pub async fn mark_confirmed(&self, nonce: u64) -> Result<(), NonceManagerError> {
        sqlx::query(
            "UPDATE pending_registrations \
             SET status = 'confirmed', confirmed_at = CURRENT_TIMESTAMP \
             WHERE nonce = $1",
        )
        .bind(nonce as i64)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Mark a registration as failed, incrementing the attempt counter.
    pub async fn mark_failed(&self, nonce: u64, error: &str) -> Result<(), NonceManagerError> {
        sqlx::query(
            "UPDATE pending_registrations \
             SET status = 'failed', last_error = $1, attempt_count = attempt_count + 1 \
             WHERE nonce = $2",
        )
        .bind(error)
        .bind(nonce as i64)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Mark a registration as cancelled.
    pub async fn mark_cancelled(&self, nonce: u64) -> Result<(), NonceManagerError> {
        sqlx::query("UPDATE pending_registrations SET status = 'cancelled' WHERE nonce = $1")
            .bind(nonce as i64)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// Get failed registrations that haven't exceeded `max_attempts`.
    pub async fn get_retriable(&self) -> Result<Vec<PendingRegistration>, NonceManagerError> {
        let rows = sqlx::query_as::<_, PendingRegistration>(
            "SELECT id, user_address, identity_commitment, nonce, tx_hash, status, \
                    gas_price, attempt_count, max_attempts, last_error \
             FROM pending_registrations \
             WHERE status = 'failed' AND attempt_count < max_attempts \
             ORDER BY nonce ASC",
        )
        .fetch_all(&self.db)
        .await?;
        Ok(rows)
    }

    /// Get submitted transactions that have been pending longer than `stuck_timeout_secs`.
    pub async fn get_stuck_transactions(
        &self,
    ) -> Result<Vec<PendingRegistration>, NonceManagerError> {
        let timeout = self.config.stuck_timeout_secs as f64;
        let rows = sqlx::query_as::<_, PendingRegistration>(
            "SELECT id, user_address, identity_commitment, nonce, tx_hash, status, \
                    gas_price, attempt_count, max_attempts, last_error \
             FROM pending_registrations \
             WHERE status = 'submitted' \
               AND submitted_at < CURRENT_TIMESTAMP - make_interval(secs => $1) \
             ORDER BY nonce ASC",
        )
        .bind(timeout)
        .fetch_all(&self.db)
        .await?;
        Ok(rows)
    }

    /// On startup, check pending/submitted registrations from a previous run
    /// and reconcile their state against the chain.
    pub async fn recover_pending<P: Provider>(
        &self,
        provider: &P,
    ) -> Result<(), NonceManagerError> {
        let pending = sqlx::query_as::<_, PendingRegistration>(
            "SELECT id, user_address, identity_commitment, nonce, tx_hash, status, \
                    gas_price, attempt_count, max_attempts, last_error \
             FROM pending_registrations \
             WHERE status IN ('submitted', 'queued') \
             ORDER BY nonce ASC",
        )
        .fetch_all(&self.db)
        .await?;

        info!("Recovering {} pending registrations", pending.len());

        for reg in &pending {
            if let Some(ref tx_hash_bytes) = reg.tx_hash {
                let tx_hash = TxHash::from_slice(tx_hash_bytes);
                match provider.get_transaction_receipt(tx_hash).await {
                    Ok(Some(receipt)) => {
                        if receipt.status() {
                            self.mark_confirmed(reg.nonce as u64).await?;
                            info!("Recovered nonce {}: confirmed on-chain", reg.nonce);
                        } else {
                            self.mark_failed(reg.nonce as u64, "reverted on-chain (recovered)")
                                .await?;
                            warn!("Recovered nonce {}: reverted on-chain", reg.nonce);
                        }
                    }
                    Ok(None) => {
                        info!("Recovered nonce {}: still pending on-chain", reg.nonce);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to check receipt for nonce {}: {:?}",
                            reg.nonce, e
                        );
                    }
                }
            } else {
                // Queued but never submitted — mark failed so it becomes retriable
                self.mark_failed(reg.nonce as u64, "never submitted (crash recovery)")
                    .await?;
                warn!(
                    "Recovered nonce {}: was queued but never submitted",
                    reg.nonce
                );
            }
        }

        // Re-sync nonce from chain (chain may have advanced beyond our local state)
        let chain_nonce = provider
            .get_transaction_count(self.wallet_address)
            .await
            .map_err(|e| NonceManagerError::Provider(e.to_string()))?;

        let mut guard = self.current_nonce.lock().await;
        if chain_nonce > *guard {
            *guard = chain_nonce;
            sqlx::query(
                "UPDATE nonce_state SET current_nonce = $1, last_synced_at = CURRENT_TIMESTAMP \
                 WHERE wallet_address = $2",
            )
            .bind(*guard as i64)
            .bind(self.wallet_address.as_slice())
            .execute(&self.db)
            .await?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ManagedRLNRegister — queues registrations through a channel
// ---------------------------------------------------------------------------

pub struct ManagedRLNRegister {
    tx: mpsc::Sender<RegistrationTask>,
}

impl ManagedRLNRegister {
    pub fn new(tx: mpsc::Sender<RegistrationTask>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl RLNRegister for ManagedRLNRegister {
    type Error = NonceManagerError;

    async fn register_user(
        &self,
        address: &Address,
        identity_commitment: U256,
    ) -> Result<(), Self::Error> {
        let (result_tx, result_rx) = oneshot::channel();
        let task = RegistrationTask {
            address: *address,
            identity_commitment,
            result_tx,
        };

        self.tx
            .send(task)
            .await
            .map_err(|_| NonceManagerError::ChannelClosed)?;

        result_rx
            .await
            .map_err(|_| NonceManagerError::ChannelClosed)?
    }
}

// ---------------------------------------------------------------------------
// process_registrations — sequential nonce-aware tx submission
// ---------------------------------------------------------------------------

/// Receives registration tasks from the channel, allocates nonces, and submits
/// transactions on-chain. Runs until the channel is closed.
pub async fn process_registrations<P: Provider + Clone>(
    mut rx: mpsc::Receiver<RegistrationTask>,
    nm: Arc<NonceManager>,
    contract: KarmaRLNSC::KarmaRLNSCInstance<P>,
) {
    while let Some(task) = rx.recv().await {
        let address = task.address;
        let commitment = task.identity_commitment;

        let result = submit_registration(&nm, &contract, &address, &commitment).await;
        let _ = task.result_tx.send(result);
    }
    info!("Registration channel closed, processor shutting down");
}

async fn submit_registration<P: Provider + Clone>(
    nm: &NonceManager,
    contract: &KarmaRLNSC::KarmaRLNSCInstance<P>,
    address: &Address,
    commitment: &U256,
) -> Result<(), NonceManagerError> {
    let nonce = nm.allocate_nonce(address, commitment).await?;

    let gas_price_val = if nm.config.min_gas_price > 0 {
        Some(nm.config.min_gas_price)
    } else {
        None
    };

    let mut call = contract.register(*commitment, *address).nonce(nonce);
    if let Some(gp) = gas_price_val {
        call = call.gas_price(gp);
    }

    match call.send().await {
        Ok(pending_tx) => {
            let tx_hash = *pending_tx.tx_hash();
            nm.mark_submitted(nonce, tx_hash, gas_price_val.map(|g| g as i64))
                .await?;
            info!(
                "Submitted registration for {} with nonce {} (tx: {})",
                address, nonce, tx_hash
            );
            Ok(())
        }
        Err(e) => {
            let err_msg = e.to_string();
            nm.mark_failed(nonce, &err_msg).await?;
            error!(
                "Failed to submit registration for {} with nonce {}: {}",
                address, nonce, err_msg
            );
            Err(NonceManagerError::Contract(err_msg))
        }
    }
}

// ---------------------------------------------------------------------------
// monitor_stuck_transactions — periodic check for stuck/retriable txs
// ---------------------------------------------------------------------------

/// Periodically checks for stuck (submitted but unconfirmed) and retriable
/// (failed under max_attempts) transactions, and resubmits them.
pub async fn monitor_stuck_transactions<P: Provider + Clone>(
    nm: Arc<NonceManager>,
    contract: KarmaRLNSC::KarmaRLNSCInstance<P>,
    provider: P,
) {
    let interval = std::time::Duration::from_secs(nm.config.monitor_interval_secs);

    loop {
        tokio::time::sleep(interval).await;

        // --- Handle stuck (submitted but unconfirmed) transactions ---
        match nm.get_stuck_transactions().await {
            Ok(stuck) => {
                for reg in stuck {
                    handle_stuck_tx(&nm, &contract, &provider, &reg).await;
                }
            }
            Err(e) => {
                error!("Failed to query stuck transactions: {:?}", e);
            }
        }

        // --- Retry failed registrations ---
        match nm.get_retriable().await {
            Ok(retriable) => {
                for reg in retriable {
                    retry_registration(&nm, &contract, &reg).await;
                }
            }
            Err(e) => {
                error!("Failed to query retriable registrations: {:?}", e);
            }
        }
    }
}

async fn handle_stuck_tx<P: Provider + Clone>(
    nm: &NonceManager,
    contract: &KarmaRLNSC::KarmaRLNSCInstance<P>,
    provider: &P,
    reg: &PendingRegistration,
) {
    let nonce = reg.nonce as u64;

    if let Some(ref tx_hash_bytes) = reg.tx_hash {
        let tx_hash = TxHash::from_slice(tx_hash_bytes);

        // Check if it was actually confirmed while we weren't looking
        match provider.get_transaction_receipt(tx_hash).await {
            Ok(Some(receipt)) => {
                if receipt.status() {
                    let _ = nm.mark_confirmed(nonce).await;
                    info!("Stuck tx nonce {} was actually confirmed", nonce);
                } else {
                    let _ = nm.mark_failed(nonce, "reverted on-chain").await;
                    warn!("Stuck tx nonce {} reverted on-chain", nonce);
                }
                return;
            }
            Ok(None) => {
                // Still pending — resubmit with bumped gas
            }
            Err(e) => {
                warn!("Failed to check receipt for stuck nonce {}: {:?}", nonce, e);
                return;
            }
        }

        // Resubmit with higher gas price (2x previous or min_gas_price, whichever is larger)
        let prev_gas = reg.gas_price.unwrap_or(0) as u128;
        let bumped_gas = std::cmp::max(nm.config.min_gas_price, prev_gas.saturating_mul(2));
        let address = Address::from_slice(&reg.user_address);
        let commitment = U256::from_be_slice(&reg.identity_commitment);

        warn!(
            "Resubmitting stuck nonce {} with gas_price {} (was {})",
            nonce, bumped_gas, prev_gas
        );

        let call = contract
            .register(commitment, address)
            .nonce(nonce)
            .gas_price(bumped_gas);

        match call.send().await {
            Ok(pending_tx) => {
                let new_hash = *pending_tx.tx_hash();
                let _ = nm
                    .mark_submitted(nonce, new_hash, Some(bumped_gas as i64))
                    .await;
                info!(
                    "Resubmitted nonce {} with gas_price {} (tx: {})",
                    nonce, bumped_gas, new_hash
                );
            }
            Err(e) => {
                let _ = nm
                    .mark_failed(nonce, &format!("resubmit failed: {}", e))
                    .await;
                error!("Failed to resubmit nonce {}: {}", nonce, e);
            }
        }
    }
}

async fn retry_registration<P: Provider + Clone>(
    nm: &NonceManager,
    contract: &KarmaRLNSC::KarmaRLNSCInstance<P>,
    reg: &PendingRegistration,
) {
    let nonce = reg.nonce as u64;
    let address = Address::from_slice(&reg.user_address);
    let commitment = U256::from_be_slice(&reg.identity_commitment);

    info!(
        "Retrying failed registration nonce {} (attempt {})",
        nonce,
        reg.attempt_count + 1
    );

    let gas_price = nm.config.min_gas_price;
    let mut call = contract.register(commitment, address).nonce(nonce);
    if gas_price > 0 {
        call = call.gas_price(gas_price);
    }

    match call.send().await {
        Ok(pending_tx) => {
            let tx_hash = *pending_tx.tx_hash();
            let _ = nm
                .mark_submitted(nonce, tx_hash, Some(gas_price as i64))
                .await;
            info!("Retry submitted nonce {} (tx: {})", nonce, tx_hash);
        }
        Err(e) => {
            let _ = nm
                .mark_failed(nonce, &format!("retry failed: {}", e))
                .await;
            error!("Failed to retry nonce {}: {}", nonce, e);
        }
    }
}
