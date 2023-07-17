use std::collections::HashMap;
use tokio::spawn;
use tokio::sync::mpsc;
use topos_core::uci::CertificateId;
use topos_p2p::PeerId;
use topos_tce_broadcast::DoubleEchoCommand;
use tracing::Span;

use topos_tce_broadcast::task_manager_channels::task::Events::ReachedThresholdOfReady;
use topos_tce_broadcast::task_manager_channels::{task::Events, TaskManager, Thresholds};

pub(crate) async fn processing_double_echo(n: u64) {
    let (message_sender, message_receiver) = mpsc::channel(1024);
    let (task_completion_sender, task_completion_receiver) = mpsc::channel(1024);
    let (event_sender, mut event_receiver) = mpsc::channel(1024);

    let task_manager = TaskManager {
        message_receiver,
        task_completion: task_completion_receiver,
        task_context: HashMap::new(),
        thresholds: Thresholds {
            echo: n as usize,
            ready: n as usize,
            delivery: n as usize,
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
            ReachedThresholdOfReady { 0: _ } => {
                count += 1;
            }
            _ => {}
        }

        if count == 10 {
            break;
        }
    }
}
