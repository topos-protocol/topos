use crate::task_manager_futures::{
    task::{Events, TaskStatus},
    TaskManager, Thresholds,
};
use crate::{CertificateId, DoubleEchoCommand};
use futures::stream::FuturesUnordered;
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
