use criterion::async_executor::FuturesExecutor;
use criterion::{criterion_group, criterion_main, Criterion};
mod task_manager;

pub fn criterion_benchmark(c: &mut Criterion) {
    let certificates = 10_000;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    c.bench_function("double_echo", |b| {
        b.to_async(FuturesExecutor).iter(|| async {
            runtime.block_on(async { task_manager::processing_double_echo(certificates).await })
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
