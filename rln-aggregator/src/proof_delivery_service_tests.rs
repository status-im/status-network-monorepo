use crate::proof_delivery_service::ProofDeliveryServerConfig;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proof_delivery_service::ProofDeliveryServer;
    use crate::prover_proto::RlnAggFilter;
    use crate::prover_proto::rln_aggregator_client::RlnAggregatorClient;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time::Duration;
    use tonic::IntoRequest;

    #[tokio::test]
    async fn test_client_connected_limit() -> anyhow::Result<()> {
        let mut cfg = ProofDeliveryServerConfig::default();
        cfg.client_connected_limit = 1;

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let (tx, rx) = tokio::sync::broadcast::channel(10);
        let server = ProofDeliveryServer::new(cfg, addr, (tx, rx));

        tokio::spawn(async move { server.serve().await });

        tokio::time::sleep(Duration::from_secs(2)).await;

        let mut client_1 = RlnAggregatorClient::connect("http://127.0.0.1:8080").await?;
        // println!("client_1: {:?}", client_1);
        let filter_1 = RlnAggFilter::default().into_request();
        let _gp_1 = client_1.get_proofs(filter_1).await?;

        let mut client_2 = RlnAggregatorClient::connect("http://127.0.0.1:8080").await?;
        // println!("client_2: {:?}", client_2);
        let filter_2 = RlnAggFilter::default().into_request();
        let gp_2 = client_2.get_proofs(filter_2).await;

        match gp_2 {
            Ok(_) => panic!("Expect an error"),
            Err(e) => {
                assert_eq!(e.message(), "Semaphore full")
            }
        }

        Ok(())
    }
}
