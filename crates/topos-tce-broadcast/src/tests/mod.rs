use crate::double_echo::*;
use crate::*;
use rstest::*;
use std::collections::HashSet;
use std::time::Duration;
use tce_transport::ReliableBroadcastParams;
use tokio::sync::mpsc::Receiver;
use topos_crypto::messages::MessageSigner;
use topos_test_sdk::constants::*;
use topos_test_sdk::storage::create_validator_store;

const CHANNEL_SIZE: usize = 10;
const PRIVATE_KEY: &str = "47d361f6becb933a77d7e01dee7b1c1859b656adbd8428bf7bf9519503e5d5d6";

#[fixture]
fn small_config() -> TceParams {
    TceParams {
        nb_peers: 10,
        broadcast_params: ReliableBroadcastParams {
            echo_threshold: 8,
            ready_threshold: 5,
            delivery_threshold: 8,
        },
    }
}

#[fixture]
fn medium_config() -> TceParams {
    TceParams {
        nb_peers: 50,
        broadcast_params: ReliableBroadcastParams {
            echo_threshold: 33,
            ready_threshold: 16,
            delivery_threshold: 32,
        },
    }
}

#[derive(Debug)]
struct TceParams {
    nb_peers: usize,
    broadcast_params: ReliableBroadcastParams,
}

struct Context {
    event_receiver: Receiver<ProtocolEvents>,
    broadcast_receiver: broadcast::Receiver<CertificateDeliveredWithPositions>,
}

async fn create_context(params: TceParams) -> (DoubleEcho, Context) {
    let validator_store = create_validator_store::partial_1(vec![]).await;
    let (subscriptions_view_sender, subscriptions_view_receiver) = mpsc::channel(CHANNEL_SIZE);

    let (_cmd_sender, cmd_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (event_sender, event_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (_double_echo_shutdown_sender, double_echo_shutdown_receiver) =
        mpsc::channel::<oneshot::Sender<()>>(1);
    let (task_manager_message_sender, task_manager_message_receiver) = mpsc::channel(CHANNEL_SIZE);

    let message_signer = MessageSigner::new(PRIVATE_KEY).unwrap();

    let mut validators = HashSet::new();
    let validator_id = ValidatorId::from(message_signer.public_address);
    validators.insert(validator_id);

    let (broadcast_sender, broadcast_receiver) = broadcast::channel(CHANNEL_SIZE);
    let mut double_echo = DoubleEcho::new(
        params.broadcast_params,
        validator_id,
        message_signer,
        validators,
        task_manager_message_sender.clone(),
        cmd_receiver,
        event_sender,
        double_echo_shutdown_receiver,
        validator_store,
        broadcast_sender,
    );

    // List of peers
    let mut peers = HashSet::new();
    for i in 0..params.nb_peers {
        let peer = topos_p2p::utils::local_key_pair(Some(i as u8))
            .public()
            .to_peer_id();
        peers.insert(peer);
    }

    // Subscriptions
    double_echo.subscriptions.echo = peers.clone();
    double_echo.subscriptions.ready = peers.clone();
    double_echo.subscriptions.network_size = params.nb_peers;

    let msg = SubscriptionsView {
        echo: peers.clone(),
        ready: peers.clone(),
        network_size: params.nb_peers,
    };

    subscriptions_view_sender.send(msg).await.unwrap();

    double_echo.spawn_task_manager(subscriptions_view_receiver, task_manager_message_receiver);

    (
        double_echo,
        Context {
            event_receiver,
            broadcast_receiver,
        },
    )
}

async fn reach_echo_threshold(double_echo: &mut DoubleEcho, cert: &Certificate) {
    let selected = double_echo
        .subscriptions
        .echo
        .iter()
        .take(double_echo.params.echo_threshold)
        .cloned()
        .collect::<Vec<_>>();

    let message_signer = MessageSigner::new(PRIVATE_KEY).unwrap();
    let validator_id = ValidatorId::from(message_signer.public_address);

    let mut payload = Vec::new();
    payload.extend_from_slice(cert.id.as_array());
    payload.extend_from_slice(validator_id.as_bytes());

    let signature = message_signer.sign_message(&payload).unwrap();

    for p in selected {
        double_echo
            .handle_echo(p, cert.id, validator_id, signature)
            .await;
    }
}

async fn reach_ready_threshold(double_echo: &mut DoubleEcho, cert: &Certificate) {
    let selected = double_echo
        .subscriptions
        .ready
        .iter()
        .take(double_echo.params.ready_threshold)
        .cloned()
        .collect::<Vec<_>>();

    let message_signer = MessageSigner::new(PRIVATE_KEY).unwrap();

    let validator_id = ValidatorId::from(message_signer.public_address);

    let mut payload = Vec::new();
    payload.extend_from_slice(cert.id.as_array());
    payload.extend_from_slice(validator_id.as_bytes());

    let signature = message_signer.sign_message(&payload).unwrap();

    for p in selected {
        double_echo
            .handle_ready(p, cert.id, validator_id, signature)
            .await;
    }
}

async fn reach_delivery_threshold(double_echo: &mut DoubleEcho, cert: &Certificate) {
    let selected = double_echo
        .subscriptions
        .ready
        .iter()
        .take(double_echo.params.delivery_threshold)
        .cloned()
        .collect::<Vec<_>>();

    let message_signer = MessageSigner::new(PRIVATE_KEY).unwrap();
    let validator_id = ValidatorId::from(message_signer.public_address);

    let mut payload = Vec::new();
    payload.extend_from_slice(cert.id.as_array());
    payload.extend_from_slice(validator_id.as_bytes());

    let signature = message_signer.sign_message(&payload).unwrap();

    for p in selected {
        double_echo
            .handle_ready(p, cert.id, validator_id, signature)
            .await;
    }
}

#[rstest]
#[case::small_config(small_config())]
#[case(medium_config())]
#[tokio::test]
#[trace]
#[timeout(Duration::from_secs(10))]
async fn trigger_success_path_upon_reaching_threshold(#[case] params: TceParams) {
    let (mut double_echo, mut ctx) = create_context(params).await;

    let dummy_cert =
        Certificate::new_with_default_fields(PREV_CERTIFICATE_ID, SOURCE_SUBNET_ID_1, &[])
            .expect("Dummy certificate");

    // Trigger Echo upon dispatching
    double_echo.broadcast(dummy_cert.clone(), true).await;

    assert!(matches!(
        ctx.event_receiver.recv().await,
        Some(ProtocolEvents::Broadcast { certificate_id }) if certificate_id == dummy_cert.id
    ));

    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Gossip { .. })
    ));
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Echo { .. })
    ));

    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));

    // Trigger Ready upon reaching the Echo threshold
    reach_echo_threshold(&mut double_echo, &dummy_cert).await;

    assert!(matches!(
        ctx.event_receiver.recv().await,
        Some(ProtocolEvents::Ready { .. })
    ));

    // Trigger Delivery upon reaching the Delivery threshold
    reach_delivery_threshold(&mut double_echo, &dummy_cert).await;
    let x = ctx.broadcast_receiver.recv().await;
    assert!(matches!(
            x,
        Ok(CertificateDeliveredWithPositions(topos_core::types::CertificateDelivered { certificate, .. }, _)) if certificate == dummy_cert
    ));
}

#[rstest]
#[case::small_config(small_config())]
#[case(medium_config())]
#[tokio::test]
#[trace]
#[timeout(Duration::from_secs(4))]
async fn trigger_ready_when_reached_enough_ready(#[case] params: TceParams) {
    let (mut double_echo, mut ctx) = create_context(params).await;

    let dummy_cert =
        Certificate::new_with_default_fields(PREV_CERTIFICATE_ID, SOURCE_SUBNET_ID_1, &[])
            .expect("Dummy certificate");

    // Trigger Echo upon dispatching
    double_echo.broadcast(dummy_cert.clone(), true).await;

    assert!(matches!(
        ctx.event_receiver.recv().await,
        Some(ProtocolEvents::Broadcast { certificate_id }) if certificate_id == dummy_cert.id
    ));

    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Gossip { .. })
    ));

    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Echo { .. })
    ));

    // Trigger Ready upon reaching the Ready threshold
    reach_ready_threshold(&mut double_echo, &dummy_cert).await;

    assert!(matches!(
        ctx.event_receiver.recv().await,
        Some(ProtocolEvents::Ready { .. })
    ));
}
