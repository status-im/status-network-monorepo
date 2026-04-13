use crate::proof_delivery_service::ProofDeliveryServerConfig;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proof_delivery_service::ProofDeliveryServer;
    use crate::prover_proto::rln_aggregator_client::RlnAggregatorClient;
    use crate::prover_proto::rln_proof_reply::Resp;
    use crate::prover_proto::{RlnAggFilter, RlnProofError, RlnProofReply};
    use anyhow::Context;
    use futures::StreamExt;
    use rand::prelude::StdRng;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;
    use tokio::sync::broadcast::error::{RecvError, SendError};
    use tonic::{IntoRequest, Status};
    use tracing::{debug, error, info};
    // internal
    use crate::proof_reduce_service::ProofReduceService;

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_client_connected_limit() -> anyhow::Result<()> {
        // Test rln-aggregator connected client limit

        let mut cfg = ProofDeliveryServerConfig::default();
        cfg.client_connected_limit = 1;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        debug!("addr: {}", addr);

        let (tx, rx) = tokio::sync::broadcast::channel(10);
        let server = ProofDeliveryServer::new(cfg, (tx, rx));

        tokio::spawn(async move { server.serve_with(listener).await });

        let channel_1 = tonic::transport::Channel::from_shared(format!("http://{}", addr))?
            .connect()
            .await?;
        let mut client_1 = RlnAggregatorClient::new(channel_1);
        let filter_1 = RlnAggFilter::default().into_request();
        let _gp_1 = client_1.get_proofs(filter_1).await?;

        // let mut client_2 = RlnAggregatorClient::connect("http://127.0.0.1:8080").await?;
        let channel_2 = tonic::transport::Channel::from_shared(format!("http://{}", addr))?
            .connect()
            .await?;
        let mut client_2 = RlnAggregatorClient::new(channel_2);
        let filter_2 = RlnAggFilter::default().into_request();
        let gp_2 = client_2.get_proofs(filter_2).await;

        match gp_2 {
            Ok(_) => panic!("Expect an error"),
            Err(e) => {
                debug!("e: {}", e.to_string());
                assert_eq!(e.message(), "Semaphore full")
            }
        }

        Ok(())
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_broadcast_lagged() -> anyhow::Result<()> {
        let (tx, mut rx) = tokio::sync::broadcast::channel(2);

        tx.send(10)?;
        tx.send(20)?;
        tx.send(30)?;

        // The receiver lagged behind
        // assert!(rx.recv().await.is_err());
        match rx.recv().await {
            Err(RecvError::Lagged(n)) => {
                error!("Already {} skipped message", n);
            }
            _ => panic!("Expect an error"),
        }

        // At this point, we can abort or continue with lost messages

        assert_eq!(20, rx.recv().await?);
        assert_eq!(30, rx.recv().await?);

        Ok(())
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_broadcast_lagged_2() -> anyhow::Result<()> {
        let (bcast_tx, mut bcast_rx) = tokio::sync::broadcast::channel(2);
        let (client_tx, mut client_rx) = tokio::sync::mpsc::channel(2);

        let handle_client = tokio::spawn(async move {
            info!("Starting handle_client");
            let mut count = 0;
            loop {
                match client_rx.recv().await {
                    None => {
                        error!("Client_rx has been closed...");
                        break;
                    }
                    Some(v) => {
                        info!("[handle_client] received value: {}", v);
                        count += 1;
                    }
                }
            }

            Ok::<i32, SendError<u8>>(count)
        });

        let handle_recv = tokio::spawn(async move {
            info!("Starting handle_recv");
            let mut count: i32 = 0;
            loop {
                match bcast_rx.recv().await {
                    Err(RecvError::Lagged(n)) => {
                        error!("Already {} skipped message", n);
                        break;
                    }
                    Err(RecvError::Closed) => break,
                    Ok(v) => {
                        count += 1;
                        client_tx.send(v).await.unwrap();
                    }
                }
            }
            Ok::<i32, SendError<u8>>(count)
        });

        let handle_send = tokio::spawn(async move {
            info!("Starting handle_send");
            bcast_tx.send(10u8)?;
            // tokio::time::sleep(Duration::from_secs(1)).await;
            bcast_tx.send(20)?;
            // tokio::time::sleep(Duration::from_secs(1)).await;
            bcast_tx.send(30)?;
            Ok::<i32, SendError<u8>>(3i32)
        });

        let res = tokio::join!(handle_client, handle_recv, handle_send);

        println!("res: {:?}", res);

        Ok(())
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_broadcast_lagged_3() -> anyhow::Result<()> {
        let (bcast_tx, mut bcast_rx) = tokio::sync::broadcast::channel(2);
        let (client_tx, mut client_rx) = tokio::sync::mpsc::channel(2);

        let handle_client = tokio::spawn(async move {
            info!("Starting handle_client");
            let mut count = 0;
            loop {
                match client_rx.recv().await {
                    None => {
                        error!("Client_rx has been closed...");
                        break;
                    }
                    Some(v) => {
                        info!("[handle_client] received value: {}", v);
                        count += 1;
                    }
                }
            }

            Ok::<i32, SendError<u8>>(count)
        });

        let handle_recv = tokio::spawn(async move {
            info!("Starting handle_recv");
            let mut count: i32 = 0;
            loop {
                tokio::select! {
                    _ = client_tx.closed() => {
                        error!("[handle_recv] client disconnected");
                        break;
                    },
                    result = bcast_rx.recv() => {
                        match result {
                            Err(RecvError::Lagged(n)) => {
                                error!("Already {} skipped message", n);
                                break;
                            },
                            Err(RecvError::Closed) => break,
                            Ok(v) => {
                                count += 1;
                                client_tx.send(v).await.unwrap();
                            },
                        }
                    }
                }
            }
            Ok::<i32, SendError<u8>>(count)
        });

        let handle_send = tokio::spawn(async move {
            info!("Starting handle_send");
            bcast_tx.send(10u8)?;
            tokio::time::sleep(Duration::from_secs(1)).await;
            bcast_tx.send(20)?;
            tokio::time::sleep(Duration::from_secs(1)).await;
            bcast_tx.send(30)?;
            Ok::<i32, SendError<u8>>(3i32)
        });

        let res = tokio::join!(handle_client, handle_recv, handle_send);

        println!("res: {:?}", res);

        Ok(())
    }

    struct FastMockProverProof {
        id: u64,
        url: String,
        sender: tokio::sync::mpsc::Sender<RlnProofReply>,
    }

    impl FastMockProverProof {
        fn new(
            id: u64,
            url: String,
            sender: tokio::sync::mpsc::Sender<RlnProofReply>,
        ) -> Self {
            Self {
                id,
                url: url.clone(),
                sender,
            }
        }
    }

    impl FastMockProverProof {
        async fn serve(&mut self) -> anyhow::Result<()> {
            let mut _rng: StdRng = rand::make_rng();

            let mut i = 0u64;
            loop {
                // let proof_reply = RlnProofReply::default();
                let proof_reply = RlnProofReply {
                    resp: Some(Resp::Error(RlnProofError {
                        error: format!("index: {}", i),
                    })),
                };

                self.sender.send(proof_reply).await.context(format!(
                    "[client {} {}] failed to send proof to channel",
                    self.id, self.url
                ))?;

                i += 1;
            }
        }
    }

    #[ignore]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[tracing_test::traced_test]
    async fn test_client_too_slow() -> anyhow::Result<()> {
        // Test rln-aggregator client too slow disconnection

        let mut cfg = ProofDeliveryServerConfig::default();
        cfg.client_connected_limit = 2;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        debug!("addr: {}", addr);
        let (tx, rx) = tokio::sync::mpsc::channel(128);
        let (bcast_tx, bcast_rx) = tokio::sync::broadcast::channel(2);
        let server = ProofDeliveryServer::new(cfg, (bcast_tx.clone(), bcast_rx));

        tokio::spawn(async move { server.serve_with(listener).await });
        // tokio::time::sleep(Duration::from_secs(2)).await;

        let mut mock = FastMockProverProof::new(0, "Mock".to_string(), tx);
        tokio::spawn(async move { mock.serve().await });
        // tokio::time::sleep(Duration::from_secs(2)).await;

        let mut pr_service = ProofReduceService::new(rx, bcast_tx.clone());
        tokio::spawn(async move { pr_service.serve().await } );

        let channel_2 = tonic::transport::Channel::from_shared(format!("http://{}", addr))?
            .connect()
            .await?;
        let mut client_2 = RlnAggregatorClient::new(channel_2);
        let filter_2 = RlnAggFilter::default().into_request();

        let h_c2 = tokio::spawn(async move {
            debug!("Client 2 get proof...");
            let mut stdout = tokio::io::stdout();
            let mut count_2 = 0;
            let mut gp_2 = client_2.get_proofs(filter_2).await?.into_inner();
            while let Some(p) = gp_2.next().await {
                let p_ = p?;
                stdout
                    .write_all(format!("[client 2] received {:?}", p_).as_bytes())
                    .await?;
                count_2 += 1;
                stdout
                    .write_all(format!("[client 2] received {} messages\n", count_2).as_bytes())
                    .await?;
                stdout.flush().await?;
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
