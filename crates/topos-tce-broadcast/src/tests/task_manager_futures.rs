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

    let (message_sender, message_receiver) = mpsc::channel(1024);
    let (task_completion_sender, mut task_completion_receiver) = mpsc::channel(1024);

    let task_manager = TaskManager {
        message_receiver,
        task_completion_sender,
        tasks: Default::default(),
        running_tasks: FuturesUnordered::new(),
        thresholds: Thresholds {
            echo: n,
            ready: n,
            delivery: n,
        },
    };

    spawn(task_manager.run());

    let certificate_id = CertificateId::from_array([0u8; topos_core::uci::CERTIFICATE_ID_LENGTH]);

    let mut echos = vec![];

    for _ in 0..n {
        let echo = DoubleEchoCommand::Echo {
            from_peer: PeerId::random(),
            certificate_id: certificate_id.clone(),
            ctx: Span::current(),
        };

        echos.push(echo);
    }

    for echo in echos {
        message_sender.send(echo).await.unwrap();
    }

    while let Some((id, status)) = task_completion_receiver.recv().await {
        if id == certificate_id && status == TaskStatus::Success {
            return;
        }
    }
}
