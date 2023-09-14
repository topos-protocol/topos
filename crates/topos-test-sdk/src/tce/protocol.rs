use ethers::prelude::LocalWallet;
use futures::Stream;
use std::collections::HashSet;

use topos_tce_broadcast::{ReliableBroadcastClient, ReliableBroadcastConfig};
use topos_tce_transport::{ProtocolEvents, ReliableBroadcastParams, ValidatorId};

pub async fn create_reliable_broadcast_client(
    tce_params: ReliableBroadcastParams,
) -> (
    ReliableBroadcastClient,
    impl Stream<Item = ProtocolEvents> + Unpin,
) {
    let mut validators = HashSet::new();
    let validator_id = ValidatorId::from("0xb4973cdb10894d1d1547673bd758589034c2bba5");
    validators.insert(validator_id.clone());

    let wallet: LocalWallet = "0x47d361f6becb933a77d7e01dee7b1c1859b656adbd8428bf7bf9519503e5d5d6"
        .parse()
        .unwrap();

    let config = ReliableBroadcastConfig {
        tce_params,
        validator_id,
        validators,
        wallet,
    };

    ReliableBroadcastClient::new(config).await
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
