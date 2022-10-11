use std::{collections::HashMap, future::IntoFuture, net::UdpSocket, str::FromStr};

use futures::{FutureExt, Stream, StreamExt};
use libp2p::{
    identity::{self, Keypair},
    Multiaddr, PeerId,
};
use tce_transport::{ReliableBroadcastParams, TrbpEvents};
use tokio::{spawn, sync::mpsc};
use tonic::transport::{channel, Channel};
use topos_core::api::tce::v1::api_service_client::ApiServiceClient;
use topos_p2p::{Client, Event, Runtime};
use topos_tce::AppContext;
use topos_tce_broadcast::{
    DoubleEchoCommand, ReliableBroadcastClient, ReliableBroadcastConfig, SamplerCommand,
};
use topos_tce_storage::{Connection, InMemoryStorage, StorageClient};

#[derive(Debug)]
pub struct TestAppContext {
    pub id: String,
    pub peer_id: PeerId,
    pub command_sampler: mpsc::Sender<SamplerCommand>,
    pub command_broadcast: mpsc::Sender<DoubleEchoCommand>,
    pub(crate) api_grpc_client: Option<ApiServiceClient<Channel>>,
}

pub type Seed = u8;
pub type Port = u16;
pub type PeerConfig = (Seed, Port, Keypair, Multiaddr);

pub async fn start_peer_pool<F>(
    peer_number: u8,
    correct_sample: usize,
    g: F,
) -> HashMap<String, TestAppContext>
where
    F: Fn(usize, f32) -> usize,
{
    let mut clients = HashMap::new();
    let peers = build_peer_config_pool(peer_number);

    for (index, (seed, port, keypair, addr)) in peers.iter().enumerate() {
        let peer_id = format!("peer_{index}");

        let storage = InMemoryStorage::default();
        let (connection, store) = Connection::new(async move { Ok(storage) }.boxed());
        spawn(connection.into_future());

        let (rb_client, trb_events) = create_reliable_broadcast_client(
            &peer_id,
            create_reliable_broadcast_params(correct_sample, &g),
            store.clone(),
        );
        let (client, event_stream, runtime) =
            create_network_worker(*seed, *port, addr.clone(), &peers).await;

        let (command_sampler, command_broadcast) = rb_client.get_command_channels();

        let socket = UdpSocket::bind("0.0.0.0:0").expect("Can't find an available port");
        let addr = socket.local_addr().ok().unwrap();
        let api_port = addr.port();

        let (api_client, api_events) = topos_tce_api::Runtime::builder()
            .serve_addr(addr)
            .build_and_launch()
            .await;
        let app = AppContext::new(store, rb_client, client, api_client);

        spawn(runtime.run());
        spawn(app.run(event_stream, trb_events, api_events));
        let api_endpoint = format!("http://127.0.0.1:{api_port}");

        let channel = channel::Endpoint::from_str(&api_endpoint)
            .unwrap()
            .connect_lazy();
        let api_grpc_client = ApiServiceClient::new(channel);

        let client = TestAppContext {
            id: peer_id.clone(),
            peer_id: keypair.public().to_peer_id(),
            command_sampler,
            command_broadcast,
            api_grpc_client: Some(api_grpc_client),
        };
        clients.insert(peer_id, client);
    }

    clients
}

fn build_peer_config_pool(peer_number: u8) -> Vec<PeerConfig> {
    (1..=peer_number)
        .into_iter()
        .map(|id| {
            let (peer_id, port, addr) = local_peer(id);

            (id, port, peer_id, addr)
        })
        .collect()
}

fn local_peer(peer_index: u8) -> (Keypair, Port, Multiaddr) {
    let peer_id: Keypair = keypair_from_seed(peer_index);
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Can't find an available port");
    let port = socket.local_addr().unwrap().port();
    let local_listen_addr: Multiaddr = format!(
        "/ip4/127.0.0.1/tcp/{}/p2p/{}",
        port,
        peer_id.public().to_peer_id()
    )
    .parse()
    .unwrap();

    (peer_id, port, local_listen_addr)
}

fn keypair_from_seed(seed: u8) -> Keypair {
    let mut bytes = [0u8; 32];
    bytes[0] = seed;
    let secret_key = identity::ed25519::SecretKey::from_bytes(&mut bytes)
        .expect("this returns `Err` only if the length is wrong; the length is correct; qed");
    identity::Keypair::Ed25519(secret_key.into())
}

async fn create_network_worker(
    seed: u8,
    _port: u16,
    addr: Multiaddr,
    peers: &[PeerConfig],
) -> (Client, impl Stream<Item = Event> + Unpin + Send, Runtime) {
    let key = keypair_from_seed(seed);
    let _peer_id = key.public().to_peer_id();

    let known_peers = if seed == 1 {
        vec![]
    } else {
        peers
            .iter()
            .filter_map(|(current_seed, _, key, addr)| {
                if *current_seed == 1 {
                    Some((key.public().to_peer_id(), addr.clone().into()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };

    topos_p2p::network::builder()
        .peer_key(key.clone())
        .known_peers(known_peers)
        .listen_addr(addr)
        .build()
        .await
        .expect("Cannot create network")
}

fn create_reliable_broadcast_client(
    peer_id: &str,
    trbp_params: ReliableBroadcastParams,
    store: StorageClient,
) -> (
    ReliableBroadcastClient,
    impl Stream<Item = Result<TrbpEvents, ()>> + Unpin,
) {
    let config = ReliableBroadcastConfig {
        store,
        trbp_params,
        my_peer_id: peer_id.to_string(),
    };

    ReliableBroadcastClient::new(config)
}

fn create_reliable_broadcast_params<F>(correct_sample: usize, g: F) -> ReliableBroadcastParams
where
    F: Fn(usize, f32) -> usize,
{
    let mut params = ReliableBroadcastParams::default();
    params.ready_sample_size = correct_sample;
    params.echo_sample_size = correct_sample;
    params.delivery_sample_size = correct_sample;

    let e_ratio: f32 = 0.66;
    let r_ratio: f32 = 0.33;
    let d_ratio: f32 = 0.66;

    params.echo_threshold = g(params.echo_sample_size, e_ratio);
    params.ready_threshold = g(params.ready_sample_size, r_ratio);
    params.delivery_threshold = g(params.delivery_sample_size, d_ratio);

    params
}

#[allow(dead_code)]
pub struct TestNodeContext {
    pub(crate) peer_id: PeerId,
    pub(crate) peer_addr: Multiaddr,
    pub(crate) client: Client,
    stream: Box<dyn Stream<Item = topos_p2p::Event> + Unpin + Send>,
}

impl TestNodeContext {
    #[allow(dead_code)]
    pub(crate) async fn next_event(&mut self) -> Option<topos_p2p::Event> {
        self.stream.next().await
    }
}
