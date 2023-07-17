use futures::stream::FuturesUnordered;
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

    for i in 0..10 {
        certificates.push(CertificateId::from_array(
            [i; topos_core::uci::CERTIFICATE_ID_LENGTH],
        ));
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

    while let Some(event) = task_manager.task_completed_receiver.recv().await {
        count += 1;

        if count == 10 {
            break;
        }
    }
}
