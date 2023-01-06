use dockertest::{
    Composition, DockerTest, Image, LogAction, LogOptions, LogPolicy, LogSource, PullPolicy, Source,
};
use fs_extra::dir::{copy, create_all, CopyOptions};
use rstest::*;
use serial_test::serial;
use std::future::Future;
use std::str::FromStr;
use tokio::sync::oneshot;
use topos_core::uci::{Address, Certificate, CertificateId, SubnetId};
use web3::transports::Http;
use web3::types::{BlockNumber, Log};

mod common;
use common::subnet_test_data::{generate_test_keystore_file, TEST_KEYSTORE_FILE_PASSWORD};
use topos_sequencer_subnet_runtime_proxy::{RuntimeProxyConfig, RuntimeProxyWorker};

const SUBNET_TCC_JSON_DEFINITION: &'static str = "ToposCoreContract.json";
const SUBNET_TOKEN_DEPLOYER_JSON_DEFINITION: &'static str = "TokenDeployer.json";
const SUBNET_CHAIN_ID: u64 = 100;
const SUBNET_RPC_PORT: u32 = 8545;
const TOPOS_SUBNET_JSONRPC_ENDPOINT: &'static str = "http://127.0.0.1:8545";
const TOPOS_SUBNET_JSONRPC_ENDPOINT_WS: &'static str = "ws://127.0.0.1:8545/ws";
const TEST_SECRET_ETHEREUM_KEY: &'static str =
    "5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133";
const DOCKER_IMAGE: &'static str = "ghcr.io/toposware/polygon-edge";
const DOCKER_IMAGE_TAG: &str = "develop";
const SUBNET_STARTUP_DELAY: u64 = 10; // seconds left for subnet startup
const TOPOS_SMART_CONTRACTS_BUILD_PATH: &str = "TOPOS_SMART_CONTRACTS_BUILD_PATH";

const SOURCE_SUBNET_ID: SubnetId = [1u8; 32];
const PREV_CERTIFICATE_ID: CertificateId = [4u8; 32];
const CERTIFICATE_ID: CertificateId = [5u8; 32];
const SENDER_ID: Address = [6u8; 20];
const RECEIVER_ID: Address = [7u8; 20];

async fn deploy_contracts(
    web3_client: &mut web3::Web3<Http>,
) -> Result<
    (
        web3::contract::Contract<Http>,
        web3::contract::Contract<Http>,
    ),
    Box<dyn std::error::Error>,
> {
    println!("Deploying subnet smart contract...");
    let eth_private_key = secp256k1::SecretKey::from_str(TEST_SECRET_ETHEREUM_KEY)?;

    println!("Getting Token deployer definition...");
    // Deploy subnet smart contract (currently topos core contract)
    let token_deployer_contract_file_path = match std::env::var(TOPOS_SMART_CONTRACTS_BUILD_PATH) {
        Ok(path) => path + "/" + SUBNET_TOKEN_DEPLOYER_JSON_DEFINITION,
        Err(_e) => {
            println!("Error reading contract build path from `TOPOS_SMART_CONTRACTS_BUILD_PATH` environment variable, using current folder {}",
                     std::env::current_dir().unwrap().display());
            String::from(SUBNET_TOKEN_DEPLOYER_JSON_DEFINITION)
        }
    };
    println!(
        "Parsing token deployer contract file {}",
        token_deployer_contract_file_path
    );
    let token_deployer_contract_file =
        std::fs::File::open(token_deployer_contract_file_path).unwrap();
    let token_deployer_contract_json: serde_json::Value =
        serde_json::from_reader(token_deployer_contract_file)
            .expect("error while reading or parsing");
    let token_deployer_contract_abi = token_deployer_contract_json.get("abi").unwrap().to_string();
    let token_deployer_bytecode = token_deployer_contract_json
        .get("bytecode")
        .unwrap()
        .to_string()
        .replace('"', "");
    // Deploy token deployer contract, check result
    let token_deployer_contract = match web3::contract::Contract::deploy(
        web3_client.eth(),
        token_deployer_contract_abi.as_bytes(),
    )?
    .confirmations(1)
    .options(web3::contract::Options::with(|opt| {
        opt.gas = Some(2_000_000.into());
    }))
    .sign_with_key_and_execute(
        token_deployer_bytecode,
        (),
        &eth_private_key,
        Some(SUBNET_CHAIN_ID),
    )
    .await
    {
        Ok(token_deployer_contract) => {
            println!(
                "Token deployer contract deployment ok, new contract address: {}",
                token_deployer_contract.address()
            );
            Ok(token_deployer_contract)
        }
        Err(e) => {
            println!("Token deployer contract deployment error {}", e);
            Err(Box::<dyn std::error::Error>::from(e))
        }
    }?;

    println!("Getting Topos Core Contract definition...");
    // Deploy subnet smart contract (currently topos core contract)
    let tcc_contract_file_path = match std::env::var(TOPOS_SMART_CONTRACTS_BUILD_PATH) {
        Ok(path) => path + "/" + SUBNET_TCC_JSON_DEFINITION,
        Err(_e) => {
            println!("Error reading contract build path from `TOPOS_SMART_CONTRACTS_BUILD_PATH` environment variable, using current folder {}",
                     std::env::current_dir().unwrap().display());
            String::from(SUBNET_TCC_JSON_DEFINITION)
        }
    };
    println!(
        "Parsing smart contract contract file {}",
        tcc_contract_file_path
    );
    let tcc_contract_file = std::fs::File::open(tcc_contract_file_path).unwrap();
    let tcc_contract_json: serde_json::Value =
        serde_json::from_reader(tcc_contract_file).expect("error while reading or parsing");
    let tcc_contract_abi = tcc_contract_json.get("abi").unwrap().to_string();
    let tcc_bytecode = tcc_contract_json
        .get("bytecode")
        .unwrap()
        .to_string()
        .replace('"', "");
    // Deploy contract, check result
    match web3::contract::Contract::deploy(web3_client.eth(), tcc_contract_abi.as_bytes())?
        .confirmations(1)
        .options(web3::contract::Options::with(|opt| {
            opt.gas = Some(3_000_000.into());
        }))
        .sign_with_key_and_execute(
            tcc_bytecode,
            (
                token_deployer_contract.address(),
                SOURCE_SUBNET_ID.to_owned(),
            ),
            &eth_private_key,
            Some(SUBNET_CHAIN_ID),
        )
        .await
    {
        Ok(subnet_smart_contract) => {
            println!(
                "Subnet contract deployment ok, new contract address: {}",
                subnet_smart_contract.address()
            );
            Ok((subnet_smart_contract, token_deployer_contract))
        }
        Err(e) => {
            println!("Subnet contract deployment error {}", e);
            Err(Box::from(e))
        }
    }
}

fn spawn_subnet_node(
    stop_subnet_receiver: tokio::sync::oneshot::Receiver<()>,
    subnet_ready_sender: tokio::sync::oneshot::Sender<()>,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
    let handle = tokio::task::spawn_blocking(move || {
        let source = Source::DockerHub;

        let img = Image::with_repository(DOCKER_IMAGE)
            .source(Source::Local)
            .tag(DOCKER_IMAGE_TAG)
            .pull_policy(PullPolicy::IfNotPresent);
        let mut polygon_edge_node = Composition::with_image(img);
        let current_dir: String = std::env::current_dir()
            .expect("current_dir")
            .to_str()
            .unwrap()
            .to_string();

        // There is no option in dockertest to change running user, so all blockchain data files
        // will be created with root privileges. So we copy keys and network config to the temp folder
        // that should be removed manually after the test
        let copy_options = CopyOptions {
            overwrite: true,
            copy_inside: true,
            ..Default::default()
        };
        let test_data_dir_name = current_dir.clone()
            + "/tests/temp/data_"
            + &std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("valid system time duration")
                .as_millis()
                .to_string();
        if let Err(err) = create_all(&test_data_dir_name, false) {
            eprintln!("Unable to create temporary test data directory {err}");
        };
        if let Err(err) = copy(
            current_dir.clone() + "/tests/artifacts/test-chain-1",
            test_data_dir_name.clone() + "/test-chain-1",
            &copy_options,
        ) {
            eprintln!("Unable to copy test chain directory {err}");
        };
        if let Err(err) = copy(
            current_dir.clone() + "/tests/artifacts/genesis",
            test_data_dir_name.clone() + "/genesis",
            &copy_options,
        ) {
            eprintln!("Unable to copy genesis directory {err}");
        };

        // Define options
        polygon_edge_node
            .bind_mount(
                test_data_dir_name.clone() + "/test-chain-1",
                "/data/test-chain-1",
            )
            .bind_mount(test_data_dir_name.clone() + "/genesis", "/genesis")
            .port_map(SUBNET_RPC_PORT, SUBNET_RPC_PORT);
        let mut polygon_edge_node_docker = DockerTest::new().with_default_source(source);
        // Setup command for polygon edge binary
        let cmd: Vec<String> = vec![
            "server".to_string(),
            "--data-dir".to_string(),
            "/data/test-chain-1".to_string(),
            "--chain".to_string(),
            "/genesis/genesis.json".to_string(),
        ];
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
            let container = ops.handle(DOCKER_IMAGE);
            println!(
                "Running container with id: {} name: {} ...",
                container.id(),
                container.name()
            );
            // TODO: use polling of network block number or some other means to learn when subnet node has started
            tokio::time::sleep(tokio::time::Duration::from_secs(SUBNET_STARTUP_DELAY)).await;
            subnet_ready_sender
                .send(())
                .expect("subnet ready channel available");
            println!("Waiting for signal to close...");
            stop_subnet_receiver.await.unwrap();
            println!("Container id={} execution finished", container.id());
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
    println!(
        "Looking for event signature {} and from address {}",
        hex::encode(&event_topic),
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
}

impl Drop for Context {
    fn drop(&mut self) {
        // TODO: cleanup if necessary
    }
}

#[fixture]
async fn context_running_subnet_node() -> Context {
    let (subnet_stop_sender, subnet_stop_receiver) = oneshot::channel::<()>();
    let (subnet_ready_sender, subnet_ready_receiver) = oneshot::channel::<()>();
    println!("Starting subnet node...");
    let subnet_node_handle = match spawn_subnet_node(subnet_stop_receiver, subnet_ready_sender) {
        Ok(subnet_node_handle) => subnet_node_handle,
        Err(e) => {
            println!("Failed to start substrate subnet node as part of test context, error details {}, panicking!!!", e);
            panic!("Unable to start substrate subnet node");
        }
    };
    println!("Subnet node started...");
    // Web3 client for setup/mocking of contracts
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    let mut i = 0;
    let http: Http = loop {
        i += 1;
        break match Http::new(TOPOS_SUBNET_JSONRPC_ENDPOINT) {
            Ok(http) => {
                println!("Connected to subnet node...");
                Some(http)
            }
            Err(e) => {
                if i < 10 {
                    eprintln!("Unable to connect to subnet node: {e}");
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
    subnet_ready_receiver
        .await
        .expect("subnet ready channel error");
    // Perform subnet smart contract deployment
    let (subnet_contract, erc20_contract) = match deploy_contracts(&mut web3_client).await.map_err(
        |e| {
            println!("Failed to deploy subnet contract: {}", e);
            e
        },
    ) {
        Ok(contract) => contract,
        Err(e) => {
            println!("Failed to deploy subnet contract as part of the test context, error details {}, panicking!!!", e);
            panic!("Unable to deploy subnet contract");
        }
    };
    // Context with subnet container working in the background and deployed contract ready
    Context {
        subnet_contract,
        erc20_contract,
        subnet_node_handle: Some(subnet_node_handle),
        subnet_stop_sender: Some(subnet_stop_sender),
        web3_client,
    }
}

/// Test to start subnet and deploy subnet smart contract
#[rstest]
#[tokio::test]
#[serial]
async fn test_subnet_node_contract_deployment(
    context_running_subnet_node: impl Future<Output = Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_subnet_node.await;
    println!("Subnet running in the background with deployed contract");
    context.shutdown().await?;
    println!("Subnet node test finished");
    Ok(())
}

/// Test subnet client RPC connection to subnet
#[rstest]
#[tokio::test]
#[serial]
async fn test_subnet_node_get_block_info(
    context_running_subnet_node: impl Future<Output = Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    //Context with subnet
    let context = context_running_subnet_node.await;
    let eth_private_key = hex::decode(TEST_SECRET_ETHEREUM_KEY)?;
    let _eth_address =
        topos_sequencer_subnet_client::subnet_contract::derive_eth_address(&eth_private_key)?;
    match topos_sequencer_subnet_client::SubnetClient::new(
        TOPOS_SUBNET_JSONRPC_ENDPOINT_WS.as_ref(),
        eth_private_key,
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
                    println!(
                        "Block info successfully retrieved for block {}",
                        block_info.number
                    );
                }
                Err(e) => {
                    eprintln!("Error getting next finalized block {e}");
                }
            }
        }
        Err(e) => {
            eprintln!("Unable to get block info, error {}", e);
            panic!("Unable to get block info");
        }
    }
    context.shutdown().await?;
    println!("Subnet node test finished");
    Ok(())
}

/// Test runtime initialization
#[rstest]
#[tokio::test]
#[serial]
async fn test_create_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let keystore_file_path = generate_test_keystore_file()?;
    println!("Creating runtime proxy...");
    let runtime_proxy_worker = RuntimeProxyWorker::new(RuntimeProxyConfig {
        subnet_id: SOURCE_SUBNET_ID,
        endpoint: TOPOS_SUBNET_JSONRPC_ENDPOINT.to_string(),
        subnet_contract: "0x0000000000000000000000000000000000000000".to_string(),
        keystore_file: keystore_file_path,
        keystore_password: TEST_KEYSTORE_FILE_PASSWORD.to_string(),
    })?;
    let runtime_proxy =
        topos_sequencer_subnet_runtime_proxy::testing::get_runtime(&runtime_proxy_worker);
    let runtime_proxy = runtime_proxy.lock();
    println!("New runtime proxy created:{:?}", &runtime_proxy);
    Ok(())
}

/// Test mint call
#[rstest]
#[tokio::test]
#[serial]
async fn test_subnet_certificate_push_call(
    context_running_subnet_node: impl Future<Output = Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_subnet_node.await;
    let keystore_file_path = generate_test_keystore_file()?;
    let subnet_smart_contract_address =
        "0x".to_string() + &hex::encode(context.subnet_contract.address());
    let runtime_proxy_worker = RuntimeProxyWorker::new(RuntimeProxyConfig {
        subnet_id: SOURCE_SUBNET_ID,
        endpoint: TOPOS_SUBNET_JSONRPC_ENDPOINT_WS.to_string(),
        subnet_contract: subnet_smart_contract_address.clone(),
        keystore_file: keystore_file_path,
        keystore_password: TEST_KEYSTORE_FILE_PASSWORD.to_string(),
    })?;

    // TODO: Adjust this mock certificate when ToposCoreContract gets stable enough
    let mock_cert = Certificate {
        source_subnet_id: SOURCE_SUBNET_ID,
        id: CERTIFICATE_ID,
        prev_id: PREV_CERTIFICATE_ID,
        calls: vec![topos_core::uci::CrossChainTransaction {
            target_subnet_id: SOURCE_SUBNET_ID,
            transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                sender: SENDER_ID,
                receiver: RECEIVER_ID,
                symbol: "1".to_string(),
                amount: topos_core::uci::Amount::from(10000),
            },
        }],
    };

    println!("Sending mock certificate to subnet smart contract...");
    if let Err(e) = runtime_proxy_worker
        .eval(topos_sequencer_types::RuntimeProxyCommand::OnNewDeliveredTxns(mock_cert))
    {
        eprintln!("Failed to send OnNewDeliveredTxns command: {}", e);
        return Err(Box::from(e));
    }
    println!("Waiting for 10 seconds...");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    println!("Checking logs for event...");
    let logs = read_logs_for_address(
        &subnet_smart_contract_address,
        &context.web3_client,
        "CertStored(bytes)",
    )
    .await?;
    println!("Acquired logs from subnet smart contract: {:#?}", logs);
    assert_eq!(logs.len(), 1);
    assert_eq!(
        hex::encode(logs[0].address),
        subnet_smart_contract_address[2..]
    );
    println!("Shutting down context...");
    context.shutdown().await?;
    Ok(())
}
