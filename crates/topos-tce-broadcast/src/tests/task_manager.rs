use crate::task_manager::TaskManager;

use crate::*;
use rstest::*;
use std::collections::HashMap;
use tokio::{spawn, sync::mpsc};
use topos_p2p::PeerId;
use tracing::Span;

#[rstest]
#[tokio::test]
async fn receiving_echo_messages() {
    let (sender, receiver) = mpsc::channel(1024);
    let (task_completion_sender, task_completion_receiver) = mpsc::channel(1024);

    let task_manager = TaskManager {
        receiver,
        task_completion: task_completion_receiver,
        task_context: HashMap::new(),
    };

    spawn(task_manager.run(task_completion_sender));

    let certificate_id = CertificateId::from_array([0u8; topos_core::uci::CERTIFICATE_ID_LENGTH]);

    let mut echos = vec![];

    for _ in 0..4 {
        let echo = DoubleEchoCommand::Echo {
            from_peer: PeerId::random(),
            certificate_id,
            ctx: Span::current(),
        };

        echos.push(echo);
    }

    for echo in echos {
        sender.send(echo).await.unwrap();
    }
}
