use criterion::async_executor::FuturesExecutor;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

mod task_manager_channels;
mod task_manager_futures;

pub fn criterion_benchmark(c: &mut Criterion) {
    let echo_messages = 10;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    c.bench_function(&format!("double_echo with channels"), |b| {
        b.to_async(FuturesExecutor).iter(|| async {
            runtime.block_on(async {
                task_manager_channels::processing_double_echo(echo_messages).await
            })
        })
    });

    c.bench_function(&format!("double_echo with futures"), |b| {
        b.to_async(FuturesExecutor).iter(|| async {
            runtime.block_on(async {
                task_manager_futures::processing_double_echo(echo_messages).await
            })
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
