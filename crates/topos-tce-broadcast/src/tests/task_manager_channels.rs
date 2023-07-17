use crate::task_manager_channels::{task::Events, TaskManager, Thresholds};

use crate::task_manager_channels::task::Events::ReachedThresholdOfReady;
use crate::*;
use rstest::*;
use std::collections::HashMap;
use tokio::{spawn, sync::mpsc};
use topos_p2p::PeerId;
use tracing::Span;

#[rstest]
#[tokio::test]
async fn task_manager_channels_receiving_messages() {
    let n = 100_000;

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

            message_sender.send(echo).await.unwrap();
        }
    }

    let mut count = 0;

    while let Some(event) = event_receiver.recv().await {
        match event {
            ReachedThresholdOfReady { 0: certificate_id } => {
                println!("Threshold reached for {certificate_id:?}");
                count += 1;
            }
            _ => {}
        }

        if count == 10 {
            break;
        }
    }
}
