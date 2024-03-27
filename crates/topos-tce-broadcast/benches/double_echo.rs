use criterion::async_executor::FuturesExecutor;
use criterion::{criterion_group, criterion_main, Criterion};
use topos_test_sdk::storage::create_validator_store;
mod task_manager;

pub fn criterion_benchmark(c: &mut Criterion) {
    let certificates = 10_000;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    let store = runtime.block_on(async { create_validator_store::partial_1(&[]).await });

    c.bench_function("double_echo", |b| {
        b.to_async(FuturesExecutor).iter(|| async {
            runtime.block_on(async {
                task_manager::processing_double_echo(certificates, store.clone()).await
            })
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
