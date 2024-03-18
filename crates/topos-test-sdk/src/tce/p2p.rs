use std::error::Error;

use futures::Stream;
use libp2p::Multiaddr;
use tokio::{spawn, task::JoinHandle};
use tracing::Instrument;

use crate::p2p::keypair_from_seed;
use topos_p2p::{error::P2PError, Event, GrpcContext, GrpcRouter, NetworkClient, Runtime};

use super::NodeConfig;

pub async fn create_network_worker(
    seed: u8,
    _port: u16,
    addr: Vec<Multiaddr>,
    peers: &[NodeConfig],
    minimum_cluster_size: usize,
    router: Option<GrpcRouter>,
) -> Result<
    (
        NetworkClient,
        impl Stream<Item = Event> + Unpin + Send,
        Runtime,
    ),
    P2PError,
> {
    let key = keypair_from_seed(seed);
    let _peer_id = key.public().to_peer_id();

    let known_peers = if seed == 1 {
        vec![]
    } else {
        peers
            .iter()
            .filter_map(|config| {
                if config.seed == 1 {
                    Some((config.keypair.public().to_peer_id(), config.addr.clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    let grpc_context = if let Some(router) = router {
        GrpcContext::default().with_router(router)
    } else {
        GrpcContext::default()
    };

    topos_p2p::network::builder()
        .peer_key(key.clone())
        .known_peers(&known_peers)
        .public_addresses(addr.clone())
        .listen_addresses(addr)
        .minimum_cluster_size(minimum_cluster_size)
        .grpc_context(grpc_context)
        .allow_private_ip(true)
        .build()
        .in_current_span()
        .await
}

pub async fn bootstrap_network(
    seed: u8,
    port: u16,
    addr: Multiaddr,
    peers: &[NodeConfig],
    minimum_cluster_size: usize,
    router: Option<GrpcRouter>,
    dummy: bool,
) -> Result<
    (
        NetworkClient,
        impl Stream<Item = Event> + Unpin + Send,
        JoinHandle<Result<(), P2PError>>,
    ),
    Box<dyn Error>,
> {
    let (network_client, mut network_stream, runtime) =
        create_network_worker(seed, port, vec![addr], peers, minimum_cluster_size, router)
            .in_current_span()
            .await?;

    let runtime_join_handle = if dummy {
        spawn(runtime.run().in_current_span())
    } else {
        runtime
            .bootstrap(&mut network_stream)
            .in_current_span()
            .await?
    };

    println!("Network bootstrap done.");

    Ok((network_client, network_stream, runtime_join_handle))
}
