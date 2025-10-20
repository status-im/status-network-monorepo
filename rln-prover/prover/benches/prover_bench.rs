use criterion::Criterion;
use criterion::{BenchmarkId, Throughput};
use criterion::{criterion_group, criterion_main};
use std::io::Write;

// std
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
// third-party
use alloy::primitives::{Address, U256};
use futures::FutureExt;
use tempfile::NamedTempFile;
use tokio::sync::Notify;
use tokio::task::JoinSet;
use tonic::Response;
// internal
use prover::{AppArgs, MockUser, run_prover};

// grpc
pub mod prover_proto {
    // Include generated code (see build.rs)
    tonic::include_proto!("prover");
}
use prover_proto::{
    Address as GrpcAddress, RlnProofFilter, RlnProofReply, SendTransactionReply,
    SendTransactionRequest, U256 as GrpcU256, Wei as GrpcWei, rln_prover_client::RlnProverClient,
};

use lazy_static::lazy_static;
use std::sync::Once;

lazy_static! {
    static ref TRACING_INIT: Once = Once::new();
}

pub fn setup_tracing() {
    TRACING_INIT.call_once(|| {
        let filter = tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("h2=error".parse().unwrap())
            .add_directive("sled::pagecache=error".parse().unwrap())
            .add_directive("opentelemetry_sdk=error".parse().unwrap());

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_line_number(true)
            .with_file(true)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
            .init();
    });
}

async fn proof_sender(port: u16, addresses: Vec<Address>, proof_count: usize) {
    let chain_id = GrpcU256 {
        // FIXME: LE or BE?
        value: U256::from(1).to_le_bytes::<32>().to_vec(),
    };

    let url = format!("http://127.0.0.1:{port}");
    let mut client = RlnProverClient::connect(url).await.unwrap();

    let addr = GrpcAddress {
        value: addresses[0].to_vec(),
    };
    let wei = GrpcWei {
        // FIXME: LE or BE?
        value: U256::from(1000).to_le_bytes::<32>().to_vec(),
    };

    for i in 0..proof_count {
        let tx_hash = U256::from(42 + i).to_le_bytes::<32>().to_vec();

        let request_0 = SendTransactionRequest {
            gas_price: Some(wei.clone()),
            sender: Some(addr.clone()),
            chain_id: Some(chain_id.clone()),
            transaction_hash: tx_hash,
            estimated_gas_used: 1_000,
        };

        let request = tonic::Request::new(request_0);
        let response: Response<SendTransactionReply> =
            client.send_transaction(request).await.unwrap();

        assert!(response.into_inner().result);
    }
    // println!("[proof_sender] returning...");
}

async fn proof_collector(port: u16, proof_count: usize) -> Option<Vec<RlnProofReply>> {
    // let result = Arc::new(RwLock::new(Vec::with_capacity(proof_count)));
    let mut result = Vec::with_capacity(proof_count);

    let url = format!("http://127.0.0.1:{port}");
    let mut client = RlnProverClient::connect(url).await.unwrap();

    let request_0 = RlnProofFilter { address: None };

    let request = tonic::Request::new(request_0);
    let stream_ = client.get_proofs(request).await.unwrap();
    let mut stream = stream_.into_inner();
    // let result_2 = result.clone();
    let mut proof_received = 0;

    loop {
        let response = stream.message().await;
        if let Err(_e) = response {
            // println!("[proof_collector] error: {:?}", _e);
            break;
        }

        let response = response.unwrap();

        if response.is_none() {
            // println!("[proof_collector] response is None");
            break;
        }

        result.push(response.unwrap());
        proof_received += 1;
        if proof_received >= proof_count {
            break;
        }
    }

    // println!("[proof_collector] returning after received: {:?} proof replies", result.len());
    Some(std::mem::take(&mut result))
}

fn proof_generation_bench(c: &mut Criterion) {
    // setup_tracing();

    let rayon_num_threads = std::env::var("RAYON_NUM_THREADS").unwrap_or("".to_string());
    let proof_service_count_default = 4;
    let proof_service_count = std::env::var("PROOF_SERVICE_COUNT")
        .map(|c| u16::from_str(c.as_str()).unwrap_or(proof_service_count_default))
        .unwrap_or(proof_service_count_default);
    let proof_count_default = 5;
    let proof_count = std::env::var("PROOF_COUNT")
        .map(|c| u32::from_str(c.as_str()).unwrap_or(proof_count_default))
        .unwrap_or(proof_count_default);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // Write mock users to tempfile
    let mock_users = vec![
        MockUser {
            address: Address::from_str("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").unwrap(),
            tx_count: 0,
        },
        MockUser {
            address: Address::from_str("0xb20a608c624Ca5003905aA834De7156C68b2E1d0").unwrap(),
            tx_count: 0,
        },
    ];
    let addresses: Vec<Address> = mock_users.iter().map(|u| u.address).collect();
    let mock_users_as_str = serde_json::to_string(&mock_users).unwrap();
    let mut temp_file = NamedTempFile::new().unwrap();
    let temp_file_path = temp_file.path().to_path_buf();
    temp_file.write_all(mock_users_as_str.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let port = 50051;
    let temp_folder = tempfile::tempdir().unwrap();
    let temp_folder_tree = tempfile::tempdir().unwrap();
    // let proof_service_count = 4;
    let app_args = AppArgs {
        ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        port,
        ws_rpc_url: None,
        db_path: temp_folder.path().to_path_buf(),
        merkle_tree_folder: temp_folder_tree.path().to_path_buf(),
        merkle_tree_count: 1,
        merkle_tree_max_count: 1,
        ksc_address: None,
        rlnsc_address: None,
        tsc_address: None,
        mock_sc: Some(true),
        mock_user: Some(temp_file_path),
        config_path: Default::default(),
        no_config: true,
        metrics_ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        metrics_port: 30051,
        broadcast_channel_size: 500,
        proof_service_count,
        transaction_channel_size: 500,
        proof_sender_channel_size: 500,
        registration_min_amount: AppArgs::default_minimal_amount_for_registration(),
        rln_identifier: AppArgs::default_rln_identifier_name(),
        spam_limit: 1_000_000u64,
        no_grpc_reflection: true,
        tx_gas_quota: AppArgs::default_tx_gas_quota(),
    };

    // Tokio notify - wait for some time after spawning run_prover then notify it's ready to accept
    // connections
    let notify_start = Arc::new(Notify::new());

    // Spawn prover
    let notify_start_1 = notify_start.clone();
    rt.spawn(async move {
        tokio::spawn(run_prover(app_args));
        tokio::time::sleep(Duration::from_secs(10)).await;
        println!("Prover is ready, notifying it...");
        notify_start_1.clone().notify_one();
    });

    let notify_start_2 = notify_start.clone();

    // Wait for proof_collector to be connected and waiting for some proofs
    rt.block_on(async move {
        notify_start_2.notified().await;
        println!("Prover is ready...");
        // register_users(port, addresses_0).await;
    });

    println!("Starting benchmark...");
    // let size: usize = 1024;

    let mut group = c.benchmark_group("prover_bench");
    // group.sampling_mode(criterion::SamplingMode::Flat);

    // let proof_count = 5;
    let proof_count = proof_count as usize;

    group.throughput(Throughput::Elements(proof_count as u64));
    #[allow(clippy::uninlined_format_args)]
    let benchmark_name = format!(
        "prover_proof_{}_proof_service_{}_rt_{}",
        proof_count, proof_service_count, rayon_num_threads
    );
    group.bench_with_input(
        BenchmarkId::new(benchmark_name, proof_count),
        &proof_count,
        |b, &_s| {
            b.to_async(&rt).iter(|| {
                async {
                    let mut set = JoinSet::new();
                    set.spawn(proof_collector(port, proof_count)); // return Option<Vec<...>>
                    set.spawn(proof_sender(port, addresses.clone(), proof_count).map(|_r| None)); // Map to None
                    // Wait for proof_sender + proof_collector to complete
                    let res = set.join_all().await;
                    assert_eq!(res.len(), 2);
                    // Check proof_sender return None
                    assert_eq!(res.iter().filter(|r| r.is_none()).count(), 1);
                    // Check we receive enough proofs
                    assert_eq!(
                        res.iter()
                            .filter(|r| { r.as_ref().map(|v| v.len()).unwrap_or(0) == proof_count })
                            .count(),
                        1
                    );
                }
            });
        },
    );

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(Duration::from_secs(500))
    ;
    targets = proof_generation_bench
);
criterion_main!(benches);
