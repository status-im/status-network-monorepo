#![allow(clippy::type_complexity)]

// std
use std::net::SocketAddr;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::time::Duration;
// third-party
use alloy::{primitives::Address, providers::Provider};
use async_channel::Sender;
use bytesize::ByteSize;
use futures::TryFutureExt;
use http::Method;
use metrics::{counter, histogram};
use tokio::sync::{broadcast, mpsc};
use tonic::{
    Request, Response, Status, codegen::tokio_stream::wrappers::ReceiverStream, transport::Server,
};
use tonic_web::GrpcWebLayer;
use tower_http::cors::{Any, CorsLayer};
use tracing::{
    debug,
    // error
};
use url::Url;
// internal
use crate::error::{AppError, ProofGenerationStringError};
use crate::metrics::{
    GET_PROOFS_LISTENERS, GET_USER_TIER_INFO_REQUESTS, GaugeWrapper,
    PROOF_SERVICES_CHANNEL_QUEUE_LEN, SEND_TRANSACTION_REQUESTS,
};
use crate::proof_generation::{ProofGenerationData, ProofSendingData};
use crate::user_db::{UserDb, UserTierInfo};
use rln_proof::RlnIdentifier;
use smart_contract::{KarmaAmountExt, KarmaSC::KarmaSCInstance, MockKarmaSc};

pub mod prover_proto {

    // Include generated code (see build.rs)
    tonic::include_proto!("prover");
    // for reflection service
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("prover_descriptor");
}
use crate::user_db_types::RateLimit;
use prover_proto::{
    GetUserTierInfoReply,
    GetUserTierInfoRequest,
    // RegisterUserReply,
    // RegisterUserRequest,
    // RegistrationStatus,
    RlnProof,
    RlnProofFilter,
    RlnProofReply,
    SendTransactionReply,
    SendTransactionRequest,
    // SetTierLimitsReply, SetTierLimitsRequest,
    Tier,
    UserTierInfoError,
    UserTierInfoResult,
    get_user_tier_info_reply::Resp,
    rln_proof_reply::Resp as GetProofsResp,
    rln_prover_server::{RlnProver, RlnProverServer},
};

const PROVER_SERVICE_LIMIT_PER_CONNECTION: usize = 16;
// Timeout for all handlers of a request
const PROVER_SERVICE_GRPC_TIMEOUT: Duration = Duration::from_secs(30);
//
const PROVER_SERVICE_HTTP2_MAX_CONCURRENT_STREAM: u32 = 64;
// Http2 max frame size (e.g. 16 Kb)
const PROVER_SERVICE_HTTP2_MAX_FRAME_SIZE: ByteSize = ByteSize::kib(16);
// Max size for Message (decoding, e.g., 5 Mb)
const PROVER_SERVICE_MESSAGE_DECODING_MAX_SIZE: ByteSize = ByteSize::mib(5);
// Max size for Message (encoding, e.g., 5 Mb)
const PROVER_SERVICE_MESSAGE_ENCODING_MAX_SIZE: ByteSize = ByteSize::mib(5);

const PROVER_TX_HASH_BYTESIZE: usize = 32;

#[derive(Debug)]
pub struct ProverService<KSC: KarmaAmountExt> {
    proof_sender: Sender<ProofGenerationData>,
    user_db: UserDb,
    rln_identifier: Arc<RlnIdentifier>,
    broadcast_channel: (
        broadcast::Sender<Result<ProofSendingData, ProofGenerationStringError>>,
        broadcast::Receiver<Result<ProofSendingData, ProofGenerationStringError>>,
    ),
    karma_sc: KSC,
    // karma_rln_sc: RLNSC,
    proof_sender_channel_size: usize,
    tx_gas_quota: NonZeroU64,
    rate_limit: RateLimit,
}

#[tonic::async_trait]
impl<KSC> RlnProver for ProverService<KSC>
where
    KSC: KarmaAmountExt + Send + Sync + 'static,
    KSC::Error: std::error::Error + Send + Sync + 'static,
{
    #[tracing::instrument(skip(self), err, ret)]
    async fn send_transaction(
        &self,
        request: Request<SendTransactionRequest>,
    ) -> Result<Response<SendTransactionReply>, Status> {
        counter!(SEND_TRANSACTION_REQUESTS.name, "prover" => "grpc").increment(1);
        debug!("send_transaction request: {:?}", request);
        let req = request.into_inner();

        let sender = if let Some(sender) = req.sender {
            if let Ok(sender) = Address::try_from(sender.value.as_slice()) {
                sender
            } else {
                return Err(Status::invalid_argument("Invalid sender address"));
            }
        } else {
            return Err(Status::invalid_argument("No sender address"));
        };

        let user_id = if let Some(id) = self.user_db.get_user(&sender) {
            id.clone()
        } else {
            return Err(Status::not_found("Sender not registered"));
        };

        let tx_counter_incr = if req.estimated_gas_used <= self.tx_gas_quota.get() {
            None
        } else {
            Some(req.estimated_gas_used / self.tx_gas_quota)
        };

        // Update the counter as soon as possible (should help to prevent spamming...)
        let counter = self
            .user_db
            .on_new_tx(&sender, tx_counter_incr)
            .unwrap_or_default();

        if counter > self.rate_limit {
            return Err(Status::resource_exhausted(
                "Too many transactions sent by this user",
            ));
        }

        if req.transaction_hash.len() != PROVER_TX_HASH_BYTESIZE {
            return Err(Status::invalid_argument(
                "Invalid transaction hash (should be 32 bytes)",
            ));
        }

        // Inexpensive clone (behind Arc ptr)
        let rln_identifier = self.rln_identifier.clone();

        let proof_data = ProofGenerationData::from((
            user_id,
            rln_identifier,
            counter.into(),
            sender,
            req.transaction_hash,
        ));

        // Send some data to one of the proof services
        self.proof_sender
            .send(proof_data)
            .await
            .map_err(|e| Status::from_error(Box::new(e)))?;

        // Note: based on this link https://doc.rust-lang.org/reference/expressions/operator-expr.html#type-cast-expressions
        //       "Casting from an integer to float will produce the closest possible float *"
        histogram!(PROOF_SERVICES_CHANNEL_QUEUE_LEN.name, "prover" => "grpc")
            .record(self.proof_sender.len() as f64);

        let reply = SendTransactionReply { result: true };
        Ok(Response::new(reply))
    }

    /*
    #[tracing::instrument(skip(self), err, ret)]
    async fn register_user(
        &self,
        request: Request<RegisterUserRequest>,
    ) -> Result<Response<RegisterUserReply>, Status> {
        debug!("register_user request: {:?}", request);
        counter!(USER_REGISTERED_REQUESTS.name, "prover" => "grpc").increment(1);

        let req = request.into_inner();
        let user = if let Some(user) = req.user {
            if let Ok(user) = Address::try_from(user.value.as_slice()) {
                user
            } else {
                return Err(Status::invalid_argument("Invalid sender address"));
            }
        } else {
            return Err(Status::invalid_argument("No sender address"));
        };

        let result = self.user_db.on_new_user(&user);

        let status = match result {
            Ok(id_commitment) => {
                let id_co =
                    U256::from_le_slice(BigUint::from(id_commitment).to_bytes_le().as_slice());

                if let Err(e) = self.karma_rln_sc.register_user(&user, id_co).await {
                    // Fail to register user on smart contract
                    // Remove the user in internal Db
                    if !self.user_db.remove_user(&user, false) {
                        // Fails if DB & SC are inconsistent
                        panic!("Unable to register user to SC and to remove it from DB...");
                    }
                    return Err(Status::from_error(Box::new(e)));
                }

                RegistrationStatus::Success
            }
            Err(RegisterError::AlreadyRegistered(_a)) => RegistrationStatus::AlreadyRegistered,
            _ => RegistrationStatus::Failure,
        };

        let reply = RegisterUserReply {
            status: status.into(),
        };

        counter!(USER_REGISTERED.name, "prover" => "grpc").increment(1);
        Ok(Response::new(reply))
    }
    */

    type GetProofsStream = ReceiverStream<Result<RlnProofReply, Status>>;

    #[tracing::instrument(skip(self), err, ret)]
    async fn get_proofs(
        &self,
        request: Request<RlnProofFilter>,
    ) -> Result<Response<Self::GetProofsStream>, Status> {
        debug!("get_proofs request: {:?}", request);
        let gauge = GaugeWrapper::new(GET_PROOFS_LISTENERS.name, "prover", "grpc");

        // Channel to send proof to the connected grpc client (aka the Verifier)
        let (tx, rx) = mpsc::channel(self.proof_sender_channel_size);
        // Channel to receive a RLN proof (from one proof service)
        let mut rx2 = self.broadcast_channel.0.subscribe();
        tokio::spawn(async move {
            // FIXME: Should we send the error here?

            let gauge_ = gauge;

            while let Ok(Ok(data)) = rx2.recv().await {
                let rln_proof = RlnProof {
                    sender: data.tx_sender.to_vec(),
                    tx_hash: data.tx_hash,
                    proof: data.proof,
                };

                let resp = RlnProofReply {
                    resp: Some(GetProofsResp::Proof(rln_proof)),
                };

                if let Err(e) = tx.send(Ok(resp)).await {
                    debug!("Done: sending dummy rln proofs: {}", e);
                    break;
                };
            }

            // Note: will be dropped anyway but better be explicit here :)
            drop(gauge_);
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    #[tracing::instrument(skip(self), err, ret)]
    async fn get_user_tier_info(
        &self,
        request: Request<GetUserTierInfoRequest>,
    ) -> Result<Response<GetUserTierInfoReply>, Status> {
        debug!("request: {:?}", request);
        counter!(GET_USER_TIER_INFO_REQUESTS.name, "prover" => "grpc").increment(1);

        let req = request.into_inner();

        let user = if let Some(user) = req.user {
            if let Ok(user) = Address::try_from(user.value.as_slice()) {
                user
            } else {
                return Err(Status::invalid_argument("Invalid user address"));
            }
        } else {
            return Err(Status::invalid_argument("No user address"));
        };

        let tier_info = self.user_db.user_tier_info(&user, &self.karma_sc).await;

        match tier_info {
            Ok(tier_info) => Ok(Response::new(GetUserTierInfoReply {
                resp: Some(Resp::Res(tier_info.into())),
            })),
            Err(e) => Ok(Response::new(GetUserTierInfoReply {
                resp: Some(Resp::Error(e.into())),
            })),
        }
    }
}

pub(crate) struct GrpcProverService<P: Provider> {
    pub proof_sender: Sender<ProofGenerationData>,
    pub broadcast_channel: (
        broadcast::Sender<Result<ProofSendingData, ProofGenerationStringError>>,
        broadcast::Receiver<Result<ProofSendingData, ProofGenerationStringError>>,
    ),
    pub addr: SocketAddr,
    pub rln_identifier: RlnIdentifier,
    pub user_db: UserDb,
    pub karma_sc_info: Option<(Url, Address)>,
    // pub rln_sc_info: Option<(Url, Address)>,
    pub provider: Option<P>,
    pub proof_sender_channel_size: usize,
    pub grpc_reflection: bool,
    pub tx_gas_quota: NonZeroU64,
    pub rate_limit: RateLimit,
}

impl<P: Provider + Clone + Send + Sync + 'static> GrpcProverService<P> {
    pub(crate) async fn serve(&self) -> Result<(), AppError> {
        let karma_sc = if let Some(karma_sc_info) = self.karma_sc_info.as_ref()
            && let Some(provider) = self.provider.as_ref()
        {
            KarmaSCInstance::new(karma_sc_info.1, provider.clone())
        } else {
            panic!("Please provide karma_sc_info or use serve_with_mock");
        };

        let prover_service = ProverService {
            proof_sender: self.proof_sender.clone(),
            user_db: self.user_db.clone(),
            rln_identifier: Arc::new(self.rln_identifier.clone()),
            broadcast_channel: (
                self.broadcast_channel.0.clone(),
                self.broadcast_channel.0.subscribe(),
            ),
            karma_sc,
            proof_sender_channel_size: self.proof_sender_channel_size,
            tx_gas_quota: self.tx_gas_quota,
            rate_limit: self.rate_limit,
        };

        let reflection_service = if self.grpc_reflection {
            Some(
                tonic_reflection::server::Builder::configure()
                    .register_encoded_file_descriptor_set(prover_proto::FILE_DESCRIPTOR_SET)
                    .build_v1()?,
            )
        } else {
            None
        };

        let r = RlnProverServer::new(prover_service)
            .max_decoding_message_size(PROVER_SERVICE_MESSAGE_DECODING_MAX_SIZE.as_u64() as usize)
            .max_encoding_message_size(PROVER_SERVICE_MESSAGE_ENCODING_MAX_SIZE.as_u64() as usize)
            // Note: TODO - can be enabled later if network is a bottleneck
            //.accept_compressed(CompressionEncoding::Gzip)
            //.send_compressed(CompressionEncoding::Gzip)
            ;

        // CORS
        let cors = CorsLayer::new()
            // Allow `GET`, `POST` and `OPTIONS` when accessing the resource
            .allow_methods([
                Method::GET,
                // http POST && OPTIONS not required for grpc-web
                // Method::POST,
                // Method::OPTIONS
            ])
            // Allow requests from any origin
            // Note: TODO - to be enabled in a future version
            .allow_origin(Any)
            .allow_headers(Any);

        Server::builder()
            // service protection && limits
            // limits: connection
            .concurrency_limit_per_connection(PROVER_SERVICE_LIMIT_PER_CONNECTION)
            .timeout(PROVER_SERVICE_GRPC_TIMEOUT)
            // limits : http2
            .max_concurrent_streams(PROVER_SERVICE_HTTP2_MAX_CONCURRENT_STREAM)
            .max_frame_size(PROVER_SERVICE_HTTP2_MAX_FRAME_SIZE.as_u64() as u32)
            // perf: tcp
            .tcp_nodelay(true)
            // http 1 layer required for GrpcWebLayer
            .accept_http1(true)
            // services
            .layer(cors)
            .layer(GrpcWebLayer::new())
            .add_optional_service(reflection_service)
            .add_service(r)
            .serve(self.addr)
            .map_err(AppError::from)
            .await
    }

    pub(crate) async fn serve_with_mock(&self) -> Result<(), AppError> {
        let prover_service = ProverService {
            proof_sender: self.proof_sender.clone(),
            user_db: self.user_db.clone(),
            rln_identifier: Arc::new(self.rln_identifier.clone()),
            broadcast_channel: (
                self.broadcast_channel.0.clone(),
                self.broadcast_channel.0.subscribe(),
            ),
            karma_sc: MockKarmaSc {},
            // karma_rln_sc: MockKarmaRLNSc {},
            proof_sender_channel_size: self.proof_sender_channel_size,
            tx_gas_quota: self.tx_gas_quota,
            rate_limit: self.rate_limit,
        };

        let reflection_service = if self.grpc_reflection {
            Some(
                tonic_reflection::server::Builder::configure()
                    .register_encoded_file_descriptor_set(prover_proto::FILE_DESCRIPTOR_SET)
                    .build_v1()?,
            )
        } else {
            None
        };

        let r = RlnProverServer::new(prover_service)
            .max_decoding_message_size(PROVER_SERVICE_MESSAGE_DECODING_MAX_SIZE.as_u64() as usize)
            .max_encoding_message_size(PROVER_SERVICE_MESSAGE_ENCODING_MAX_SIZE.as_u64() as usize)
            // Note: can be enabled later if network is a bottleneck
            //.accept_compressed(CompressionEncoding::Gzip)
            //.send_compressed(CompressionEncoding::Gzip)
            ;

        // CORS
        let cors = CorsLayer::new()
            // Allow `GET`, `POST` and `OPTIONS` when accessing the resource
            .allow_methods([
                Method::GET,
                // http POST && OPTIONS not required for grpc-web
                // Method::POST,
                // Method::OPTIONS
            ])
            // Allow requests from any origin
            // Note: TODO - to be enabled in a future version
            .allow_origin(Any)
            .allow_headers(Any);

        Server::builder()
            // service protection && limits
            // limits: connection
            .concurrency_limit_per_connection(PROVER_SERVICE_LIMIT_PER_CONNECTION)
            .timeout(PROVER_SERVICE_GRPC_TIMEOUT)
            // limits : http2
            .max_concurrent_streams(PROVER_SERVICE_HTTP2_MAX_CONCURRENT_STREAM)
            .max_frame_size(PROVER_SERVICE_HTTP2_MAX_FRAME_SIZE.as_u64() as u32)
            // perf: tcp
            .tcp_nodelay(true)
            // http 1 layer required for GrpcWebLayer
            .accept_http1(true)
            // services
            .layer(cors)
            .layer(GrpcWebLayer::new())
            .add_optional_service(reflection_service)
            .add_service(r)
            .serve(self.addr)
            .map_err(AppError::from)
            .await
    }
}

/// UserTierInfo to UserTierInfoResult (Grpc message) conversion
impl From<UserTierInfo> for UserTierInfoResult {
    fn from(tier_info: UserTierInfo) -> Self {
        let mut res = UserTierInfoResult {
            current_epoch: tier_info.current_epoch.into(),
            current_epoch_slice: tier_info.current_epoch_slice.into(),
            tx_count: tier_info.epoch_tx_count,
            tier: None,
        };

        if tier_info.tier_name.is_some() && tier_info.tier_limit.is_some() {
            res.tier = Some(Tier {
                name: tier_info.tier_name.unwrap().into(),
                quota: tier_info.tier_limit.unwrap().into(),
            })
        }

        res
    }
}

/// UserTierInfoError to UserTierInfoError (Grpc message) conversion
impl<E> From<crate::user_db_error::UserTierInfoError<E>> for UserTierInfoError
where
    E: std::error::Error,
{
    fn from(value: crate::user_db_error::UserTierInfoError<E>) -> Self {
        UserTierInfoError {
            message: value.to_string(),
        }
    }
}
