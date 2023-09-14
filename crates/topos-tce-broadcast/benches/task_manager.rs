use ethers::prelude::{LocalWallet, Signer};
use ethers::utils::keccak256;
use std::collections::HashSet;
use tce_transport::{ProtocolEvents, ReliableBroadcastParams, ValidatorId};
use tokio::sync::mpsc::Receiver;
use tokio::sync::{mpsc, oneshot};
use topos_tce_broadcast::double_echo::DoubleEcho;
use topos_tce_broadcast::sampler::SubscriptionsView;
use topos_test_sdk::certificates::create_certificate_chain;
use topos_test_sdk::constants::{SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_1};

const CHANNEL_SIZE: usize = 256_000;

struct TceParams {
    nb_peers: usize,
    broadcast_params: ReliableBroadcastParams,
}

struct Context {
    event_receiver: Receiver<ProtocolEvents>,
}

pub async fn processing_double_echo(n: u64) {
    let (subscriptions_view_sender, subscriptions_view_receiver) = mpsc::channel(CHANNEL_SIZE);

    let (_cmd_sender, cmd_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (event_sender, event_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (_double_echo_shutdown_sender, double_echo_shutdown_receiver) =
        mpsc::channel::<oneshot::Sender<()>>(1);
    let (task_manager_message_sender, task_manager_message_receiver) = mpsc::channel(CHANNEL_SIZE);

    let params = TceParams {
        nb_peers: 10,
        broadcast_params: ReliableBroadcastParams {
            echo_threshold: 8,
            ready_threshold: 5,
            delivery_threshold: 8,
        },
    };

    let mut ctx = Context { event_receiver };
    let wallet: LocalWallet = "47d361f6becb933a77d7e01dee7b1c1859b656adbd8428bf7bf9519503e5d5d6"
        .parse()
        .unwrap();

    let mut validators = HashSet::new();
    let validator_id = ValidatorId::from(wallet.address());
    validators.insert(validator_id.clone());

    let mut double_echo = DoubleEcho::new(
        params.broadcast_params,
        validator_id.clone(),
        wallet.clone(),
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

    let certificates =
        create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], n as usize);

    let double_echo_selected_echo = double_echo
        .subscriptions
        .echo
        .iter()
        .take(double_echo.params.echo_threshold)
        .cloned()
        .collect::<Vec<_>>();

    let double_echo_selected_ready = double_echo
        .subscriptions
        .ready
        .iter()
        .take(double_echo.params.delivery_threshold)
        .cloned()
        .collect::<Vec<_>>();

    for cert in &certificates {
        double_echo.broadcast(cert.clone(), true).await;
    }

    for cert in &certificates {
        for p in &double_echo_selected_echo {
            let mut hash = Vec::new();
            hash.extend(cert.id.as_array().iter().cloned());
            hash.extend(validator_id.clone().as_bytes());

            let hash = keccak256(hash);
            let signature = wallet.sign_message(hash.as_slice()).await.unwrap();

            double_echo
                .handle_echo(*p, cert.id, validator_id.clone(), signature)
                .await;
        }

        for p in &double_echo_selected_ready {
            let mut hash = Vec::new();
            hash.extend(cert.id.as_array().iter().cloned());
            hash.extend(validator_id.clone().as_bytes());

            let hash = keccak256(hash);
            let signature = wallet.sign_message(hash.as_slice()).await.unwrap();

            double_echo
                .handle_ready(*p, cert.id, validator_id.clone(), signature)
                .await;
        }
    }

    let mut count = 0;

    while let Some(event) = ctx.event_receiver.recv().await {
        if let ProtocolEvents::CertificateDelivered { .. } = event {
            count += 1;

            if count == n {
                break;
            }
        }
    }
}
