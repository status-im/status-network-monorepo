// third-party
use alloy::{primitives::Address, providers::Provider, sol_types::SolEvent};
use futures::StreamExt;
use tracing::error;
// internal
use crate::error::AppError;
use crate::tier::TierLimits;
use crate::user_db::UserDb;
use smart_contract::KarmaTiers;
use smart_contract::KarmaTiers::KarmaTiersInstance;

pub(crate) struct TiersListener {
    sc_address: Address,
    user_db: UserDb,
}

impl TiersListener {
    pub(crate) fn new(sc_address: Address, user_db: UserDb) -> Self {
        Self {
            sc_address,
            user_db,
        }
    }

    /// Listen to Smart Contract specified events
    pub(crate) async fn listen<P: Provider + Clone>(&self, provider: P) -> Result<(), AppError> {
        // let provider = self.setup_provider_ws().await.map_err(AppError::from)?;

        let filter = alloy::rpc::types::Filter::new()
            .address(self.sc_address)
            .event(KarmaTiers::TiersUpdated::SIGNATURE);

        // Subscribe to logs matching the filter.
        let subscription = provider.clone().subscribe_logs(&filter).await?;
        let mut stream = subscription.into_stream();

        // Loop through the incoming event logs
        while let Some(log) = stream.next().await {
            if let Ok(_tu) = KarmaTiers::TiersUpdated::decode_log_data(log.data()) {
                let tier_limits =
                    match KarmaTiersInstance::get_tiers_from_provider(&provider, &self.sc_address)
                        .await
                    {
                        Ok(tier_limits) => tier_limits,
                        Err(e) => {
                            error!(
                                "Error while getting tiers limits from smart contract: {}",
                                e
                            );
                            return Err(AppError::KarmaTiersError(e));
                        }
                    };

                if let Err(e) = self
                    .user_db
                    .on_tier_limits_updated(TierLimits::from(tier_limits))
                {
                    // If there is an error here, we assume this is an error by the user
                    // updating the Tier limits (and thus we don't want to shut down the prover)
                    error!("Error while updating tier limits: {}", e);
                }
            } else {
                // Should never happen as TiersUpdated is empty
                eprintln!("Error decoding log data");
                // It's also useful to print the raw log data for debugging
                eprintln!("Raw log topics: {:?}", log.topics());
                eprintln!("Raw log data: {:?}", log.data());
            }
        }

        Ok(())
    }
}
