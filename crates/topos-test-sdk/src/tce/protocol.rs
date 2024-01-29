use futures::Stream;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::broadcast;
use topos_config::tce::broadcast::ReliableBroadcastParams;
use topos_core::types::ValidatorId;
use topos_crypto::messages::MessageSigner;
use topos_tce_broadcast::event::ProtocolEvents;
use topos_tce_broadcast::{ReliableBroadcastClient, ReliableBroadcastConfig};
use topos_tce_storage::types::CertificateDeliveredWithPositions;
use topos_tce_storage::validator::ValidatorStore;

pub async fn create_reliable_broadcast_client(
    validator_id: ValidatorId,
    validators: HashSet<ValidatorId>,
    message_signer: Arc<MessageSigner>,
    tce_params: ReliableBroadcastParams,
    storage: Arc<ValidatorStore>,
    sender: broadcast::Sender<CertificateDeliveredWithPositions>,
) -> (
    ReliableBroadcastClient,
    impl Stream<Item = ProtocolEvents> + Unpin,
) {
    let config = ReliableBroadcastConfig {
        tce_params,
        validator_id,
        validators,
        message_signer,
    };

    ReliableBroadcastClient::new(config, storage, sender).await
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
