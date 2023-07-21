use rand::Rng;
use tce_transport::ReliableBroadcastParams;
use tokio::spawn;
use tokio::sync::mpsc;
use topos_core::uci::CertificateId;
use topos_p2p::PeerId;
use topos_tce_broadcast::DoubleEchoCommand;

use topos_tce_broadcast::task_manager_futures::TaskManager;

pub async fn processing_double_echo(n: u64) {
    let (message_sender, message_receiver) = mpsc::channel(1024);
    let (task_completion_sender, mut task_completion_receiver) = mpsc::channel(40_000);

    let thresholds = ReliableBroadcastParams {
        echo_threshold: n as usize,
        ready_threshold: n as usize,
        delivery_threshold: n as usize,
    };

    let (task_manager, shutdown_receiver) =
        TaskManager::new(message_receiver, task_completion_sender, thresholds);

    spawn(task_manager.run(shutdown_receiver));

    let mut certificates = vec![];

    let mut rng = rand::thread_rng();

    for _ in 0..10_000 {
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
