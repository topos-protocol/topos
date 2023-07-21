use criterion::async_executor::FuturesExecutor;
use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(feature = "task-manager-channels")]
mod task_manager_channels;
#[cfg(feature = "task-manager-futures")]
mod task_manager_futures;

pub fn criterion_benchmark(c: &mut Criterion) {
    let echo_messages = 10;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    #[cfg(feature = "task-manager-channels")]
    c.bench_function("double_echo with channels", |b| {
        b.to_async(FuturesExecutor).iter(|| async {
            runtime.block_on(async {
                task_manager_channels::processing_double_echo(echo_messages).await
            })
        })
    });

    #[cfg(feature = "task-manager-futures")]
    c.bench_function("double_echo with futures", |b| {
        b.to_async(FuturesExecutor).iter(|| async {
            runtime.block_on(async {
                task_manager_futures::processing_double_echo(echo_messages).await
            })
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
