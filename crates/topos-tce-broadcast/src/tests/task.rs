use std::{future::IntoFuture, str::FromStr, sync::Arc, time::Duration};

use rstest::rstest;
use tokio::{
    spawn,
    sync::{broadcast, mpsc},
};
use topos_core::uci::Certificate;
use topos_crypto::{messages::MessageSigner, validator_id::ValidatorId};
use topos_tce_storage::validator::ValidatorStore;
use topos_test_sdk::{
    certificates::create_certificate_chain,
    constants::{SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_1},
    storage::create_validator_store,
};

use crate::{
    double_echo::broadcast_state::BroadcastState, event::ProtocolEvents,
    sampler::SubscriptionsView, task_manager::task::Task,
};

#[rstest]
#[test_log::test(tokio::test)]
#[timeout(Duration::from_secs(1))]
async fn start_with_ungossiped_cert(
    #[future(awt)]
    #[from(create_validator_store)]
    validatore_store: Arc<ValidatorStore>,
) {
    let certificate = create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 1)
        .pop()
        .unwrap()
        .certificate;
    let certificate_id = certificate.id;
    let validator_id = ValidatorId::default();
    let thresholds = topos_config::tce::broadcast::ReliableBroadcastParams {
        echo_threshold: 1,
        ready_threshold: 1,
        delivery_threshold: 1,
    };
    let (event_sender, mut event_receiver) = mpsc::channel(2);
    let (broadcast_sender, _) = broadcast::channel(1);
    let message_signer = Arc::new(
        MessageSigner::from_str("122f3ae6ade1fd136b292cea4f6243c7811160352c8821528547a1fe7c459daf")
            .unwrap(),
    );
    let need_gossip = true;
    let subscriptions = SubscriptionsView::default();

    let broadcast_state = BroadcastState::new(
        certificate,
        validator_id,
        thresholds.echo_threshold,
        thresholds.ready_threshold,
        thresholds.delivery_threshold,
        event_sender,
        subscriptions,
        need_gossip,
        message_signer,
    );

    let (task, _ctx) = Task::new(
        certificate_id,
        broadcast_state,
        validatore_store,
        broadcast_sender,
    );

    let _handle = spawn(task.into_future());

    let event = event_receiver.recv().await;
    assert!(matches!(
            event,
        Some(ProtocolEvents::Broadcast {
            certificate_id: id
        }) if id == certificate_id
    ));

    let event = event_receiver.recv().await;
    assert!(matches!(
            event,
        Some(ProtocolEvents::Gossip {
            cert: Certificate { id, .. }
        }) if id == certificate_id
    ));
}
