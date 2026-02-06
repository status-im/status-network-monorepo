use std::net::SocketAddr;
use std::sync::Arc;
// third-party
use anyhow::anyhow;
use bytesize::ByteSize;
use futures::TryFutureExt;
use tokio::sync::broadcast::{Receiver, Sender};
use tonic::{
    Request, Response, Status, codegen::http::Method,
    codegen::tokio_stream::wrappers::ReceiverStream, transport::Server,
};
use tower_http::cors::{Any, CorsLayer};
// grpc proto
use crate::prover_proto::rln_aggregator_server::{RlnAggregator, RlnAggregatorServer};
use crate::prover_proto::rln_proof_reply::Resp;
use crate::prover_proto::{
    RlnAggFilter, RlnAggProof, RlnAggProofError, RlnAggProofReply, RlnProof, RlnProofError,
    RlnProofReply, rln_agg_proof_reply,
};

pub struct ProofDeliveryServer {
    config: ProofDeliveryServerConfig,
    addr: SocketAddr,
    broadcast_channel: (Sender<RlnProofReply>, Receiver<RlnProofReply>),
}

pub struct ProofDeliveryServerConfig {
    grpc_max_decoding_message_size: u64,
    grpc_max_encoding_message_size: u64,
}

impl Default for ProofDeliveryServerConfig {
    fn default() -> Self {
        Self {
            grpc_max_decoding_message_size: ByteSize::mib(5).as_u64(),
            grpc_max_encoding_message_size: ByteSize::mib(5).as_u64(),
        }
    }
}

impl ProofDeliveryServer {
    pub(crate) fn new(
        config: ProofDeliveryServerConfig,
        addr: SocketAddr,
        (tx, rx): (Sender<RlnProofReply>, Receiver<RlnProofReply>),
    ) -> Self {
        Self {
            config,
            addr,
            broadcast_channel: (tx, rx),
        }
    }

    pub(crate) async fn serve(&self) -> anyhow::Result<()> {
        let service = ProofDeliveryService {
            broadcast_channel: (
                self.broadcast_channel.0.clone(),
                self.broadcast_channel.0.subscribe(),
            ),
        };

        let agg_server = RlnAggregatorServer::new(service)
            .max_decoding_message_size(self.config.grpc_max_decoding_message_size as usize)
            .max_encoding_message_size(self.config.grpc_max_encoding_message_size as usize);

        let cors = CorsLayer::new()
            .allow_methods([Method::GET])
            .allow_origin(Any)
            .allow_headers(Any);

        Server::builder()
            // service protection && limits
            // limits: connection
            // .concurrency_limit_per_connection(self.config.agg_service_limit_per_connection)
            // .timeout(PROVER_SERVICE_GRPC_TIMEOUT)
            // limits : http2
            // .max_concurrent_streams(PROVER_SERVICE_HTTP2_MAX_CONCURRENT_STREAM)
            // .max_frame_size(PROVER_SERVICE_HTTP2_MAX_FRAME_SIZE.as_u64() as u32)
            // perf: tcp
            .tcp_nodelay(true)
            // http 1 layer required for GrpcWebLayer
            // .accept_http1(true)
            // services
            .layer(cors)
            // .layer(GrpcWebLayer::new())
            // .add_optional_service(reflection_service)
            .add_service(agg_server)
            .serve(self.addr)
            // .map_err(AppError2::from)
            .map_err(|e| anyhow!(e))
            .await
    }
}

struct ProofDeliveryService {
    broadcast_channel: (Sender<RlnProofReply>, Receiver<RlnProofReply>),
}

#[tonic::async_trait]
impl RlnAggregator for ProofDeliveryService {
    type GetProofsStream = ReceiverStream<Result<RlnAggProofReply, Status>>;

    async fn get_proofs(
        &self,
        _request: Request<RlnAggFilter>,
    ) -> Result<Response<Self::GetProofsStream>, Status> {


        // Channel to send stuff to the connected grpc client
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        // Channel to receive a RLN proof (from one proof service)
        let mut rx2 = self.broadcast_channel.0.subscribe();

        tokio::spawn(async move {
            // Stream proofs to client, with proper disconnect detection
            loop {
                tokio::select! {
                    // Check if client disconnected (receiver dropped)
                    _ = tx.closed() => {
                        println!("[Proof delivery service] client disconnected");
                        break;
                    }
                    // Receive proofs from broadcast channel
                    result = rx2.recv() => {

                        match result {
                            Ok(rln_proof_reply) => {

                                let resp = rln_proof_reply.into();

                                // Send to the client
                                if let Err(e) = tx.send(Ok(resp)).await {
                                    println!("[Proof delivery service] Client disconnected during send: {}", e);
                                    break;
                                };

                            },
                            Err(e) => {
                                // TODO: handle the slow receiver here
                                println!("[Proof delivery service] channel receive error {:?}", e);
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

impl From<RlnProofReply> for RlnAggProofReply {
    fn from(p: RlnProofReply) -> Self {
        match p.resp {
            Some(r) => match r {
                Resp::Proof(p) => Self {
                    resp: Some(rln_agg_proof_reply::Resp::Proof(p.into())),
                },
                Resp::Error(e) => Self {
                    resp: Some(rln_agg_proof_reply::Resp::Error(e.into())),
                },
            },
            None => Self { resp: None },
        }
    }
}

impl From<RlnProof> for RlnAggProof {
    fn from(value: RlnProof) -> Self {
        Self {
            sender: value.sender,
            tx_hash: value.tx_hash,
            proof: value.proof,
        }
    }
}

impl From<RlnProofError> for RlnAggProofError {
    fn from(value: RlnProofError) -> Self {
        Self { error: value.error }
    }
}
