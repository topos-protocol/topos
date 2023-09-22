use std::collections::HashSet;
use std::sync::Arc;
use tce_transport::{ReliableBroadcastParams, ValidatorId};
use tokio::sync::{broadcast, mpsc, oneshot};
use topos_crypto::messages::MessageSigner;
use topos_tce_broadcast::double_echo::DoubleEcho;
use topos_tce_broadcast::sampler::SubscriptionsView;
use topos_tce_storage::validator::ValidatorStore;
use topos_test_sdk::certificates::create_certificate_chain;
use topos_test_sdk::constants::{SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_1};

const CHANNEL_SIZE: usize = 256_000;
const PRIVATE_KEY: &str = "47d361f6becb933a77d7e01dee7b1c1859b656adbd8428bf7bf9519503e5d5d6";

struct TceParams {
    nb_peers: usize,
    broadcast_params: ReliableBroadcastParams,
}

pub async fn processing_double_echo(n: u64, validator_store: Arc<ValidatorStore>) {
    let (subscriptions_view_sender, subscriptions_view_receiver) = mpsc::channel(CHANNEL_SIZE);

    let (_cmd_sender, cmd_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (event_sender, _event_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (broadcast_sender, mut broadcast_receiver) = broadcast::channel(CHANNEL_SIZE);
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

    let message_signer: Arc<MessageSigner> = MessageSigner::new(PRIVATE_KEY);

    let mut validators = HashSet::new();
    let validator_id = ValidatorId::from(message_signer.public_address);
    validators.insert(validator_id);

    let mut double_echo = DoubleEcho::new(
        params.broadcast_params,
        validator_id,
        message_signer.clone(),
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
        double_echo.broadcast(cert.certificate.clone(), true).await;
    }

    for cert in &certificates {
        let mut payload = Vec::new();
        payload.extend_from_slice(cert.certificate.id.as_array());
        payload.extend_from_slice(validator_id.as_bytes());

        for p in &double_echo_selected_echo {
            let signature = message_signer.sign_message(&payload).unwrap();

            double_echo
                .handle_echo(*p, cert.certificate.id, validator_id, signature)
                .await;
        }

        for p in &double_echo_selected_ready {
            let signature = message_signer.sign_message(&payload).unwrap();

            double_echo
                .handle_ready(*p, cert.certificate.id, validator_id, signature)
                .await;
        }
    }

    let mut count = 0;

    while let Ok(_event) = broadcast_receiver.recv().await {
        count += 1;

        if count == n {
            break;
        }
    }
}
