use futures::future::join_all;
use futures::Stream;
use futures::StreamExt;
use libp2p::identity::Keypair;
use libp2p::{Multiaddr, PeerId};
use rstest::*;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::broadcast;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_stream::wrappers::BroadcastStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Channel;
use tonic::Request;
use tonic::Response;
use tonic::Status;

use tonic::transport::server::Router;
use tonic::transport::Server;
use topos_core::api::grpc::tce::v1::{
    api_service_client::ApiServiceClient, console_service_client::ConsoleServiceClient,
    synchronizer_service_server::SynchronizerService as GrpcSynchronizerService,
    synchronizer_service_server::SynchronizerServiceServer,
};
use topos_core::api::grpc::tce::v1::{
    CheckpointRequest, CheckpointResponse, FetchCertificatesRequest, FetchCertificatesResponse,
};
use topos_core::api::grpc::tce::v1::{StatusRequest, StatusResponse};
use topos_core::types::CertificateDelivered;
use topos_core::types::ValidatorId;
use topos_core::uci::SubnetId;
use topos_crypto::messages::MessageSigner;
use topos_p2p::{error::P2PError, Event, GrpcRouter, NetworkClient, Runtime};
use topos_tce::{events::Events, AppContext};
use topos_tce_api::RuntimeContext;
use topos_tce_storage::StorageClient;
use topos_tce_synchronizer::SynchronizerService;
use tracing::{info, warn};

use self::gatekeeper::create_gatekeeper;
use self::p2p::{bootstrap_network, create_network_worker};
use self::protocol::{create_reliable_broadcast_client, create_reliable_broadcast_params};
use self::public_api::create_public_api;
use self::synchronizer::create_synchronizer;
use crate::p2p::local_peer;
use crate::storage::create_fullnode_store;
use crate::storage::create_validator_store;

pub mod gatekeeper;
pub mod p2p;
pub mod protocol;
pub mod public_api;
pub mod synchronizer;

#[derive(Debug)]
pub struct TceContext {
    pub node_config: NodeConfig,
    pub event_stream: mpsc::Receiver<Events>,
    pub peer_id: PeerId, // P2P ID
    pub api_entrypoint: String,
    pub api_grpc_client: ApiServiceClient<Channel>, // GRPC Client for this peer (tce node)
    pub api_context: Option<RuntimeContext>,
    pub console_grpc_client: ConsoleServiceClient<Channel>, // Console TCE GRPC Client for this peer (tce node)
    pub runtime_join_handle: JoinHandle<Result<(), ()>>,
    pub app_join_handle: JoinHandle<()>,
    pub gatekeeper_join_handle: JoinHandle<Result<(), topos_tce_gatekeeper::GatekeeperError>>,
    pub synchronizer_join_handle: JoinHandle<Result<(), topos_tce_synchronizer::SynchronizerError>>,
    pub connected_subnets: Option<Vec<SubnetId>>, // Particular subnet clients (topos nodes) connected to this tce node
    pub shutdown: (CancellationToken, mpsc::Receiver<()>),
}

impl Drop for TceContext {
    fn drop(&mut self) {
        self.app_join_handle.abort();
        self.runtime_join_handle.abort();
        self.gatekeeper_join_handle.abort();
        self.synchronizer_join_handle.abort();
    }
}

impl TceContext {
    pub async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Context performing shutdown...");

        self.shutdown.0.cancel();
        self.shutdown.1.recv().await;

        info!("Shutdown finished...");

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct NodeConfig {
    pub seed: u8,
    pub port: u16,
    pub keypair: Keypair,
    pub addr: Multiaddr,
    pub minimum_cluster_size: usize,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self::from_seed(1)
    }
}

impl NodeConfig {
    pub fn from_seed(seed: u8) -> Self {
        let (keypair, port, addr) = local_peer(seed);

        Self {
            seed,
            port,
            keypair,
            addr,
            minimum_cluster_size: 1,
        }
    }

    pub fn peer_id(&self) -> PeerId {
        self.keypair.public().to_peer_id()
    }

    pub async fn bootstrap(
        &self,
        peers: &[NodeConfig],
        router: Option<GrpcRouter>,
    ) -> Result<
        (
            NetworkClient,
            impl Stream<Item = Event> + Unpin + Send,
            JoinHandle<Result<(), ()>>,
        ),
        Box<dyn Error>,
    > {
        bootstrap_network(
            self.seed,
            self.port,
            self.addr.clone(),
            peers,
            self.minimum_cluster_size,
            router,
        )
        .await
    }

    pub async fn create(
        &self,
        peers: &[NodeConfig],
        router: Option<GrpcRouter>,
    ) -> Result<(NetworkClient, impl Stream<Item = Event>, Runtime), P2PError> {
        create_network_worker(
            self.seed,
            self.port,
            self.addr.clone(),
            peers,
            self.minimum_cluster_size,
            router,
        )
        .await
    }
}

fn default_message_signer() -> Arc<MessageSigner> {
    Arc::new(MessageSigner::new(&[5u8; 32]).unwrap())
}

#[derive(Clone)]
struct DummyService {}

#[async_trait::async_trait]
impl GrpcSynchronizerService for DummyService {
    async fn fetch_certificates(
        &self,
        _request: Request<FetchCertificatesRequest>,
    ) -> Result<Response<FetchCertificatesResponse>, Status> {
        Err(Status::unimplemented("fetch_certificates"))
    }

    async fn fetch_checkpoint(
        &self,
        _request: Request<CheckpointRequest>,
    ) -> Result<Response<CheckpointResponse>, Status> {
        Err(Status::unimplemented("fetch_checkpoint"))
    }
}

pub fn create_dummy_router() -> Router {
    Server::builder().add_service(SynchronizerServiceServer::new(DummyService {}))
}

#[fixture(
    config = NodeConfig::default(),
    peers = &[], certificates = Vec::new(),
    validator_id = ValidatorId::default(),
    validators = HashSet::default(),
    message_signer = default_message_signer())
]
pub async fn start_node(
    certificates: Vec<CertificateDelivered>,
    config: NodeConfig,
    peers: &[NodeConfig],
    validator_id: ValidatorId,
    validators: HashSet<ValidatorId>,
    message_signer: Arc<MessageSigner>,
) -> TceContext {
    let peer_id = config.keypair.public().to_peer_id();
    let fullnode_store = create_fullnode_store(vec![]).await;
    let validator_store =
        create_validator_store(certificates, futures::future::ready(fullnode_store.clone())).await;

    let known_peers = peers
        .iter()
        .map(|p| p.keypair.public().to_peer_id())
        .filter(|&p| p != peer_id)
        .collect::<Vec<_>>();

    let router = GrpcRouter::new(tonic::transport::Server::builder()).add_service(
        SynchronizerServiceServer::new(SynchronizerService {
            validator_store: validator_store.clone(),
        }),
    );

    let (network_client, network_stream, runtime_join_handle) = bootstrap_network(
        config.seed,
        config.port,
        config.addr.clone(),
        peers,
        config.minimum_cluster_size,
        Some(router),
    )
    .await
    .expect("Unable to bootstrap tce network");

    let storage_client = StorageClient::new(validator_store.clone());
    let (sender, receiver) = broadcast::channel(100);
    let (tce_cli, tce_stream) = create_reliable_broadcast_client(
        validator_id,
        validators,
        message_signer,
        create_reliable_broadcast_params(peers.len()),
        validator_store.clone(),
        sender,
    )
    .await;

    let api_storage_client = storage_client.clone();

    let (api_context, api_stream) = create_public_api(
        futures::future::ready(api_storage_client),
        receiver.resubscribe(),
        futures::future::ready(validator_store.clone()),
    )
    .await;

    let (gatekeeper_client, gatekeeper_join_handle) =
        create_gatekeeper(peer_id, known_peers).await.unwrap();

    let (synchronizer_stream, synchronizer_join_handle) = create_synchronizer(
        gatekeeper_client.clone(),
        network_client.clone(),
        validator_store.clone(),
    )
    .await;

    let (app, event_stream) = AppContext::new(
        storage_client,
        tce_cli,
        network_client,
        api_context.client,
        gatekeeper_client,
        validator_store,
    );

    let shutdown_token = CancellationToken::new();
    let shutdown_cloned = shutdown_token.clone();

    let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

    let app_join_handle = spawn(app.run(
        network_stream,
        tce_stream,
        api_stream,
        synchronizer_stream,
        BroadcastStream::new(receiver).filter_map(|v| futures::future::ready(v.ok())),
        (shutdown_token, shutdown_sender),
    ));

    TceContext {
        node_config: config,
        event_stream,
        peer_id,
        api_entrypoint: api_context.entrypoint,
        api_grpc_client: api_context.api_client,
        api_context: api_context.api_context,
        console_grpc_client: api_context.console_client,
        runtime_join_handle,
        app_join_handle,
        gatekeeper_join_handle,
        synchronizer_join_handle,
        connected_subnets: None,
        shutdown: (shutdown_cloned, shutdown_receiver),
    }
}

fn build_peer_config_pool(peer_number: u8) -> Vec<NodeConfig> {
    (1..=peer_number)
        .map(NodeConfig::from_seed)
        .map(|mut c| {
            c.minimum_cluster_size = peer_number as usize / 2;
            c
        })
        .collect()
}

pub async fn start_pool(
    peer_number: u8,
    certificates: Vec<CertificateDelivered>,
) -> HashMap<PeerId, TceContext> {
    let mut clients = HashMap::new();
    let peers = build_peer_config_pool(peer_number);

    let mut validators = Vec::new();
    let mut message_signers = Vec::new();

    for i in 1..=peer_number {
        let message_signer = Arc::new(MessageSigner::new(&[i; 32]).unwrap());
        message_signers.push(message_signer.clone());

        let validator_id = ValidatorId::from(message_signer.public_address);
        validators.push(validator_id);
    }

    let mut await_peers = Vec::new();

    for (i, config) in peers.iter().enumerate() {
        let validator_id = validators[i];
        let signer = message_signers[i].clone();
        let config_cloned = config.clone();
        let certificates_cloned = certificates.clone();
        let peers_cloned = peers.clone();
        let validators_cloned = validators.clone();

        let fut = async move {
            let client = start_node(
                certificates_cloned,
                config_cloned,
                &peers_cloned,
                validator_id,
                validators_cloned
                    .into_iter()
                    .collect::<HashSet<ValidatorId>>(),
                signer,
            )
            .await;

            (client.peer_id, client)
        };
        await_peers.push(fut);
    }

    for (user_peer_id, client) in join_all(await_peers).await {
        clients.insert(user_peer_id, client);
    }

    clients
}

pub async fn create_network(
    peer_number: usize,
    certificates: Vec<CertificateDelivered>,
) -> HashMap<PeerId, TceContext> {
    // List of peers (tce nodes) with their context
    let mut peers_context = start_pool(peer_number as u8, certificates).await;

    // Waiting for new network view
    let mut await_peers = Vec::new();
    for (_peer_id, client) in peers_context.iter_mut() {
        await_peers.push(client.console_grpc_client.status(StatusRequest {}));
    }

    assert!(!join_all(await_peers)
        .await
        .into_iter()
        .map(|res: Result<Response<StatusResponse>, _>| res
            .map(|r: tonic::Response<_>| r.into_inner().has_active_sample))
        .any(|r| r.is_err() || !r.unwrap()));

    warn!("GRPC status received and ok");
    peers_context
}
