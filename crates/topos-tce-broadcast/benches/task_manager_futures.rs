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
    let (message_sender, message_receiver) = mpsc::channel(1024);
    let (task_completion_sender, mut task_completion_receiver) = mpsc::channel(1024);

    let task_manager = TaskManager {
        message_receiver,
        task_completion_sender,
        tasks: Default::default(),
        running_tasks: FuturesUnordered::new(),
        thresholds: Thresholds {
            echo: n as usize,
            ready: n as usize,
            delivery: n as usize,
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
