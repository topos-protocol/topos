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
    let n = 10;

    let (message_sender, message_receiver) = mpsc::channel(10_240);
    let (task_completion_sender, mut task_completion_receiver) = mpsc::channel(10_240);
    let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

    let mut task_manager = TaskManager {
        message_receiver,
        task_completion_sender,
        tasks: Default::default(),
        running_tasks: FuturesUnordered::new(),
        thresholds: Thresholds {
            echo: n,
            ready: n,
            delivery: n,
        },
        shutdown_sender,
    };

    spawn(task_manager.run(shutdown_receiver));

    let mut certificates = vec![];

    let mut rng = rand::thread_rng();

    for _ in 0..10 {
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

            message_sender.send(echo).await.unwrap();
        }
    }

    let mut count = 0;

    while let Some((certificate_id, _)) = task_completion_receiver.recv().await {
        println!("Get task completion for certificate: {:?}", certificate_id);
        count += 1;

        if count == 10 {
            break;
        }
    }
}
