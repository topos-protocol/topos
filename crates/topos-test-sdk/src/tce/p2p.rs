use std::error::Error;

use futures::Stream;
use libp2p::Multiaddr;
use tokio::{spawn, task::JoinHandle};

use crate::p2p::keypair_from_seed;
use topos_p2p::{error::P2PError, Client, Event, Runtime};

use super::NodeConfig;

pub async fn create_network_worker(
    seed: u8,
    _port: u16,
    addr: Multiaddr,
    peers: &[NodeConfig],
    minimum_cluster_size: usize,
) -> Result<(Client, impl Stream<Item = Event> + Unpin + Send, Runtime), P2PError> {
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

    topos_p2p::network::builder()
        .peer_key(key.clone())
        .known_peers(&known_peers)
        .exposed_addresses(addr.clone())
        .listen_addr(addr)
        .minimum_cluster_size(minimum_cluster_size)
        .build()
        .await
}

pub async fn bootstrap_network(
    seed: u8,
    port: u16,
    addr: Multiaddr,
    peers: &[NodeConfig],
    minimum_cluster_size: usize,
) -> Result<
    (
        Client,
        impl Stream<Item = Event> + Unpin + Send,
        JoinHandle<Result<(), ()>>,
    ),
    Box<dyn Error>,
> {
    let (network_client, network_stream, runtime) =
        create_network_worker(seed, port, addr, peers, minimum_cluster_size).await?;

    let runtime = runtime.bootstrap().await?;

    let runtime_join_handle = spawn(runtime.run());
    Ok((network_client, network_stream, runtime_join_handle))
}
