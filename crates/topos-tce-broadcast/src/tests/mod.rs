use crate::double_echo::*;
use crate::*;
use ethers::prelude::{LocalWallet, Signer};
use ethers::utils::keccak256;
use rstest::*;
use std::collections::HashSet;
use std::usize;
use tce_transport::ReliableBroadcastParams;
use tokio::sync::mpsc::Receiver;
use topos_test_sdk::constants::*;

const CHANNEL_SIZE: usize = 10;

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
}

async fn create_context(params: TceParams) -> (DoubleEcho, Context) {
    let (subscriptions_view_sender, subscriptions_view_receiver) = mpsc::channel(CHANNEL_SIZE);

    let (_cmd_sender, cmd_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (event_sender, event_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (_double_echo_shutdown_sender, double_echo_shutdown_receiver) =
        mpsc::channel::<oneshot::Sender<()>>(1);
    let (task_manager_message_sender, task_manager_message_receiver) = mpsc::channel(CHANNEL_SIZE);

    let wallet: LocalWallet = "47d361f6becb933a77d7e01dee7b1c1859b656adbd8428bf7bf9519503e5d5d6"
        .parse()
        .unwrap();

    let mut validators = HashSet::new();
    let validator_id = ValidatorId::from(wallet.address());
    validators.insert(validator_id.clone());

    let mut double_echo = DoubleEcho::new(
        params.broadcast_params,
        validator_id,
        wallet,
        validators,
        task_manager_message_sender.clone(),
        cmd_receiver,
        event_sender,
        double_echo_shutdown_receiver,
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

    (double_echo, Context { event_receiver })
}

async fn reach_echo_threshold(double_echo: &mut DoubleEcho, cert: &Certificate) {
    let selected = double_echo
        .subscriptions
        .echo
        .iter()
        .take(double_echo.params.echo_threshold)
        .cloned()
        .collect::<Vec<_>>();

    let wallet: LocalWallet = "47d361f6becb933a77d7e01dee7b1c1859b656adbd8428bf7bf9519503e5d5d6"
        .parse()
        .unwrap();

    let validator_id = ValidatorId::from(wallet.address());

    let mut hash = Vec::new();
    hash.extend(cert.id.as_array().iter().cloned());
    hash.extend(validator_id.clone().as_bytes());

    let hash = keccak256(hash);

    let signature = wallet.sign_message(hash.as_slice()).await.unwrap();

    for p in selected {
        double_echo
            .handle_echo(p, cert.id, validator_id.clone(), signature)
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

    let wallet: LocalWallet = "47d361f6becb933a77d7e01dee7b1c1859b656adbd8428bf7bf9519503e5d5d6"
        .parse()
        .unwrap();

    let validator_id = ValidatorId::from(wallet.address());

    let mut hash = Vec::new();
    hash.extend(cert.id.as_array().iter().cloned());
    hash.extend(validator_id.clone().as_bytes());

    let hash = keccak256(hash);

    let signature = wallet.sign_message(hash.as_slice()).await.unwrap();

    for p in selected {
        double_echo
            .handle_ready(p, cert.id, validator_id.clone(), signature)
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

    let wallet: LocalWallet = "47d361f6becb933a77d7e01dee7b1c1859b656adbd8428bf7bf9519503e5d5d6"
        .parse()
        .unwrap();

    let validator_id = ValidatorId::from(wallet.address());

    let mut hash = Vec::new();
    hash.extend(cert.id.as_array().iter().cloned());
    hash.extend(validator_id.clone().as_bytes());

    let hash = keccak256(hash);

    let signature = wallet.sign_message(hash.as_slice()).await.unwrap();

    for p in selected {
        double_echo
            .handle_ready(p, cert.id, validator_id.clone(), signature)
            .await;
    }
}

#[rstest]
#[case::small_config(small_config())]
#[case(medium_config())]
#[tokio::test]
#[trace]
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

    assert!(matches!(
         ctx.event_receiver.recv().await,
        Some(ProtocolEvents::CertificateDelivered { certificate }) if certificate == dummy_cert
    ));
}

#[rstest]
#[case::small_config(small_config())]
#[case(medium_config())]
#[tokio::test]
#[trace]
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
