use ::topos_core::uci::{Certificate, CertificateId, SubnetId, SUBNET_ID_LENGTH};
use dockertest::{
    Composition, DockerTest, Image, LogAction, LogOptions, LogPolicy, LogSource, PullPolicy, Source,
};
use ethers::{
    abi::{ethabi::ethereum_types::U256, Address, Token},
    contract::abigen,
    core::k256::ecdsa::SigningKey,
    core::types::Filter,
    middleware::SignerMiddleware,
    prelude::Wallet,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
};
use rstest::*;
use serial_test::serial;
use std::collections::HashSet;
use std::sync::Arc;
use test_log::test;
use tokio::sync::oneshot;
use topos_sequencer_subnet_runtime::proxy::{SubnetRuntimeProxyCommand, SubnetRuntimeProxyEvent};
use tracing::{error, info, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

mod common;
use crate::common::subnet_test_data::generate_test_private_key;
use ::topos_core::api::checkpoints::TargetStreamPosition;
use topos_sequencer_subnet_runtime::{SubnetRuntimeProxyConfig, SubnetRuntimeProxyWorker};

use topos_test_sdk::constants::*;

const SUBNET_RPC_PORT: u32 = 8545;
const TEST_SECRET_ETHEREUM_KEY: &str =
    "d7e2e00b43c12cf17239d4755ed744df6ca70a933fc7c8bbb7da1342a5ff2e38";
const POLYGON_EDGE_CONTAINER: &str = "ghcr.io/topos-network/polygon-edge";
const POLYGON_EDGE_CONTAINER_TAG: &str = "develop";
const SUBNET_STARTUP_DELAY: u64 = 5; // seconds left for subnet startup
const TEST_SUBNET_ID: &str = "6464646464646464646464646464646464646464646464646464646464646464";
const ZERO_ADDRESS: &str = "0000000000000000000000000000000000000000";

const PREV_CERTIFICATE_ID_1: CertificateId = CERTIFICATE_ID_4;
const PREV_CERTIFICATE_ID_2: CertificateId = CERTIFICATE_ID_5;
const CERTIFICATE_ID_1: CertificateId = CERTIFICATE_ID_6;
const CERTIFICATE_ID_2: CertificateId = CERTIFICATE_ID_7;
const CERTIFICATE_ID_3: CertificateId = CERTIFICATE_ID_8;
const DEFAULT_GAS: u64 = 5_000_000;

//TODO I haven't find a way to parametrize version, macro accepts strictly string literal
abigen!(TokenDeployerContract, "npm:@topos-network/topos-smart-contracts@1.0.1/artifacts/contracts/topos-core/TokenDeployer.sol/TokenDeployer.json");
abigen!(ToposCoreContract, "npm:@topos-network/topos-smart-contracts@1.0.1/artifacts/contracts/topos-core/ToposCore.sol/ToposCore.json");
abigen!(ToposCoreProxyContract, "npm:@topos-network/topos-smart-contracts@1.0.1/artifacts/contracts/topos-core/ToposCoreProxy.sol/ToposCoreProxy.json");
abigen!(ToposMessagingContract, "npm:@topos-network/topos-smart-contracts@1.0.1/artifacts/contracts/topos-core/ToposMessaging.sol/ToposMessaging.json");
abigen!(IToposCore, "npm:@topos-network/topos-smart-contracts@1.0.1/artifacts/contracts/interfaces/IToposCore.sol/IToposCore.json");
abigen!(IToposMessaging, "npm:@topos-network/topos-smart-contracts@1.0.1/artifacts/contracts/interfaces/IToposMessaging.sol/IToposMessaging.json");
abigen!(
    IERC20,
    r"[
       function totalSupply() external view returns (uint)

       function balanceOf(address account) external view returns (uint)

       function transfer(address recipient, uint amount) external returns (bool)

       function allowance(address owner, address spender) external view returns (uint)

       function approve(address spender, uint amount) external returns (bool)

       function transferFrom(address sender, address recipient, uint amount) external returns (bool)
       ]"
);

type IToposCoreClient = IToposCore<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>;
type IToposMessagingClient = IToposMessaging<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>;
type IERC20Client = IERC20<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>;

fn spawn_subnet_node(
    stop_subnet_receiver: tokio::sync::oneshot::Receiver<()>,
    subnet_ready_sender: tokio::sync::oneshot::Sender<()>,
    port: u32,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
    let handle = tokio::task::spawn_blocking(move || {
        let source = Source::DockerHub;
        let img = Image::with_repository(POLYGON_EDGE_CONTAINER)
            .source(Source::Local)
            .tag(POLYGON_EDGE_CONTAINER_TAG)
            .pull_policy(PullPolicy::IfNotPresent);
        let mut polygon_edge_node = Composition::with_image(img);

        // Define docker options
        polygon_edge_node.port_map(SUBNET_RPC_PORT, port);

        let mut polygon_edge_node_docker = DockerTest::new().with_default_source(source);

        // Setup command for polygon edge binary
        let cmd: Vec<String> = vec!["standalone-test".to_string()];

        polygon_edge_node_docker.add_composition(
            polygon_edge_node
                .with_log_options(Some(LogOptions {
                    action: LogAction::Forward,
                    policy: LogPolicy::OnError,
                    source: LogSource::StdErr,
                }))
                .with_cmd(cmd),
        );
        polygon_edge_node_docker.run(|ops| async move {
            let container = ops.handle(POLYGON_EDGE_CONTAINER);
            info!(
                "Running container with id: {} name: {} ...",
                container.id(),
                container.name(),
            );
            // TODO: use polling of network block number or some other means to learn when subnet node has started
            tokio::time::sleep(tokio::time::Duration::from_secs(SUBNET_STARTUP_DELAY)).await;
            subnet_ready_sender
                .send(())
                .expect("subnet ready channel available");

            info!("Waiting for signal to close...");
            stop_subnet_receiver.await.unwrap();
            info!("Container id={} execution finished", container.id());
        })
    });

    Ok(handle)
}

#[allow(dead_code)]
struct Context {
    pub i_topos_core: IToposCoreClient,
    pub i_topos_messaging: IToposMessagingClient,
    pub subnet_node_handle: Option<tokio::task::JoinHandle<()>>,
    pub subnet_stop_sender: Option<tokio::sync::oneshot::Sender<()>>,
    pub port: u32,
}

impl Context {
    pub async fn shutdown(mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Send shutdown message to subnet node
        self.subnet_stop_sender
            .take()
            .expect("valid subnet stop channel")
            .send(())
            .expect("invalid subnet stop channel");

        // Wait for the subnet node to close
        self.subnet_node_handle
            .take()
            .expect("Valid subnet node handle")
            .await?;
        Ok(())
    }

    pub fn jsonrpc(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    pub fn jsonrpc_ws(&self) -> String {
        format!("ws://127.0.0.1:{}/ws", self.port)
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // TODO: cleanup if necessary
    }
}

async fn deploy_contracts(
    deploy_key: &str,
    endpoint: &str,
) -> Result<(IToposCoreClient, IToposMessagingClient), Box<dyn std::error::Error>> {
    let wallet: LocalWallet = deploy_key.parse()?;
    let http_provider =
        Provider::<Http>::try_from(endpoint)?.interval(std::time::Duration::from_millis(20u64));
    let chain_id = http_provider.get_chainid().await?;
    let client = Arc::new(SignerMiddleware::new(
        http_provider,
        wallet.clone().with_chain_id(chain_id.as_u64()),
    ));

    // Deploying contracts
    info!("Deploying TokenDeployer contract...");
    let token_deployer_contract = TokenDeployerContract::deploy(client.clone(), ())?
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
    let topos_core_contract = ToposCoreContract::deploy(client.clone(), ())?
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
    let admin_account: Token = Token::Array(vec![Token::Address(wallet.address())]);
    let new_admin_threshold: Token = Token::Uint(U256::from(1));
    let topos_core_proxy_encoded_params: ethers::types::Bytes =
        ethers::abi::encode(&[admin_account, new_admin_threshold]).into();

    info!("Deploying ToposCoreProxy contract...");
    let topos_core_proxy_contract = ToposCoreProxyContract::deploy(
        client.clone(),
        (topos_core_contact_address, topos_core_proxy_encoded_params),
    )?
    .gas(DEFAULT_GAS)
    .chain_id(chain_id.as_u64())
    .legacy()
    .send()
    .await?;
    info!(
        "ToposCoreProxy contract deployed to 0x{:x}",
        topos_core_proxy_contract.address()
    );

    info!("Deploying ToposMessaging contract...");
    let topos_messaging_contract = ToposMessagingContract::deploy(
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
        "ToposMessaging contract deployed to 0x{:x}",
        topos_messaging_contract.address()
    );

    let i_topos_messaging =
        IToposMessaging::new(topos_messaging_contract.address(), client.clone());
    let i_topos_core = IToposCore::new(topos_core_proxy_contract.address(), client);

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

    Ok((i_topos_core, i_topos_messaging))
}

async fn deploy_test_token(
    deploy_key: &str,
    endpoint: &str,
    topos_messaging_address: Address,
) -> Result<IERC20Client, Box<dyn std::error::Error>> {
    let wallet: LocalWallet = deploy_key.parse()?;
    let http_provider =
        Provider::<Http>::try_from(endpoint)?.interval(std::time::Duration::from_millis(20u64));
    let chain_id = http_provider.get_chainid().await?;
    let client = Arc::new(SignerMiddleware::new(
        http_provider,
        wallet.clone().with_chain_id(chain_id.as_u64()),
    ));

    let i_topos_messaging = IToposMessaging::new(topos_messaging_address, client.clone());

    // Deploy token
    let token_name: Token = Token::String("Test Token".to_string());
    let token_symbol: Token = Token::String("TKX".to_string());
    let token_mint_cap: Token = Token::Uint(U256::from(100_000_000));
    let token_address_zero: Token = Token::Address(ZERO_ADDRESS.parse()?);
    let token_daily_mint_limit: Token = Token::Uint(U256::from(100));
    let token_initial_supply: Token = Token::Uint(U256::from(10_000_000));
    let token_encoded_params: ethers::types::Bytes = ethers::abi::encode(&[
        token_name.clone(),
        token_symbol.clone(),
        token_mint_cap,
        token_address_zero,
        token_daily_mint_limit,
        token_initial_supply,
    ])
    .into();
    info!(
        "Deploying new token {} with symbol {}",
        token_name, token_symbol
    );
    if let Err(e) = i_topos_messaging
        .deploy_token(token_encoded_params)
        .legacy()
        .gas(DEFAULT_GAS)
        .send()
        .await
        .map_err(|e| {
            error!("Unable deploy token: {e}");
            e
        })?
        .await
    {
        panic!("Error deploying token: {e}");
    };

    let events = i_topos_messaging
        .event::<i_topos_messaging::TokenDeployedFilter>()
        .from_block(0);
    let events = events.query().await?;
    let token_address = events[0].token_address;
    info!("Token contract deploye to {}", token_address.to_string());

    let i_erc20 = IERC20Client::new(token_address, client);

    Ok(i_erc20)
}

#[fixture]
async fn context_running_subnet_node(#[default(8545)] port: u32) -> Context {
    let (subnet_stop_sender, subnet_stop_receiver) = oneshot::channel::<()>();
    let (subnet_ready_sender, subnet_ready_receiver) = oneshot::channel::<()>();
    info!("Starting subnet node...");

    let subnet_node_handle =
        match spawn_subnet_node(subnet_stop_receiver, subnet_ready_sender, port) {
            Ok(subnet_node_handle) => subnet_node_handle,
            Err(e) => {
                panic!("Failed to start the polygon edge subnet node as part of test context: {e}");
            }
        };
    subnet_ready_receiver
        .await
        .expect("subnet ready channel error");
    info!("Subnet node started...");

    // Deploy contracts
    let json_rpc_endpoint = format!("http://127.0.0.1:{port}");
    match deploy_contracts(TEST_SECRET_ETHEREUM_KEY, &json_rpc_endpoint).await {
        Ok((i_topos_core, i_topos_messaging)) => {
            info!("Contracts successfully deployed");
            // Context with subnet container working in the background and ready deployed contracts
            Context {
                i_topos_core,
                i_topos_messaging,
                subnet_node_handle: Some(subnet_node_handle),
                subnet_stop_sender: Some(subnet_stop_sender),
                port,
            }
        }
        Err(e) => {
            panic!("Unable to deploy contracts: {e}");
        }
    }
}

/// Test to start subnet and deploy subnet smart contract
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

// /// Test subnet client RPC connection to subnet
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
        Ok(mut subnet_client) => match subnet_client.get_next_finalized_block().await {
            Ok(block_info) => {
                info!(
                    "Block info successfully retrieved for block {}",
                    block_info.number
                );
                // Blocks must have been mined while we deployed contracts
                assert!(block_info.number > 5);
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

/// Test runtime initialization
#[rstest]
#[test(tokio::test)]
#[serial]
async fn test_create_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let test_private_key = generate_test_private_key();
    info!("Creating runtime proxy...");
    let runtime_proxy_worker = SubnetRuntimeProxyWorker::new(
        SubnetRuntimeProxyConfig {
            subnet_id: SOURCE_SUBNET_ID_1,
            endpoint: format!("localhost:{SUBNET_RPC_PORT}"),
            subnet_contract_address: "0x0000000000000000000000000000000000000000".to_string(),
            verifier: 0,
            source_head_certificate_id: None,
        },
        test_private_key,
    )
    .await?;
    let runtime_proxy = topos_sequencer_subnet_runtime::testing::get_runtime(&runtime_proxy_worker);
    let runtime_proxy = runtime_proxy.lock().await;
    info!("New runtime proxy created:{:?}", &runtime_proxy);
    Ok(())
}

/// Test push certificate to subnet smart contract
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
    let subnet_smart_contract_address =
        "0x".to_string() + &hex::encode(context.i_topos_core.address());
    let runtime_proxy_worker = SubnetRuntimeProxyWorker::new(
        SubnetRuntimeProxyConfig {
            subnet_id: SOURCE_SUBNET_ID_1,
            endpoint: context.jsonrpc(),
            subnet_contract_address: subnet_smart_contract_address.clone(),
            verifier: 0,
            source_head_certificate_id: None,
        },
        test_private_key.clone(),
    )
    .await?;

    let source_subnet_id_1 =
        topos_crypto::keys::derive_public_key(test_private_key.as_slice()).unwrap();

    let mut mock_cert = Certificate {
        source_subnet_id: SubnetId::from_array(
            TryInto::<[u8; SUBNET_ID_LENGTH]>::try_into(&source_subnet_id_1[1..33]).unwrap(),
        ),
        id: CERTIFICATE_ID_1,
        prev_id: PREV_CERTIFICATE_ID_1,
        target_subnets: vec![SOURCE_SUBNET_ID_1],
        ..Default::default()
    };
    mock_cert
        .update_signature(test_private_key.as_slice())
        .expect("valid signature update");

    info!("Sending mock certificate to subnet smart contract...");
    if let Err(e) = runtime_proxy_worker
        .eval(SubnetRuntimeProxyCommand::OnNewDeliveredCertificate {
            certificate: mock_cert.clone(),
            position: 0,
            ctx: Span::current().context(),
        })
        .await
    {
        error!("Failed to send OnNewDeliveredTxns command: {}", e);
        return Err(Box::from(e));
    }

    info!("Waiting for CrossSubnetMessageSent event");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    let provider = Provider::<Http>::try_from(format!("http://127.0.0.1:{}", context.port))?;
    let client = Arc::new(provider);
    let filter = Filter::new()
        .address(context.i_topos_core.address())
        .event("CertStored(bytes32,bytes32)")
        .from_block(0);
    let logs = client.get_logs(&filter).await?;
    if logs.is_empty() {
        panic!("Missing event");
    }

    for log in logs {
        info!(
            "CrossSubnetMessageSent received: block number {:?} from contract {}",
            log.block_number, log.address
        );
        assert_eq!(hex::encode(log.address), subnet_smart_contract_address[2..]);
        let mut expected_data = mock_cert.id.as_array().to_vec();
        expected_data.extend_from_slice(&mock_cert.tx_root_hash);
        assert_eq!(log.data.0, expected_data);
    }

    info!("Shutting down context...");
    context.shutdown().await?;
    Ok(())
}

/// Test get last checkpoints from subnet smart contract
#[rstest]
#[test(tokio::test)]
#[serial]
async fn test_subnet_certificate_get_checkpoints_call(
    #[with(8546)]
    #[future]
    context_running_subnet_node: Context,
) -> Result<(), Box<dyn std::error::Error>> {
    use topos_core::api::checkpoints;
    let context = context_running_subnet_node.await;
    let subnet_smart_contract_address =
        "0x".to_string() + &hex::encode(context.i_topos_core.address());
    let subnet_jsonrpc_endpoint = "http://".to_string() + &context.jsonrpc();

    // Get checkpoints when contract is empty
    let subnet_client = topos_sequencer_subnet_client::SubnetClient::new(
        &subnet_jsonrpc_endpoint,
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

/// Test get subnet id from subnet smart contract
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
    let subnet_jsonrpc_endpoint = "http://".to_string() + &context.jsonrpc();

    // Create subnet client
    let subnet_client = topos_sequencer_subnet_client::SubnetClient::new(
        &subnet_jsonrpc_endpoint,
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

    let expected_subnet_id = hex::decode(TEST_SUBNET_ID)
        .unwrap()
        .as_slice()
        .try_into()
        .unwrap();
    assert_eq!(retrieved_subnet_id, expected_subnet_id);

    info!("Shutting down context...");
    context.shutdown().await?;
    Ok(())
}

/// Test perform send token and check for transaction
/// in the certificate (by observing target subnets)
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
    let subnet_jsonrpc_endpoint = "http://".to_string() + &context.jsonrpc();
    let subnet_smart_contract_address =
        "0x".to_string() + &hex::encode(context.i_topos_core.address());

    // Create runtime proxy worker
    info!("Creating subnet runtime proxy");
    let mut runtime_proxy_worker = SubnetRuntimeProxyWorker::new(
        SubnetRuntimeProxyConfig {
            subnet_id: SOURCE_SUBNET_ID_1,
            endpoint: context.jsonrpc(),
            subnet_contract_address: subnet_smart_contract_address.clone(),
            verifier: 0,
            source_head_certificate_id: None,
        },
        test_private_key.clone(),
    )
    .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    info!("Set source head certificate to 0");
    if let Err(e) = runtime_proxy_worker
        .set_source_head_certificate_id(Some(CERTIFICATE_ID_1))
        .await
    {
        panic!("Unable to set source head certificate id: {e}");
    }

    // Deploy token contract
    let i_erc20 = deploy_test_token(
        &hex::encode(&test_private_key),
        &subnet_jsonrpc_endpoint,
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
        .i_topos_messaging
        .send_token(
            TARGET_SUBNET_ID_2.into(),
            "00000000000000000000000000000000000000AA".parse()?,
            i_erc20.address(),
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
            if let SubnetRuntimeProxyEvent::NewCertificate { cert, ctx: _ } = event {
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
