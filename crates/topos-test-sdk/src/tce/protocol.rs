use futures::Stream;

use topos_tce_broadcast::{ReliableBroadcastClient, ReliableBroadcastConfig};
use topos_tce_transport::{ProtocolEvents, ReliableBroadcastParams};

pub fn create_reliable_broadcast_client(
    tce_params: ReliableBroadcastParams,
    peer_id: String,
) -> (
    ReliableBroadcastClient,
    impl Stream<Item = Result<ProtocolEvents, ()>> + Unpin,
) {
    let config = ReliableBroadcastConfig { tce_params };

    ReliableBroadcastClient::new(config, peer_id)
}

pub fn create_reliable_broadcast_params<F>(correct_sample: usize, g: F) -> ReliableBroadcastParams
where
    F: Fn(usize, f32) -> usize,
{
    let mut params = ReliableBroadcastParams {
        ..Default::default()
    };

    let e_ratio: f32 = 0.5;
    let r_ratio: f32 = 0.5;
    let d_ratio: f32 = 0.5;

    params.echo_threshold = g(correct_sample, e_ratio);
    params.ready_threshold = g(correct_sample, r_ratio);
    params.delivery_threshold = g(correct_sample, d_ratio);

    params
}
