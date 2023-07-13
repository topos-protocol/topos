use criterion::async_executor::FuturesExecutor;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::collections::HashMap;
use tokio::spawn;
use tokio::sync::mpsc;
use topos_core::uci::CertificateId;
use topos_p2p::PeerId;
use topos_tce_broadcast::DoubleEchoCommand;
use tracing::Span;

use topos_tce_broadcast::task_manager::{task::Events, TaskManager, Thresholds};

async fn processing_double_echo(n: u64) {
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

pub fn criterion_benchmark(c: &mut Criterion) {
    let iterations = 10;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    let echo = runtime.block_on(async { processing_double_echo(iterations).await });

    c.bench_function(&format!("double {iterations}"), |b| {
        b.to_async(FuturesExecutor).iter(|| async { echo })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
