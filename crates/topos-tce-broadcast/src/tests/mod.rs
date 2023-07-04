use crate::double_echo::*;
use crate::*;
use rstest::*;
use std::collections::HashSet;
use std::usize;
use tce_transport::ReliableBroadcastParams;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::sync::broadcast::Receiver;
use tokio::time::Duration;

use topos_test_sdk::constants::*;

const CHANNEL_SIZE: usize = 10;
const WAIT_EVENT_TIMEOUT: Duration = Duration::from_secs(1);

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
    subscriptions_view_sender: Sender<SubscriptionsView>,
    cmd_sender: Sender<DoubleEchoCommand>,
    double_echo_shutdown_sender: Sender<oneshot::Sender<()>>,
}

fn create_context(params: TceParams) -> (DoubleEcho, Context) {
    let (subscriptions_view_sender, subscriptions_view_receiver) = mpsc::channel(CHANNEL_SIZE);

    let (cmd_sender, cmd_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (event_sender, event_receiver) = broadcast::channel(CHANNEL_SIZE);
    let (double_echo_shutdown_sender, double_echo_shutdown_receiver) =
        mpsc::channel::<oneshot::Sender<()>>(1);

    let mut double_echo = DoubleEcho::new(
        params.broadcast_params,
        cmd_receiver,
        subscriptions_view_receiver,
        event_sender,
        double_echo_shutdown_receiver,
        String::new(),
        0,
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

    (
        double_echo,
        Context {
            event_receiver,
            // subscribers_update_sender,
            subscriptions_view_sender,
            cmd_sender,
            double_echo_shutdown_sender,
        },
    )
}

fn reach_echo_threshold(double_echo: &mut DoubleEcho, cert: &Certificate) {
    let selected = double_echo
        .subscriptions
        .echo
        .iter()
        .take(double_echo.params.echo_threshold)
        .cloned()
        .collect::<Vec<_>>();

    for p in selected {
        double_echo.consume_echo(p, &cert.id);
    }
}

fn reach_ready_threshold(double_echo: &mut DoubleEcho, cert: &Certificate) {
    let selected = double_echo
        .subscriptions
        .ready
        .iter()
        .take(double_echo.params.ready_threshold)
        .cloned()
        .collect::<Vec<_>>();

    for p in selected {
        double_echo.consume_ready(p, &cert.id);
    }
}

fn reach_delivery_threshold(double_echo: &mut DoubleEcho, cert: &Certificate) {
    let selected = double_echo
        .subscriptions
        .ready
        .iter()
        .take(double_echo.params.delivery_threshold)
        .cloned()
        .collect::<Vec<_>>();

    for p in selected {
        double_echo.consume_ready(p, &cert.id);
    }
}

#[rstest]
#[case::small_config(small_config())]
#[case(medium_config())]
#[tokio::test]
#[trace]
async fn trigger_success_path_upon_reaching_threshold(#[case] params: TceParams) {
    let (mut double_echo, mut ctx) = create_context(params);

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
    double_echo.broadcast(dummy_cert.clone(), true);

    assert_eq!(ctx.event_receiver.len(), 3);

    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Gossip { .. })
    ));
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Broadcast { certificate_id }) if certificate_id == dummy_cert.id
    ));
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Echo { .. })
    ));

    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Err(TryRecvError::Empty)
    ));
    assert_eq!(ctx.event_receiver.len(), 0);

    // Trigger Ready upon reaching the Echo threshold
    reach_echo_threshold(&mut double_echo, &dummy_cert);
    double_echo.state_change_follow_up();

    assert_eq!(ctx.event_receiver.len(), 1);
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Ready { .. })
    ));

    // Trigger Delivery upon reaching the Delivery threshold
    reach_delivery_threshold(&mut double_echo, &dummy_cert);
    double_echo.state_change_follow_up();

    assert_eq!(ctx.event_receiver.len(), 1);
    assert!(matches!(
         ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::CertificateDelivered { certificate }) if certificate == dummy_cert
    ));
}

#[rstest]
#[case::small_config(small_config())]
#[case(medium_config())]
#[tokio::test]
#[trace]
async fn trigger_ready_when_reached_enough_ready(#[case] params: TceParams) {
    let (mut double_echo, mut ctx) = create_context(params);

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
    double_echo.broadcast(dummy_cert.clone(), true);

    assert_eq!(ctx.event_receiver.len(), 3);
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Gossip { .. })
    ));
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Broadcast { certificate_id }) if certificate_id == dummy_cert.id
    ));
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Echo { .. })
    ));

    // Trigger Ready upon reaching the Ready threshold
    reach_ready_threshold(&mut double_echo, &dummy_cert);
    double_echo.state_change_follow_up();

    assert_eq!(ctx.event_receiver.len(), 1);
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Ready { .. })
    ));
}

#[rstest]
#[case::small_config(small_config())]
#[case(medium_config())]
#[tokio::test]
#[trace]
async fn process_after_delivery_until_sending_ready(#[case] params: TceParams) {
    let (mut double_echo, mut ctx) = create_context(params);

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
    double_echo.broadcast(dummy_cert.clone(), true);

    assert_eq!(ctx.event_receiver.len(), 3);
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Gossip { .. })
    ));
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Broadcast { certificate_id }) if certificate_id == dummy_cert.id
    ));
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Echo { .. })
    ));

    // Trigger Delivery upon reaching the Delivery threshold
    reach_delivery_threshold(&mut double_echo, &dummy_cert);
    double_echo.state_change_follow_up();

    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::CertificateDelivered { certificate }) if certificate == dummy_cert
    ));

    // Trigger Ready upon reaching the Echo threshold
    reach_echo_threshold(&mut double_echo, &dummy_cert);
    double_echo.state_change_follow_up();

    assert_eq!(ctx.event_receiver.len(), 1);
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(ProtocolEvents::Ready { .. })
    ));
}

#[rstest]
#[case::small_config(small_config())]
#[tokio::test]
async fn buffering_certificate(#[case] params: TceParams) {
    let (double_echo, mut ctx) = create_context(params);

    let subscriptions = double_echo.subscriptions.clone();

    spawn(double_echo.run());

    // Wait to receive subscribers
    tokio::time::sleep(WAIT_EVENT_TIMEOUT).await;

    let le_cert = Certificate::default();
    ctx.cmd_sender
        .send(DoubleEchoCommand::Broadcast {
            need_gossip: true,
            cert: le_cert.clone(),
            ctx: Span::current(),
        })
        .await
        .expect("Cannot send broadcast command");

    assert_eq!(ctx.event_receiver.len(), 0);
    ctx.subscriptions_view_sender
        .send(subscriptions.clone())
        .await
        .expect("Cannot send expected view");

    let mut received_gossip_commands: Vec<Certificate> = Vec::new();
    let assertion = async {
        while let Ok(event) = ctx.event_receiver.recv().await {
            if let ProtocolEvents::Gossip { cert, .. } = event {
                received_gossip_commands.push(cert);
            }
        }
    };

    let _ = tokio::time::timeout(Duration::from_secs(1), assertion).await;

    assert_eq!(received_gossip_commands.len(), 1);
    assert_eq!(received_gossip_commands[0].id, le_cert.id);

    // Test shutdown
    info!("Waiting for double echo to shutdown...");
    let (sender, receiver) = oneshot::channel();
    ctx.double_echo_shutdown_sender
        .send(sender)
        .await
        .expect("Valid shutdown signal sending");
    assert_eq!(receiver.await, Ok(()));
}
