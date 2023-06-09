use futures::Stream;

use topos_tce_broadcast::{ReliableBroadcastClient, ReliableBroadcastConfig};
use topos_tce_transport::{ProtocolEvents, ReliableBroadcastParams};

pub async fn create_reliable_broadcast_client(
    tce_params: ReliableBroadcastParams,
    peer_id: String,
    storage: topos_tce_storage::StorageClient,
    network: topos_p2p::Client,
) -> (
    ReliableBroadcastClient,
    impl Stream<Item = Result<ProtocolEvents, ()>> + Unpin,
) {
    let config = ReliableBroadcastConfig { tce_params };

    ReliableBroadcastClient::new(config, peer_id, storage, network).await
}

pub fn create_reliable_broadcast_params(number_of_nodes: usize) -> ReliableBroadcastParams {
    let mut params = ReliableBroadcastParams {
        ..Default::default()
    };
    let f = (number_of_nodes.saturating_sub(1)) / 3;

    params.echo_threshold = 1 + ((number_of_nodes.saturating_add(f)) / 2);
    params.ready_threshold = 1 + f;
    params.delivery_threshold = 2 * f + 1;

    params
}
