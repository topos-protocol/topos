use rand::Rng;
use tokio::spawn;
use tokio::sync::mpsc;

use tce_transport::ReliableBroadcastParams;
use topos_core::uci::CertificateId;
use topos_p2p::PeerId;
use topos_tce_broadcast::task_manager_channels::TaskManager;
use topos_tce_broadcast::DoubleEchoCommand;

pub async fn processing_double_echo(n: u64) {
    let (message_sender, message_receiver) = mpsc::channel(1024);
    let (event_sender, mut event_receiver) = mpsc::channel(1024);

    let threshold = ReliableBroadcastParams {
        echo_threshold: n as usize,
        ready_threshold: n as usize,
        delivery_threshold: n as usize,
    };

    let (task_manager, shutdown_receiver) = TaskManager::new(message_receiver, threshold);

    spawn(task_manager.run(event_sender, shutdown_receiver));

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

    while (event_receiver.recv().await).is_some() {
        count += 1;

        if count == n {
            break;
        }
    }
}
