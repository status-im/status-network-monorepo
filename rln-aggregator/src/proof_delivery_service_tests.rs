use crate::proof_delivery_service::ProofDeliveryServerConfig;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proof_delivery_service::ProofDeliveryServer;
    use crate::prover_proto::RlnAggFilter;
    use crate::prover_proto::rln_aggregator_client::RlnAggregatorClient;
    use futures::StreamExt;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time::Duration;
    // use tokio::io::AsyncWriteExt;
    use crate::MockProverProof;
    use tonic::{IntoRequest, Status};
    use tracing::{debug, info};

    #[tokio::test]
    async fn test_client_connected_limit() -> anyhow::Result<()> {
        // Test rln-aggregator connected client limit

        let mut cfg = ProofDeliveryServerConfig::default();
        cfg.client_connected_limit = 1;

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let (tx, rx) = tokio::sync::broadcast::channel(10);
        let server = ProofDeliveryServer::new(cfg, addr, (tx, rx));

        tokio::spawn(async move { server.serve().await });

        tokio::time::sleep(Duration::from_secs(2)).await;

        let mut client_1 = RlnAggregatorClient::connect("http://127.0.0.1:8080").await?;
        let filter_1 = RlnAggFilter::default().into_request();
        let _gp_1 = client_1.get_proofs(filter_1).await?;

        let mut client_2 = RlnAggregatorClient::connect("http://127.0.0.1:8080").await?;
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

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_client_too_slow() -> anyhow::Result<()> {
        // Test rln-aggregator client too slow disconnection

        let mut cfg = ProofDeliveryServerConfig::default();
        cfg.client_connected_limit = 2;

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let (tx, rx) = tokio::sync::broadcast::channel(2);
        let server = ProofDeliveryServer::new(cfg, addr, (tx.clone(), rx));

        tokio::spawn(async move { server.serve().await });
        tokio::time::sleep(Duration::from_secs(2)).await;

        let mut mock = MockProverProof::new(0, "Mock".to_string(), tx);
        tokio::spawn(async move { mock.serve().await });
        tokio::time::sleep(Duration::from_secs(2)).await;

        /*
        let mut client_1 = RlnAggregatorClient::connect("http://127.0.0.1:8080").await?;
        let filter_1 = RlnAggFilter::default().into_request();
        */
        let mut client_2 = RlnAggregatorClient::connect("http://127.0.0.1:8080").await?;
        let filter_2 = RlnAggFilter::default().into_request();

        /*
        let h_c1 = tokio::spawn(async move {
            let mut stdout = tokio::io::stdout();
            let mut count_1 = 0;
            let mut gp_1 = client_1.get_proofs(filter_1).await?.into_inner();
            while let Some(p) = gp_1.next().await {
                p?;
                count_1 += 1;
                // println!("[client 1] received {} messages", count_1);
                // stdout.write_all(format!("[client 1] received {} messages", count_1).as_bytes()).await?;
                // stdout.flush().await?;
            }
            Ok::<u64, Status>(count_1)
        });
        */

        let h_c2 = tokio::spawn(async move {
            debug!("Client 2 get proof...");
            // let mut stdout = tokio::io::stdout();
            let mut count_2 = 0;
            let mut gp_2 = client_2.get_proofs(filter_2).await?.into_inner();
            while let Some(p) = gp_2.next().await {
                p?;
                count_2 += 1;
                //  stdout.write_all(format!("[client 2] received {} messages\n", count_2).as_bytes()).await?;
                //  stdout.flush().await?;
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            Ok::<u64, Status>(count_2)
        });

        info!("Joining h_c1 & h_c2...");
        // let res = tokio::try_join!(h_c1, h_c2);
        let res = h_c2.await;
        println!("res: {:?}", res);

        Ok(())
    }
}
