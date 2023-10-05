use futures::Stream;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast;
use topos_crypto::messages::MessageSigner;
use topos_tce_broadcast::{ReliableBroadcastClient, ReliableBroadcastConfig};
use topos_tce_storage::types::CertificateDeliveredWithPositions;
use topos_tce_storage::validator::ValidatorStore;
use topos_tce_transport::{ProtocolEvents, ReliableBroadcastParams, ValidatorId};

const PRIVATE_KEY: &str = "47d361f6becb933a77d7e01dee7b1c1859b656adbd8428bf7bf9519503e5d5d6";

pub async fn create_reliable_broadcast_client(
    tce_params: ReliableBroadcastParams,
    storage: Arc<ValidatorStore>,
    sender: broadcast::Sender<CertificateDeliveredWithPositions>,
) -> (
    ReliableBroadcastClient,
    impl Stream<Item = ProtocolEvents> + Unpin,
) {
    let message_signer = Arc::new(MessageSigner::from_str(PRIVATE_KEY).unwrap());

    let mut validators = HashSet::new();
    let validator_id = ValidatorId::from(message_signer.public_address);
    validators.insert(validator_id);

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
