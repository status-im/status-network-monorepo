/*
use alloy::network::{EthereumWallet};
use alloy::providers::{Identity, RootProvider, fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller}, Provider, WsConnect, ProviderBuilder};
use alloy::providers::fillers::WalletFiller;
*/
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::transports::TransportError;

/*
pub type AlloyWsProvider = FillProvider<
    JoinFill<
        Identity,
        JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
    >,
    RootProvider,
>;

pub type AlloyWsProviderWithSigner = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>
        >,
        WalletFiller<EthereumWallet>
    >,
    RootProvider
>;
*/

#[allow(clippy::let_and_return)]
pub async fn ws_provider(rpc_url: String) -> Result<impl Provider, TransportError> {
    let ws = WsConnect::new(rpc_url);
    let provider = ProviderBuilder::new().connect_ws(ws).await;
    provider
}
