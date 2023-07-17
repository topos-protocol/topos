use crate::task_manager_futures::{
    task::{Events, TaskStatus},
    TaskManager, Thresholds,
};
use crate::{CertificateId, DoubleEchoCommand};
use futures::stream::FuturesUnordered;
use rand::Rng;
use rstest::*;
use std::collections::HashMap;
use tokio::{spawn, sync::mpsc};
use topos_p2p::PeerId;
use tracing::Span;

#[rstest]
#[tokio::test]
async fn task_manager_futures_receiving_messages() {
    let n = 100_000;

    let mut task_manager = TaskManager::new(Thresholds {
        echo: n,
        ready: n,
        delivery: n,
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

        if count == 10 {
            break;
        }
    }
}
