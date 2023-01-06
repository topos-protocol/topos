use super::{Behaviour, Client, Event, Runtime};
use crate::{
    behaviour::{
        discovery::DiscoveryBehaviour, peer_info::PeerInfoBehaviour,
        transmission::TransmissionBehaviour,
    },
    constant::{
        COMMAND_STREAM_BUFFER, DISCOVERY_PROTOCOL, EVENT_STREAM_BUFFER, TRANSMISSION_PROTOCOL,
    },
    error::P2PError,
};
use futures::Stream;
use libp2p::{
    core::upgrade,
    dns::TokioDnsConfig,
    identity::Keypair,
    kad::store::MemoryStore,
    mplex, noise,
    swarm::SwarmBuilder,
    tcp::{GenTcpConfig, TokioTcpTransport},
    Multiaddr, PeerId, Transport,
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

pub fn builder<'a>() -> NetworkBuilder<'a> {
    NetworkBuilder::default()
}

const TWO_HOURS: Duration = Duration::from_secs(60 * 60 * 2);

#[derive(Default)]
pub struct NetworkBuilder<'a> {
    discovery_protocol: Option<&'static str>,
    transmission_protocol: Option<&'static str>,
    peer_key: Option<Keypair>,
    listen_addr: Option<Multiaddr>,
    exposed_addresses: Option<Multiaddr>,
    store: Option<MemoryStore>,
    known_peers: &'a [(PeerId, Multiaddr)],
    local_port: Option<u8>,
}

impl<'a> NetworkBuilder<'a> {
    pub fn peer_key(mut self, peer_key: Keypair) -> Self {
        self.peer_key = Some(peer_key);

        self
    }

    pub fn exposed_addresses(mut self, addr: Multiaddr) -> Self {
        self.exposed_addresses = Some(addr);

        self
    }

    pub fn listen_addr(mut self, addr: Multiaddr) -> Self {
        self.listen_addr = Some(addr);

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

    pub fn transmission_protocol(mut self, protocol: &'static str) -> Self {
        self.transmission_protocol = Some(protocol);

        self
    }

    pub fn discovery_protocol(mut self, protocol: &'static str) -> Self {
        self.discovery_protocol = Some(protocol);

        self
    }

    pub async fn build(mut self) -> Result<(Client, impl Stream<Item = Event>, Runtime), P2PError> {
        let peer_key = self.peer_key.ok_or(P2PError::MissingPeerKey)?;
        let peer_id = peer_key.public().to_peer_id();

        let noise_keys = noise::Keypair::<noise::X25519Spec>::new().into_authentic(&peer_key)?;

        let (command_sender, command_receiver) = mpsc::channel(COMMAND_STREAM_BUFFER);
        let (event_sender, event_receiver) = mpsc::channel(EVENT_STREAM_BUFFER);

        let behaviour = Behaviour {
            peer_info: PeerInfoBehaviour::new(
                self.transmission_protocol.unwrap_or(TRANSMISSION_PROTOCOL),
                &peer_key,
            ),

            discovery: DiscoveryBehaviour::create(
                peer_key.clone(),
                Cow::Borrowed(
                    self.discovery_protocol
                        .unwrap_or(DISCOVERY_PROTOCOL)
                        .as_bytes(),
                ),
                self.known_peers,
                false,
            ),
            transmission: TransmissionBehaviour::create(),
        };

        let transport = {
            let dns_tcp =
                TokioDnsConfig::system(TokioTcpTransport::new(GenTcpConfig::new().nodelay(true)))
                    .unwrap();

            let tcp = TokioTcpTransport::new(GenTcpConfig::default().nodelay(true));
            dns_tcp.or_transport(tcp)
        };

        let transport = transport
            .upgrade(upgrade::Version::V1)
            .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
            .multiplex(mplex::MplexConfig::new())
            .timeout(TWO_HOURS)
            .boxed();

        let swarm = SwarmBuilder::new(transport, behaviour, peer_id)
            .executor(Box::new(|future| {
                tokio::spawn(future);
            }))
            .build();

        Ok((
            Client {
                local_peer_id: peer_id,
                sender: command_sender,
            },
            ReceiverStream::new(event_receiver),
            Runtime {
                swarm,
                command_receiver,
                event_sender,
                local_peer_id: peer_id,
                listening_on: self
                    .listen_addr
                    .take()
                    .expect("P2P runtime expect a MultiAddr"),
                addresses: self
                    .exposed_addresses
                    .take()
                    .expect("P2P runtime expect a MultiAddr"),
                bootstrapped: false,
                pending_requests: HashMap::new(),
                pending_dial: HashMap::new(),
                active_listeners: HashSet::new(),
                peers: HashSet::new(),
                pending_record_requests: HashMap::new(),
            },
        ))
    }
}
