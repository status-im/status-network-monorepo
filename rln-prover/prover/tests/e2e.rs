#[allow(unexpected_cfgs)]
#[cfg(feature = "anvil")]
use std::net::{IpAddr, Ipv4Addr};
#[allow(unexpected_cfgs)]
#[cfg(feature = "anvil")]
use std::time::Duration;

#[allow(unexpected_cfgs)]
#[cfg(feature = "anvil")]
#[tokio::test]
#[traced_test]
async fn test_slashing() {
    let provider = ProviderBuilder::new().connect_anvil_with_wallet();

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

    // Write mock users to tempfile
    let mock_users_as_str = serde_json::to_string(&mock_users).unwrap();
    let mut temp_file = NamedTempFile::new().unwrap();
    let temp_file_path = temp_file.path().to_path_buf();
    temp_file.write_all(mock_users_as_str.as_bytes()).unwrap();
    temp_file.flush().unwrap();
    debug!(
        "Mock user temp file path: {}",
        temp_file_path.to_str().unwrap()
    );
    //

    let temp_folder = tempfile::tempdir().unwrap();
    let temp_folder_tree = tempfile::tempdir().unwrap();

    let port = 60052;
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
        metrics_port: 70031,
        broadcast_channel_size: 500,
        proof_service_count: 8,
        transaction_channel_size: 500,
        proof_sender_channel_size: 500,
        registration_min_amount: AppArgs::default_minimal_amount_for_registration(),
        rln_identifier: AppArgs::default_rln_identifier_name(),
        spam_limit: AppArgs::default_spam_limit(),
        no_grpc_reflection: true,
        tx_gas_quota: AppArgs::default_tx_gas_quota(),
    };

    info!("Starting prover with args: {:?}", app_args);
    let prover_handle = task::spawn(run_prover(app_args));
    // Wait for the prover to be ready
    // Note: if unit test is failing - maybe add an optional notification when service is ready
    tokio::time::sleep(Duration::from_secs(5)).await;
}
