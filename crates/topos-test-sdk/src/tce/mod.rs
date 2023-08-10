use std::collections::HashMap;
use std::error::Error;
use std::future::IntoFuture;

use futures::future::join_all;
use futures::Stream;
use libp2p::identity::Keypair;
use libp2p::{Multiaddr, PeerId};
use rstest::*;
use tokio::spawn;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tonic::transport::Channel;
use tonic::Response;
use topos_core::api::grpc::tce::v1::{
    api_service_client::ApiServiceClient, console_service_client::ConsoleServiceClient,
};
use topos_core::api::grpc::tce::v1::{PushPeerListRequest, StatusRequest, StatusResponse};
use topos_core::uci::Certificate;
use topos_core::uci::SubnetId;
use topos_p2p::{error::P2PError, Client, Event, Runtime};
use topos_tce::{events::Events, AppContext};
use topos_tce_api::RuntimeContext;
use tracing::{info, warn};

use crate::p2p::local_peer;
use crate::storage::create_rocksdb;
use crate::wait_for_event;

use self::gatekeeper::create_gatekeeper;
use self::p2p::{bootstrap_network, create_network_worker};
use self::protocol::{create_reliable_broadcast_client, create_reliable_broadcast_params};
use self::public_api::create_public_api;
use self::synchronizer::create_synchronizer;

pub mod gatekeeper;
pub mod p2p;
pub mod protocol;
pub mod public_api;
pub mod synchronizer;

#[derive(Debug)]
pub struct TceContext {
    pub event_stream: mpsc::Receiver<Events>,
    pub peer_id: PeerId, // P2P ID
    pub api_entrypoint: String,
    pub api_grpc_client: ApiServiceClient<Channel>, // GRPC Client for this peer (tce node)
    pub api_context: Option<RuntimeContext>,
    pub console_grpc_client: ConsoleServiceClient<Channel>, // Console TCE GRPC Client for this peer (tce node)
    pub runtime_join_handle: JoinHandle<Result<(), ()>>,
    pub app_join_handle: JoinHandle<()>,
    pub storage_join_handle: JoinHandle<Result<(), topos_tce_storage::errors::StorageError>>,
    pub gatekeeper_join_handle: JoinHandle<Result<(), topos_tce_gatekeeper::GatekeeperError>>,
    pub synchronizer_join_handle: JoinHandle<Result<(), topos_tce_synchronizer::SynchronizerError>>,
    pub connected_subnets: Option<Vec<SubnetId>>, // Particular subnet clients (topos nodes) connected to this tce node
    pub shutdown: (CancellationToken, mpsc::Receiver<()>),
}

impl Drop for TceContext {
    fn drop(&mut self) {
        self.app_join_handle.abort();
        self.runtime_join_handle.abort();
        self.storage_join_handle.abort();
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
    ) -> Result<
        (
            Client,
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
        )
        .await
    }

    pub async fn create(
        &self,
        peers: &[NodeConfig],
    ) -> Result<(Client, impl Stream<Item = Event>, Runtime), P2PError> {
        create_network_worker(
            self.seed,
            self.port,
            self.addr.clone(),
            peers,
            self.minimum_cluster_size,
        )
        .await
    }
}

#[fixture(config = NodeConfig::default(), peers = &[], certificates = Vec::new())]
pub async fn start_node(
    certificates: Vec<Certificate>,
    config: NodeConfig,
    peers: &[NodeConfig],
) -> TceContext {
    let peer_id = config.keypair.public().to_peer_id();
    let peer_id_str = peer_id.to_base58();

    let (network_client, network_stream, runtime_join_handle) = bootstrap_network(
        config.seed,
        config.port,
        config.addr.clone(),
        peers,
        config.minimum_cluster_size,
    )
    .await
    .expect("Unable to bootstrap tce network");

    let (_, (storage, storage_client, storage_stream)) =
        create_rocksdb(&peer_id_str, certificates).await;

    let storage_join_handle = spawn(storage.into_future());

    let (tce_cli, tce_stream) = create_reliable_broadcast_client(
        create_reliable_broadcast_params(peers.len()),
        config.keypair.public().to_peer_id().to_string(),
        storage_client.clone(),
    )
    .await;

    let api_storage_client = storage_client.clone();

    let (api_context, api_stream) =
        create_public_api::partial_1(futures::future::ready(api_storage_client)).await;

    let (gatekeeper_client, gatekeeper_join_handle) = create_gatekeeper(peer_id).await.unwrap();

    let (synchronizer_client, synchronizer_stream, synchronizer_join_handle) =
        create_synchronizer(gatekeeper_client.clone(), network_client.clone()).await;

    let (app, event_stream) = AppContext::new(
        storage_client,
        tce_cli,
        network_client,
        api_context.client,
        gatekeeper_client,
        synchronizer_client,
    );

    let shutdown_token = CancellationToken::new();
    let shutdown_cloned = shutdown_token.clone();

    let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

    let app_join_handle = spawn(app.run(
        network_stream,
        tce_stream,
        api_stream,
        storage_stream,
        synchronizer_stream,
        (shutdown_token, shutdown_sender),
    ));

    TceContext {
        event_stream,
        peer_id,
        api_entrypoint: api_context.entrypoint,
        api_grpc_client: api_context.api_client,
        api_context: api_context.api_context,
        console_grpc_client: api_context.console_client,
        runtime_join_handle,
        app_join_handle,
        storage_join_handle,
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

pub async fn start_pool(peer_number: u8) -> HashMap<PeerId, TceContext> {
    let mut clients = HashMap::new();
    let peers = build_peer_config_pool(peer_number);

    let mut await_peers = Vec::new();

    for config in &peers {
        let fut = async {
            let client = start_node(vec![], config.clone(), &peers).await;

            (client.peer_id, client)
        };

        await_peers.push(fut);
    }

    for (user_peer_id, client) in join_all(await_peers).await {
        clients.insert(user_peer_id, client);
    }

    clients
}

pub async fn create_network(peer_number: usize) -> HashMap<PeerId, TceContext> {
    // List of peers (tce nodes) with their context
    let mut peers_context = start_pool(peer_number as u8).await;
    let all_peers: Vec<PeerId> = peers_context.keys().cloned().collect();

    warn!("Pool created, waiting for peers to connect...");
    // Force TCE nodes to recreate subscriptions and subscribers
    let mut await_peers = Vec::new();
    for (peer_id, client) in peers_context.iter_mut() {
        await_peers.push(
            client
                .console_grpc_client
                .push_peer_list(PushPeerListRequest {
                    request_id: None,
                    peers: all_peers
                        .iter()
                        .filter_map(|key| {
                            if key == peer_id {
                                None
                            } else {
                                Some(key.to_string())
                            }
                        })
                        .collect::<Vec<_>>(),
                }),
        );
    }

    assert!(!join_all(await_peers).await.iter().any(|res| res.is_err()));
    warn!("Peers connected");

    for (peer_id, client) in peers_context.iter_mut() {
        wait_for_event!(
            client.event_stream.recv(),
            matches: Events::StableSample,
            peer_id,
            15000
        );
    }

    warn!("Stable sample received");

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
