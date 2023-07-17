use futures::stream::FuturesUnordered;
use rand::Rng;
use std::collections::HashMap;
use tokio::spawn;
use tokio::sync::mpsc;
use topos_core::uci::CertificateId;
use topos_p2p::PeerId;
use topos_tce_broadcast::DoubleEchoCommand;
use tracing::Span;

use topos_tce_broadcast::task_manager_futures::task::TaskStatus;
use topos_tce_broadcast::task_manager_futures::{task::Events, TaskManager, Thresholds};

pub async fn processing_double_echo(n: u64) {
    let mut task_manager = TaskManager::new(Thresholds {
        echo: n as usize,
        ready: n as usize,
        delivery: n as usize,
    })
    .await;

    let mut certificates = vec![];

    let mut rng = rand::thread_rng();

    for _ in 0..10_000 {
        let mut id = [0u8; 32];
        rng.fill(&mut id);
        let cert_id = CertificateId::from_array(id);
        certificates.push(cert_id);
    }

    for cert in certificates {
        for _ in 0..n {
            let echo = DoubleEchoCommand::Echo {
                from_peer: PeerId::random(),
                certificate_id: cert.clone(),
                ctx: Span::current(),
            };

            task_manager.add_message(echo).await.unwrap();
        }
    }

    let mut count = 0;

    while let Some((certificate_id, _)) = task_manager.task_completed_receiver.recv().await {
        count += 1;

        task_manager.remove_finished_task(certificate_id);
        if count == n {
            break;
        }
    }
}
