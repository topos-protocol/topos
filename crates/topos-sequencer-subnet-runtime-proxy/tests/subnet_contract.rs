use dockertest::{Composition, DockerTest, Image, PullPolicy, Source};
use rstest::*;
use serial_test::serial;
use std::future::Future;
use std::str::FromStr;
use tokio::sync::oneshot;
use topos_core::uci::Certificate;
use web3::transports::http::Http;
use web3::types::{BlockNumber, Log};

mod common;
use common::subnet_test_data::{
    generate_test_keystore_file, SUBNET_ERC20_CONTRACT_ABI, SUBNET_ERC20_CONTRACT_CODE,
    TEST_KEYSTORE_FILE_PASSWORD,
};
use topos_sequencer_subnet_runtime_proxy::{RuntimeProxyConfig, RuntimeProxyWorker};

const SUBNET_ID: u32 = 0x01;
const SUBNET_CONTRACT_JSON_DEFINITION: &'static str = "ToposCoreContract.json";
const SUBNET_CHAIN_ID: u64 = 43;
const SUBNET_RPC_ENDPOINT: &'static str = "http://127.0.0.1:9933/";
const SUBNET_RPC_PORT: u32 = 9933;
const SUBNET_WEBSOCKET_ENDPOINT: &'static str = "ws://127.0.0.1:9944";
const SUBNET_WEBSOCKET_PORT: u32 = 9944;
const TEST_SECRET_ETHEREUM_KEY: &'static str =
    "99B3C12287537E38C90A9219D4CB074A89A16E9CDB20BF85728EBD97C343E342";
const DOCKER_IMAGE: &'static str = "ghcr.io/toposware/substrate-subnet-node";
const DOCKER_IMAGE_TAG: &str = "test";
const SUBNET_STARTUP_DELAY: u64 = 10; // seconds left for subnet startup
const TOPOS_SMART_CONTRACTS_BUILD_PATH: &str = "TOPOS_SMART_CONTRACTS_BUILD_PATH";

async fn deploy_contracts(
    web3_client: &web3::Web3<Http>,
) -> Result<
    (
        web3::contract::Contract<Http>,
        web3::contract::Contract<Http>,
    ),
    Box<dyn std::error::Error>,
> {
    println!("Deploying subnet smart contract...");
    let eth_private_key = secp256k1::SecretKey::from_str(TEST_SECRET_ETHEREUM_KEY)?;

    // Deploy some common ERC20 token contract for test purposes
    let erc20_contract = match web3::contract::Contract::deploy(
        web3_client.eth(),
        SUBNET_ERC20_CONTRACT_ABI.as_bytes(),
    )?
    .confirmations(1)
    .options(web3::contract::Options::with(|opt| {
        opt.gas = Some(100_000_000.into());
    }))
    .sign_with_key_and_execute(
        SUBNET_ERC20_CONTRACT_CODE,
        ("Test Token".to_owned(), "TST".to_owned()),
        &eth_private_key,
        Some(SUBNET_CHAIN_ID),
    )
    .await
    {
        Ok(erc20_contract) => {
            println!(
                "ERC20 contract deployment ok, new contract address: {}",
                erc20_contract.address()
            );
            erc20_contract
        }
        Err(e) => {
            println!("ERC20 contract deployment error {}", e);
            return Err(Box::from(e));
        }
    };

    // Deploy subnet smart contract (currently topos core contract)
    let contract_file_path = match std::env::var(TOPOS_SMART_CONTRACTS_BUILD_PATH) {
        Ok(path) => path + "/" + SUBNET_CONTRACT_JSON_DEFINITION,
        Err(_e) => {
            println!("Error reading contract build path from `TOPOS_SMART_CONTRACTS_BUILD_PATH` environment variable, using current folder {}",
                     std::env::current_dir().unwrap().display());
            String::from(SUBNET_CONTRACT_JSON_DEFINITION)
        }
    };
    println!(
        "Parsing smart contract contract file {}",
        contract_file_path
    );
    let contract_file = std::fs::File::open(contract_file_path).unwrap();
    let core_contract_json: serde_json::Value =
        serde_json::from_reader(contract_file).expect("error while reading or parsing");
    let subnet_contract_abi = core_contract_json.get("abi").unwrap().to_string();
    let subnet_bytecode = core_contract_json
        .get("bytecode")
        .unwrap()
        .to_string()
        .replace('"', "");
    // Deploy contract, check result
    match web3::contract::Contract::deploy(web3_client.eth(), subnet_contract_abi.as_bytes())?
        .confirmations(1)
        .options(web3::contract::Options::with(|opt| {
            opt.gas = Some(100_000_000.into());
        }))
        .sign_with_key_and_execute(
            subnet_bytecode,
            web3::types::U256::from(SUBNET_ID),
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
            Ok((subnet_smart_contract, erc20_contract))
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
        let mut substrate_subnet_node = Composition::with_image(img);
        substrate_subnet_node
            .port_map(SUBNET_RPC_PORT, SUBNET_RPC_PORT)
            .port_map(SUBNET_WEBSOCKET_PORT, SUBNET_WEBSOCKET_PORT);
        let mut substrate_subnet_node_docker = DockerTest::new().with_default_source(source);
        substrate_subnet_node_docker.add_composition(substrate_subnet_node);
        substrate_subnet_node_docker.run(|ops| async move {
            let container = ops.handle(DOCKER_IMAGE);
            println!(
                "Running container with id: {} name: {} ...",
                container.id(),
                container.name()
            );
            // TODO: use polling of network block number or some other means to learn when sunbstrate node has started
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
    // Web3 client for setup/mocking of contracts
    let http = web3::transports::http::Http::new(SUBNET_RPC_ENDPOINT).unwrap();
    let web3_client = web3::Web3::new(http);
    // Wait for subnet node to start
    subnet_ready_receiver
        .await
        .expect("subnet ready channel error");
    // Perform subnet smart contract deployment
    let (subnet_contract, erc20_contract) = match deploy_contracts(&web3_client).await.map_err(
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
#[ignore = "needs to be updated to the new smart contract api"]
async fn test_subnet_node_contract_deployment(
    context_running_subnet_node: impl Future<Output = Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_subnet_node.await;
    println!("Subnet running in the background with deployed contract");
    context.shutdown().await?;
    println!("Subnet node test finished");
    Ok(())
}

/// Test subxt client RPC connection to subnet
#[rstest]
#[tokio::test]
#[serial]
#[ignore = "needs to be updated to the new smart contract api"]
async fn test_subnet_node_get_nonce(
    context_running_subnet_node: impl Future<Output = Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    //Context with subnet
    let context = context_running_subnet_node.await;
    let eth_private_key = hex::decode(TEST_SECRET_ETHEREUM_KEY)?;
    let eth_address =
        topos_sequencer_subxt_client::subnet_contract::derive_eth_address(&eth_private_key)?;
    match topos_sequencer_subxt_client::Subxt::new(
        SUBNET_WEBSOCKET_ENDPOINT.as_ref(),
        eth_private_key,
    )
    .await
    {
        Ok(subxt) => {
            let nonce = subxt.get_eth_nonce(&eth_address).await?;
            assert_eq!(nonce, 2);
        }
        Err(e) => {
            eprintln!("Unable to get nonce, error {}", e);
            panic!("Unable to get eth nonce");
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
#[ignore = "needs to be updated to the new smart contract api"]
async fn test_create_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let keystore_file_path = generate_test_keystore_file()?;
    println!("Creating runtime proxy...");
    let runtime_proxy_worker = RuntimeProxyWorker::new(RuntimeProxyConfig {
        subnet_id: SUBNET_ID.to_string(),
        endpoint: SUBNET_RPC_ENDPOINT.to_string(),
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
#[ignore = "needs to be updated to the new smart contract api"]
async fn test_subnet_mint_call(
    context_running_subnet_node: impl Future<Output = Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_subnet_node.await;
    let keystore_file_path = generate_test_keystore_file()?;
    let subnet_smart_contract_address =
        "0x".to_string() + &hex::encode(context.subnet_contract.address());
    let runtime_proxy_worker = RuntimeProxyWorker::new(RuntimeProxyConfig {
        subnet_id: SUBNET_ID.to_string(),
        endpoint: SUBNET_WEBSOCKET_ENDPOINT.to_string(),
        subnet_contract: subnet_smart_contract_address.clone(),
        keystore_file: keystore_file_path,
        keystore_password: TEST_KEYSTORE_FILE_PASSWORD.to_string(),
    })?;

    // TODO: Adjust this mock certificate when ToposCoreContract gets stable enough
    let mock_cert = Certificate {
        initial_subnet_id: "1".to_string(),
        cert_id: "9735390752919344387".to_string(),
        prev_cert_id: "3389456612167443923".to_string(),
        calls: vec![topos_core::uci::CrossChainTransaction {
            terminal_subnet_id: SUBNET_ID.to_string(),
            sender_addr: "0x000000000000000000000000000000000000FFFF".to_string(),
            recipient_addr: subnet_smart_contract_address.clone(),
            transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                asset_id: "1".to_string(),
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
        "CertificateApplied(bool)",
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
