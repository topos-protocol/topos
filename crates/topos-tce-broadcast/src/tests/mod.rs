use crate::double_echo::*;
use crate::*;
use rstest::*;
use std::collections::HashSet;
use std::usize;
use tce_transport::ReliableBroadcastParams;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

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

#[derive(Clone, Debug)]
struct TceParams {
    nb_peers: usize,
    broadcast_params: ReliableBroadcastParams,
}

struct Context {
    event_receiver: Receiver<ProtocolEvents>,
    cmd_sender: Sender<DoubleEchoCommand>,
    subscriptions: SubscriptionsView,
    #[allow(dead_code)]
    double_echo_handle: JoinHandle<()>,
    #[allow(dead_code)]
    shutdown_sender: Sender<oneshot::Sender<()>>,
}

async fn create_context(params: TceParams) -> Context {
    let (_subscriptions_view_sender, subscriptions_view_receiver) = mpsc::channel(CHANNEL_SIZE);

    let (cmd_sender, cmd_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (event_sender, event_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (double_echo_shutdown_sender, double_echo_shutdown_receiver) =
        mpsc::channel::<oneshot::Sender<()>>(1);

    let mut double_echo = DoubleEcho::new(
        params.broadcast_params,
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

    let subscriptions = SubscriptionsView {
        echo: peers.clone(),
        ready: peers.clone(),
        network_size: params.nb_peers,
    };

    // Subscriptions
    double_echo.subscriptions.echo = peers.clone();
    double_echo.subscriptions.ready = peers.clone();
    double_echo.subscriptions.network_size = params.nb_peers;

    let double_echo_handle = spawn(double_echo.run(subscriptions_view_receiver));

    Context {
        event_receiver,
        cmd_sender,
        subscriptions,
        double_echo_handle,
        shutdown_sender: double_echo_shutdown_sender,
    }
}

async fn reach_echo_threshold(
    cmd_sender: Sender<DoubleEchoCommand>,
    params: TceParams,
    cert: &Certificate,
    subscriptions: SubscriptionsView,
) {
    let selected = subscriptions
        .echo
        .iter()
        .take(params.broadcast_params.echo_threshold)
        .cloned()
        .collect::<Vec<_>>();

    for p in selected {
        let _ = cmd_sender
            .send(DoubleEchoCommand::Echo {
                from_peer: p,
                certificate_id: cert.id,
            })
            .await;
    }
}

async fn reach_ready_threshold(
    cmd_sender: Sender<DoubleEchoCommand>,
    params: TceParams,
    cert: &Certificate,
    subscriptions: SubscriptionsView,
) {
    let selected = subscriptions
        .ready
        .iter()
        .take(params.broadcast_params.ready_threshold)
        .cloned()
        .collect::<Vec<_>>();

    for p in selected {
        let _ = cmd_sender
            .send(DoubleEchoCommand::Ready {
                from_peer: p,
                certificate_id: cert.id,
            })
            .await;
    }
}

async fn reach_delivery_threshold(
    cmd_sender: Sender<DoubleEchoCommand>,
    params: TceParams,
    cert: &Certificate,
    subscriptions: SubscriptionsView,
) {
    let selected = subscriptions
        .ready
        .iter()
        .take(params.broadcast_params.delivery_threshold)
        .cloned()
        .collect::<Vec<_>>();

    for p in selected {
        let _ = cmd_sender
            .send(DoubleEchoCommand::Ready {
                from_peer: p,
                certificate_id: cert.id,
            })
            .await;
    }
}

#[rstest]
#[case::small_config(small_config())]
#[case(medium_config())]
#[tokio::test]
#[trace]
async fn trigger_success_path_upon_reaching_threshold(#[case] params: TceParams) {
    let mut ctx = create_context(params.clone()).await;

    let dummy_cert = Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_1,
        Default::default(),
        Default::default(),
        &[],
        0,
        Default::default(),
    )
    .expect("Dummy certificate");

    // Trigger Echo upon dispatching
    let _ = ctx
        .cmd_sender
        .send(DoubleEchoCommand::Broadcast {
            cert: dummy_cert.clone(),
            need_gossip: true,
        })
        .await;

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

    println!("{:#?}", ctx.event_receiver.try_recv());

    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));

    // Trigger Ready upon reaching the Echo threshold
    reach_echo_threshold(
        ctx.cmd_sender.clone(),
        params.clone(),
        &dummy_cert,
        ctx.subscriptions.clone(),
    )
    .await;

    assert!(matches!(
        ctx.event_receiver.recv().await,
        Some(ProtocolEvents::Ready { .. })
    ));

    // Trigger Delivery upon reaching the Delivery threshold
    reach_delivery_threshold(
        ctx.cmd_sender.clone(),
        params.clone(),
        &dummy_cert,
        ctx.subscriptions.clone(),
    )
    .await;

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
    let mut ctx = create_context(params.clone()).await;

    let dummy_cert = Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_1,
        Default::default(),
        Default::default(),
        &[],
        0,
        Default::default(),
    )
    .expect("Dummy certificate");

    // Trigger Echo upon dispatching
    let _ = ctx
        .cmd_sender
        .send(DoubleEchoCommand::Broadcast {
            cert: dummy_cert.clone(),
            need_gossip: true,
        })
        .await;

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

    // println!("{:#?}", ctx.event_receiver.try_recv());

    // Trigger Ready upon reaching the Echo threshold
    reach_ready_threshold(
        ctx.cmd_sender.clone(),
        params.clone(),
        &dummy_cert,
        ctx.subscriptions.clone(),
    )
    .await;

    assert!(matches!(
        ctx.event_receiver.recv().await,
        Some(ProtocolEvents::Ready { .. })
    ));
}
