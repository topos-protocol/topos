#![allow(unused_variables)]
mod behaviour;
mod client;
mod command;
pub mod config;
pub mod constants;
pub mod error;
mod event;
mod runtime;
#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::convert::Infallible;

pub(crate) use behaviour::Behaviour;
pub use client::NetworkClient;
pub use client::RetryPolicy;
pub use command::Command;
pub use event::Event;
use http::Request;
use http::Response;
pub use libp2p::Multiaddr;
pub use libp2p::PeerId;
pub use runtime::Runtime;

use hyper::Body;
use tonic::body::BoxBody;
use tonic::server::NamedService;
use tonic::transport::server::Router;
use topos_core::api::grpc::p2p::info_service_server::InfoService;
use topos_core::api::grpc::p2p::info_service_server::InfoServiceServer;
use tower::Service;

pub mod network;

pub const TOPOS_GOSSIP: &str = "topos_gossip";
pub const TOPOS_ECHO: &str = "topos_echo";
pub const TOPOS_READY: &str = "topos_ready";

#[macro_export]
macro_rules! protocol_name {
    ($i:expr) => {
        format!("/{}", $i)
    };
}

#[derive(Debug)]
pub(crate) struct GrpcP2pInfo {}
#[async_trait::async_trait]
impl InfoService for GrpcP2pInfo {}

pub use behaviour::grpc::GrpcContext;

pub struct GrpcRouter {
    server: Router,
    protocols: HashSet<String>,
}

impl GrpcRouter {
    pub fn new(mut server: tonic::transport::Server) -> Self {
        let mut protocols = HashSet::new();
        protocols.insert(protocol_name!(InfoServiceServer::<GrpcP2pInfo>::NAME));

        Self {
            server: server.add_optional_service::<InfoServiceServer<GrpcP2pInfo>>(None),
            protocols,
        }
    }

    pub fn add_service<S>(mut self, service: S) -> Self
    where
        S: Service<Request<Body>, Response = Response<BoxBody>, Error = Infallible>
            + NamedService
            + Clone
            + Send
            + 'static,
        S::Future: Send + 'static,
    {
        self.protocols.insert(protocol_name!(S::NAME));
        self.server = self.server.add_service(service);

        self
    }
}

pub mod utils {
    use std::future::IntoFuture;

    use libp2p::{identity, PeerId};
    use tokio::{sync::mpsc, sync::oneshot};
    use tonic::server::NamedService;
    use topos_core::api::grpc::GrpcClient;

    use tracing::debug;

    use crate::{command::Command, error::P2PError};

    #[derive(Clone)]
    pub struct GrpcOverP2P {
        pub(crate) proxy_sender: mpsc::Sender<Command>,
    }

    impl GrpcOverP2P {
        pub fn new(proxy_sender: mpsc::Sender<Command>) -> Self {
            Self { proxy_sender }
        }

        pub async fn create<C, S>(&self, peer: PeerId) -> Result<C::Output, P2PError>
        where
            C: GrpcClient<Output = C>,
            S: NamedService,
        {
            debug!("Creating new instance of GRPC client for P2P");
            let (sender, recv) = oneshot::channel();
            let id = uuid::Uuid::new_v4();

            let _ = self
                .proxy_sender
                .send(Command::NewProxiedQuery {
                    protocol: S::NAME,
                    peer,
                    id,
                    response: sender,
                })
                .await;

            let connection = recv.await?;

            let connected = connection.into_future().await?;

            Ok(C::init(connected.channel))
        }
    }

    /// build peer_id keys, generate for now - either from the seed or purely random one
    pub fn local_key_pair(secret_key_seed: Option<u8>) -> identity::Keypair {
        // todo: load from protobuf encoded|base64 encoded config.local_key_pair
        match secret_key_seed {
            Some(seed) => {
                let mut bytes = [0u8; 32];
                bytes[0] = seed;
                identity::Keypair::ed25519_from_bytes(bytes).expect("Invalid keypair")
            }
            None => identity::Keypair::generate_ed25519(),
        }
    }

    pub fn local_key_pair_from_slice(slice: &[u8]) -> identity::Keypair {
        // todo: load from protobuf encoded|base64 encoded config.local_key_pair
        let mut bytes = [0u8; 32];
        if slice.len() <= 32 {
            bytes[..slice.len()].clone_from_slice(slice);
        } else {
            bytes.clone_from_slice(&slice[..32]);
        }

        identity::Keypair::ed25519_from_bytes(bytes).expect("Invalid keypair")
    }

    pub fn keypair_from_protobuf_encoding(priv_key: &[u8]) -> identity::Keypair {
        identity::Keypair::from_protobuf_encoding(priv_key).expect("Invalid keypair retrieval")
    }
}

#[test]
pub fn generate_from_secp256k1() {
    // Key living in the AWS SM or FS at libp2p/libp2p.key
    let edge_dec_privkey =
        hex::decode("08021220eb5ce97bd3e7729ac4ab077b83881426cebf19e58a9d9760d1cedfc53d772d6c")
            .expect("Failed to hex decode");

    use std::str::FromStr;

    let edge_peerid =
        PeerId::from_str("16Uiu2HAkxA7KW9GC2T3tQg3zHvjrnDPqfQUKTfzU3wbts8AsV6kH").unwrap();

    let keypair = utils::keypair_from_protobuf_encoding(&edge_dec_privkey);

    // Verify that we end up with the same PeerId
    assert_eq!(keypair.public().to_peer_id(), edge_peerid);
}
