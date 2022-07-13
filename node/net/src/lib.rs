//! Implementation of libp2p transport layer
//!
//! Inits connection.
//! Translates network messages to protocol events and vice versa.
//! Does discovery.
//! Helps in sampling peers.
//!

mod behavior;
mod discovery_behavior;
mod transmission_behavior;

use behavior::Behavior;
use std::time::Duration;

use libp2p::{
    core::upgrade,
    dns::TokioDnsConfig,
    futures::StreamExt,
    identity, mplex, noise,
    swarm::{SwarmBuilder, SwarmEvent},
    tcp::TokioTcpConfig,
    Multiaddr, PeerId, Transport,
};

use tokio::sync::{mpsc, oneshot};

/// Configuration parameters
pub struct NetworkWorkerConfig {
    pub known_peers: Vec<(PeerId, Multiaddr)>,
    pub local_port: u16,
    pub secret_key_seed: Option<u8>,
    pub local_key_pair: Option<String>,
}

/// Host interface
pub enum NetworkWorkerEvents {}

/// Transport handle
///
/// Communication is made using polling 'next_event()' and pushing commands to 'tx_commands'.
pub struct NetworkWorker {
    pub rx_events: mpsc::UnboundedReceiver<NetworkEvents>,
    pub tx_commands: mpsc::UnboundedSender<NetworkCommands>,
    pub my_peer_id: PeerId,
}

/// Network events
#[derive(Debug)]
pub enum NetworkEvents {
    KadPeersChanged {
        new_peers: Vec<PeerId>,
    },
    TransmissionOnReq {
        from: PeerId,
        data: Vec<u8>,
        respond_to: oneshot::Sender<Vec<u8>>,
    },
    TransmissionOnResp {
        for_ext_req_id: String,
        from: PeerId,
        data: Vec<u8>,
    },
}

/// Network commands
#[derive(Debug)]
pub enum NetworkCommands {
    TransmissionReq {
        ext_req_id: String,
        to: Vec<PeerId>,
        data: Vec<u8>,
    },
}

impl NetworkWorker {
    pub fn new(config: NetworkWorkerConfig) -> Self {
        let (tx_events, rx_events) = mpsc::unbounded_channel();
        let (tx_commands, mut rx_commands) = mpsc::unbounded_channel();
        let local_key = Self::local_key_pair(&config);
        log::info!("Local peerId: {:?}", local_key.public().to_peer_id());
        let me = Self {
            rx_events,
            tx_commands,
            my_peer_id: local_key.public().to_peer_id(),
        };

        let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
            .into_authentic(&local_key)
            .expect("Signing libp2p-noise static DH keypair failed.");

        let local_listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", config.local_port)
            .parse()
            .unwrap();

        // launch networking task
        tokio::spawn(async move {
            // Kick off the swarm
            let p2p_transport = TokioDnsConfig::system(TokioTcpConfig::new().nodelay(true))
                .expect("DNS config made")
                .upgrade(upgrade::Version::V1)
                .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
                .multiplex(mplex::MplexConfig::new())
                .timeout(Duration::from_secs(60 * 60 * 2))
                .boxed();

            let mut swarm = {
                let behavior =
                    Behavior::new(local_key.clone(), config.known_peers.clone(), tx_events);

                SwarmBuilder::new(p2p_transport, behavior, local_key.public().to_peer_id())
                    .executor(Box::new(|fut| {
                        tokio::spawn(fut);
                    }))
                    .build()
            };

            // Listen on all interfaces and whatever port the OS assigns
            swarm.listen_on(local_listen_addr).expect("Bind port");

            // gossip launch
            for known_peer in config.known_peers.clone() {
                log::info!(
                    "---- adding gossip peer:{} at {}",
                    &known_peer.0,
                    &known_peer.1
                );

                // we need to dial peer so that gossipsub would be aware of it
                match swarm.dial(known_peer.1.clone()) {
                    Ok(_) => log::debug!("Dialed {:?}", &known_peer.1),
                    Err(e) => log::debug!("Dial {:?} failed: {:?}", &known_peer.1, e),
                }
            }

            // networking loop
            loop {
                tokio::select! {
                    // poll commands
                    Some(command) = rx_commands.recv() => {
                        log::debug!("command: {:?}", &command);
                        match command {
                            NetworkCommands::TransmissionReq { .. } => {
                                let bh = swarm.behaviour_mut();
                                let _ = bh.transmission.eval(command);
                            },
                        }
                    },
                    // poll swarm events
                    event = swarm.select_next_some() => {
                        if let SwarmEvent::NewListenAddr { address, .. } = event {
                            log::info!("Listening on {:?}", address);
                        } else {
                            log::debug!("swarm event: {:?}", &event);
                        }
                    }
                }
            }
        }); //< end of spawn networking task

        me
    }

    /// Command execution
    pub async fn eval(&mut self, cmd: NetworkCommands) {
        let _ = self.tx_commands.send(cmd);
    }

    /// 'Selectable' events' stream
    pub async fn next_event(&mut self) -> Result<NetworkEvents, ()> {
        let mb_event = self.rx_events.recv().await;
        return match mb_event {
            Some(e) => Ok(e),
            _ => Err(()),
        };
    }

    /// build peer_id keys, generate for now - either from the seed or purely random one
    fn local_key_pair(config: &NetworkWorkerConfig) -> identity::Keypair {
        let id_keys = match config.secret_key_seed {
            Some(seed) => {
                let mut bytes = [0u8; 32];
                bytes[0] = seed;
                let secret_key = identity::ed25519::SecretKey::from_bytes(&mut bytes).expect(
                    "this returns `Err` only if the length is wrong; the length is correct; qed",
                );
                identity::Keypair::Ed25519(secret_key.into())
            }
            None => identity::Keypair::generate_ed25519(),
        };
        // todo: load from protobuf encoded|base64 encoded config.local_key_pair
        id_keys
    }
}
