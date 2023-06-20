use apgpk_lib::core::{task, Msg};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::{atomic::AtomicBool, Arc};

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_single_thread");
    group.sample_size(10);
    group.bench_function("task_single_thread", |b| {
        b.iter(|| {
            let exit = Arc::new(AtomicBool::new(false));
            let (tx, _rx) = std::sync::mpsc::channel::<Msg>();
            task(
                "test".to_string(),
                black_box(1),
                &["AAAAAAAA".to_string(), "BBBBBBBB".to_string()],
                &exit,
                &tx,
            )
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
