use crate::task_manager_channels::{TaskManager, Thresholds};

use crate::*;
use rand::Rng;
use rstest::*;
use std::collections::HashMap;
use tokio::{spawn, sync::mpsc};
use topos_p2p::PeerId;

#[rstest]
#[tokio::test]
async fn task_manager_channels_receiving_messages() {
    let n = 5;

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

    let mut rng = rand::thread_rng();

    for _ in 0..10 {
        let mut id = [0u8; 32];
        rng.fill(&mut id);
        let cert_id = CertificateId::from_array(id);
        certificates.push(cert_id);
    }

    for certificate_id in certificates {
        for _ in 0..n {
            let echo = DoubleEchoCommand::Echo {
                from_peer: PeerId::random(),
                certificate_id,
            };

            message_sender.send(echo).await.unwrap();
        }
    }

    let mut count = 0;

    while (event_receiver.recv().await).is_some() {
        count += 1;

        if count == n {
            break;
        }
    }
}
