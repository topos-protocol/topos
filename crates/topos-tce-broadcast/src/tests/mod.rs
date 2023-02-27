use crate::double_echo::*;
use crate::mem_store::TceMemStore;
use crate::sampler::SubscribersUpdate;
use crate::*;
use rand::seq::IteratorRandom;
use rstest::*;
use std::collections::HashSet;
use std::usize;
use tce_transport::ReliableBroadcastParams;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::sync::broadcast::Receiver;
use tokio::time::Duration;
use topos_p2p::PeerId;

const PREV_CERTIFICATE_ID: topos_core::uci::CertificateId = CertificateId::from_array([4u8; 32]);
const SOURCE_SUBNET_ID: topos_core::uci::SubnetId = [1u8; 32];
const CHANNEL_SIZE: usize = 10;
const WAIT_EVENT_TIMEOUT: Duration = Duration::from_secs(1);

#[fixture]
fn small_config() -> TceParams {
    TceParams {
        nb_peers: 10,
        echo_subscribers_sample_size: 3,
        ready_subscribers_sample_size: 3,
        broadcast_params: ReliableBroadcastParams {
            echo_threshold: 8,
            echo_sample_size: 10,
            ready_threshold: 5,
            ready_sample_size: 10,
            delivery_threshold: 8,
            delivery_sample_size: 10,
        },
    }
}

#[fixture]
fn medium_config() -> TceParams {
    TceParams {
        nb_peers: 50,
        echo_subscribers_sample_size: 5,
        ready_subscribers_sample_size: 5,
        broadcast_params: ReliableBroadcastParams {
            echo_threshold: 8,
            echo_sample_size: 20,
            ready_threshold: 5,
            ready_sample_size: 20,
            delivery_threshold: 8,
            delivery_sample_size: 20,
        },
    }
}

fn get_sample(peers: &[PeerId], sample_size: usize) -> HashSet<PeerId> {
    let mut rng = rand::thread_rng();
    HashSet::from_iter(peers.iter().cloned().choose_multiple(&mut rng, sample_size))
}

#[derive(Debug)]
struct TceParams {
    nb_peers: usize,
    echo_subscribers_sample_size: usize,
    ready_subscribers_sample_size: usize,
    broadcast_params: ReliableBroadcastParams,
}

struct Context {
    event_receiver: Receiver<TceEvents>,
    subscribers_update_sender: Sender<SubscribersUpdate>,
    subscriptions_view_sender: Sender<SubscriptionsView>,
    cmd_sender: Sender<DoubleEchoCommand>,
    double_echo_shutdown_sender: Sender<oneshot::Sender<()>>,
}

fn create_context(params: TceParams) -> (DoubleEcho, Context) {
    let (subscribers_update_sender, subscribers_update_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (subscriptions_view_sender, subscriptions_view_receiver) = mpsc::channel(CHANNEL_SIZE);

    let (cmd_sender, cmd_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (event_sender, event_receiver) = broadcast::channel(CHANNEL_SIZE);
    let (double_echo_shutdown_sender, double_echo_shutdown_receiver) =
        mpsc::channel::<oneshot::Sender<()>>(1);

    let mut double_echo = DoubleEcho::new(
        params.broadcast_params.clone(),
        cmd_receiver,
        subscriptions_view_receiver,
        subscribers_update_receiver,
        event_sender,
        Box::new(TceMemStore::default()),
        double_echo_shutdown_receiver,
        String::new(),
    );

    // List of peers
    let mut peers = Vec::new();
    for i in 0..params.nb_peers {
        let peer = topos_p2p::utils::local_key_pair(Some(i as u8))
            .public()
            .to_peer_id();
        peers.push(peer);
    }

    // Subscriptions
    double_echo.subscriptions.echo = get_sample(&peers, params.broadcast_params.echo_sample_size);
    double_echo.subscriptions.ready = get_sample(&peers, params.broadcast_params.ready_sample_size);
    double_echo.subscriptions.delivery =
        get_sample(&peers, params.broadcast_params.delivery_sample_size);

    // Subscribers
    double_echo.subscribers.echo = get_sample(&peers, params.echo_subscribers_sample_size);
    double_echo.subscribers.ready = get_sample(&peers, params.ready_subscribers_sample_size);

    (
        double_echo,
        Context {
            event_receiver,
            subscribers_update_sender,
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
        double_echo.handle_echo(p, &cert.id);
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
        double_echo.handle_ready(p, &cert.id);
    }
}

fn reach_delivery_threshold(double_echo: &mut DoubleEcho, cert: &Certificate) {
    let selected = double_echo
        .subscriptions
        .delivery
        .iter()
        .take(double_echo.params.delivery_threshold)
        .cloned()
        .collect::<Vec<_>>();

    for p in selected {
        double_echo.handle_ready(p, &cert.id);
    }
}

#[rstest]
#[case(small_config())]
#[case(medium_config())]
#[tokio::test]
#[trace]
async fn trigger_success_path_upon_reaching_threshold(#[case] params: TceParams) {
    let (mut double_echo, mut ctx) = create_context(params);

    let dummy_cert = Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID,
        Default::default(),
        Default::default(),
        &vec![],
        0,
    )
    .expect("Dummy certificate");

    // Trigger Echo upon dispatching
    double_echo.handle_broadcast(dummy_cert.clone());

    assert_eq!(ctx.event_receiver.len(), 2);

    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(TceEvents::Gossip { peers, .. }) if peers.len() == double_echo.gossip_peers().len()
    ));
    assert!(matches!(
            ctx.event_receiver.try_recv(),
            Ok(TceEvents::Echo { peers, .. }) if peers.len() == double_echo.subscribers.echo.len()));

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
        Ok(TceEvents::Ready { peers, .. }) if peers.len() == double_echo.subscribers.ready.len()
    ));

    // Trigger Delivery upon reaching the Delivery threshold
    reach_delivery_threshold(&mut double_echo, &dummy_cert);
    double_echo.state_change_follow_up();

    assert_eq!(ctx.event_receiver.len(), 1);
    assert!(matches!(
         ctx.event_receiver.try_recv(),
        Ok(TceEvents::CertificateDelivered { certificate }) if certificate == dummy_cert
    ));
}

#[rstest]
#[case(small_config())]
#[case(medium_config())]
#[tokio::test]
#[trace]
async fn trigger_ready_when_reached_enough_ready(#[case] params: TceParams) {
    let (mut double_echo, mut ctx) = create_context(params);

    let dummy_cert = Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID,
        Default::default(),
        Default::default(),
        &vec![],
        0,
    )
    .expect("Dummy certificate");

    // Trigger Echo upon dispatching
    double_echo.handle_broadcast(dummy_cert.clone());

    assert_eq!(ctx.event_receiver.len(), 2);
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(TceEvents::Gossip { .. })
    ));
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(TceEvents::Echo { .. })
    ));

    // Trigger Ready upon reaching the Ready threshold
    reach_ready_threshold(&mut double_echo, &dummy_cert);
    double_echo.state_change_follow_up();

    assert_eq!(ctx.event_receiver.len(), 1);
    assert!(matches!(
        ctx.event_receiver.try_recv(),
        Ok(TceEvents::Ready { peers, .. }) if peers.len() == double_echo.subscribers.ready.len()
    ));
}

#[rstest]
#[case(small_config())]
#[tokio::test]
async fn buffering_certificate(#[case] params: TceParams) {
    let (double_echo, mut ctx) = create_context(params);

    let subscribers = double_echo.subscribers.clone();
    let subscriptions = double_echo.subscriptions.clone();

    spawn(double_echo.run());

    // Add subscribers
    for &peer in subscribers.echo.iter() {
        ctx.subscribers_update_sender
            .send(SubscribersUpdate::NewEchoSubscriber(peer))
            .await
            .expect("Added new subscriber");
    }
    for &peer in subscribers.ready.iter() {
        ctx.subscribers_update_sender
            .send(SubscribersUpdate::NewReadySubscriber(peer))
            .await
            .expect("Added new subscriber");
    }

    // Wait to receive subscribers
    tokio::time::sleep(WAIT_EVENT_TIMEOUT).await;

    let le_cert = Certificate::default();
    ctx.cmd_sender
        .send(DoubleEchoCommand::Broadcast {
            cert: le_cert.clone(),
            ctx: Span::current().context(),
        })
        .await
        .expect("Cannot send broadcast command");

    assert_eq!(ctx.event_receiver.len(), 0);
    ctx.subscriptions_view_sender
        .send(subscriptions.clone())
        .await
        .expect("Cannot send expected view");

    let mut received_gossip_commands: Vec<(HashSet<PeerId>, Certificate)> = Vec::new();
    let assertion = async {
        loop {
            while let Ok(event) = ctx.event_receiver.try_recv() {
                if let TceEvents::Gossip { peers, cert, .. } = event {
                    received_gossip_commands.push((peers.into_iter().collect(), cert));
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };

    let _ = tokio::time::timeout(Duration::from_secs(1), assertion).await;

    // Check if gossip Event is sent to all peers
    let all_gossip_peers = subscriptions
        .get_subscriptions()
        .into_iter()
        .chain(subscribers.get_subscribers().into_iter())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    assert_eq!(received_gossip_commands.len(), 1);
    assert_eq!(received_gossip_commands[0].0, all_gossip_peers);
    assert_eq!(received_gossip_commands[0].1.id, le_cert.id);

    // Test shutdown
    info!("Waiting for double echo to shutdown...");
    let (sender, receiver) = oneshot::channel();
    ctx.double_echo_shutdown_sender
        .send(sender)
        .await
        .expect("Valid shutdown signal sending");
    assert_eq!(receiver.await, Ok(()));
}
