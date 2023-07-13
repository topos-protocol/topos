use crate::task_manager::{task::Events, TaskManager, Thresholds};

use crate::*;
use rstest::*;
use std::collections::HashMap;
use tokio::{spawn, sync::mpsc};
use topos_p2p::PeerId;
use tracing::Span;

#[rstest]
#[tokio::test]
async fn receiving_echo_messages() {
    let n = 1_000_000;

    let (message_sender, message_receiver) = mpsc::channel(1024);
    let (task_completion_sender, task_completion_receiver) = mpsc::channel(1024);
    let (event_sender, mut event_receiver) = mpsc::channel(1024);

    let task_manager = TaskManager {
        message_receiver,
        task_completion: task_completion_receiver,
        task_context: HashMap::new(),
        thresholds: Thresholds {
            echo: n,
            ready: n,
            delivery: n,
        },
    };

    spawn(task_manager.run(task_completion_sender, event_sender));

    let certificate_id = CertificateId::from_array([0u8; topos_core::uci::CERTIFICATE_ID_LENGTH]);

    let mut echos = vec![];

    for _ in 0..n {
        let echo = DoubleEchoCommand::Echo {
            from_peer: PeerId::random(),
            certificate_id,
            ctx: Span::current(),
        };

        echos.push(echo);
    }

    for echo in echos {
        message_sender.send(echo).await.unwrap();
    }

    while let Some(event) = event_receiver.recv().await {
        if event == Events::ReachedThresholdOfReady(certificate_id) {
            return;
        }
    }
}
