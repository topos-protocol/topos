use super::{Behaviour, Event, NetworkClient, Runtime};
use crate::{
    behaviour::{discovery::DiscoveryBehaviour, gossip, grpc, peer_info::PeerInfoBehaviour},
    config::{DiscoveryConfig, NetworkConfig},
    constants::{
        self, COMMAND_STREAM_BUFFER_SIZE, DISCOVERY_PROTOCOL, EVENT_STREAM_BUFFER,
        PEER_INFO_PROTOCOL,
    },
    error::P2PError,
    utils::GrpcOverP2P,
    GrpcContext,
};
use futures::Stream;
use libp2p::{
    core::upgrade,
    dns::TokioDnsConfig,
    identity::Keypair,
    kad::store::MemoryStore,
    noise,
    swarm::SwarmBuilder,
    tcp::{tokio::Transport, Config},
    Multiaddr, PeerId, Transport as TransportTrait,
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;

pub fn builder<'a>() -> NetworkBuilder<'a> {
    NetworkBuilder::default()
}

const TWO_HOURS: Duration = Duration::from_secs(60 * 60 * 2);

#[derive(Default)]
pub struct NetworkBuilder<'a> {
    discovery_protocol: Option<&'static str>,
    peer_key: Option<Keypair>,
    listen_addresses: Option<Vec<Multiaddr>>,
    public_addresses: Option<Vec<Multiaddr>>,
    store: Option<MemoryStore>,
    known_peers: &'a [(PeerId, Multiaddr)],
    local_port: Option<u8>,
    config: NetworkConfig,
    grpc_context: GrpcContext,
}

impl<'a> NetworkBuilder<'a> {
    pub fn grpc_context(mut self, grpc_context: GrpcContext) -> Self {
        self.grpc_context = grpc_context;

        self
    }

    pub fn discovery_config(mut self, config: DiscoveryConfig) -> Self {
        self.config.discovery = config;

        self
    }

    pub fn publish_retry(mut self, retry: usize) -> Self {
        self.config.publish_retry = retry;

        self
    }

    pub fn minimum_cluster_size(mut self, size: usize) -> Self {
        self.config.minimum_cluster_size = size;

        self
    }

    pub fn peer_key(mut self, peer_key: Keypair) -> Self {
        self.peer_key = Some(peer_key);

        self
    }

    pub fn public_addresses(mut self, addresses: Vec<Multiaddr>) -> Self {
        self.public_addresses = Some(addresses);

        self
    }

    pub fn listen_addresses(mut self, addresses: Vec<Multiaddr>) -> Self {
        self.listen_addresses = Some(addresses);

        self
    }

    pub fn store(mut self, store: MemoryStore) -> Self {
        self.store = Some(store);

        self
    }

    pub fn known_peers(mut self, known_peers: &'a [(PeerId, Multiaddr)]) -> Self {
        self.known_peers = known_peers;

        self
    }

    pub fn local_port(mut self, port: u8) -> Self {
        self.local_port = Some(port);

        self
    }

    pub fn discovery_protocol(mut self, protocol: &'static str) -> Self {
        self.discovery_protocol = Some(protocol);

        self
    }

    pub async fn build(
        mut self,
    ) -> Result<(NetworkClient, impl Stream<Item = Event>, Runtime), P2PError> {
        let peer_key = self.peer_key.ok_or(P2PError::MissingPeerKey)?;
        let peer_id = peer_key.public().to_peer_id();

        let (command_sender, command_receiver) = mpsc::channel(*COMMAND_STREAM_BUFFER_SIZE);
        let (event_sender, event_receiver) = mpsc::channel(*EVENT_STREAM_BUFFER);

        let gossipsub = gossip::Behaviour::new(peer_key.clone()).await;

        let grpc = grpc::Behaviour::new(self.grpc_context);

        let behaviour = Behaviour {
            gossipsub,
            peer_info: PeerInfoBehaviour::new(PEER_INFO_PROTOCOL, &peer_key),
            discovery: DiscoveryBehaviour::create(
                &self.config.discovery,
                peer_key.clone(),
                Cow::Borrowed(
                    self.discovery_protocol
                        .unwrap_or(DISCOVERY_PROTOCOL)
                        .as_bytes(),
                ),
                self.known_peers,
                false,
            ),
            grpc,
        };

        let transport = {
            let dns_tcp =
                TokioDnsConfig::system(Transport::new(Config::new().nodelay(true))).unwrap();

            let tcp = Transport::new(Config::default().nodelay(true));
            dns_tcp.or_transport(tcp)
        };

        let mut multiplex_config = libp2p::yamux::Config::default();
        multiplex_config.set_window_update_mode(libp2p::yamux::WindowUpdateMode::on_read());
        multiplex_config.set_max_buffer_size(1024 * 1024 * 16);

        let transport = transport
            .upgrade(upgrade::Version::V1)
            .authenticate(noise::Config::new(&peer_key)?)
            .multiplex(multiplex_config)
            .timeout(TWO_HOURS)
            .boxed();

        let swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id)
            .idle_connection_timeout(constants::IDLE_CONNECTION_TIMEOUT)
            .build();
        let (shutdown_channel, shutdown) = mpsc::channel::<oneshot::Sender<()>>(1);

        let grpc_over_p2p = GrpcOverP2P::new(command_sender.clone());

        let listen_addr = self
            .listen_addresses
            .take()
            .expect("Node requires at least one address to listen for incoming connections");

        let public_addresses = self
            .public_addresses
            .map(|addresses| {
                if addresses.is_empty() {
                    listen_addr.clone()
                } else {
                    addresses
                }
            })
            .unwrap_or(listen_addr.clone());

        Ok((
            NetworkClient {
                retry_ttl: self.config.client_retry_ttl,
                local_peer_id: peer_id,
                sender: command_sender,
                grpc_over_p2p,
                shutdown_channel,
            },
            ReceiverStream::new(event_receiver),
            Runtime {
                swarm,
                config: self.config,
                peer_set: self.known_peers.iter().map(|(p, _)| *p).collect(),
                command_receiver,
                event_sender,
                local_peer_id: peer_id,
                listening_on: listen_addr,
                public_addresses,
                bootstrapped: false,
                active_listeners: HashSet::new(),
                pending_record_requests: HashMap::new(),
                shutdown,
            },
        ))
    }

    pub fn is_bootnode(mut self, is_bootnode: bool) -> Self {
        self.config.is_bootnode = is_bootnode;

        self
    }
}
