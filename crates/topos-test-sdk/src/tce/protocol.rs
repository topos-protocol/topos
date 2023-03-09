use futures::Stream;

use topos_tce_broadcast::{ReliableBroadcastClient, ReliableBroadcastConfig};
use topos_tce_transport::{ReliableBroadcastParams, TceEvents};

pub fn create_reliable_broadcast_client(
    tce_params: ReliableBroadcastParams,
    peer_id: String,
) -> (
    ReliableBroadcastClient,
    impl Stream<Item = Result<TceEvents, ()>> + Unpin,
) {
    let config = ReliableBroadcastConfig { tce_params };

    ReliableBroadcastClient::new(config, peer_id)
}

pub fn create_reliable_broadcast_params<F>(correct_sample: usize, g: F) -> ReliableBroadcastParams
where
    F: Fn(usize, f32) -> usize,
{
    let mut params = ReliableBroadcastParams {
        ready_sample_size: correct_sample,
        echo_sample_size: correct_sample,
        delivery_sample_size: correct_sample,
        ..Default::default()
    };

    let e_ratio: f32 = 0.5;
    let r_ratio: f32 = 0.5;
    let d_ratio: f32 = 0.5;

    params.echo_threshold = g(params.echo_sample_size, e_ratio);
    params.ready_threshold = g(params.ready_sample_size, r_ratio);
    params.delivery_threshold = g(params.delivery_sample_size, d_ratio);

    params
}
