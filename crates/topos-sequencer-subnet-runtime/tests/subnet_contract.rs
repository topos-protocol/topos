#![allow(unknown_lints)]
use crate::common::abi;
use ethers::{
    abi::{ethabi::ethereum_types::U256, Address},
    core::types::Filter,
    middleware::SignerMiddleware,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::{Block, H256},
};
use rstest::*;
use serial_test::serial;
use std::collections::HashSet;
use std::process::{Child, Command};
use std::sync::Arc;
use test_log::test;
use tokio::sync::Mutex;
use topos_core::uci::{Certificate, CertificateId, SubnetId, SUBNET_ID_LENGTH};
use topos_sequencer_subnet_runtime::proxy::{SubnetRuntimeProxyCommand, SubnetRuntimeProxyEvent};
use tracing::{error, info, warn, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

mod common;
use crate::common::subnet_test_data::generate_test_private_key;
use topos_core::api::grpc::checkpoints::TargetStreamPosition;
use topos_sequencer_subnet_runtime::{SubnetRuntimeProxyConfig, SubnetRuntimeProxyWorker};

use topos_test_sdk::constants::*;

// Local test network with default 2 seconds block
const STANDALONE_SUBNET_BLOCK_TIME: u64 = 2;
// Local test network with 12 seconds block, usefull for multiple transactions in one block tests
const STANDALONE_SUBNET_WITH_LONG_BLOCKS_BLOCK_TIME: u64 = 12;

const SUBNET_RPC_PORT: u32 = 8545;
// Account 0x4AAb25B4fAd0Beaac466050f3A7142A502f4Cf0a
const TEST_SECRET_ETHEREUM_KEY: &str =
    "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const TEST_ETHEREUM_ACCOUNT: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
const TEST_SUBNET_ID: &str = "6464646464646464646464646464646464646464646464646464646464646464";
const TOKEN_SYMBOL: &str = "TKX";

// Accounts pre-filled in STANDALONE_SUBNET_WITH_LONG_BLOCKS
const TEST_ACCOUNT_ALITH_KEY: &str =
    "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
const TEST_ACCOUNT_ALITH_ACCOUNT: &str = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8";
const TEST_ACCOUNT_BALATHAR_KEY: &str =
    "5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a";
const TEST_ACCOUNT_BALATHAR_ACCOUNT: &str = "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC";
const TEST_ACCOUNT_CEZAR_KEY: &str =
    "7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6";
const TEST_ACCOUNT_CEZAR_ACCOUNT: &str = "0x90F79bf6EB2c4f870365E785982E1f101E93b906";

const PREV_CERTIFICATE_ID_1: CertificateId = CERTIFICATE_ID_4;
const PREV_CERTIFICATE_ID_2: CertificateId = CERTIFICATE_ID_5;
const CERTIFICATE_ID_1: CertificateId = CERTIFICATE_ID_6;
const CERTIFICATE_ID_2: CertificateId = CERTIFICATE_ID_7;
const CERTIFICATE_ID_3: CertificateId = CERTIFICATE_ID_8;
const DEFAULT_GAS: u64 = 5_000_000;

fn spawn_subnet_node(
    port: u32,
    block_time: u64, // Block time in seconds
) -> std::io::Result<Child> {
    // Ignore output, too verbose
    let child = Command::new("anvil")
        .args([
            "--block-time",
            &block_time.to_string(),
            "--port",
            &port.to_string(),
        ])
        .stdout(std::process::Stdio::null())
        .spawn();

    child
}

#[allow(dead_code)]
struct Context {
    pub i_topos_core: abi::IToposCoreClient,
    pub i_topos_messaging: abi::IToposMessagingClient,
    pub i_erc20_messaging: abi::IERC20MessagingClient,
    pub subnet_node_handle: Option<std::process::Child>,
    pub port: u32,
}

impl Context {
    pub async fn shutdown(mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Wait for the subnet node to close
        self.subnet_node_handle
            .take()
            .expect("Valid subnet node handle")
            .kill()
            .expect("Could not kill anvil subprocess");
        Ok(())
    }

    pub fn jsonrpc(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn jsonrpc_ws(&self) -> String {
        format!("ws://127.0.0.1:{}", self.port)
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if let Some(mut child) = self.subnet_node_handle.take() {
            child.kill().expect("Could not kill anvil subprocess");
        }
    }
}

async fn create_new_erc20msg_client(
    deploy_key: &str,
    endpoint: &str,
    erc20_messaging_contract_address: Address,
) -> Result<abi::IERC20MessagingClient, Box<dyn std::error::Error>> {
    let wallet: LocalWallet = deploy_key.parse()?;
    let http_provider =
        Provider::<Http>::try_from(endpoint)?.interval(std::time::Duration::from_millis(20u64));
    let chain_id = http_provider.get_chainid().await?;
    let client = Arc::new(SignerMiddleware::new(
        http_provider,
        wallet.with_chain_id(chain_id.as_u64()),
    ));
    Ok(abi::IERC20Messaging::new(
        erc20_messaging_contract_address,
        client,
    ))
}

async fn create_new_erc20_client(
    deploy_key: &str,
    endpoint: &str,
    erc20_contract_address: Address,
) -> Result<abi::IERC20Client, Box<dyn std::error::Error>> {
    let wallet: LocalWallet = deploy_key.parse()?;
    let http_provider =
        Provider::<Http>::try_from(endpoint)?.interval(std::time::Duration::from_millis(20u64));
    let chain_id = http_provider.get_chainid().await?;
    let client = Arc::new(SignerMiddleware::new(
        http_provider,
        wallet.with_chain_id(chain_id.as_u64()),
    ));
    let i_erc20 = abi::IERC20::new(erc20_contract_address, client);
    Ok(i_erc20)
}

async fn deploy_contracts(
    deploy_key: &str,
    endpoint: &str,
) -> Result<
    (
        abi::IToposCoreClient,
        abi::IToposMessagingClient,
        abi::IERC20MessagingClient,
    ),
    Box<dyn std::error::Error>,
> {
    use ethers::abi::Token;

    let wallet: LocalWallet = deploy_key.parse()?;
    let http_provider =
        Provider::<Http>::try_from(endpoint)?.interval(std::time::Duration::from_millis(20u64));
    let chain_id = http_provider.get_chainid().await?;
    let wallet_account = wallet.address();
    let client = Arc::new(SignerMiddleware::new(
        http_provider,
        wallet.with_chain_id(chain_id.as_u64()),
    ));

    // Deploying contracts
    info!("Deploying TokenDeployer contract...");
    let token_deployer_contract = abi::TokenDeployerContract::deploy(client.clone(), ())?
        .gas(DEFAULT_GAS)
        .chain_id(chain_id.as_u64())
        .legacy()
        .send()
        .await?;
    info!(
        "TokenDeployer contract deployed to 0x{:x}",
        token_deployer_contract.address()
    );

    info!("Deploying ToposCore contract...");
    let topos_core_contract = abi::ToposCoreContract::deploy(client.clone(), ())?
        .gas(DEFAULT_GAS)
        .chain_id(chain_id.as_u64())
        .legacy()
        .send()
        .await?;
    info!(
        "ToposCore contract deployed to 0x{:x}",
        topos_core_contract.address()
    );

    let topos_core_contact_address: Token = Token::Address(topos_core_contract.address());
    let admin_account = vec![wallet_account];
    let new_admin_threshold = U256::from(1);

    info!("Deploying ToposCoreProxy contract...");
    let topos_core_proxy_contract =
        abi::ToposCoreProxyContract::deploy(client.clone(), topos_core_contact_address)?
            .gas(DEFAULT_GAS)
            .chain_id(chain_id.as_u64())
            .legacy()
            .send()
            .await?;
    info!(
        "ToposCoreProxy contract deployed to 0x{:x}",
        topos_core_proxy_contract.address()
    );
    let i_topos_core = abi::IToposCore::new(topos_core_proxy_contract.address(), client.clone());

    if let Err(e) = i_topos_core
        .initialize(admin_account, new_admin_threshold)
        .legacy()
        .gas(DEFAULT_GAS)
        .send()
        .await
        .map_err(|e| {
            error!("Unable to initalize topos core contract: {e}");
            e
        })?
        .await
    {
        panic!("Error setting network subnet id: {e}");
    }

    info!("Deploying ERC20Messaging contract...");
    let erc20_messaging_contract = abi::ERC20MessagingContract::deploy(
        client.clone(),
        (
            token_deployer_contract.address(),
            topos_core_proxy_contract.address(),
        ),
    )?
    .gas(DEFAULT_GAS)
    .chain_id(chain_id.as_u64())
    .legacy()
    .send()
    .await?;
    info!(
        "ERC20 contract deployed to 0x{:x}",
        erc20_messaging_contract.address()
    );

    let i_topos_messaging =
        abi::IToposMessaging::new(erc20_messaging_contract.address(), client.clone());
    let i_erc20_messaging = abi::IERC20Messaging::new(erc20_messaging_contract.address(), client);

    // Set network subnet id
    info!(
        "Updating new contract subnet network id to {}",
        SOURCE_SUBNET_ID_1.to_string()
    );

    if let Err(e) = i_topos_core
        .set_network_subnet_id(SOURCE_SUBNET_ID_1.as_array().to_owned())
        .legacy()
        .gas(DEFAULT_GAS)
        .send()
        .await
        .map_err(|e| {
            error!("Unable to set network id: {e}");
            e
        })?
        .await
    {
        panic!("Error setting network subnet id: {e}");
    }

    match i_topos_core.network_subnet_id().await {
        Ok(subnet_id) => {
            info!("Network subnet id {:?} successfully set", subnet_id);
        }
        Err(e) => {
            error!("Error retrieving subnet id: {e}");
        }
    }

    Ok((i_topos_core, i_topos_messaging, i_erc20_messaging))
}

async fn deploy_test_token(
    deploy_key: &str,
    endpoint: &str,
    topos_messaging_address: Address,
) -> Result<abi::IERC20Client, Box<dyn std::error::Error>> {
    use ethers::abi::Token;

    let wallet: LocalWallet = deploy_key.parse()?;
    let http_provider =
        Provider::<Http>::try_from(endpoint)?.interval(std::time::Duration::from_millis(20u64));
    let chain_id = http_provider.get_chainid().await?;
    let client = Arc::new(SignerMiddleware::new(
        http_provider,
        wallet.with_chain_id(chain_id.as_u64()),
    ));

    let ierc20_messaging = abi::IERC20Messaging::new(topos_messaging_address, client.clone());

    // Deploy token
    let token_name: Token = Token::String("Test Token".to_string());
    let token_symbol: Token = Token::String(TOKEN_SYMBOL.to_string());
    let token_mint_cap: Token = Token::Uint(U256::from(100_000_000));
    let token_daily_mint_limit: Token = Token::Uint(U256::from(100));
    let token_initial_supply: Token = Token::Uint(U256::from(10_000_000));
    let token_encoded_params: ethers::types::Bytes = ethers::abi::encode(&[
        token_name.clone(),
        token_symbol.clone(),
        token_mint_cap,
        token_daily_mint_limit,
        token_initial_supply,
    ])
    .into();
    info!(
        "Deploying new token {} with symbol {}",
        token_name, token_symbol
    );

    let deploy_query = ierc20_messaging
        .deploy_token(token_encoded_params)
        .legacy()
        .gas(DEFAULT_GAS);

    let deploy_result = deploy_query.send().await.map_err(|e| {
        error!("Unable deploy token: {e}");
        e
    })?;

    match deploy_result.await {
        Ok(r) => {
            info!("Token deployed: {:?}", r);
        }
        Err(e) => {
            error!("Unable to deploy token: {e}");
        }
    }

    let events = ierc20_messaging
        .event::<abi::ierc20_messaging::TokenDeployedFilter>()
        .from_block(0);
    let events = events.query().await?;
    if events.is_empty() {
        panic!(
            "Missing TokenDeployed event. Token contract is not deployed to test subnet. Could \
             not execute test"
        );
    }
    let token_address = events[0].token_address;
    info!("Token contract deployed to {}", token_address.to_string());

    let i_erc20 = abi::IERC20Client::new(token_address, client);

    Ok(i_erc20)
}

async fn check_received_certificate(
    mut runtime_proxy_worker: SubnetRuntimeProxyWorker,
    received_certificates: Arc<Mutex<Vec<(u64, Certificate)>>>,
    expected_block_numbers: Vec<u64>,
    expected_blocks: Vec<Block<H256>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let start_height = *expected_block_numbers.first().unwrap();
    let end_height = *expected_block_numbers.last().unwrap();
    while let Ok(event) = runtime_proxy_worker.next_event().await {
        if let SubnetRuntimeProxyEvent::NewCertificate {
            cert,
            block_number,
            ctx: _,
        } = event
        {
            info!(
                "New certificate event received, block number: {} cert id: {} target subnets: \
                 {:?} state root {}",
                block_number,
                cert.id,
                cert.target_subnets,
                hex::encode(cert.state_root)
            );
            let mut received_certificates = received_certificates.lock().await;
            received_certificates.push((block_number, *cert));

            if received_certificates
                .iter()
                .take(end_height as usize + 1)
                .map(|(height, _cert)| height)
                .copied()
                .collect::<Vec<_>>()
                == expected_block_numbers
            {
                info!(
                    "Received all certificates for blocks from {} to {}",
                    start_height, end_height
                );
                // Check if state root matches for all blocks
                for expected_height in expected_block_numbers {
                    let index = (expected_height - start_height) as usize;
                    let received_certificate = &received_certificates[index].1;
                    if expected_blocks[index].state_root
                        != ethers::types::TxHash(received_certificate.state_root)
                    {
                        error!(
                            "State root mismatch, block: {:#?}\n received certificate: {:#?}",
                            expected_blocks[index], received_certificates[index].1
                        );
                        panic!("State root mismatch");
                    }
                }
                info!(
                    "State root check successfully passed for blocks from {} to {}",
                    start_height, end_height
                );

                return Ok::<(), Box<dyn std::error::Error>>(());
            }
        }
    }
    panic!("Expected event not received");
}

#[fixture]
async fn context_running_subnet_node(
    #[default(8545)] port: u32,
    #[default(STANDALONE_SUBNET_BLOCK_TIME)] block_time: u64,
) -> Context {
    info!(
        "Starting subnet node on port {}, block time: {}s",
        port, block_time
    );

    let subnet_node_handle = match spawn_subnet_node(port, block_time) {
        Ok(subnet_node_handle) => subnet_node_handle,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                panic!(
                    "Could not find Anvil binary. Please install and add to path Foundry tools \
                     including Anvil"
                );
            } else {
                panic!("Failed to start the Anvil subnet node as part of test context: {e}");
            }
        }
    };
    // Wait a bit for anvil subprocess to spin itself up
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    info!("Subnet node started...");

    // Deploy contracts
    let json_rpc_endpoint = format!("http://127.0.0.1:{port}");
    match deploy_contracts(TEST_SECRET_ETHEREUM_KEY, &json_rpc_endpoint).await {
        Ok((i_topos_core, i_topos_messaging, i_erc20_messaging)) => {
            info!("Contracts successfully deployed");
            // Context with subnet container working in the background and ready deployed contracts
            Context {
                i_topos_core,
                i_topos_messaging,
                i_erc20_messaging,
                subnet_node_handle: Some(subnet_node_handle),
                port,
            }
        }
        Err(e) => {
            panic!("Unable to deploy contracts: {e}");
        }
    }
}

// Test to start subnet and deploy subnet smart contract
#[rstest]
#[test(tokio::test)]
#[serial]
async fn test_subnet_node_contract_deployment(
    #[with(8544)]
    #[future]
    context_running_subnet_node: Context,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_subnet_node.await;
    info!("Subnet running in the background with deployed contract");
    context.shutdown().await?;
    info!("Subnet node test finished");
    Ok(())
}

// Test subnet client RPC connection to subnet
#[rstest]
#[test(tokio::test)]
#[serial]
async fn test_subnet_node_get_block_info(
    #[with(8545)]
    #[future]
    context_running_subnet_node: Context,
) -> Result<(), Box<dyn std::error::Error>> {
    //Context with subnet
    let context = context_running_subnet_node.await;
    match topos_sequencer_subnet_client::SubnetClientListener::new(
        &context.jsonrpc_ws(),
        &("0x".to_string() + &hex::encode(context.i_topos_core.address())),
    )
    .await
    {
        Ok(mut subnet_client) => match subnet_client.get_finalized_block(6).await {
            Ok(block_info) => {
                info!(
                    "Block info successfully retrieved for block {}",
                    block_info.number
                );
                // Blocks must have been mined while we deployed contracts
                assert!(block_info.number == 6);
            }
            Err(e) => {
                panic!("Error getting next finalized block: {e}");
            }
        },
        Err(e) => {
            panic!("Unable to get block info, error {e}");
        }
    }
    context.shutdown().await?;
    info!("Subnet node test finished");
    Ok(())
}

// Test runtime initialization
#[rstest]
#[test(tokio::test)]
#[serial]
async fn test_create_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let test_private_key = generate_test_private_key();
    info!("Creating runtime proxy...");
    let runtime_proxy_worker = SubnetRuntimeProxyWorker::new(
        SubnetRuntimeProxyConfig {
            subnet_id: SOURCE_SUBNET_ID_1,
            http_endpoint: format!("http://localhost:{SUBNET_RPC_PORT}"),
            ws_endpoint: format!("ws://localhost:{SUBNET_RPC_PORT}"),
            subnet_contract_address: "0x0000000000000000000000000000000000000000".to_string(),
            verifier: 0,
            source_head_certificate_id: None,
            start_block: None,
        },
        test_private_key,
    )
    .await?;
    let runtime_proxy = topos_sequencer_subnet_runtime::testing::get_runtime(&runtime_proxy_worker);
    let runtime_proxy = runtime_proxy.lock().await;
    info!("New runtime proxy created:{:?}", &runtime_proxy);
    Ok(())
}

// Test push certificate to subnet smart contract
#[rstest]
#[test(tokio::test)]
#[serial]
async fn test_subnet_certificate_push_call(
    #[with(8546)]
    #[future]
    context_running_subnet_node: Context,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_subnet_node.await;
    let test_private_key = generate_test_private_key();
    let admin_key = hex::decode(TEST_SECRET_ETHEREUM_KEY).unwrap();
    let subnet_smart_contract_address =
        "0x".to_string() + &hex::encode(context.i_topos_core.address());
    let runtime_proxy_worker = SubnetRuntimeProxyWorker::new(
        SubnetRuntimeProxyConfig {
            subnet_id: SOURCE_SUBNET_ID_1,
            http_endpoint: context.jsonrpc(),
            ws_endpoint: context.jsonrpc_ws(),
            subnet_contract_address: subnet_smart_contract_address.clone(),
            verifier: 0,
            source_head_certificate_id: None,
            start_block: None,
        },
        admin_key.clone(),
    )
    .await?;

    let source_subnet_id_1 =
        topos_crypto::keys::derive_public_key(test_private_key.as_slice()).unwrap();

    let mut certs = Vec::new();

    let new_cert = |id, prev_id| {
        let mut mock_cert = Certificate {
            source_subnet_id: SubnetId::from_array(
                TryInto::<[u8; SUBNET_ID_LENGTH]>::try_into(&source_subnet_id_1[1..33]).unwrap(),
            ),
            id,
            prev_id,
            target_subnets: vec![SOURCE_SUBNET_ID_1],
            receipts_root_hash: *id.as_array(), // just to have different receipt root
            ..Default::default()
        };
        mock_cert
            .update_signature(test_private_key.as_slice())
            .expect("valid signature update");

        mock_cert
    };

    certs.push(new_cert(CERTIFICATE_ID_1, PREV_CERTIFICATE_ID_1));
    certs.push(new_cert(CERTIFICATE_ID_2, PREV_CERTIFICATE_ID_2));
    certs.push(new_cert(CERTIFICATE_ID_15, CERTIFICATE_ID_14));

    info!("Sending mock certificate to subnet smart contract...");

    // Multiple push
    for (idx, mock_cert) in certs.iter().enumerate() {
        info!(
            "Push #{idx} for the Certificate: {:?}, Receipt root: {:?}",
            mock_cert.id, mock_cert.receipts_root_hash
        );
        if let Err(e) = runtime_proxy_worker
            .eval(SubnetRuntimeProxyCommand::OnNewDeliveredCertificate {
                certificate: mock_cert.clone(),
                position: idx as u64,
                ctx: Span::current().context(),
            })
            .await
        {
            error!("Failed to send OnNewDeliveredTxns command: {}", e);
            return Err(Box::from(e));
        }
    }

    info!("Waiting for CrossSubnetMessageSent event");
    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
    let provider = Provider::<Http>::try_from(format!("http://127.0.0.1:{}", context.port))?;
    let client = Arc::new(provider);
    let filter = Filter::new()
        .address(context.i_topos_core.address())
        .event("CertStored(bytes32,bytes32)")
        .from_block(0);

    let logs = client.get_logs(&filter).await?;
    info!("ALL LOGS: {:?}", logs);

    let expected_logs = certs
        .iter()
        .map(|c| {
            let mut log = c.id.as_array().to_vec();
            log.extend_from_slice(&c.receipts_root_hash);
            log
        })
        .collect::<Vec<_>>();

    assert_eq!(
        logs.len(),
        expected_logs.len(),
        "should have as much logs as pushed Certificates"
    );

    for log in logs {
        info!(
            "CrossSubnetMessageSent received: block number {:?} from contract {}",
            log.block_number, log.address
        );
        assert_eq!(hex::encode(log.address), subnet_smart_contract_address[2..]);
        assert!(
            expected_logs.iter().any(|l| *l == log.data.0),
            "discrepencies in the logs"
        );
    }

    info!("Shutting down context...");
    context.shutdown().await?;
    Ok(())
}

// Test get last checkpoints from subnet smart contract
#[rstest]
#[test(tokio::test)]
#[serial]
async fn test_subnet_certificate_get_checkpoints_call(
    #[with(8546)]
    #[future]
    context_running_subnet_node: Context,
) -> Result<(), Box<dyn std::error::Error>> {
    use topos_core::api::grpc::checkpoints;
    let context = context_running_subnet_node.await;
    let subnet_smart_contract_address =
        "0x".to_string() + &hex::encode(context.i_topos_core.address());
    let subnet_jsonrpc_http = context.jsonrpc();

    // Get checkpoints when contract is empty
    let subnet_client = topos_sequencer_subnet_client::SubnetClient::new(
        &subnet_jsonrpc_http,
        Some(hex::decode(TEST_SECRET_ETHEREUM_KEY).unwrap()),
        &subnet_smart_contract_address,
    )
    .await
    .expect("Valid subnet client");
    let target_stream_positions = match subnet_client.get_checkpoints(&TARGET_SUBNET_ID_1).await {
        Ok(result) => result,
        Err(e) => {
            panic!("Unable to get latest certificate id and position: {e}");
        }
    };
    assert_eq!(
        target_stream_positions,
        Vec::<checkpoints::TargetStreamPosition>::new()
    );

    let test_certificates = vec![
        (
            Certificate {
                source_subnet_id: SOURCE_SUBNET_ID_1,
                id: CERTIFICATE_ID_1,
                prev_id: PREV_CERTIFICATE_ID_1,
                target_subnets: vec![TARGET_SUBNET_ID_1],
                ..Default::default()
            },
            0,
        ),
        (
            Certificate {
                source_subnet_id: SOURCE_SUBNET_ID_2,
                id: CERTIFICATE_ID_2,
                prev_id: PREV_CERTIFICATE_ID_2,
                target_subnets: vec![TARGET_SUBNET_ID_1],
                ..Default::default()
            },
            0,
        ),
        (
            Certificate {
                source_subnet_id: SOURCE_SUBNET_ID_1,
                id: CERTIFICATE_ID_3,
                prev_id: CERTIFICATE_ID_1,
                target_subnets: vec![TARGET_SUBNET_ID_1],
                ..Default::default()
            },
            1,
        ),
    ];

    for (test_cert, test_cert_position) in test_certificates.iter() {
        info!("Pushing certificate id={}", test_cert.id);
        match subnet_client
            .push_certificate(test_cert, *test_cert_position as u64)
            .await
        {
            Ok(_) => {
                info!("Certificate id={} pushed", test_cert.id);
            }
            Err(e) => {
                panic!("Unable to push certificate: {e}");
            }
        }
    }

    info!("Getting latest checkpoints ");
    let target_stream_positions = match subnet_client.get_checkpoints(&TARGET_SUBNET_ID_1).await {
        Ok(result) => result,
        Err(e) => {
            panic!("Unable to get the latest certificate id and position: {e}");
        }
    };

    let expected_positions = vec![
        TargetStreamPosition {
            target_subnet_id: TARGET_SUBNET_ID_1,
            source_subnet_id: SOURCE_SUBNET_ID_1,
            certificate_id: Some(CERTIFICATE_ID_3),
            position: 1,
        },
        TargetStreamPosition {
            target_subnet_id: TARGET_SUBNET_ID_1,
            source_subnet_id: SOURCE_SUBNET_ID_2,
            certificate_id: Some(CERTIFICATE_ID_2),
            position: 0,
        },
    ]
    .into_iter()
    .collect::<HashSet<TargetStreamPosition>>();

    assert_eq!(
        target_stream_positions
            .into_iter()
            .collect::<std::collections::HashSet<TargetStreamPosition>>(),
        expected_positions
    );

    info!("Shutting down context...");
    context.shutdown().await?;
    Ok(())
}

// Test get subnet id from subnet smart contract
#[rstest]
#[test(tokio::test)]
#[serial]
async fn test_subnet_id_call(
    #[with(8546)]
    #[future]
    context_running_subnet_node: Context,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_subnet_node.await;
    let subnet_smart_contract_address =
        "0x".to_string() + &hex::encode(context.i_topos_core.address());
    let subnet_jsonrpc_http = context.jsonrpc();

    // Create subnet client
    let subnet_client = topos_sequencer_subnet_client::SubnetClient::new(
        &subnet_jsonrpc_http,
        Some(hex::decode(TEST_SECRET_ETHEREUM_KEY).unwrap()),
        &subnet_smart_contract_address,
    )
    .await
    .expect("Valid subnet client");

    // Get subnet id
    let retrieved_subnet_id = match subnet_client.get_subnet_id().await {
        Ok(result) => {
            info!("Retrieved subnet id {result}");
            result
        }
        Err(e) => {
            panic!("Unable to get subnet id: {e}");
        }
    };

    let expected_subnet_id: SubnetId = hex::decode(TEST_SUBNET_ID)
        .unwrap()
        .as_slice()
        .try_into()
        .unwrap();
    assert_eq!(retrieved_subnet_id, expected_subnet_id);

    info!("Shutting down context...");
    context.shutdown().await?;
    Ok(())
}

// Test perform send token and check for transaction
// in the certificate (by observing target subnets)
#[rstest]
#[test(tokio::test)]
#[serial]
async fn test_subnet_send_token_processing(
    #[with(8546)]
    #[future]
    context_running_subnet_node: Context,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_subnet_node.await;
    let test_private_key = hex::decode(TEST_SECRET_ETHEREUM_KEY).unwrap();
    let subnet_jsonrpc_http = context.jsonrpc();
    let subnet_smart_contract_address =
        "0x".to_string() + &hex::encode(context.i_topos_core.address());

    // Create runtime proxy worker
    info!("Creating subnet runtime proxy");
    let mut runtime_proxy_worker = SubnetRuntimeProxyWorker::new(
        SubnetRuntimeProxyConfig {
            subnet_id: SOURCE_SUBNET_ID_1,
            http_endpoint: context.jsonrpc(),
            ws_endpoint: context.jsonrpc_ws(),
            subnet_contract_address: subnet_smart_contract_address.clone(),
            verifier: 0,
            source_head_certificate_id: None,
            start_block: None,
        },
        test_private_key.clone(),
    )
    .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    info!("Set source head certificate to 0");
    if let Err(e) = runtime_proxy_worker
        .set_source_head_certificate_id(Some((CERTIFICATE_ID_1, 0)))
        .await
    {
        panic!("Unable to set source head certificate id: {e}");
    }

    // Deploy token contract
    let i_erc20 = deploy_test_token(
        &hex::encode(&test_private_key),
        &subnet_jsonrpc_http,
        context.i_topos_messaging.address(),
    )
    .await?;

    // Approve token spending
    if let Err(e) = i_erc20
        .approve(context.i_topos_messaging.address(), U256::from(10))
        .legacy()
        .gas(DEFAULT_GAS)
        .send()
        .await?
        .await
    {
        panic!("Unable to perform token approval {e}");
    }

    // Perform send token
    info!("Sending token");
    if let Err(e) = context
        .i_erc20_messaging
        .send_token(
            TARGET_SUBNET_ID_2.into(),
            TOKEN_SYMBOL.into(),
            "00000000000000000000000000000000000000AA".parse()?,
            U256::from(2),
        )
        .legacy()
        .gas(DEFAULT_GAS)
        .send()
        .await?
        .await
    {
        panic!("Unable to send token: {e}");
    };
    info!("Waiting for certificate with send token transaction...");
    let assertion = async move {
        while let Ok(event) = runtime_proxy_worker.next_event().await {
            if let SubnetRuntimeProxyEvent::NewCertificate {
                cert,
                block_number: _,
                ctx: _,
            } = event
            {
                info!(
                    "New certificate event received, cert id: {} target subnets: {:?}",
                    cert.id, cert.target_subnets
                );
                if cert.target_subnets.len() == 1 && cert.target_subnets == vec![TARGET_SUBNET_ID_2]
                {
                    info!(
                        "Received certificate with requested target subnet {}",
                        cert.target_subnets[0]
                    );
                    return Ok::<(), Box<dyn std::error::Error>>(());
                }
            }
        }
        panic!("Expected event not received");
    };

    // Set big timeout to prevent flaky fails. Instead fail/panic early in the test to indicate actual error
    if tokio::time::timeout(std::time::Duration::from_secs(10), assertion)
        .await
        .is_err()
    {
        panic!("Timeout waiting for command");
    }

    info!("Shutting down context...");
    context.shutdown().await?;
    Ok(())
}

// Test sync of blocks and generating certificates from genesis block
// and test sync from particular source head received from tce
#[rstest]
#[test(tokio::test)]
#[timeout(std::time::Duration::from_secs(600))]
#[serial]
async fn test_sync_from_genesis_and_particular_source_head(
    #[with(8546)]
    #[future]
    context_running_subnet_node: Context,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_subnet_node.await;
    let test_private_key = hex::decode(TEST_SECRET_ETHEREUM_KEY).unwrap();
    let subnet_jsonrpc_http = context.jsonrpc();
    let subnet_smart_contract_address =
        "0x".to_string() + &hex::encode(context.i_topos_core.address());

    // Wait for some time to simulate network history
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    // Get block height
    let http_provider = Provider::<Http>::try_from(subnet_jsonrpc_http.clone())?
        .interval(std::time::Duration::from_millis(20u64));
    let subnet_height = http_provider.get_block_number().await?.as_u64();

    // Create runtime proxy worker
    info!("Creating subnet runtime proxy");
    let runtime_proxy_worker = SubnetRuntimeProxyWorker::new(
        SubnetRuntimeProxyConfig {
            subnet_id: SOURCE_SUBNET_ID_1,
            http_endpoint: context.jsonrpc(),
            ws_endpoint: context.jsonrpc_ws(),
            subnet_contract_address: subnet_smart_contract_address.clone(),
            verifier: 0,
            source_head_certificate_id: None,
            start_block: None,
        },
        test_private_key.clone(),
    )
    .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    info!("Manually set source head certificate to 0 as TCE is not available");
    if let Err(e) = runtime_proxy_worker
        .set_source_head_certificate_id(None)
        .await
    {
        panic!("Unable to set source head certificate id: {e}");
    }

    info!("Waiting for the certificates from zero until height {subnet_height}...");
    let received_certificates = Arc::new(Mutex::new(Vec::new()));

    // Test sync from genesis block
    {
        let expected_block_numbers = (0..=subnet_height).collect::<Vec<_>>();
        let mut expected_blocks = Vec::new();
        for height in &expected_block_numbers {
            match http_provider.get_block(*height).await {
                Ok(block_info) => expected_blocks.push(block_info.expect("valid block")),
                Err(e) => {
                    panic!("Unable to get block number {}: {}", height, e);
                }
            }
        }
        let received_certificates = received_certificates.clone();

        // Set big timeout to prevent flaky fails. Instead fail/panic early in the test to indicate actual error
        if tokio::time::timeout(
            std::time::Duration::from_secs(60),
            check_received_certificate(
                runtime_proxy_worker,
                received_certificates,
                expected_block_numbers,
                expected_blocks,
            ),
        )
        .await
        .is_err()
        {
            panic!("Timeout waiting for command");
        }
    }

    //---------------------------------------------------------------------
    // Now, instantiate new subnet runtime and sync from known position to
    // test sync from particular point
    //---------------------------------------------------------------------
    //
    // Get block height
    let http_provider = Provider::<Http>::try_from(subnet_jsonrpc_http)?
        .interval(std::time::Duration::from_millis(20u64));
    let subnet_height = http_provider.get_block_number().await?.as_u64();
    const SYNC_START_BLOCK_NUMBER: u64 = 11;

    // Create second runtime proxy worker
    info!("Creating second subnet runtime proxy worker");
    let runtime_proxy_worker_2 = SubnetRuntimeProxyWorker::new(
        SubnetRuntimeProxyConfig {
            subnet_id: SOURCE_SUBNET_ID_1,
            http_endpoint: context.jsonrpc(),
            ws_endpoint: context.jsonrpc_ws(),
            subnet_contract_address: subnet_smart_contract_address.clone(),
            verifier: 0,
            source_head_certificate_id: None,
            start_block: None,
        },
        test_private_key.clone(),
    )
    .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    info!(
        "Manually set source head certificate to certificate from block {}",
        SYNC_START_BLOCK_NUMBER
    );
    let last_certificate_retrieved =
        received_certificates.lock().await[SYNC_START_BLOCK_NUMBER as usize - 1].clone();
    received_certificates.lock().await.clear();
    if let Err(e) = runtime_proxy_worker_2
        .set_source_head_certificate_id(Some((
            last_certificate_retrieved.1.id,
            last_certificate_retrieved.0,
        )))
        .await
    {
        panic!("Unable to set source head certificate id: {e}");
    }

    // Test sync from 11 block
    {
        let expected_block_numbers = (SYNC_START_BLOCK_NUMBER..=subnet_height).collect::<Vec<_>>();
        let mut expected_blocks = Vec::new();
        for height in &expected_block_numbers {
            match http_provider.get_block(*height).await {
                Ok(block_info) => expected_blocks.push(block_info.expect("valid block")),
                Err(e) => {
                    panic!("Unable to get block number {}: {}", height, e);
                }
            }
        }
        let received_certificates = Arc::new(Mutex::new(Vec::new()));

        // Set big timeout to prevent flaky fails. Instead fail/panic early in the test to indicate actual error
        if tokio::time::timeout(
            std::time::Duration::from_secs(60),
            check_received_certificate(
                runtime_proxy_worker_2,
                received_certificates,
                expected_block_numbers,
                expected_blocks,
            ),
        )
        .await
        .is_err()
        {
            panic!("Timeout waiting for command");
        }
    }

    info!("Shutting down context...");

    context.shutdown().await?;
    Ok(())
}

// Test sync of blocks and generating certificates start block parameter
#[rstest]
#[test(tokio::test)]
#[timeout(std::time::Duration::from_secs(600))]
#[serial]
async fn test_sync_from_start_block(
    #[with(8546)]
    #[future]
    context_running_subnet_node: Context,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_subnet_node.await;
    let test_private_key = hex::decode(TEST_SECRET_ETHEREUM_KEY).unwrap();
    let subnet_jsonrpc_http = context.jsonrpc();
    let subnet_smart_contract_address =
        "0x".to_string() + &hex::encode(context.i_topos_core.address());

    // Wait for some time to simulate network history
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    // Get block height
    let http_provider = Provider::<Http>::try_from(subnet_jsonrpc_http.clone())?
        .interval(std::time::Duration::from_millis(20u64));
    let subnet_height = http_provider.get_block_number().await?.as_u64();

    // Define start block as current subnet height reduced by 5
    let start_block: u64 = subnet_height - 5;

    // Create runtime proxy worker
    info!("Creating subnet runtime proxy");
    let runtime_proxy_worker = SubnetRuntimeProxyWorker::new(
        SubnetRuntimeProxyConfig {
            subnet_id: SOURCE_SUBNET_ID_1,
            http_endpoint: context.jsonrpc(),
            ws_endpoint: context.jsonrpc_ws(),
            subnet_contract_address: subnet_smart_contract_address.clone(),
            verifier: 0,
            source_head_certificate_id: None,
            start_block: Some(start_block),
        },
        test_private_key.clone(),
    )
    .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    info!("Manually set source head certificate to 0 as TCE is not available");
    if let Err(e) = runtime_proxy_worker
        .set_source_head_certificate_id(None)
        .await
    {
        panic!("Unable to set source head certificate id: {e}");
    }

    info!(
        "Syncing from the start block {} to current height {}",
        start_block, subnet_height
    );

    let received_certificates = Arc::new(Mutex::new(Vec::new()));

    // Test sync from start block
    {
        let expected_block_numbers = (start_block..=subnet_height).collect::<Vec<_>>();
        let mut expected_blocks = Vec::new();
        for height in &expected_block_numbers {
            match http_provider.get_block(*height).await {
                Ok(block_info) => expected_blocks.push(block_info.expect("valid block")),
                Err(e) => {
                    panic!("Unable to get block number {}: {}", height, e);
                }
            }
        }
        let received_certificates = received_certificates.clone();

        // Set big timeout to prevent flaky fails. Instead fail/panic early in the test to indicate actual error
        if tokio::time::timeout(
            std::time::Duration::from_secs(60),
            check_received_certificate(
                runtime_proxy_worker,
                received_certificates,
                expected_block_numbers,
                expected_blocks,
            ),
        )
        .await
        .is_err()
        {
            panic!("Timeout waiting for command");
        }
    }

    info!("Shutting down context...");

    context.shutdown().await?;
    Ok(())
}

// Test multiple send token events in a block
// Test is slow, block time is 12 seconds
#[rstest]
#[test(tokio::test)]
#[timeout(std::time::Duration::from_secs(600))]
#[serial]
async fn test_subnet_multiple_send_token_in_a_block(
    #[with(8546, STANDALONE_SUBNET_WITH_LONG_BLOCKS_BLOCK_TIME)]
    #[future]
    context_running_subnet_node: Context,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_subnet_node.await;
    let test_private_key = hex::decode(TEST_SECRET_ETHEREUM_KEY).unwrap();
    let subnet_jsonrpc_http = context.jsonrpc();
    let subnet_smart_contract_address =
        "0x".to_string() + &hex::encode(context.i_topos_core.address());
    let number_of_send_token_transactions: usize = 4;

    warn!("Block time is intentionally long, this is slow test...");

    // Create runtime proxy worker
    info!("Creating subnet runtime proxy");
    let mut runtime_proxy_worker = SubnetRuntimeProxyWorker::new(
        SubnetRuntimeProxyConfig {
            subnet_id: SOURCE_SUBNET_ID_1,
            http_endpoint: context.jsonrpc(),
            ws_endpoint: context.jsonrpc_ws(),
            subnet_contract_address: subnet_smart_contract_address.clone(),
            verifier: 0,
            source_head_certificate_id: None,
            start_block: None,
        },
        test_private_key.clone(),
    )
    .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    info!("Set source head certificate to 0");
    if let Err(e) = runtime_proxy_worker
        .set_source_head_certificate_id(Some((CERTIFICATE_ID_1, 0)))
        .await
    {
        panic!("Unable to set source head certificate id: {e}");
    }

    // Deploy token contract
    let i_erc20 = deploy_test_token(
        &hex::encode(&test_private_key),
        &subnet_jsonrpc_http,
        context.i_topos_messaging.address(),
    )
    .await?;

    info!("Reading balance of the main account...");
    match i_erc20.balance_of(TEST_ETHEREUM_ACCOUNT.parse()?).await {
        Ok(balance) => {
            info!("Balance of admin account is {:?}", balance);
        }
        Err(e) => {
            error!("Unable to read balance {e}");
        }
    }

    // Send token to other addresses
    let test_accounts: Vec<_> = vec![
        TEST_ACCOUNT_ALITH_ACCOUNT.parse()?,
        TEST_ACCOUNT_BALATHAR_ACCOUNT.parse()?,
        TEST_ACCOUNT_CEZAR_ACCOUNT.parse()?,
    ];
    info!("Transferring tokens to {} accounts", test_accounts.len());
    for test_account in test_accounts {
        info!(
            "Transferring token to address {}",
            "0x".to_string() + &hex::encode(test_account)
        );
        if let Err(e) = i_erc20
            .transfer(test_account, U256::from(10))
            .legacy()
            .gas(DEFAULT_GAS)
            .send()
            .await?
            .await
        {
            panic!("Unable to perform token transfer {e}");
        }
    }
    info!("Tokens transferred");

    let mut erc20_clients = vec![
        create_new_erc20_client(
            TEST_SECRET_ETHEREUM_KEY,
            &subnet_jsonrpc_http,
            i_erc20.address(),
        )
        .await
        .expect("Valid erc20 client"),
        create_new_erc20_client(
            TEST_ACCOUNT_ALITH_KEY,
            &subnet_jsonrpc_http,
            i_erc20.address(),
        )
        .await
        .expect("Valid erc20 client"),
        create_new_erc20_client(
            TEST_ACCOUNT_BALATHAR_KEY,
            &subnet_jsonrpc_http,
            i_erc20.address(),
        )
        .await
        .expect("Valid erc20 client"),
        create_new_erc20_client(
            TEST_ACCOUNT_CEZAR_KEY,
            &subnet_jsonrpc_http,
            i_erc20.address(),
        )
        .await
        .expect("Valid erc20 client"),
    ];

    info!("Approve token spending");
    for erc20_client in &mut erc20_clients {
        if let Err(e) = erc20_client
            .approve(context.i_topos_messaging.address(), U256::from(10))
            .legacy()
            .gas(DEFAULT_GAS)
            .send()
            .await?
            .await
        {
            panic!("Unable to perform token approval {e}");
        } else {
            info!("Token spending approved for {}", erc20_client.address());
        }
    }
    info!("All token spending approved");

    info!("Initializing multiple i_erc20_messaging subnet clients");
    let mut target_subnets = vec![
        (
            TARGET_SUBNET_ID_5,
            create_new_erc20msg_client(
                TEST_SECRET_ETHEREUM_KEY,
                &subnet_jsonrpc_http,
                context.i_erc20_messaging.address(),
            )
            .await
            .expect("Valid client"),
        ),
        (
            TARGET_SUBNET_ID_4,
            create_new_erc20msg_client(
                TEST_ACCOUNT_ALITH_KEY,
                &subnet_jsonrpc_http,
                context.i_erc20_messaging.address(),
            )
            .await
            .expect("Valid client"),
        ),
        (
            TARGET_SUBNET_ID_3,
            create_new_erc20msg_client(
                TEST_ACCOUNT_BALATHAR_KEY,
                &subnet_jsonrpc_http,
                context.i_erc20_messaging.address(),
            )
            .await
            .expect("Valid client"),
        ),
        (
            TARGET_SUBNET_ID_2,
            create_new_erc20msg_client(
                TEST_ACCOUNT_CEZAR_KEY,
                &subnet_jsonrpc_http,
                context.i_erc20_messaging.address(),
            )
            .await
            .expect("Valid client"),
        ),
    ];

    // Perform multiple send token actions
    info!("Sending multiple transactions in parallel");
    let mut handles = Vec::new();
    for i in 1..=number_of_send_token_transactions {
        let (target_subnet, i_erc20_messaging) = target_subnets.pop().unwrap();
        let i_erc20_messaging_address = i_erc20_messaging.address();
        let handle = tokio::spawn(async move {
            info!(
                "Sending transaction {} to target subnet {} erc20 messaging account {}",
                i,
                &target_subnet,
                "0x".to_string() + &hex::encode(i_erc20_messaging_address)
            );
            if let Err(e) = i_erc20_messaging
                .send_token(
                    target_subnet.into(),
                    TOKEN_SYMBOL.into(),
                    "00000000000000000000000000000000000000AA".parse().unwrap(),
                    U256::from(i),
                )
                .legacy()
                .gas(DEFAULT_GAS)
                .send()
                .await
                .map_err(|e| {
                    error!("Unable to send token, contract error: {e}");
                })
                .unwrap()
                .await
            {
                error!("Unable to send token {e}");
                panic!("Unable to send token: {e}");
            };
            info!("Transaction {} sent", i);
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.await.expect("Send token task correctly finished");
    }
    info!("All token transactions sent!");

    info!("Waiting for certificate with send token transaction...");
    let mut received_certificates = Vec::new();
    let assertion = async move {
        while let Ok(event) = runtime_proxy_worker.next_event().await {
            if let SubnetRuntimeProxyEvent::NewCertificate {
                cert,
                block_number,
                ctx: _,
            } = event
            {
                info!(
                    "New certificate event received, block number: {} cert id: {} target subnets: \
                     {:?}",
                    block_number, cert.id, cert.target_subnets
                );
                if !cert.target_subnets.is_empty() {
                    received_certificates.push(cert);
                    let target_subnets = received_certificates
                        .iter()
                        .flat_map(|c| c.target_subnets.iter())
                        .collect::<Vec<_>>();
                    if target_subnets.len() == number_of_send_token_transactions {
                        info!("Received all expected target subnets {:?}", target_subnets);
                        return Ok::<(), Box<dyn std::error::Error>>(());
                    }
                }
            } else {
                info!("Received subnet event: {:?}", event);
            }
        }
        panic!("Expected event not received");
    };

    // Set big timeout to prevent flaky failures. Instead fail/panic early in the test to indicate actual error
    if tokio::time::timeout(std::time::Duration::from_secs(120), assertion)
        .await
        .is_err()
    {
        panic!("Timeout waiting for command");
    }

    info!("Shutting down context...");
    context.shutdown().await?;
    Ok(())
}
