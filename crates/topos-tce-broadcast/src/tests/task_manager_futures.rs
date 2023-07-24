use crate::sampler::SubscriptionsView;
use crate::task_manager_futures::TaskManager;
use crate::{CertificateId, DoubleEchoCommand};
use rand::Rng;
use rstest::*;
use tce_transport::ReliableBroadcastParams;
use tokio::sync::broadcast;
use tokio::{spawn, sync::mpsc};
use topos_p2p::PeerId;

#[rstest]
#[tokio::test]
async fn task_manager_futures_receiving_messages() {
    let n = 5;

    let (message_sender, message_receiver) = mpsc::channel(1024);
    let (task_completion_sender, mut task_completion_receiver) = mpsc::channel(1024);
    let (subscription_view_sender, subscription_view_receiver) = broadcast::channel(1024);
    let (event_sender, _event_receiver) = mpsc::channel(1024);

    let thresholds = ReliableBroadcastParams {
        echo_threshold: n,
        ready_threshold: n,
        delivery_threshold: n,
    };

    let (task_manager, shutdown_receiver) = TaskManager::new(
        message_receiver,
        task_completion_sender,
        subscription_view_receiver,
        event_sender,
        thresholds,
    );

    let subscription_view = SubscriptionsView::default();

    subscription_view_sender.send(subscription_view);

    spawn(task_manager.run(shutdown_receiver));

    let mut certificates = vec![];

    let mut rng = rand::thread_rng();

    for _ in 0..10 {
        let mut id = [0u8; 32];
        rng.fill(&mut id);
        let cert_id = CertificateId::from_array(id);
        certificates.push(cert_id);
    }

    for certificate_id in certificates {
        for _ in 0..n {
            let echo = DoubleEchoCommand::Echo {
                from_peer: PeerId::random(),
                certificate_id,
            };

            message_sender.send(echo).await.unwrap();
        }
    }

    let mut count = 0;

    while let Some((_, _)) = task_completion_receiver.recv().await {
        count += 1;

        if count == n {
            break;
        }
    }
}
