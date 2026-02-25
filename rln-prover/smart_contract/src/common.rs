use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::transports::TransportError;

#[allow(clippy::let_and_return)]
pub async fn ws_provider(rpc_url: String) -> Result<impl Provider, TransportError> {
    let ws = WsConnect::new(rpc_url);
    let provider = ProviderBuilder::new().connect_ws(ws).await;
    provider
}
