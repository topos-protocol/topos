use dockertest::{
    Composition, DockerTest, Image, LogAction, LogOptions, LogPolicy, LogSource, PullPolicy, Source,
};
use rstest::*;
use secp256k1::SecretKey;
use serial_test::serial;
use std::collections::HashSet;
use std::str::FromStr;
use test_log::test;
use tokio::sync::oneshot;
use topos_core::uci::{Certificate, CertificateId, SubnetId, SUBNET_ID_LENGTH};
use topos_sequencer_subnet_runtime::proxy::SubnetRuntimeProxyCommand;
use tracing::{error, info, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use web3::contract::tokens::Tokenize;
use web3::ethabi::Token;
use web3::transports::Http;
use web3::types::{BlockNumber, Log, H160, U256};

mod common;
use crate::common::subnet_test_data::generate_test_private_key;
use topos_core::api::checkpoints::TargetStreamPosition;
use topos_sequencer_subnet_runtime::{SubnetRuntimeProxyConfig, SubnetRuntimeProxyWorker};

use topos_test_sdk::constants::*;

const SUBNET_TCC_JSON_DEFINITION: &str = "topos-core/ToposCore.sol/ToposCore.json";
const SUBNET_TCC_PROXY_JSON_DEFINITION: &str = "topos-core/ToposCoreProxy.sol/ToposCoreProxy.json";
const SUBNET_TC_MESSAGING_JSON_DEFINITION: &str =
    "topos-core/ToposMessaging.sol/ToposMessaging.json";
const SUBNET_ITCC_JSON_DEFINITION: &str = "interfaces/IToposCore.sol/IToposCore.json";
const SUBNET_TOKEN_DEPLOYER_JSON_DEFINITION: &str =
    "topos-core/TokenDeployer.sol/TokenDeployer.json";
const SUBNET_CHAIN_ID: u64 = 100;
const SUBNET_RPC_PORT: u32 = 8545;
const TEST_SECRET_ETHEREUM_KEY: &str =
    "d7e2e00b43c12cf17239d4755ed744df6ca70a933fc7c8bbb7da1342a5ff2e38";
const TEST_ETHEREUM_ACCOUNT: &str = "0x4AAb25B4fAd0Beaac466050f3A7142A502f4Cf0a";
const POLYGON_EDGE_CONTAINER: &str = "ghcr.io/topos-network/polygon-edge";
const POLYGON_EDGE_CONTAINER_TAG: &str = "develop";
const SUBNET_STARTUP_DELAY: u64 = 5; // seconds left for subnet startup
const TOPOS_SMART_CONTRACTS_BUILD_PATH_VAR: &str = "TOPOS_SMART_CONTRACTS_BUILD_PATH";
const TEST_SUBNET_ID: &str = "6464646464646464646464646464646464646464646464646464646464646464";

const PREV_CERTIFICATE_ID_1: CertificateId = CERTIFICATE_ID_4;
const PREV_CERTIFICATE_ID_2: CertificateId = CERTIFICATE_ID_5;
const CERTIFICATE_ID_1: CertificateId = CERTIFICATE_ID_6;
const CERTIFICATE_ID_2: CertificateId = CERTIFICATE_ID_7;
const CERTIFICATE_ID_3: CertificateId = CERTIFICATE_ID_8;

async fn deploy_contract<T, U, Z>(
    contract_file_path: &str,
    web3_client: &mut web3::Web3<Http>,
    eth_private_key: &SecretKey,
    params: (Option<T>, Option<U>, Option<Z>),
) -> Result<web3::contract::Contract<Http>, Box<dyn std::error::Error>>
where
    T: Tokenize,
    (T, U): Tokenize,
    (T, U, Z): Tokenize,
{
    info!("Parsing  contract file {}", contract_file_path);
    let contract_file = std::fs::File::open(contract_file_path).unwrap();
    let contract_json: serde_json::Value =
        serde_json::from_reader(contract_file).expect("error while reading or parsing");
    let contract_abi = contract_json.get("abi").unwrap().to_string();
    let contract_bytecode = contract_json
        .get("bytecode")
        .unwrap()
        .to_string()
        .replace('"', "");
    // Deploy contract, check result
    let deployment = web3::contract::Contract::deploy(web3_client.eth(), contract_abi.as_bytes())?
        .confirmations(1)
        .options(web3::contract::Options::with(|opt| {
            opt.gas = Some(5_200_000.into());
        }));

    let deployment_result = if params.0.is_none() {
        // Contract without constructor
        deployment
            .sign_with_key_and_execute(
                contract_bytecode,
                (),
                eth_private_key,
                Some(SUBNET_CHAIN_ID),
            )
            .await
    } else if params.1.is_none() {
        // Contract with 1 arguments
        deployment
            .sign_with_key_and_execute(
                contract_bytecode,
                params.0.unwrap(),
                eth_private_key,
                Some(SUBNET_CHAIN_ID),
            )
            .await
    } else if params.2.is_none() {
        // Contract with 2 arguments
        deployment
            .sign_with_key_and_execute(
                contract_bytecode,
                (params.0.unwrap(), params.1.unwrap()),
                eth_private_key,
                Some(SUBNET_CHAIN_ID),
            )
            .await
    } else {
        // Contract with 3 arguments
        deployment
            .sign_with_key_and_execute(
                contract_bytecode,
                (params.0.unwrap(), params.1.unwrap(), params.2.unwrap()),
                eth_private_key,
                Some(SUBNET_CHAIN_ID),
            )
            .await
    };

    match deployment_result {
        Ok(contract) => {
            info!(
                "Contract {} deployment ok, new contract address: {}",
                contract_file_path,
                contract.address()
            );
            Ok(contract)
        }
        Err(e) => {
            error!("Contract {} deployment error {}", contract_file_path, e);
            Err(Box::<dyn std::error::Error>::from(e))
        }
    }
}

fn get_contract_interface(
    contract_file_path: &str,
    address: H160,
    web3_client: &mut web3::Web3<Http>,
) -> Result<web3::contract::Contract<Http>, Box<dyn std::error::Error>> {
    info!("Parsing  contract file {}", contract_file_path);
    let contract_file = std::fs::File::open(contract_file_path).unwrap();
    let contract_json: serde_json::Value =
        serde_json::from_reader(contract_file).expect("error while reading or parsing");
    let contract_abi = contract_json.get("abi").unwrap().to_string();
    let contract =
        web3::contract::Contract::from_json(web3_client.eth(), address, contract_abi.as_bytes())?;
    Ok(contract)
}

async fn deploy_contracts(
    web3_client: &mut web3::Web3<Http>,
) -> Result<
    (
        web3::contract::Contract<Http>,
        web3::contract::Contract<Http>,
    ),
    Box<dyn std::error::Error>,
> {
    info!("Deploying subnet smart contract...");
    let eth_private_key = secp256k1::SecretKey::from_str(TEST_SECRET_ETHEREUM_KEY)?;

    // Deploy TokenDeployer contract
    info!("Getting Token deployer definition...");
    // Deploy subnet smart contract (currently topos core contract)
    let token_deployer_contract_file_path = match std::env::var(
        TOPOS_SMART_CONTRACTS_BUILD_PATH_VAR,
    ) {
        Ok(path) => path + "/" + SUBNET_TOKEN_DEPLOYER_JSON_DEFINITION,
        Err(_e) => {
            error!("Error reading contract build path from `{TOPOS_SMART_CONTRACTS_BUILD_PATH_VAR}` environment variable, using current folder {}",
                     std::env::current_dir().unwrap().display());
            String::from(SUBNET_TOKEN_DEPLOYER_JSON_DEFINITION)
        }
    };
    let token_deployer_contract = deploy_contract::<u8, u8, u8>(
        &token_deployer_contract_file_path,
        web3_client,
        &eth_private_key,
        (Option::<u8>::None, Option::<u8>::None, Option::<u8>::None),
    )
    .await?;

    // Deploy ToposCore contract
    info!("Getting Topos Core Contract definition...");
    // Deploy subnet smart contract (currently topos core contract)
    let tcc_contract_file_path = match std::env::var(TOPOS_SMART_CONTRACTS_BUILD_PATH_VAR) {
        Ok(path) => path + "/" + SUBNET_TCC_JSON_DEFINITION,
        Err(_e) => {
            error!("Error reading contract build path from `{TOPOS_SMART_CONTRACTS_BUILD_PATH_VAR}` environment variable, using current folder {}",
                     std::env::current_dir().unwrap().display());
            String::from(SUBNET_TCC_JSON_DEFINITION)
        }
    };
    let topos_core_contract = deploy_contract(
        &tcc_contract_file_path,
        web3_client,
        &eth_private_key,
        (Option::<u8>::None, Option::<u8>::None, Option::<u8>::None),
    )
    .await?;

    // Deploy ToposCoreProxy contract
    info!("Getting Topos Core Proxy Contract definition...");
    // Deploy subnet smart contract proxy
    let tcc_proxy_contract_file_path = match std::env::var(TOPOS_SMART_CONTRACTS_BUILD_PATH_VAR) {
        Ok(path) => path + "/" + SUBNET_TCC_PROXY_JSON_DEFINITION,
        Err(_e) => {
            error!("Error reading contract build path from `{TOPOS_SMART_CONTRACTS_BUILD_PATH_VAR}` environment variable, using current folder {}",
                     std::env::current_dir().unwrap().display());
            String::from(SUBNET_TCC_PROXY_JSON_DEFINITION)
        }
    };
    let admin_account: Token = Token::Array(vec![Token::Address(H160::from_slice(
        hex::decode(&TEST_ETHEREUM_ACCOUNT[2..]).unwrap().as_slice(),
    ))]);
    let new_admin_threshold: Token = Token::Uint(U256::from(1));
    let topos_core_proxy_encoded_params =
        web3::ethabi::encode(&[admin_account, new_admin_threshold]);
    let topos_core_proxy_contract = deploy_contract(
        &tcc_proxy_contract_file_path,
        web3_client,
        &eth_private_key,
        (
            Some(topos_core_contract.address()),
            Some(topos_core_proxy_encoded_params),
            Option::<u8>::None,
        ),
    )
    .await?;

    // Deploy ToposMessaging contract
    info!("Getting Topos Messaging Contract definition...");
    // Deploy subnet topos messaging protocol
    let tc_messaging_contract_file_path = match std::env::var(TOPOS_SMART_CONTRACTS_BUILD_PATH_VAR)
    {
        Ok(path) => path + "/" + SUBNET_TC_MESSAGING_JSON_DEFINITION,
        Err(_e) => {
            error!("Error reading contract build path from `{TOPOS_SMART_CONTRACTS_BUILD_PATH_VAR}` environment variable, using current folder {}",
                     std::env::current_dir().unwrap().display());
            String::from(SUBNET_TC_MESSAGING_JSON_DEFINITION)
        }
    };
    let _topos_messaging_contract = deploy_contract(
        &tc_messaging_contract_file_path,
        web3_client,
        &eth_private_key,
        (
            Some(token_deployer_contract.address()),
            Some(topos_core_proxy_contract.address()),
            Option::<u8>::None,
        ),
    )
    .await?;

    // Make interface contract from ToposCoreProxy address
    let tcc_interface_file_path = match std::env::var(TOPOS_SMART_CONTRACTS_BUILD_PATH_VAR) {
        Ok(path) => path + "/" + SUBNET_ITCC_JSON_DEFINITION,
        Err(_e) => {
            error!("Error reading contract build path from `{TOPOS_SMART_CONTRACTS_BUILD_PATH_VAR}` environment variable, using current folder {}",
                     std::env::current_dir().unwrap().display());
            String::from(SUBNET_ITCC_JSON_DEFINITION)
        }
    };
    let topos_core_contract = get_contract_interface(
        tcc_interface_file_path.as_str(),
        topos_core_proxy_contract.address(),
        web3_client,
    )?;

    // Set subnet id on topos core smart contract
    let options = web3::contract::Options {
        gas: Some(5_200_000.into()),
        ..Default::default()
    };
    match topos_core_contract
        .signed_call_with_confirmations(
            "setNetworkSubnetId",
            SOURCE_SUBNET_ID_1.as_array().to_owned(),
            options,
            1_usize,
            &eth_private_key,
        )
        .await
    {
        Ok(v) => {
            info!("Subnet id successfully set on smart contract subnet_id: {SOURCE_SUBNET_ID_1} value {v:?}");
        }
        Err(e) => {
            panic!("Failed to set subnet id on smart contract {e}");
        }
    }

    Ok((topos_core_contract, token_deployer_contract))
}

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

async fn read_logs_for_address(
    contract_address: &str,
    web3_client: &web3::Web3<Http>,
    event_signature: &str,
) -> Result<Vec<Log>, Box<dyn std::error::Error>> {
    let event_topic: [u8; 32] = tiny_keccak::keccak256(event_signature.as_bytes());
    info!(
        "Looking for event signature {} and from address {}",
        hex::encode(event_topic),
        contract_address
    );
    let filter = web3::types::FilterBuilder::default()
        .from_block(BlockNumber::Number(web3::types::U64::from(0)))
        .to_block(BlockNumber::Number(web3::types::U64::from(1000)))
        .address(vec![
            web3::ethabi::Address::from_str(contract_address).unwrap()
        ])
        .topics(
            Some(vec![web3::types::H256::from(event_topic)]),
            None,
            None,
            None,
        )
        .build();
    Ok(web3_client.eth().logs(filter).await?)
}

#[allow(dead_code)]
struct Context {
    pub subnet_contract: web3::contract::Contract<Http>,
    pub erc20_contract: web3::contract::Contract<Http>,
    pub subnet_node_handle: Option<tokio::task::JoinHandle<()>>,
    pub subnet_stop_sender: Option<tokio::sync::oneshot::Sender<()>>,
    pub web3_client: web3::Web3<Http>,
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

    let json_rpc_endpoint = format!("http://127.0.0.1:{port}");

    subnet_ready_receiver
        .await
        .expect("subnet ready channel error");
    info!("Subnet node started...");
    let mut i = 0;
    let http: Http = loop {
        i += 1;
        break match Http::new(&json_rpc_endpoint) {
            Ok(http) => {
                info!("Connected to subnet node...");
                Some(http)
            }
            Err(e) => {
                if i < 10 {
                    error!("Unable to connect to subnet node: {e}");
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }
                None
            }
        };
    }
    .expect("Unable to connect to subnet node");

    let mut web3_client = web3::Web3::new(http);
    // Wait for subnet node to start
    // Perform subnet smart contract deployment
    let (subnet_contract, erc20_contract) =
        match deploy_contracts(&mut web3_client).await.map_err(|e| {
            info!("Failed to deploy subnet contract: {}", e);
            e
        }) {
            Ok(contract) => contract,
            Err(e) => {
                panic!("Failed to deploy subnet contract as part of the test context: {e}");
            }
        };
    // Context with subnet container working in the background and deployed contract ready
    Context {
        subnet_contract,
        erc20_contract,
        subnet_node_handle: Some(subnet_node_handle),
        subnet_stop_sender: Some(subnet_stop_sender),
        web3_client,
        port,
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

/// Test subnet client RPC connection to subnet
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
    let eth_private_key = hex::decode(TEST_SECRET_ETHEREUM_KEY)?;
    let _eth_address =
        topos_sequencer_subnet_client::subnet_contract::derive_eth_address(&eth_private_key)?;
    match topos_sequencer_subnet_client::SubnetClientListener::new(
        &context.jsonrpc_ws(),
        &("0x".to_string() + &hex::encode(context.subnet_contract.address())),
    )
    .await
    {
        Ok(mut subnet_client) => {
            match subnet_client
                .get_next_finalized_block(
                    &("0x".to_string() + &hex::encode(context.subnet_contract.address())),
                )
                .await
            {
                Ok(block_info) => {
                    info!(
                        "Block info successfully retrieved for block {}",
                        block_info.number
                    );
                    assert!(block_info.number > 0 && block_info.number < 100);
                }
                Err(e) => {
                    panic!("Error getting next finalized block {e}");
                }
            }
        }
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
        "0x".to_string() + &hex::encode(context.subnet_contract.address());
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
    info!("Waiting for 10 seconds...");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    info!("Checking logs for event...");
    let logs = read_logs_for_address(
        &subnet_smart_contract_address,
        &context.web3_client,
        "CertStored(bytes32,bytes32)",
    )
    .await?;
    info!("Acquired logs from subnet smart contract: {:#?}", logs);
    assert_eq!(logs.len(), 1);
    assert_eq!(
        hex::encode(logs[0].address),
        subnet_smart_contract_address[2..]
    );
    // event CertStored(CertificateId id, bytes32 txRoot);
    let mut expected_data = mock_cert.id.as_array().to_vec();
    expected_data.extend_from_slice(&mock_cert.tx_root_hash);
    assert_eq!(logs[0].data.0, expected_data);
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
        "0x".to_string() + &hex::encode(context.subnet_contract.address());
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
        "0x".to_string() + &hex::encode(context.subnet_contract.address());
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
